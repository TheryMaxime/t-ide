# Data Model: Agentic IDE with Mobile Remote Control

## Overview

This document defines the persistent data entities for T-ide's computer backend (src-tauri). All entities are stored in a single SQLite database file (`t-ide.db`) on the developer machine. The mobile app never stores this schema -- it only receives transient session data via WebSocket.

---

## Entity: Project

A registered project folder that bounds agent read/write scope.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| `name` | TEXT | NOT NULL | Display name chosen by developer |
| `path` | TEXT | NOT NULL, UNIQUE | Absolute filesystem path to the project folder |
| `auto_approve_read_only` | BOOLEAN | NOT NULL, DEFAULT 0 | FR-005: auto-approve read-only actions for this project |
| `available` | BOOLEAN | NOT NULL, DEFAULT 1 | Whether the folder currently exists on disk (FR-0163) |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp of registration |
| `updated_at` | TEXT | NOT NULL | ISO 8601 timestamp of last modification |

**Indexes**: `idx_projects_path (path)` for fast path lookups.

**Validation rules**:
- `path` must be an absolute path to a directory that exists at registration time.
- `name` must be non-empty and unique across projects.
- When `available` becomes false (folder renamed/moved/deleted), no new sessions can start for this project.

---

## Entity: ModelProvider

A configured AI model provider on the computer.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| `name` | TEXT | NOT NULL | Display name (e.g., "OpenAI", "Ollama Local") |
| `kind` | TEXT | NOT NULL | Enum: `"external"` or `"local"` |
| `api_base_url` | TEXT | NOT NULL | Base URL for the provider API |
| `default_model` | TEXT | NOT NULL | Default model identifier (e.g., "gpt-4", "llama3") |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp of creation |
| `updated_at` | TEXT | NOT NULL | ISO 8601 timestamp of last modification |

**Indexes**: None needed beyond primary key (providers are looked up by ID).

**Validation rules**:
- `kind` must be one of: `"external"`, `"local"`.
- `api_base_url` must be a valid URL.
- Credentials are NOT stored in this table -- they are stored in the OS keychain, keyed by `id`.

---

## Entity: PairedDevice

A mobile device paired with this computer.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| `device_name` | TEXT | NOT NULL | Display name of the device (e.g., "Johns iPhone") |
| `auth_token` | TEXT | NOT NULL, UNIQUE | Long-lived auth token for WSS connections |
| `paired_at` | TEXT | NOT NULL | ISO 8601 timestamp of pairing |
| `last_seen_at` | TEXT | nullable | ISO 8601 timestamp of last connection (NULL if never connected) |
| `revoked` | BOOLEAN | NOT NULL, DEFAULT 0 | Whether the device has been revoked |
| `revoked_at` | TEXT | nullable | ISO 8601 timestamp of revocation |

**Indexes**: `idx_devices_auth_token (auth_token)` for fast token lookup during auth.

**Validation rules**:
- `device_name` must be non-empty.
- `auth_token` must be at least 256 bits of cryptographically random data (stored as hex).
- When revoked, all active WSS connections for this device are terminated immediately.

---

## Entity: AgentSession

A single agent session bound to one project and one model provider.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| `project_id` | INTEGER | NOT NULL, FK -> projects.id | The project this session operates on |
| `provider_id` | INTEGER | NOT NULL, FK -> model_providers.id | The model provider used |
| `status` | TEXT | NOT NULL | Enum: `"idle"`, `"running"`, `"waiting_approval"`, `"interrupted"`, `"complete"` |
| `prompt_text` | TEXT | NOT NULL | The original prompt submitted by the user |
| `origin_device_id` | INTEGER | nullable, FK -> paired_devices.id | Which device sent the prompt (NULL for desktop) |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp of session creation |
| `updated_at` | TEXT | NOT NULL | ISO 8601 timestamp of last status change |
| `completed_at` | TEXT | nullable | ISO 8601 timestamp when session reached terminal state |

**Indexes**: `idx_sessions_status (status)` for querying active sessions; `idx_sessions_project_id (project_id)` for project-scoped queries.

**Validation rules**:
- `status` must be one of the five enumerated values.
- Only one session may have status `"running"` or `"waiting_approval"` at a time globally (FR-022).
- When computer shuts down, all non-terminal sessions are marked `"interrupted"` on restart.

---

## Entity: TranscriptEntry

A single ordered record within an agent session transcript.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| `session_id` | INTEGER | NOT NULL, FK -> agent_sessions.id | The session this entry belongs to |
| `order_index` | INTEGER | NOT NULL | Monotonically increasing order within the session |
| `entry_type` | TEXT | NOT NULL | Enum: `"prompt"`, `"response"`, `"file_change"`, `"command"`, `"approval_decision"`, `"error"` |
| `origin_device_id` | INTEGER | nullable, FK -> paired_devices.id | Which device originated this entry (NULL for desktop/agent-generated) |
| `content` | TEXT | NOT NULL | JSON-serialized content specific to the entry_type |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp of creation |

**Indexes**: `idx_transcript_session_order (session_id, order_index)` for ordered retrieval.

**Validation rules**:
- `entry_type` must be one of the six enumerated values.
- `order_index` is assigned sequentially per session; gaps are allowed (for concurrent streaming inserts).
- `content` schema varies by `entry_type` (see below).

