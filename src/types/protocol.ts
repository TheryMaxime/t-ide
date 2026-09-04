/**
 * TypeScript mirror of `contracts/websocket-protocol.md`.
 *
 * Only the messages used by User Story 1 are modelled; pairing and history
 * messages are added by their own stories.
 */

export type ConnectionState =
  'connected' | 'reconnecting' | 'computer_unavailable' | 'not_same_network';

export type EntryType =
  'prompt' | 'response' | 'file_change' | 'command' | 'approval_decision' | 'error';

export type ApprovalKind = 'file_change' | 'command';

export type Decision = 'approved' | 'denied';

export interface TranscriptEntry {
  order_index: number;
  entry_type: EntryType;
  origin_device_name: string | null;
  content: Record<string, unknown>;
  created_at: string;
}

export interface ApprovalRequest {
  id: number;
  request_type: ApprovalKind;
  details: Record<string, unknown>;
  expires_at: string;
}

export interface SessionSummary {
  id: number;
  project_name: string;
  status: string;
  prompt_text: string;
  created_at: string;
}

export interface Project {
  id: number;
  name: string;
  path: string;
  available: boolean;
}

export interface ModelProvider {
  id: number;
  name: string;
  kind: 'external' | 'local';
  api_base_url: string;
  default_model: string;
}

export type ClientMessage =
  | { type: 'session.list' }
  | { type: 'session.create'; project_id: number; provider_id: number; prompt: string }
  | { type: 'session.cancel'; session_id: number }
  | { type: 'prompt.send'; session_id: number; text: string; model?: string }
  | { type: 'approval.respond'; request_id: number; decision: Decision }
  | { type: 'disconnect' };

export type ServerMessage =
  | { type: 'connection.state'; state: ConnectionState }
  | { type: 'session.list_response'; sessions: SessionSummary[] }
  | { type: 'session.created'; session_id: number; status: string }
  | { type: 'session.cancelled'; session_id: number }
  | { type: 'prompt.sent'; session_id: number }
  | { type: 'transcript.entry'; session_id: number; entry: TranscriptEntry }
  | { type: 'transcript.complete'; session_id: number }
  | { type: 'approval.request'; session_id: number; request: ApprovalRequest }
  | {
      type: 'approval.responded';
      request_id: number;
      resolved_by_device_name: string;
      decision: Decision;
    }
  | { type: 'error'; code: string; message: string; busy_project?: string; resolved_by?: string };
