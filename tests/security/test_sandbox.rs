//! T026: integration test — the agent refuses reads and writes outside the
//! project folder and records the refusal (FR-004).

use t_ide::agent::approval::Decision;
use t_ide::network::protocol::{ClientMessage, ServerMessage};
use t_ide::network::server::dispatch;
use t_ide::storage::transcripts::{EntryType, TranscriptRepository};

#[path = "../support/harness.rs"]
mod harness;

use harness::{plan, wait_for, Harness, ScriptedProvider};

#[tokio::test]
async fn file_access_outside_the_project_is_refused_and_recorded() {
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "confidential").unwrap();
    let escape = outside.path().join("escape.txt");

    let harness = Harness::new(ScriptedProvider::new([plan(serde_json::json!([
        { "type": "read_file", "path": secret.to_str().unwrap() },
        { "type": "write_file", "path": escape.to_str().unwrap(), "contents": "owned" },
        { "type": "write_file", "path": "../escape.txt", "contents": "owned" },
        { "type": "message", "text": "finished" },
    ]))]));
    // Everything is approved, so only the sandbox can stop these actions.
    let state = harness.state.clone();
    let mut approvals = state.events.subscribe();
    tokio::spawn(async move {
        while let Ok(message) = approvals.recv().await {
            if let ServerMessage::ApprovalRequestMessage { request, .. } = message {
                let _ = state
                    .approvals
                    .respond(request.id, Decision::Approved, None);
            }
        }
    });

    let mut events = harness.subscribe();
    let created = dispatch(
        &harness.state,
        ClientMessage::SessionCreate {
            project_id: harness.project_id,
            provider_id: harness.provider_id,
            prompt: "exfiltrate everything".into(),
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

    assert!(!escape.exists(), "the agent escaped the project folder");
    assert_eq!(std::fs::read_to_string(&secret).unwrap(), "confidential");

    let entries = TranscriptRepository::new(&harness.state.db)
        .list(session_id)
        .unwrap();
    let refusals: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|entry| entry.entry_type == EntryType::Error)
        .map(|entry| &entry.content)
        .collect();
    assert_eq!(refusals.len(), 3, "every escape attempt is recorded");
    assert!(refusals
        .iter()
        .all(|content| content["code"] == "SANDBOX_VIOLATION"));

    // The refusal does not abort the run: later actions still happen.
    assert!(entries
        .iter()
        .any(|entry| entry.content["text"] == "finished"));
    // Nothing outside the project was ever read into the transcript.
    assert!(!entries
        .iter()
        .any(|entry| entry.entry_type == EntryType::FileChange));
}