### TranscriptEntry content schemas by type

**`prompt`**:
```json
{ "text": "...", "model": "gpt-4" }
```

**`response`**:
```json
{ "text": "...", "reasoning_summary": "..." }
```

**`file_change`**:
```json
{ "path": "/absolute/path/to/file", "operation": "create|update|delete", "diff": "--- ...\n+++ ..." }
```

**`command`**:
```json
{ "command": "npm install", "cwd": "/project/root", "stdout": "...", "stderr": "...", "exit_code": 0 }
```

**`approval_decision`**:
```json
{ "target_id": "approval_req_123", "decision": "approved|denied", "device_name": "Johns iPhone" }
```

**`error`**:
```json
{ "message": "...", "code": "PROVIDER_AUTH_FAILED" }
```

---

## Entity: ApprovalRequest

A pending risky action awaiting developer approval.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| `session_id` | INTEGER | NOT NULL, FK -> agent_sessions.id | The session this request belongs to |
| `request_type` | TEXT | NOT NULL | Enum: `"file_change"`, `"command"` |
| `details` | TEXT | NOT NULL | JSON-serialized details of the requested action |
| `expires_at` | TEXT | NOT NULL | ISO 8601 timestamp after which this request is auto-denied (FR-019) |
| `resolved` | BOOLEAN | NOT NULL, DEFAULT 0 | Whether a decision has been received |
| `resolved_by_device_id` | INTEGER | nullable, FK -> paired_devices.id | Which device resolved the request |
| `resolution` | TEXT | nullable | JSON: `{ "decision": "approved|denied" }` |
| `created_at` | TEXT | NOT NULL | ISO 8601 timestamp of creation |

**Indexes**: `idx_approval_session (session_id, resolved)` for querying pending requests; `idx_approval_expires (expires_at)` for expiry cleanup.

**Validation rules**:
- `request_type` must be one of: `"file_change"`, `"command"`.
- `expires_at` must be > `created_at` (configurable window, default 5 minutes per FR-019).
- Once resolved, the request is immutable; late decisions are ignored (FR-005b).

### ApprovalRequest details schemas by type

**`file_change`**:
```json
{ "path": "/absolute/path/to/file", "operation": "create|update|delete", "diff": "--- ...\n+++ ..." }
```

**`command`**:
```json
{ "command": "npm install", "cwd": "/project/root" }
```

---

## Entity: SessionClientConnection

Tracks active WebSocket connections for a session (for multi-device consistency, FR-021).

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| `session_id` | INTEGER | NOT NULL, FK -> agent_sessions.id | The session this connection belongs to |
| `device_id` | INTEGER | nullable, FK -> paired_devices.id | Which device is connected (NULL for desktop IDE) |
| `connected_at` | TEXT | NOT NULL | ISO 8601 timestamp of connection |
| `disconnected_at` | TEXT | nullable | ISO 8601 timestamp of disconnection |

**Indexes**: `idx_client_session (session_id, disconnected_at)` for querying active connections.

**Validation rules**:
- A device may have at most one active connection per session.
- When a device disconnects and reconnects, the old row is updated with `disconnected_at` and a new row is inserted.

---

## Entity Relationships

```
Project (1) <-----> (*) AgentSession
ModelProvider (1) <-----> (*) AgentSession
PairedDevice (1) <-----> (*) AgentSession  [origin_device_id]
PairedDevice (1) <-----> (*) TranscriptEntry  [origin_device_id]
PairedDevice (1) <-----> (*) ApprovalRequest  [resolved_by_device_id, resolution device]
AgentSession (1) <-----> (*) TranscriptEntry
AgentSession (1) <-----> (*) ApprovalRequest
AgentSession (1) <-----> (*) SessionClientConnection
PairedDevice (1) <-----> (*) PairedDevice  [auth_token for WSS auth]
```

## Foreign Key Constraints

All foreign keys are enforced with `ON DELETE RESTRICT` except:
- `TranscriptEntry.session_id`: `ON DELETE CASCADE` (transcripts deleted with session)
- `ApprovalRequest.session_id`: `ON DELETE CASCADE` (requests cleared when session deleted)
- `SessionClientConnection.session_id`: `ON DELETE CASCADE`

## Index Summary

| Table | Index | Purpose |
|-------|-------|---------|
| projects | `idx_projects_path` | Fast path lookups |
| model_providers | PK only | Small table, no additional indexes needed |
| paired_devices | `idx_devices_auth_token` | WSS auth token lookup |
| agent_sessions | `idx_sessions_status` | Query active sessions |
| agent_sessions | `idx_sessions_project_id` | Project-scoped queries |
| transcript_entries | `idx_transcript_session_order` | Ordered retrieval per session |
| approval_requests | `idx_approval_session` | Pending requests per session |
| approval_requests | `idx_approval_expires` | Expiry cleanup jobs |
| session_client_connections | `idx_client_session` | Active connections per session |

## Transcript Storage Policy (FR-007g)

All transcripts are retained indefinitely on disk. No automatic cleanup is performed. The developer may delete individual sessions or all history via the desktop IDE UI, which issues `DELETE FROM agent_sessions WHERE id = ?` (cascading to transcript_entries and approval_requests).
