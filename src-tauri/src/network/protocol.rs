//! WebSocket JSON message envelope (T019).
//!
//! Mirrors `contracts/websocket-protocol.md`: every frame is a JSON object with
//! a `type` discriminator and message-specific fields.

use serde::{Deserialize, Serialize};

use crate::agent::approval::Decision;
use crate::storage::transcripts::TranscriptEntry;

/// Messages sent by a client (desktop IDE or mobile app) to the computer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "auth.verify")]
    AuthVerify { token: String },

    #[serde(rename = "session.subscribe")]
    SessionSubscribe { session_id: i64 },

    #[serde(rename = "session.list")]
    SessionList,

    #[serde(rename = "session.create")]
    SessionCreate {
        project_id: i64,
        provider_id: i64,
        prompt: String,
    },

    #[serde(rename = "session.cancel")]
    SessionCancel { session_id: i64 },

    #[serde(rename = "session.resume")]
    SessionResume { session_id: i64, from_index: i64 },

    #[serde(rename = "prompt.send")]
    PromptSend {
        session_id: i64,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    },

    #[serde(rename = "approval.respond")]
    ApprovalRespond { request_id: i64, decision: Decision },

    #[serde(rename = "device.pair.request")]
    DevicePairRequest {
        pairing_code: String,
        device_name: String,
    },

    #[serde(rename = "device.list")]
    DeviceList,

    #[serde(rename = "device.revoke")]
    DeviceRevoke { device_id: i64 },

    #[serde(rename = "projects.list")]
    ProjectsList,

    #[serde(rename = "connection.state")]
    ConnectionState { state: ConnectionState },

    #[serde(rename = "disconnect")]
    Disconnect,
}

/// Messages pushed by the computer to a client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "auth.verified")]
    AuthVerified { device_name: String, status: String },

    #[serde(rename = "session.list_response")]
    SessionListResponse { sessions: Vec<SessionSummary> },

    #[serde(rename = "session.created")]
    SessionCreated { session_id: i64, status: String },

    #[serde(rename = "session.cancelled")]
    SessionCancelled { session_id: i64 },

    #[serde(rename = "prompt.sent")]
    PromptSent { session_id: i64 },

    #[serde(rename = "transcript.entry")]
    TranscriptEntryMessage {
        session_id: i64,
        entry: TranscriptEntryPayload,
    },

    #[serde(rename = "transcript.catchup")]
    TranscriptCatchup {
        entries: Vec<TranscriptEntryPayload>,
    },

    #[serde(rename = "transcript.complete")]
    TranscriptComplete { session_id: i64 },

    #[serde(rename = "approval.request")]
    ApprovalRequestMessage {
        session_id: i64,
        request: ApprovalRequestPayload,
    },

    #[serde(rename = "approval.responded")]
    ApprovalResponded {
        request_id: i64,
        resolved_by_device_name: String,
        decision: Decision,
    },

    #[serde(rename = "device.pair.response")]
    DevicePairResponse {
        status: PairingStatus,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auth_token: Option<String>,
    },

    #[serde(rename = "device.list_response")]
    DeviceListResponse { devices: Vec<DeviceSummary> },

    #[serde(rename = "device.revoked")]
    DeviceRevoked { device_id: i64, device_name: String },

    #[serde(rename = "projects.list_response")]
    ProjectsListResponse { projects: Vec<ProjectSummary> },

    #[serde(rename = "connection.state")]
    ConnectionState { state: ConnectionState },

    #[serde(rename = "error")]
    Error {
        code: String,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        busy_project: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resolved_by: Option<String>,
    },
}

impl ServerMessage {
    /// Build an error frame from a backend error.
    pub fn from_error(error: &crate::Error) -> Self {
        let (busy_project, resolved_by) = match error {
            crate::Error::SessionBusy { project } => (Some(project.clone()), None),
            crate::Error::AlreadyResolved { resolved_by } => (None, Some(resolved_by.clone())),
            _ => (None, None),
        };
        ServerMessage::Error {
            code: error.code().to_string(),
            message: error.to_string(),
            busy_project,
            resolved_by,
        }
    }
}

/// Connection state reported to (and by) mobile clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Connected,
    Reconnecting,
    ComputerUnavailable,
    NotSameNetwork,
}

/// Outcome of a pairing attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairingStatus {
    Ok,
    Expired,
    Invalid,
    MaxAttempts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: i64,
    pub project_name: String,
    pub status: String,
    pub prompt_text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSummary {
    pub id: i64,
    pub device_name: String,
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    pub revoked: bool,
}

/// Wire form of a transcript entry: device names, never device ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptEntryPayload {
    pub order_index: i64,
    pub entry_type: crate::storage::transcripts::EntryType,
    pub origin_device_name: Option<String>,
    pub content: serde_json::Value,
    pub created_at: String,
}

impl TranscriptEntryPayload {
    pub fn new(entry: &TranscriptEntry, origin_device_name: Option<String>) -> Self {
        Self {
            order_index: entry.order_index,
            entry_type: entry.entry_type,
            origin_device_name,
            content: entry.content.clone(),
            created_at: entry.created_at.clone(),
        }
    }
}

/// Wire form of an approval request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequestPayload {
    pub id: i64,
    pub request_type: crate::agent::approval::ApprovalKind,
    pub details: serde_json::Value,
    pub expires_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_client_frames_from_the_contract() {
        let create: ClientMessage = serde_json::from_str(
            r#"{ "type": "session.create", "project_id": 1, "provider_id": 2, "prompt": "hi" }"#,
        )
        .unwrap();
        assert_eq!(
            create,
            ClientMessage::SessionCreate {
                project_id: 1,
                provider_id: 2,
                prompt: "hi".into()
            }
        );

        let respond: ClientMessage = serde_json::from_str(
            r#"{ "type": "approval.respond", "request_id": 7, "decision": "approved" }"#,
        )
        .unwrap();
        assert_eq!(
            respond,
            ClientMessage::ApprovalRespond {
                request_id: 7,
                decision: Decision::Approved
            }
        );

        let resume: ClientMessage = serde_json::from_str(
            r#"{ "type": "session.resume", "session_id": 3, "from_index": 12 }"#,
        )
        .unwrap();
        assert_eq!(
            resume,
            ClientMessage::SessionResume {
                session_id: 3,
                from_index: 12
            }
        );

        assert!(serde_json::from_str::<ClientMessage>(r#"{ "type": "nope" }"#).is_err());
    }

    #[test]
    fn serializes_server_frames_with_a_type_tag() {
        let message = ServerMessage::SessionCreated {
            session_id: 5,
            status: "running".into(),
        };
        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(json["type"], "session.created");
        assert_eq!(json["session_id"], 5);
    }

    #[test]
    fn maps_backend_errors_onto_protocol_codes() {
        let busy = ServerMessage::from_error(&crate::Error::SessionBusy {
            project: "demo".into(),
        });
        let json = serde_json::to_value(&busy).unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["code"], "SESSION_BUSY");
        assert_eq!(json["busy_project"], "demo");

        let resolved = ServerMessage::from_error(&crate::Error::AlreadyResolved {
            resolved_by: "phone".into(),
        });
        let json = serde_json::to_value(&resolved).unwrap();
        assert_eq!(json["code"], "ALREADY_RESOLVED");
        assert_eq!(json["resolved_by"], "phone");
        assert!(json.get("busy_project").is_none());
    }
}
