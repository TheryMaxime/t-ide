# Research: Agentic IDE with Mobile Remote Control

## Decision 1: Desktop Application Framework -- Tauri 2.0

**Decision**: Use Tauri 2.0 for the desktop IDE.

**Rationale**: 
- Tauri 2.0 provides a Rust backend (for security-sensitive operations like credential storage, file sandboxing, agent execution) with a WebView frontend (for React UI).
- Native binary distribution across Windows/macOS/Linux without Electron's ~150MB baseline per instance.
- Built-in IPC between Rust backend and WebView frontend eliminates need for separate HTTP server for desktop-local communication.
- Tauri 2.0's plugin system allows extending the Rust core with additional capabilities (e.g., file watcher, process management).

**Alternatives considered**:
- **Electron + Node.js**: Larger binary footprint, JavaScript in backend reduces security isolation. Rejected -- Rust provides better memory safety for sandbox operations.
- **Flutter Desktop**: Would require separate language for mobile (Dart) and desktop (Dart), increasing maintenance burden vs. React Native for mobile + Rust for desktop.
- **Wails 2**: Go-based alternative to Tauri; Go lacks Rust's async ecosystem maturity for agent workloads. Rejected -- Rust provides better crates for async I/O, process management, and FFI with ML runtimes.

## Decision 2: Backend HTTP Framework -- Axum

**Decision**: Use Axum (Tokio ecosystem) for the local network WebSocket/HTTP server.

**Rationale**:
- Built on hyper/tower, excellent WebSocket support via `axum-extra`.
- Tokio integration is native; agent session management uses async Rust throughout.
- Type-safe routing and middleware align with Rust ergonomics.
- Actix-web was considered but Axum's simpler API and tower-based middleware stack reduce complexity for this use case.

**Alternatives considered**:
- **Actix-web**: More mature ecosystem but more complex middleware model. Rejected -- Axum provides sufficient WebSocket support with less cognitive overhead.
- **warp**: Deprecated in favor of axum. Not considered further.

## Decision 3: Local Storage -- SQLite

**Decision**: Use SQLite (via `sqlx` for compile-time checked queries) for all local persistence.

**Rationale**:
- Single-file database, zero external dependencies -- ideal for a desktop application.
- Supports the required schema: transcripts (ordered entries), sessions (lifecycle state), projects (registry), paired devices (trust store), approval requests (pending/resolved).
- `sqlx` provides compile-time query validation and migration support via `sqlx migrate`.

**Alternatives considered**:
- **RocksDB**: Better write throughput but unnecessary complexity for single-user desktop workload. Rejected -- SQLite is sufficient for transcript storage at expected scale.
- **JSON files per session**: Simpler schema but no transactional guarantees, harder to query across sessions. Rejected -- SQL queries needed for session history browsing (User Story 5).
- **PostgreSQL/MySQL**: External database dependency adds installation burden. Rejected -- single-machine deployment with SQLite is simpler and meets all requirements.

## Decision 4: Real-Time Communication Protocol -- WebSocket

**Decision**: Use a custom WebSocket protocol over TLS for all client-computer communication.

**Rationale**:
- Bidirectional streaming required for agent output (FR-003), approval requests (FR-005a), and session reconnection (FR-017).
- WebSocket provides persistent connection with low overhead compared to HTTP polling.
- TLS encryption satisfies FR-012 (encrypted in transit) via WSS (WebSocket Secure).

**Protocol design**: JSON messages over WebSocket frames with the following message types:
- `session.list` / `session.create` / `session.cancel` -- session lifecycle
- `transcript.stream` -- ordered transcript entries pushed to connected clients
- `approval.request` / `approval.respond` -- approval workflow
- `prompt.send` / `prompt.status` -- prompt submission and status updates
- `device.pair` / `device.list` / `device.revoke` -- pairing management
- `projects.list` / `projects.select` -- project registry

**Alternatives considered**:
- **gRPC-Web**: Strongly typed but WebSocket framing adds complexity. Rejected -- custom JSON protocol is simpler for this scope and easier to debug.
- **Server-Sent Events + HTTP polling**: Asymmetric (server-to-client only) for streaming; would need separate channel for client-to-server actions. Rejected -- bidirectional WebSocket is cleaner.

## Decision 5: Model Provider Integration -- OpenAI-Compatible HTTP Client

**Decision**: Implement a generic OpenAI-compatible API client that works with any provider exposing an OpenAI-compatible endpoint (OpenAI, Anthropic via proxy, Ollama, LM Studio, etc.).

