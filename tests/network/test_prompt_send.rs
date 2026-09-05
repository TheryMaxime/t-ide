//! T024: contract test for `prompt.send` and `transcript.entry` streaming.

use t_ide::agent::session::{SessionRepository, SessionStatus};
use t_ide::network::protocol::{ClientMessage, ServerMessage};
use t_ide::network::server::{dispatch, encode};
use t_ide::storage::transcripts::{EntryType, TranscriptRepository};

#[path = "../support/harness.rs"]
mod harness;

use harness::{plan, wait_for, Harness, ScriptedProvider};

#[tokio::test]
async fn prompt_send_streams_transcript_entries_in_contract_shape() {
    let harness = Harness::new(ScriptedProvider::new([plan(serde_json::json!([
        { "type": "message", "text": "I inspected the project" }
    ]))]));
    let mut events = harness.subscribe();

    // A session that exists but has not run yet accepts a prompt.
    let session = SessionRepository::new(&harness.state.db)
        .create(harness.project_id, harness.provider_id, "initial", None)
        .unwrap();

    let reply = dispatch(
        &harness.state,
        ClientMessage::PromptSend {
            session_id: session.id,
            text: "look around".into(),
            model: Some("llama3".into()),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        reply,
        ServerMessage::PromptSent {
            session_id: session.id
        }
    );

    // The prompt itself is streamed first, in the shape the contract defines.
    let frame = wait_for(&mut events, |message| match message {
        ServerMessage::TranscriptEntryMessage { entry, .. }
            if entry.entry_type == EntryType::Prompt =>
        {
            Some(message.clone())
        }
        _ => None,
    })
    .await;
    let wire: serde_json::Value = serde_json::from_str(&encode(&frame)).unwrap();
    assert_eq!(wire["type"], "transcript.entry");
    assert_eq!(wire["session_id"], session.id);
    assert_eq!(wire["entry"]["entry_type"], "prompt");
    assert_eq!(wire["entry"]["order_index"], 0);
    assert_eq!(wire["entry"]["content"]["text"], "look around");
    assert_eq!(wire["entry"]["content"]["model"], "llama3");
    assert!(wire["entry"]["origin_device_name"].is_null());
    assert!(wire["entry"]["created_at"].is_string());

    wait_for(&mut events, |message| match message {
        ServerMessage::TranscriptComplete { session_id } if *session_id == session.id => Some(()),
        _ => None,
    })
    .await;

    // Reasoning and the agent's message are recorded too (FR-007).
    let entries = TranscriptRepository::new(&harness.state.db)
        .list(session.id)
        .unwrap();
    let kinds: Vec<EntryType> = entries.iter().map(|entry| entry.entry_type).collect();
    assert_eq!(
        kinds,
        vec![EntryType::Prompt, EntryType::Response, EntryType::Response]
    );
    assert_eq!(entries[2].content["text"], "I inspected the project");
    assert_eq!(
        SessionRepository::new(&harness.state.db)
            .get(session.id)
            .unwrap()
            .status,
        SessionStatus::Complete
    );
}

#[tokio::test]
async fn prompt_send_is_refused_for_a_finished_session() {
    let harness = Harness::new(ScriptedProvider::new([plan(serde_json::json!([]))]));
    let sessions = SessionRepository::new(&harness.state.db);
    let session = sessions
        .create(harness.project_id, harness.provider_id, "done", None)
        .unwrap();
    sessions
        .update_status(session.id, SessionStatus::Complete)
        .unwrap();

    let error = dispatch(
        &harness.state,
        ClientMessage::PromptSend {
            session_id: session.id,
            text: "more work".into(),
            model: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "SESSION_NOT_ACTIVE");
}

#[tokio::test]
async fn prompt_send_for_an_unknown_session_is_not_found() {
    let harness = Harness::new(ScriptedProvider::new([plan(serde_json::json!([]))]));
    let error = dispatch(
        &harness.state,
        ClientMessage::PromptSend {
            session_id: 4242,
            text: "hello".into(),
            model: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "NOT_FOUND");
}
