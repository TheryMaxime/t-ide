//! Agent engine.
//!
//! [`AgentEngine`] turns a developer prompt into a sequence of agent actions,
//! runs them inside the project sandbox with explicit approval for anything
//! that changes the machine, and records every step in the session transcript
//! (T028, T030, T031, T032).

pub mod approval;
pub mod provider;
pub mod session;

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::agent::approval::{ApprovalKind, ApprovalService, Decision};
use crate::agent::provider::{
    CompletionRequest, ModelProviderRepository, ProviderMessage, SharedProvider,
};
use crate::agent::session::{
    AgentSession, CancelToken, CancellationRegistry, SessionRepository, SessionStatus,
};
use crate::network::events::SessionEvents;
use crate::network::protocol::{ServerMessage, TranscriptEntryPayload};
use crate::security::sandbox::Sandbox;
use crate::storage::projects::{Project, ProjectRepository};
use crate::storage::transcripts::{EntryType, TranscriptRepository};
use crate::storage::Database;
use crate::{Error, Result};

/// Instructions handed to the model. The engine only understands the JSON
/// action plan described here, so the contract is stated explicitly.
const SYSTEM_PROMPT: &str = "You are the T-ide coding agent. You work strictly inside the user's \
project folder. Reply with a single JSON object and nothing else, shaped as: {\"reasoning\": \
\"...\", \"actions\": [{\"type\": \"read_file\", \"path\": \"relative/path\"}, {\"type\": \
\"write_file\", \"path\": \"relative/path\", \"contents\": \"...\"}, {\"type\": \"delete_file\", \
\"path\": \"relative/path\"}, {\"type\": \"run_command\", \"command\": \"...\"}, {\"type\": \
\"message\", \"text\": \"...\"}], \"summary\": \"...\"}. Never reference paths outside the \
project folder.";

/// A single step the model asked the agent to perform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentAction {
    /// Read-only action: subject to the project's auto-approve setting.
    ReadFile {
        path: String,
    },
    WriteFile {
        path: String,
        contents: String,
    },
    DeleteFile {
        path: String,
    },
    RunCommand {
        command: String,
    },
    /// Plain text for the transcript; performs no work.
    Message {
        text: String,
    },
}

impl AgentAction {
    /// Whether the action only reads state (FR-005).
    pub fn is_read_only(&self) -> bool {
        matches!(
            self,
            AgentAction::ReadFile { .. } | AgentAction::Message { .. }
        )
    }
}

/// The model's answer to a prompt.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPlan {
    #[serde(default)]
    pub reasoning: String,
    #[serde(default)]
    pub actions: Vec<AgentAction>,
    #[serde(default)]
    pub summary: String,
}

impl AgentPlan {
    /// Parse a model reply, tolerating a Markdown code fence around the JSON.
    pub fn parse(reply: &str) -> Result<Self> {
        let trimmed = strip_code_fence(reply.trim());
        serde_json::from_str(trimmed)
            .map_err(|err| Error::Provider(format!("model did not return an action plan: {err}")))
    }
}

fn strip_code_fence(reply: &str) -> &str {
    let Some(rest) = reply.strip_prefix("```") else {
        return reply;
    };
    let rest = rest.strip_prefix("json").unwrap_or(rest);
    let rest = rest.trim_start_matches(['\r', '\n']).trim_end();
    rest.strip_suffix("```").unwrap_or(rest).trim()
}

/// Runs prompts for a session against a model provider.
#[derive(Clone)]
pub struct AgentEngine {
    db: Arc<Database>,
    events: SessionEvents,
    approvals: ApprovalService,
    cancellations: CancellationRegistry,
    approval_timeout: Duration,
}

impl AgentEngine {
    pub fn new(
        db: Arc<Database>,
        events: SessionEvents,
        approvals: ApprovalService,
        cancellations: CancellationRegistry,
        approval_timeout: Duration,
    ) -> Self {
        Self {
            db,
            events,
            approvals,
            cancellations,
            approval_timeout,
        }
    }

