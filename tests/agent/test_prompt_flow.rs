//! T025: integration test for a full prompt run — file-change approval,
//! rejection, and command approval.

use std::time::Duration;

use t_ide::agent::approval::Decision;
use t_ide::network::protocol::{ClientMessage, ServerMessage};
use t_ide::network::server::{dispatch, AppState};
use t_ide::storage::transcripts::{EntryType, TranscriptRepository};

#[path = "../support/harness.rs"]
mod harness;

use harness::{plan, wait_for, Harness, ScriptedProvider};

/// Answer every approval request with `decision`, like a device would.
fn auto_respond(state: AppState, decision: Decision) {
    let mut events = state.events.subscribe();
    tokio::spawn(async move {
        while let Ok(message) = events.recv().await {
            if let ServerMessage::ApprovalRequestMessage { request, .. } = message {
                let _ = state.approvals.respond(request.id, decision, None);
            }
        }
    });
}

async fn run(harness: &Harness, prompt: &str) -> i64 {
    let mut events = harness.subscribe();
    let created = dispatch(
        &harness.state,
        ClientMessage::SessionCreate {
            project_id: harness.project_id,
            provider_id: harness.provider_id,
            prompt: prompt.into(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let session_id = match created {
        ServerMessage::SessionCreated { session_id, .. } => session_id,
        other => panic!("expected session.created, got {other:?}"),
    };
    wait_for(&mut events, |message| match message {
        ServerMessage::TranscriptComplete { session_id: id } if *id == session_id => Some(()),
        _ => None,
    })
    .await;
    session_id
}

#[tokio::test]
async fn approved_file_change_and_command_are_applied_and_recorded() {
    let harness = Harness::new(ScriptedProvider::new([plan(serde_json::json!([
        { "type": "write_file", "path": "greeting.txt", "contents": "hello" },
        { "type": "run_command", "command": "echo ran-in-project" },
    ]))]));
    auto_respond(harness.state.clone(), Decision::Approved);

    let session_id = run(&harness, "write a greeting and run it").await;

    assert_eq!(
        std::fs::read_to_string(harness.project_file("greeting.txt")).unwrap(),
        "hello"
    );

    let entries = TranscriptRepository::new(&harness.state.db)
        .list(session_id)
        .unwrap();
    let approvals: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|entry| entry.entry_type == EntryType::ApprovalDecision)
        .map(|entry| &entry.content)
        .collect();
    assert_eq!(approvals.len(), 2);
    assert!(approvals
        .iter()
        .all(|content| content["decision"] == "approved"));

    let file_change = entries
        .iter()
        .find(|entry| entry.entry_type == EntryType::FileChange)
        .expect("file change recorded");
    assert_eq!(file_change.content["path"], "greeting.txt");
    assert_eq!(file_change.content["operation"], "create");

    let command = entries
        .iter()
        .find(|entry| entry.entry_type == EntryType::Command)
        .expect("command recorded");
    assert_eq!(command.content["exit_code"], 0);
    assert!(command.content["stdout"]
        .as_str()
        .unwrap()
        .contains("ran-in-project"));
}

#[tokio::test]
async fn denied_file_change_is_not_applied() {
    let harness = Harness::new(ScriptedProvider::new([plan(serde_json::json!([
        { "type": "write_file", "path": "denied.txt", "contents": "nope" },
    ]))]));
    auto_respond(harness.state.clone(), Decision::Denied);

    let session_id = run(&harness, "write a file").await;

    assert!(!harness.project_file("denied.txt").exists());
    let entries = TranscriptRepository::new(&harness.state.db)
        .list(session_id)
        .unwrap();
    let decision = entries
        .iter()
        .find(|entry| entry.entry_type == EntryType::ApprovalDecision)
        .expect("decision recorded");
    assert_eq!(decision.content["decision"], "denied");
    assert!(!entries
        .iter()
        .any(|entry| entry.entry_type == EntryType::FileChange));
}

#[tokio::test]
async fn read_only_actions_skip_approval_when_the_project_opts_in() {
    let harness = Harness::new(ScriptedProvider::new([plan(serde_json::json!([
        { "type": "read_file", "path": "notes.txt" },
    ]))]));
    harness.auto_approve_read_only();
    std::fs::write(harness.project_file("notes.txt"), "content").unwrap();

    // Nothing answers approvals: the run can only finish if reads are
    // auto-approved (FR-005).
    let session_id = run(&harness, "read the notes").await;

    let entries = TranscriptRepository::new(&harness.state.db)
        .list(session_id)
        .unwrap();
    let read = entries
        .iter()
        .find(|entry| entry.entry_type == EntryType::FileChange)
        .expect("read recorded");
    assert_eq!(read.content["operation"], "read");
    assert_eq!(read.content["bytes"], 7);
}

#[tokio::test]
async fn an_unanswered_approval_is_treated_as_a_denial() {
    let harness = Harness::build(
        ScriptedProvider::new([plan(serde_json::json!([
            { "type": "write_file", "path": "timeout.txt", "contents": "nope" },
        ]))]),
        |mut config| {
            config.approval_timeout_secs = 1;
            config
        },
    );
    assert_eq!(
        harness.state.config.approval_timeout(),
        Duration::from_secs(1)
    );

    let session_id = run(&harness, "write a file nobody approves").await;

    assert!(!harness.project_file("timeout.txt").exists());
    let entries = TranscriptRepository::new(&harness.state.db)
        .list(session_id)
        .unwrap();
    let decision = entries
        .iter()
        .find(|entry| entry.entry_type == EntryType::ApprovalDecision)
        .expect("decision recorded");
    assert_eq!(decision.content["decision"], "denied");
}
