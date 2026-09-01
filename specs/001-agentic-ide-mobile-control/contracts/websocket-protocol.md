# Contract: WebSocket Protocol (Computer <-> Clients)

## Overview

All communication between the desktop IDE, mobile app, and the computer backend uses WebSocket over TLS (WSS). The protocol is JSON-based with a message type field and payload. Messages are sent as text frames.

## Connection Lifecycle

### 1. Connect
```
Client -> Computer: WSS connection to wss://{computer-ip}:{port}/ws
Headers: Authorization: Bearer {auth_token}
```

- The `auth_token` is the long-lived token from pairing (PairedDevice.auth_token).
- Unpaired devices receive HTTP 401 and are disconnected.
- Revoked devices receive HTTP 403 and are disconnected.

### 2. Authenticate & Subscribe
After WSS connection is established, client sends:
```json
{ "type": "auth.verify", "token": "{auth_token}" }
```

Computer responds:
```json
{ "type": "auth.verified", "device_name": "...", "status": "ok" }
```

Then client subscribes to session updates (if any active):
```json
{ "type": "session.subscribe", "session_id": {id} }
```

### 3. Stream Active Session
Computer pushes transcript entries:
```json
{ "type": "transcript.entry", "session_id": {id}, "entry": { ... } }
```

Where `entry` matches the TranscriptEntry schema from data-model.md with fields:
- `order_index`, `entry_type`, `origin_device_name`, `content`, `created_at`

### 4. Disconnect
Client sends:
```json
{ "type": "disconnect" }
```

Computer closes WSS connection. If the client disconnects unexpectedly, the computer marks the connection inactive in SessionClientConnection but keeps the session alive (FR-017).

---

## Message Types

### Session Management

#### session.list
**Direction**: Client -> Computer
```json
{ "type": "session.list" }
```
**Response** (Computer -> Client):
```json
{ "type": "session.list_response", "sessions": [ { id, project_name, status, prompt_text, created_at }, ... ] }
```

#### session.create
**Direction**: Client -> Computer
```json
{ "type": "session.create", "project_id": {id}, "provider_id": {id}, "prompt": "..." }
```
**Response**:
```json
{ "type": "session.created", "session_id": {id}, "status": "running" }
```

If a session is already running:
```json
{ "type": "error", "code": "SESSION_BUSY", "message": "Project X is currently running a session", "busy_project": "X" }
```

#### session.cancel
**Direction**: Client -> Computer
```json
{ "type": "session.cancel", "session_id": {id} }
```
**Response**:
```json
{ "type": "session.cancelled", "session_id": {id} }
```

### Prompt Submission

#### prompt.send
**Direction**: Client -> Computer
```json
{ "type": "prompt.send", "session_id": {id}, "text": "...", "model": "..." }
```
**Response**:
```json
{ "type": "prompt.sent", "session_id": {id} }
```

If session is not in a state that accepts prompts:
```json
{ "type": "error", "code": "SESSION_NOT_ACTIVE", "message": "Session is not running" }
```

### Transcript Streaming

#### transcript.entry (pushed by computer)
**Direction**: Computer -> Client
```json
{ "type": "transcript.entry", "session_id": {id}, "entry": { order_index, entry_type, origin_device_name, content, created_at } }
```

Emitted for every agent action: reasoning output, file changes, command execution, approval decisions.

### Approval Workflow

#### approval.request (pushed by computer)
**Direction**: Computer -> Client
```json
{ "type": "approval.request", "session_id": {id}, "request": { id, request_type, details, expires_at } }
```

Where `details` matches the ApprovalRequest schema from data-model.md.

#### approval.respond
**Direction**: Client -> Computer
```json
{ "type": "approval.respond", "request_id": {id}, "decision": "approved|denied" }
```
**Response**:
```json
{ "type": "approval.responded", "request_id": {id}, "resolved_by_device_name": "...", "decision": "approved|denied" }
```

If already resolved:
```json
{ "type": "error", "code": "ALREADY_RESOLVED", "message": "This request was already answered by {device_name}", "resolved_by": "{device_name}" }
```

### Device Pairing (mobile app only)

#### device.pair.request
**Direction**: Mobile -> Computer
```json
{ "type": "device.pair.request", "pairing_code": "123456", "device_name": "Johns iPhone" }
```
**Response**:
```json
{ "type": "device.pair.response", "status": "ok|expired|invalid|max_attempts", "message": "..." }
```

If ok: includes `auth_token` for subsequent connections.

#### device.list
**Direction**: Client -> Computer
```json
{ "type": "device.list" }
```
**Response**:
```json
{ "type": "device.list_response", "devices": [ { id, device_name, paired_at, last_seen_at, revoked }, ... ] }
```

#### device.revoke
**Direction**: Client -> Computer
```json
{ "type": "device.revoke", "device_id": {id} }
```
**Response**:
```json
{ "type": "device.revoked", "device_id": {id}, "device_name": "..." }
```

### Project Management

#### projects.list
**Direction**: Client -> Computer
```json
{ "type": "projects.list" }
```
**Response**:
```json
{ "type": "projects.list_response", "projects": [ { id, name, path, available }, ... ] }
```

### Connection State (mobile app)

#### connection.state (pushed by computer or client)
**Direction**: Bidirectional
```json
{ "type": "connection.state", "state": "connected|reconnecting|computer_unavailable|not_same_network" }
```

- `connected`: WSS connected and authenticated.
- `reconnecting`: Lost connection, attempting to reconnect (FR-017).
- `computer_unavailable`: Cannot reach the computer on the network.
- `not_same_network`: Detected not on same LAN as computer (FR-Edge case).

---

## Error Codes

| Code | Meaning |
|------|---------|
| SESSION_BUSY | A session is already running for the target project |
| SESSION_NOT_ACTIVE | Session exists but is not in a state to accept prompts |
| ALREADY_RESOLVED | Approval request was already answered by another device |
| PAIRING_EXPIRED | Pairing code has expired (FR-009) |
| MAX_ATTEMPTS_REACHED | Too many failed pairing attempts (FR-009) |
| INVALID_TOKEN | Auth token is invalid or revoked |
| PROVIDER_ERROR | Model provider rejected the request (FR-007e) |
| SANDBOX_VIOLATION | Agent attempted to access file outside project scope (FR-004) |

## Reconnection Protocol (FR-017)

When a mobile client reconnects:
1. Sends `auth.verify` with stored token.
2. If valid, sends `session.resume` with last known `session_id` and `last_order_index`.
3. Computer responds with all transcript entries where `order_index > last_order_index`.
4. Client acknowledges receipt; computer stops streaming once caught up.

```json
{ "type": "session.resume", "session_id": {id}, "from_index": {n} }
// Response:
{ "type": "transcript.catchup", "entries": [ ... ] }
// Final:
{ "type": "transcript.complete", "session_id": {id} }
```

## Security Notes

- All WSS connections require valid auth token (FR-012).
- Auth tokens are never transmitted in pairing codes.
- Provider credentials are stored only on the computer; never included in any message.
- Transcript entries never contain provider API keys or raw credential data (FR-007c, FR-007d).