    /// Run one prompt to completion and return the session's final status.
    ///
    /// The prompt, the model's reasoning, every file change, every command, and
    /// every approval decision are appended to the transcript and streamed to
    /// connected clients as they happen (FR-003, FR-007).
    pub async fn run_prompt(
        &self,
        session_id: i64,
        prompt: &str,
        origin_device_id: Option<i64>,
        provider: SharedProvider,
        model: Option<String>,
    ) -> Result<SessionStatus> {
        let sessions = SessionRepository::new(&self.db);
        let session = sessions.get(session_id)?;
        if session.status.is_terminal() {
            return Err(Error::SessionNotActive);
        }
        let project = ProjectRepository::new(&self.db).get(session.project_id)?;
        let sandbox = Sandbox::new(&project.path)?;
        let provider_row = ModelProviderRepository::new(&self.db).get(session.provider_id)?;
        let model = model.unwrap_or(provider_row.default_model);

        let token = self.cancellations.register(session_id);
        sessions.update_status(session_id, SessionStatus::Running)?;

        self.record(
            session_id,
            EntryType::Prompt,
            origin_device_id,
            serde_json::json!({ "text": prompt, "model": model }),
        )?;

        let context = RunContext {
            session: &session,
            project: &project,
            sandbox: &sandbox,
            origin_device_id,
            token: &token,
        };
        let outcome = self.run_inner(&context, prompt, provider, model).await;

        let status = match outcome {
            Ok(status) => status,
            Err(err) => {
                self.record(
                    session_id,
                    EntryType::Error,
                    None,
                    serde_json::json!({ "message": err.to_string(), "code": err.code() }),
                )?;
                SessionStatus::Complete
            }
        };

        self.cancellations.finish(session_id);
        let final_status = sessions.update_status(session_id, status)?.status;
        self.events
            .publish(ServerMessage::TranscriptComplete { session_id });
        Ok(final_status)
    }

    async fn run_inner(
        &self,
        context: &RunContext<'_>,
        prompt: &str,
        provider: SharedProvider,
        model: String,
    ) -> Result<SessionStatus> {
        let RunContext {
            session,
            sandbox,
            token,
            ..
        } = context;
        let request = CompletionRequest {
            model,
            messages: vec![
                ProviderMessage {
                    role: "system".into(),
                    content: SYSTEM_PROMPT.into(),
                },
                ProviderMessage {
                    role: "user".into(),
                    content: format!(
                        "Project folder: {}\n\nTask: {prompt}",
                        sandbox.root().display()
                    ),
                },
            ],
        };

        let reply = tokio::select! {
            biased;
            _ = token.cancelled() => return self.interrupted(session.id),
            reply = provider.complete_boxed(request) => reply?,
        };

        let plan = AgentPlan::parse(&reply.text)?;
        self.record(
            session.id,
            EntryType::Response,
            None,
            serde_json::json!({ "text": plan.reasoning, "summary": plan.summary }),
        )?;

        for action in plan.actions {
            if token.is_cancelled() {
                return self.interrupted(session.id);
            }
            match self.run_action(context, &action).await {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {}
                // A refused or failed action is recorded and the run continues
                // with the next step (FR-004).
                Err(err) => self.record(
                    session.id,
                    EntryType::Error,
                    None,
                    serde_json::json!({ "message": err.to_string(), "code": err.code() }),
                )?,
            }
        }

        Ok(SessionStatus::Complete)
    }

