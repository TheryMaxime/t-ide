---

description: "Task list template for feature implementation"
---

# Tasks: Agentic IDE with Mobile Remote Control

**Input**: Design documents from `/specs/001-agentic-ide-mobile-control/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/websocket-protocol.md, quickstart.md

**Tests**: Included. The constitution (Principle II) and FR-025/FR-026/SC-009 require every functional
requirement to map to at least one automated test that fails when the requirement is violated, so contract
and integration test tasks are included per user story.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each
story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Desktop backend**: `src-tauri/src/`
- **Desktop frontend**: `src/`
- **Mobile app**: `mobile/`
- **Tests**: `tests/`

(Per plan.md's Project Structure section.)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [X] T001 Create top-level project structure (`src-tauri/`, `src/`, `mobile/`, `tests/`) per plan.md's Project Structure section
- [X] T002 Initialize Tauri 2.0 project in `src-tauri/Cargo.toml` with `tokio`, `axum`, `axum-extra` (WebSocket), `tokio-tungstenite`, `sqlx` (or `rusqlite`), `keyring`, and `mdns` dependencies
- [X] T003 [P] Initialize React 19 + TypeScript desktop frontend in `src/` with `package.json` (Vite/Tauri frontend scaffold)
- [X] T004 [P] Initialize React Native (Expo) mobile app in `mobile/` with `package.json`
- [X] T005 [P] Configure Rust linting/formatting (`clippy`, `rustfmt.toml`) for `src-tauri/`
- [X] T006 [P] Configure ESLint/Prettier for desktop frontend `src/` and mobile app `mobile/`
- [X] T007 Setup SQLite migration framework (`sqlx migrate` or equivalent) with initial empty migration in `src-tauri/migrations/`

**Checkpoint**: Toolchains installed and building; no functional code yet.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T008 Create SQLite schema migration for all entities (`projects`, `model_providers`, `paired_devices`, `agent_sessions`, `transcript_entries`, `approval_requests`, `session_client_connections`) with indexes and foreign keys per data-model.md in `src-tauri/migrations/0001_initial.sql`
- [X] T009 [P] Implement Project repository (CRUD, path uniqueness, availability check) in `src-tauri/src/storage/projects.rs`
- [X] T010 [P] Implement ModelProvider repository and provider abstraction scaffold in `src-tauri/src/agent/provider.rs`
- [X] T011 [P] Implement PairedDevice repository (create, list, revoke) in `src-tauri/src/storage/pairing.rs`
- [X] T012 [P] Implement AgentSession repository and status state machine (idle→running→waiting_approval→interrupted/complete) in `src-tauri/src/agent/session.rs`
- [X] T013 [P] Implement TranscriptEntry repository (ordered insert/query) in `src-tauri/src/storage/transcripts.rs`
- [X] T014 [P] Implement ApprovalRequest repository (create, resolve-once semantics) in `src-tauri/src/agent/approval.rs`
- [X] T015 [P] Implement SessionClientConnection repository (track active connections per session/device) in `src-tauri/src/storage/connections.rs`
- [X] T016 Implement file system sandbox enforcement (refuse read/write outside project folder, log refusal) in `src-tauri/src/security/sandbox.rs`
- [X] T017 Implement credential storage via OS keychain (`keyring` crate), never exposed to frontend/mobile in `src-tauri/src/security/credentials.rs`
- [X] T018 Setup Axum HTTP/WSS server skeleton (TLS, `/ws` route) in `src-tauri/src/network/server.rs`
- [X] T019 Implement WebSocket JSON message envelope (type-tagged parse/dispatch per contracts/websocket-protocol.md) in `src-tauri/src/network/server.rs`
- [X] T020 Implement global single-session mutex enforcing at most one running/waiting_approval session across all projects (FR-022) in `src-tauri/src/agent/session.rs`
- [X] T021 Configure structured error handling and logging infrastructure in `src-tauri/src/main.rs`
- [X] T022 Configure app/environment configuration management (app data dir, config file) in `src-tauri/src/main.rs`

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Run a prompt on the computer from the desktop IDE (Priority: P1) 🎯 MVP

**Goal**: A developer opens the desktop IDE, submits a prompt against a registered project, and the agent
reads/edits files and runs commands with the developer approving or rejecting every change, all recorded in
a transcript.

**Independent Test**: Open the desktop IDE on a sample project, submit a prompt that requires editing a file
and running a command, and confirm the change is applied only after explicit approval and that the
transcript records every step.

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T023 [P] [US1] Contract test for `session.create` / `session.cancel` messages in `tests/network/test_session_lifecycle.rs`
- [ ] T024 [P] [US1] Contract test for `prompt.send` and `transcript.entry` streaming in `tests/network/test_prompt_send.rs`
- [ ] T025 [P] [US1] Integration test: full prompt run with file-change approval, rejection, and command approval in `tests/agent/test_prompt_flow.rs`
- [ ] T026 [P] [US1] Integration test: agent refuses read/write outside project folder and records refusal in `tests/security/test_sandbox.rs`

### Implementation for User Story 1

- [ ] T027 [US1] Implement OpenAI-compatible model provider HTTP client (external + local) in `src-tauri/src/agent/provider.rs` (depends on T010)
- [ ] T028 [US1] Implement prompt processing engine (reads files, proposes changes, requests command execution, scoped to project) in `src-tauri/src/agent/mod.rs` (depends on T016, T027)
- [ ] T029 [US1] Implement streaming of agent output (reasoning, file changes, command output) to connected clients as `transcript.entry` messages in `src-tauri/src/network/server.rs` (depends on T019, T028)
- [ ] T030 [US1] Implement approval gating for file changes and commands, with per-project auto-approve-read-only setting (FR-005) in `src-tauri/src/agent/approval.rs` (depends on T014, T028)
- [ ] T031 [US1] Implement prompt cancellation stopping the agent within 5 seconds (FR-006) in `src-tauri/src/agent/session.rs` (depends on T012, T028)
- [ ] T032 [US1] Record transcript entries for every prompt, response, file change, command, and approval decision in `src-tauri/src/storage/transcripts.rs` (depends on T013, T028)
- [ ] T033 [US1] Implement `session.list` / `session.create` / `session.cancel` / `prompt.send` WebSocket handlers wiring engine to protocol in `src-tauri/src/network/server.rs` (depends on T018, T028, T031)
- [ ] T034 [US1] [P] Implement Project registration UI (add/rename/remove, availability state) in `src/components/ProjectList/`
- [ ] T035 [US1] [P] Implement Session panel UI (prompt input, streamed transcript, approve/reject controls) in `src/components/SessionPanel/`
- [ ] T036 [US1] [P] Implement Model Provider settings UI (add provider, choose default, select per session) in `src/components/Settings/`
- [ ] T037 [US1] Implement desktop WebSocket client hook connecting frontend to backend session state in `src/hooks/useSession.ts` (depends on T033)

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently on the desktop alone.

---

## Phase 4: User Story 2 - Pair a mobile app with the computer (Priority: P1)

**Goal**: A developer pairs their phone with the computer using a displayed single-use code; the phone
appears in the desktop's trusted device list and can be revoked at any time.

**Independent Test**: Pair a phone with a computer using a displayed code, confirm the device is listed as
trusted on the desktop, revoke it, and confirm the phone can no longer connect.

### Tests for User Story 2 ⚠️

- [ ] T038 [P] [US2] Contract test for `device.pair.request` / `device.pair.response` in `tests/network/test_pairing.rs`
- [ ] T039 [P] [US2] Integration test: pairing code expiry (5 min) and 5-attempt limit in `tests/network/test_pairing_limits.rs`
- [ ] T040 [P] [US2] Integration test: revoking a device terminates its active WSS connection and blocks reconnection in `tests/network/test_device_revoke.rs`

### Implementation for User Story 2

- [ ] T041 [US2] Implement pairing code generation (6-digit, 5-minute expiry, 5-attempt limit) in `src-tauri/src/network/auth.rs` (depends on T011)
- [ ] T042 [US2] Implement auth token issuance and WSS authentication middleware (401 unpaired, 403 revoked) in `src-tauri/src/network/auth.rs` (depends on T018, T041)
- [ ] T043 [US2] Implement `device.list` / `device.revoke` handlers, terminating active connections on revoke in `src-tauri/src/storage/pairing.rs` (depends on T011, T015, T042)
- [ ] T044 [US2] Implement mDNS service advertisement for computer discovery on LAN (FR-013b) in `src-tauri/src/network/discovery.rs`
- [ ] T045 [US2] [P] Implement pairing code display UI on desktop IDE home screen in `src/components/Settings/`
- [ ] T046 [US2] [P] Implement paired devices management UI (list, name, last-seen, revoke) in `src/components/Settings/`
- [ ] T047 [US2] [P] Implement mobile pairing screen (code entry, computer discovery) in `mobile/app/Home.tsx` (depends on T044)
- [ ] T048 [US2] [P] Implement mobile secure auth-token storage and WSS client bootstrap in `mobile/src/services/auth.ts` (depends on T042)

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently (desktop sessions + pairing).

---

## Phase 5: User Story 3 - Send a prompt from the phone and watch it run (Priority: P1)

**Goal**: A paired phone can pick a project, send a prompt, and watch the agent run on the computer with
live streamed output, matching what the desktop IDE shows.

**Independent Test**: With a paired phone, send a prompt from the phone, and verify the agent runs on the
computer and the phone displays the same streamed transcript as the desktop IDE.

### Tests for User Story 3 ⚠️

- [ ] T049 [P] [US3] Integration test: mobile-submitted prompt runs on computer and desktop shows the same live session in `tests/network/test_mobile_prompt.rs`
- [ ] T050 [P] [US3] Integration test: reconnection after network loss resumes session and shows missed output (FR-017) in `tests/network/test_reconnect.rs`
- [ ] T051 [P] [US3] Integration test: prompt attempt while computer unavailable shows explicit state, no silent queueing in `tests/network/test_computer_unavailable.rs`

### Implementation for User Story 3

- [ ] T052 [US3] Implement `projects.list` handler exposing registered projects to connected clients in `src-tauri/src/network/server.rs` (depends on T009, T019)
- [ ] T053 [US3] Implement `session.resume` / `transcript.catchup` / `transcript.complete` reconnection protocol in `src-tauri/src/network/server.rs` (depends on T013, T015, T019)
- [ ] T054 [US3] Implement `connection.state` broadcasting (connected/reconnecting/computer_unavailable/not_same_network) in `src-tauri/src/network/server.rs` (depends on T019)
- [ ] T055 [US3] [P] Implement mobile project selection screen in `mobile/app/SessionList.tsx` (depends on T052)
- [ ] T056 [US3] [P] Implement mobile live session view with streamed transcript in `mobile/app/SessionView.tsx` (depends on T053)
- [ ] T057 [US3] [P] Implement mobile WebSocket client hook with automatic reconnect in `mobile/src/hooks/useWebSocket.ts` (depends on T053, T054)
- [ ] T058 [US3] Implement mobile prompt submission (text entry, send, cancel controls) in `mobile/app/SessionView.tsx` (depends on T056, T057)
- [ ] T059 [US3] [P] Implement mobile "computer unavailable" / "not on same network" state UI in `mobile/app/Home.tsx` (depends on T054)

**Checkpoint**: All P1 stories (US1, US2, US3) complete — the core remote-control product is demonstrable end to end.

---

## Phase 6: User Story 4 - Approve risky actions from the phone (Priority: P2)

**Goal**: While a remote prompt runs, the developer can approve or deny risky agent actions from their
phone, with either device able to answer and first response winning.

**Independent Test**: Trigger a prompt that requires an approval, confirm the request appears on the phone
with the command and target paths, approve it, and verify the action then executes on the computer.

### Tests for User Story 4 ⚠️

- [ ] T060 [P] [US4] Contract test for `approval.request` / `approval.respond` first-response-wins and `ALREADY_RESOLVED` error in `tests/network/test_approval.rs`
- [ ] T061 [P] [US4] Integration test: approval request expires as a denial after configured window (default 5 min) in `tests/agent/test_approval_expiry.rs`

### Implementation for User Story 4

- [ ] T062 [US4] Implement multi-device approval broadcast (`approval.request` pushed to every connected client) in `src-tauri/src/agent/approval.rs` (depends on T014, T030)
- [ ] T063 [US4] Implement first-response-wins resolution, withdrawal notice, and single-decision enforcement (FR-005a/FR-005b) in `src-tauri/src/agent/approval.rs` (depends on T062)
- [ ] T064 [US4] Implement approval expiry background job treating unanswered requests as denied (FR-019) in `src-tauri/src/agent/approval.rs` (depends on T014)
- [ ] T065 [US4] [P] Implement approval request UI on desktop showing command text/affected paths and withdrawal-on-resolve in `src/components/ApprovalRequest/` (depends on T063)
- [ ] T066 [US4] [P] Implement approval request UI on mobile with full command/path details in `mobile/app/SessionView.tsx` (depends on T063)
- [ ] T067 [US4] [P] Implement mobile push notification for pending approvals and finished/failed prompts while backgrounded (FR-020) in `mobile/src/services/notifications.ts`

**Checkpoint**: All user stories should now be independently functional (US1–US4).

---

## Phase 7: User Story 5 - Review session history across devices (Priority: P3)

**Goal**: The developer can open a past session on either device and see the full attributed transcript,
consistently across desktop and mobile.

**Independent Test**: Run prompts from both the desktop and the phone, then open the session history on
each device and confirm both show the same ordered, attributed transcript.

### Tests for User Story 5 ⚠️

- [ ] T068 [P] [US5] Integration test: session history matches entry-for-entry across desktop and mobile in `tests/storage/test_history.rs`
- [ ] T069 [P] [US5] Integration test: mobile shows "history unavailable" (not stale data) when computer is unreachable in `tests/network/test_history_offline.rs`

### Implementation for User Story 5

- [ ] T070 [US5] Implement `session.list` full-history query (paginated, ordered) in `src-tauri/src/storage/transcripts.rs` (depends on T013)
- [ ] T071 [US5] Implement session/transcript deletion (single session and delete-all) cascading to transcript_entries and approval_requests (FR-007g) in `src-tauri/src/storage/transcripts.rs` (depends on T013, T014)
- [ ] T072 [US5] [P] Implement desktop session history browsing UI (session list, per-entry origin device/time) in `src/pages/Session.tsx` (depends on T070)
- [ ] T073 [US5] [P] Implement mobile history browsing screen (fetch on demand, cache only the session currently viewed) in `mobile/app/SessionList.tsx` (depends on T070)
- [ ] T074 [US5] [P] Implement storage usage display and delete controls in `src/components/Settings/` (depends on T071)

**Checkpoint**: All user stories should now be independently functional.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] T075 [P] Update `README.md` with build/run/test commands from quickstart.md (constitution Documentation and Testing Standards)
- [ ] T076 [P] Implement session-interrupted marking on unexpected shutdown/agent termination and restart recovery (FR-024) in `src-tauri/src/agent/session.rs`
- [ ] T077 [P] Implement on-disk file-change conflict detection since prompt start, warning before applying (FR-023) in `src-tauri/src/agent/mod.rs`
- [ ] T078 [P] Security test sweep: verify provider credentials never appear in transcripts, logs, error messages, or WSS traffic (SC-006a) in `tests/security/test_credential_isolation.rs`
- [ ] T079 [P] Performance test: verify streamed output latency meets SC-002 (visible within 3s) and SC-003 (95% within 1s) in `tests/network/test_streaming_perf.rs`
- [ ] T080 Run all quickstart.md validation scenarios (VS-001 through VS-010) end to end and record results

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-7)**: All depend on Foundational phase completion
  - US1, US2 can proceed in parallel once Foundational is done
  - US3 depends on US1 (needs a running agent session to send prompts to) and US2 (needs pairing to connect as mobile)
  - US4 depends on US1 (approval gating exists) and benefits from US2/US3 (multi-device) being in place
  - US5 depends on US1 (transcripts exist) and benefits from US2/US3 (multi-device attribution) being in place
- **Polish (Phase 8)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 3 (P1)**: Requires US1 (agent session engine) and US2 (pairing/auth) to be functionally meaningful
- **User Story 4 (P2)**: Requires US1 (approval gating baseline); most valuable once US2/US3 exist
- **User Story 5 (P3)**: Requires US1 (transcripts); most valuable once US2/US3 exist for cross-device attribution

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Repositories/models before services/engines
- Engines/services before WebSocket handlers
- Backend handlers before frontend/mobile UI that consumes them
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel (T003-T006)
- All Foundational repository tasks marked [P] can run in parallel (T009-T015)
- Once Foundational completes, US1 and US2 can be staffed in parallel
- All tests for a user story marked [P] can run in parallel
- UI tasks marked [P] within a story (desktop and mobile) can run in parallel once their backend dependency lands

---

## Parallel Example: User Story 1

```bash
# Launch all tests for User Story 1 together:
Task: "Contract test for session.create / session.cancel in tests/network/test_session_lifecycle.rs"
Task: "Contract test for prompt.send and transcript.entry streaming in tests/network/test_prompt_send.rs"
Task: "Integration test for full prompt run with approvals in tests/agent/test_prompt_flow.rs"
Task: "Integration test for sandbox refusal in tests/security/test_sandbox.rs"

# Launch independent UI tasks for User Story 1 together (after backend wiring lands):
Task: "Implement Project registration UI in src/components/ProjectList/"
Task: "Implement Session panel UI in src/components/SessionPanel/"
Task: "Implement Model Provider settings UI in src/components/Settings/"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test User Story 1 independently on the desktop
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (desktop-only MVP!)
3. Add User Story 2 → Test independently (pairing works, no remote prompts yet)
4. Add User Story 3 → Test independently → Deploy/Demo (full remote-control MVP)
5. Add User Story 4 → Test independently → Deploy/Demo (remote approvals)
6. Add User Story 5 → Test independently → Deploy/Demo (cross-device history)
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (desktop agent engine)
   - Developer B: User Story 2 (pairing/auth)
3. Once US1 + US2 land:
   - Developer A: User Story 3 (mobile prompt/streaming)
   - Developer B: User Story 4 (approvals)
4. User Story 5 (history) picked up by whoever is free next
5. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
