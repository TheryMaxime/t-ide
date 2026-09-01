# Implementation Plan: Agentic IDE with Mobile Remote Control

**Branch**: `001-agentic-ide-mobile-control` | **Date**: 2026-09-01 | **Spec**: `/specs/001-agentic-ide-mobile-control/spec.md`

**Input**: Feature specification from `/specs/001-agentic-ide-mobile-control/spec.md`

## Summary

T-ide is an agentic IDE that runs on the developer computer with a companion mobile app for remote control. The desktop IDE registers project folders, accepts natural-language prompts, and runs an agent that reads/writes files and executes commands within the project scope. A paired mobile device can send prompts, watch streamed output in real time, approve risky actions, and review session history -- all over the local network with encrypted mutual authentication. Only one agent session runs at a time across all projects; approvals are first-response-wins across devices.

## Technical Context

**Language/Version**: Rust 1.80+ (stable), React 19 with TypeScript for desktop UI, React Native 0.76+ for mobile app

**Primary Dependencies**:
- **Tauri 2.0** -- Desktop application framework (Rust backend + WebView frontend)
- **Tokio** -- Async runtime for Rust backend
- **Axum** -- HTTP/WebSocket server for local network communication
- **SQLite** (via `sqlx` or `rusqlite`) -- Local transcript and metadata storage
- **WebSocket** (`tokio-tungstenite`) -- Real-time bidirectional streaming between desktop IDE, mobile app, and agent
- **Tauri 2.0 IPC** -- Internal communication between Rust backend and React frontend
- **Bring-your-own-model SDKs** -- OpenAI-compatible API client, local model inference (e.g., llama.cpp bindings or HTTP endpoint)

**Storage**: SQLite on local disk for transcripts, session state, paired devices, project registry. File system for actual file reads/writes within project scope.

**Testing**: `cargo test` for Rust backend unit/integration tests; Vitest + React Testing Library for desktop frontend; Detox or Maestro for mobile E2E tests; contract tests for WebSocket protocol.

**Target Platform**: 
- Desktop: Windows 10+, macOS 13+, Linux (x86_64, aarch64)
- Mobile: iOS 15+, Android 24+

**Project Type**: Desktop application (Tauri 2.0 + Rust backend + React frontend) with companion mobile app (React Native).

**Performance Goals**: 
- Prompt visible on phone within 3 seconds of sending (SC-002)
- 95% of streamed output appears on phone within 1 second (SC-003)
- Agent stops within 5 seconds of cancellation request (FR-006)

**Constraints**: 
- LAN-only connectivity for this feature (no internet relay)
- Single-user tool -- no multi-device collaboration beyond the developers own devices
- Provider credentials never leave the computer
- Exactly one agent session at a time across all projects
- No offline queuing of prompts on mobile

**Scale/Scope**: Single developer, several registered projects, indefinite transcript storage (developer-managed cleanup).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Compliance | Notes |
|-----------|------------|-------|
| I. Documentation-First | PASS | This plan + spec.md precede implementation. All artifacts documented before code written. |
| II. Tests Are Mandatory | PASS | Testing strategy defined per layer (unit, integration, contract, E2E). Every FR maps to at least one test. |
| III. Spec-Driven Change Flow | PASS | This plan derives directly from spec.md requirements. Ambiguities resolved in research phase before implementation. |
| IV. Living Documentation | PASS | Plan includes update-in-same-change policy aligned with constitution. |
| V. Simplicity and Explicitness | PASS | Tauri 2.0 + Rust + React is a straightforward stack. No speculative abstractions introduced. SQLite chosen over PostgreSQL for simplicity (single-machine storage). |

**GATE RESULT**: All gates pass. Proceeding to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/001-agentic-ide-mobile-control/
+-- plan.md              # This file
+-- research.md          # Phase 0 output
+-- data-model.md        # Phase 1 output
+-- quickstart.md        # Phase 1 output
+-- contracts/           # Phase 1 output (WebSocket protocol, API contracts)
+-- tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
# Desktop IDE (Tauri 2.0 app)
src-tauri/
+-- Cargo.toml
+-- src/
|   +-- main.rs            # Tauri entry point
|   +-- agent/             # Agent engine: prompt processing, file ops, command execution
|   |   +-- session.rs     # Session lifecycle (idle -> running -> waiting_approval -> complete/interrupted)
|   |   +-- provider.rs    # Model provider abstraction + credential management
|   |   +-- approval.rs    # Approval request handling (first-response-wins across devices)
|   +-- storage/           # SQLite persistence layer
|   |   +-- transcripts.rs # Session transcript read/write
|   |   +-- projects.rs    # Project registry CRUD
|   |   +-- pairing.rs     # Paired device management
|   +-- network/           # Local network communication
|   |   +-- server.rs      # HTTP + WebSocket server (Axum)
|   |   +-- discovery.rs   # mDNS/Bonjour service advertisement for phone auto-discovery
|   |   +-- auth.rs        # Pairing code generation, mutual TLS / token-based auth
|   +-- security/          # Security primitives
|       +-- sandbox.rs     # File system sandbox (project-scoped read/write)
|       +-- credentials.rs # Credential storage (OS keychain), never exposed to frontend/mobile
+-- resources/             # Static assets
+-- build.rs              # Tauri build configuration

# Desktop Frontend (React + TypeScript, served by Tauri)
src/
+-- components/            # Reusable UI components
|   +-- SessionPanel/      # Active session view with streamed transcript
|   +-- ProjectList/       # Registered projects list
|   +-- ApprovalRequest/   # Approval dialog component
|   +-- Settings/          # Model provider config, pairing management
+-- pages/                 # Top-level pages (Home, Session, Settings)
+-- hooks/                 # React hooks for Tauri IPC and WebSocket connections
+-- services/              # API client abstractions
+-- types/                 # TypeScript type definitions

# Mobile App (separate project or workspace package)
mobile/
+-- app/                   # Expo/React Native screens
|   +-- Home.tsx           # Computer discovery + pairing screen
|   +-- SessionList.tsx    # Active/past sessions
|   +-- SessionView.tsx    # Live streamed transcript view
|   +-- Settings.tsx       # Profile, paired devices management
+-- src/
|   +-- hooks/             # WebSocket connection hooks
|   +-- services/          # API client for computer communication
|   +-- types/             # TypeScript types matching desktop contracts

# Tests
tests/
+-- agent/                 # Agent engine unit tests
+-- storage/               # Storage layer integration tests
+-- network/               # Network/auth contract tests
+-- frontend/              # React component tests (Vitest + RTL)
+-- e2e/                   # End-to-end test scripts

# Configuration
.tauri/                    # Tauri configuration
package.json               # Desktop frontend dependencies
```

**Structure Decision**: Tauri 2.0 provides the desktop application shell with Rust backend and WebView frontend. The mobile app is a separate React Native project in the `mobile/` directory (or monorepo package). SQLite for local storage avoids external database dependencies. WebSocket protocol serves as the single communication contract between all clients and the computer.

## Complexity Tracking

> No constitution violations to justify -- all gates passed on first evaluation.