    /// Execute one action. Returns a terminal status when the run must stop.
    async fn run_action(
        &self,
        context: &RunContext<'_>,
        action: &AgentAction,
    ) -> Result<Option<SessionStatus>> {
        let RunContext {
            session,
            sandbox,
            origin_device_id,
            token,
            ..
        } = context;
        if let AgentAction::Message { text } = action {
            self.record(
                session.id,
                EntryType::Response,
                None,
                serde_json::json!({ "text": text }),
            )?;
            return Ok(None);
        }

        let (kind, details) = describe(action, sandbox)?;
        let decision = self.decide(context, action, kind, &details).await?;
        self.record(
            session.id,
            EntryType::ApprovalDecision,
            *origin_device_id,
            serde_json::json!({ "decision": decision.as_str(), "request": details }),
        )?;
        if decision == Decision::Denied {
            return Ok(None);
        }
        if token.is_cancelled() {
            return self.interrupted(session.id).map(Some);
        }

        match action {
            AgentAction::ReadFile { path } => {
                let contents = sandbox.read_to_string(path)?;
                self.record(
                    session.id,
                    EntryType::FileChange,
                    None,
                    serde_json::json!({
                        "path": path,
                        "operation": "read",
                        "bytes": contents.len(),
                    }),
                )?;
            }
            AgentAction::WriteFile { path, contents } => {
                let existed = sandbox.resolve(path)?.exists();
                sandbox.write(path, contents)?;
                self.record(
                    session.id,
                    EntryType::FileChange,
                    None,
                    serde_json::json!({
                        "path": path,
                        "operation": if existed { "update" } else { "create" },
                        "bytes": contents.len(),
                    }),
                )?;
            }
            AgentAction::DeleteFile { path } => {
                sandbox.remove_file(path)?;
                self.record(
                    session.id,
                    EntryType::FileChange,
                    None,
                    serde_json::json!({ "path": path, "operation": "delete" }),
                )?;
            }
            AgentAction::RunCommand { command } => {
                let output = match self.run_command(command, sandbox, token).await? {
                    Some(output) => output,
                    None => return self.interrupted(session.id).map(Some),
                };
                self.record(session.id, EntryType::Command, None, output)?;
            }
            AgentAction::Message { .. } => unreachable!("handled above"),
        }
        Ok(None)
    }

    /// Ask for approval unless the project auto-approves read-only actions.
    async fn decide(
        &self,
        context: &RunContext<'_>,
        action: &AgentAction,
        kind: ApprovalKind,
        details: &serde_json::Value,
    ) -> Result<Decision> {
        if action.is_read_only() && context.project.auto_approve_read_only {
            return Ok(Decision::Approved);
        }
        let session_id = context.session.id;
        let token = context.token;

        let sessions = SessionRepository::new(&self.db);
        sessions.update_status(session_id, SessionStatus::WaitingApproval)?;
        let decision = tokio::select! {
            biased;
            _ = token.cancelled() => Decision::Denied,
            decision = self
                .approvals
                .request(session_id, kind, details, self.approval_timeout) => decision?,
        };
        if !token.is_cancelled() {
            sessions.update_status(session_id, SessionStatus::Running)?;
        }
        Ok(decision)
    }

    /// Run an approved command inside the project folder.
    ///
    /// Returns `None` when the run was cancelled while the command was still
    /// executing; the child process is killed in that case (FR-006).
    async fn run_command(
        &self,
        command: &str,
        sandbox: &Sandbox,
        token: &CancelToken,
    ) -> Result<Option<serde_json::Value>> {
        let mut child = shell_command(command)
            .current_dir(sandbox.root())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let status = tokio::select! {
            biased;
            _ = token.cancelled() => {
                let _ = child.kill().await;
                return Ok(None);
            }
            status = child.wait() => status?,
        };
        let stdout = read_pipe(child.stdout.take()).await;
        let stderr = read_pipe(child.stderr.take()).await;

        Ok(Some(serde_json::json!({
            "command": command,
            "cwd": sandbox.root().display().to_string(),
            "exit_code": status.code(),
            "stdout": stdout,
            "stderr": stderr,
        })))
    }

    fn interrupted(&self, session_id: i64) -> Result<SessionStatus> {
        self.record(
            session_id,
            EntryType::Error,
            None,
            serde_json::json!({ "message": "prompt cancelled", "code": "CANCELLED" }),
        )?;
        Ok(SessionStatus::Interrupted)
    }