**Rationale**:
- FR-007a requires supporting external providers the developer holds credentials for.
- FR-007b requires supporting locally-running models.
- Most local model servers (Ollama, LM Studio, llama.cpp server) expose OpenAI-compatible APIs.
- Credentials stored in OS keychain (Windows Credential Manager / macOS Keychain / Linux libsecret via `keyring` crate).

**Alternatives considered**:
- **Provider-specific adapters**: More control per provider but higher maintenance burden. Rejected -- OpenAI-compatible API is the de facto standard for local model servers.
- **LangChain/LlamaIndex bindings**: Overly complex for a single-model-at-a-time use case. Rejected -- direct HTTP client is simpler and more transparent.

## Decision 6: Mobile App Framework -- React Native (Expo)

**Decision**: Use React Native with Expo for the mobile companion app.

**Rationale**:
- Shares TypeScript/React knowledge with desktop frontend, reducing context switching.
- Expo provides managed build pipeline for iOS and Android from a single codebase.
- WebSocket client support is mature (`ws` or native `WebSocket` API).
- Push notifications (FR-020) supported via Expo Notifications.

**Alternatives considered**:
- **Flutter**: Would require learning Dart; React Native shares more code patterns with desktop frontend. Rejected -- React Native reduces cross-platform knowledge overhead.
- **PWA**: Cannot access push notifications or native UI patterns required for approval requests (FR-018). Rejected -- native app needed for mobile UX requirements.

## Decision 7: Device Discovery -- mDNS/Bonjour

**Decision**: Use mDNS (multicast DNS) service advertisement for automatic computer discovery on the local network (FR-013b).

**Rationale**:
- FR-013b requires the phone to find its paired computer without typing an address.
- mDNS is built into iOS, Android, macOS, Windows 10+, and Linux (Avahi).
- Tauri backend can advertise a service via `mdns` crate; mobile app discovers it via native mDNS APIs.

**Alternatives considered**:
- **UPnP port mapping**: Requires router support, not reliable on all networks. Rejected -- mDNS works without network configuration.
- **Manual IP entry**: Violates FR-013b requirement for automatic discovery. Not a viable alternative.

## Decision 8: Authentication -- Pairing Code + Token-Based Auth

**Decision**: Use short-lived pairing codes (6-digit, 5-minute expiry) to establish mutual authentication tokens.

**Rationale**:
- FR-008 requires single-use code pairing; FR-009 requires 5-minute expiry and 5-attempt limit.
- After pairing, the mobile app stores a long-lived auth token for subsequent connections.
- Token rotation on reconnect provides session continuity without re-pairing.
- FR-012 (encrypted in transit) satisfied by requiring WSS; pairing code prevents unpaired devices from obtaining tokens.

**Alternatives considered**:
- **QR code scanning**: Also viable (FR-058 mentions both methods). Rejected -- QR codes require camera access and are less accessible than manual code entry for the pairing flow described in the spec.
- **Certificate-based mutual TLS**: Stronger security but adds PKI complexity unnecessary for LAN-only trust model. Rejected -- token-based auth with WSS is sufficient for single-user local network trust.

## Decision 9: Credential Storage -- OS Keychain

**Decision**: Store provider credentials exclusively in the OS keychain via the `keyring` crate.

**Rationale**:
- FR-007c requires credentials stored only on the computer, never sent to mobile app or appearing in logs/transcripts.
- `keyring` crate provides cross-platform access to Windows Credential Manager, macOS Keychain, and Linux libsecret.
- Credentials are read from keychain at runtime by the Rust backend; never serialized or transmitted.

**Alternatives considered**:
- **Encrypted file storage**: Simpler but less secure than OS keychain. Rejected -- OS keychain is the standard for credential storage on each platform.
- **Environment variables**: Not persistent across restarts, not user-friendly to configure. Rejected -- keychain provides seamless storage without manual setup.

## Decision 10: Session Concurrency -- Single Global Mutex

**Decision**: Enforce exactly one agent session at a time using a global async mutex on the computer (FR-022).

**Rationale**:
- FR-022 explicitly requires at most one agent session across all projects.
- A simple `tokio::sync::Mutex` or atomic flag provides this guarantee with minimal complexity.
- Second prompts are queued or refused with a clear message naming the busy project (FR-022).

**Alternatives considered**:
- **Per-project sessions with global limit of 1**: More complex state machine for no user benefit in single-user context. Rejected -- global mutex is simpler and meets requirements exactly.
