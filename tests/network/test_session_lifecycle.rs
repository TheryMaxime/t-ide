//! T023: contract test for `session.create` / `session.cancel`.

use std::time::{Duration, Instant};

use t_ide::agent::session::{SessionRepository, SessionStatus};
use t_ide::network::protocol::{ClientMessage, ServerMessage};
use t_ide::network::server::{dispatch, encode, parse};
use t_ide::storage::transcripts::EntryType;

#[path = "../support/harness.rs"]
mod harness;

use harness::{plan, wait_for, Harness, ScriptedProvider};

fn create_frame(project_id: i64, provider_id: i64) -> String {
    serde_json::json!({
        "type": "session.create",
        "project_id": project_id,
        "provider_id": provider_id,
        "prompt": "add a greeting",
    })
    .to_string()
}

#[tokio::test]
async fn session_create_starts_a_run_and_answers_with_session_created() {
    let harness = Harness::new(ScriptedProvider::new([plan(serde_json::json!([
        { "type": "message", "text": "nothing to do" }
    ]))]));
    let mut events = harness.subscribe();

    let message = parse(&create_frame(harness.project_id, harness.provider_id)).unwrap();
    let reply = dispatch(&harness.state, message).await.unwrap().unwrap();

    let session_id = match &reply {
        ServerMessage::SessionCreated { session_id, status } => {
            assert_eq!(status, "running");
            *session_id
        }
        other => panic!("expected session.created, got {other:?}"),
    };
    let wire: serde_json::Value = serde_json::from_str(&encode(&reply)).unwrap();
    assert_eq!(wire["type"], "session.created");
    assert_eq!(wire["session_id"], session_id);

    wait_for(&mut events, |message| match message {
        ServerMessage::TranscriptComplete { session_id } => Some(*session_id),
        _ => None,
    })
    .await;
    assert_eq!(
        SessionRepository::new(&harness.state.db)
            .get(session_id)
            .unwrap()
            .status,
        SessionStatus::Complete
    );
}

#[tokio::test]
async fn a_second_session_is_refused_while_one_is_running() {
    let harness = Harness::new(
        ScriptedProvider::new([plan(serde_json::json!([]))]).with_delay(Duration::from_secs(30)),
    );
    let mut events = harness.subscribe();

    dispatch(
        &harness.state,
        ClientMessage::SessionCreate {
            project_id: harness.project_id,
            provider_id: harness.provider_id,
            prompt: "first".into(),
        },
    )
    .await
    .unwrap();

    // The prompt entry is streamed once the session holds the global slot.
    wait_for(&mut events, |message| match message {
        ServerMessage::TranscriptEntryMessage { entry, .. }
            if entry.entry_type == EntryType::Prompt =>
        {
            Some(())
        }
        _ => None,
    })
    .await;

    let error = dispatch(
        &harness.state,
        ClientMessage::SessionCreate {
            project_id: harness.project_id,
            provider_id: harness.provider_id,
            prompt: "second".into(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "SESSION_BUSY");

    let wire: serde_json::Value =
        serde_json::from_str(&encode(&ServerMessage::from_error(&error))).unwrap();
    assert_eq!(wire["code"], "SESSION_BUSY");
    assert_eq!(wire["busy_project"], "demo");
}

#[tokio::test]
async fn session_cancel_stops_the_agent_within_five_seconds() {
    let harness = Harness::new(
        ScriptedProvider::new([plan(serde_json::json!([]))]).with_delay(Duration::from_secs(120)),
    );
    let mut events = harness.subscribe();

    let created = dispatch(
        &harness.state,
        ClientMessage::SessionCreate {
            project_id: harness.project_id,
            provider_id: harness.provider_id,
            prompt: "long running".into(),
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
        ServerMessage::TranscriptEntryMessage { entry, .. }
            if entry.entry_type == EntryType::Prompt =>
        {
            Some(())
        }
        _ => None,
    })
    .await;

    let started = Instant::now();
    let cancelled = dispatch(&harness.state, ClientMessage::SessionCancel { session_id })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cancelled, ServerMessage::SessionCancelled { session_id });

    wait_for(&mut events, |message| match message {
        ServerMessage::TranscriptComplete { session_id: id } if *id == session_id => Some(()),
        _ => None,
    })
    .await;
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "cancellation took {:?}",
        started.elapsed()
    );
    assert_eq!(
        SessionRepository::new(&harness.state.db)
            .get(session_id)
            .unwrap()
            .status,
        SessionStatus::Interrupted
    );
}

#[tokio::test]
async fn cancelling_a_finished_session_is_an_error() {
    let harness = Harness::new(ScriptedProvider::new([plan(serde_json::json!([]))]));
    let mut events = harness.subscribe();

    let created = dispatch(
        &harness.state,
        ClientMessage::SessionCreate {
            project_id: harness.project_id,
            provider_id: harness.provider_id,
            prompt: "quick".into(),
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

    let error = dispatch(&harness.state, ClientMessage::SessionCancel { session_id })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "SESSION_NOT_ACTIVE");
}