    /// Append a transcript entry and stream it to every connected client.
    fn record(
        &self,
        session_id: i64,
        entry_type: EntryType,
        origin_device_id: Option<i64>,
        content: serde_json::Value,
    ) -> Result<()> {
        let entry = TranscriptRepository::new(&self.db).append(
            session_id,
            entry_type,
            origin_device_id,
            &content,
        )?;
        let origin_device_name = match origin_device_id {
            Some(device_id) => Some(
                crate::storage::pairing::PairedDeviceRepository::new(&self.db)
                    .get(device_id)?
                    .device_name,
            ),
            None => None,
        };
        self.events.publish(ServerMessage::TranscriptEntryMessage {
            session_id,
            entry: TranscriptEntryPayload::new(&entry, origin_device_name),
        });
        Ok(())
    }
}

/// Everything one prompt run needs while executing its actions.
struct RunContext<'a> {
    session: &'a AgentSession,
    project: &'a Project,
    sandbox: &'a Sandbox,
    origin_device_id: Option<i64>,
    token: &'a CancelToken,
}

/// Approval kind and client-visible details for an action.
fn describe(action: &AgentAction, sandbox: &Sandbox) -> Result<(ApprovalKind, serde_json::Value)> {
    Ok(match action {
        AgentAction::ReadFile { path } => (
            ApprovalKind::FileChange,
            serde_json::json!({
                "path": sandbox.resolve(path)?.display().to_string(),
                "operation": "read",
            }),
        ),
        AgentAction::WriteFile { path, contents } => (
            ApprovalKind::FileChange,
            serde_json::json!({
                "path": sandbox.resolve(path)?.display().to_string(),
                "operation": "write",
                "bytes": contents.len(),
            }),
        ),
        AgentAction::DeleteFile { path } => (
            ApprovalKind::FileChange,
            serde_json::json!({
                "path": sandbox.resolve(path)?.display().to_string(),
                "operation": "delete",
            }),
        ),
        AgentAction::RunCommand { command } => (
            ApprovalKind::Command,
            serde_json::json!({
                "command": command,
                "cwd": sandbox.root().display().to_string(),
            }),
        ),
        AgentAction::Message { text } => (
            ApprovalKind::Command,
            serde_json::json!({ "message": text }),
        ),
    })
}

/// Drain a child process pipe into a lossy UTF-8 string.
async fn read_pipe<R>(pipe: Option<R>) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;

    let mut buffer = Vec::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_end(&mut buffer).await;
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

fn shell_command(command: &str) -> tokio::process::Command {
    if cfg!(windows) {
        let mut shell = tokio::process::Command::new("cmd");
        shell.arg("/C").arg(command);
        shell
    } else {
        let mut shell = tokio::process::Command::new("sh");
        shell.arg("-c").arg(command);
        shell
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plans_with_and_without_a_code_fence() {
        let plain = AgentPlan::parse(
            r#"{ "reasoning": "r", "actions": [{ "type": "message", "text": "hi" }], "summary": "s" }"#,
        )
        .unwrap();
        assert_eq!(plain.reasoning, "r");
        assert_eq!(
            plain.actions,
            vec![AgentAction::Message { text: "hi".into() }]
        );

        let fenced = AgentPlan::parse(
            "```json\n{ \"actions\": [{ \"type\": \"run_command\", \"command\": \"ls\" }] }\n```",
        )
        .unwrap();
        assert_eq!(
            fenced.actions,
            vec![AgentAction::RunCommand {
                command: "ls".into()
            }]
        );

        assert!(matches!(
            AgentPlan::parse("sorry, I cannot"),
            Err(Error::Provider(_))
        ));
    }

    #[test]
    fn only_reads_and_messages_are_read_only() {
        assert!(AgentAction::ReadFile { path: "a".into() }.is_read_only());
        assert!(AgentAction::Message { text: "a".into() }.is_read_only());
        assert!(!AgentAction::WriteFile {
            path: "a".into(),
            contents: String::new()
        }
        .is_read_only());
        assert!(!AgentAction::RunCommand {
            command: "rm -rf /".into()
        }
        .is_read_only());
    }
}
