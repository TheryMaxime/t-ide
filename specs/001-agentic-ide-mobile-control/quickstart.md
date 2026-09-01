# Quickstart: Validation Guide for Agentic IDE with Mobile Remote Control

## Prerequisites

1. **Desktop**: Windows 10+, macOS 13+, or Linux with Rust 1.80+ toolchain installed.
2. **Mobile**: iOS 15+ device or Android 24+ device/emulator.
3. **Network**: Desktop and mobile on the same LAN (Wi-Fi or Ethernet).
4. **Model Provider**: At least one configured model provider:
   - External: OpenAI API key, Anthropic API key, or similar.
   - Local: Ollama running locally (`ollama serve`) or LM Studio with API mode enabled.

## Build & Run

### Desktop IDE

```bash
# Install Tauri CLI
cargo install tauri-cli --version "^2"

# Install frontend dependencies
npm install

# Build and run the desktop app
cargo tauri dev
```

Expected outcome: Desktop IDE window opens showing the home screen with "Register Project" button.

### Mobile App

```bash
cd mobile/
npx expo start
```

Scan the QR code with Expo Go (iOS) or install the Expo Go app and connect to the URL (Android).

## Validation Scenarios

### VS-001: Register a Project and Start a Desktop Session

**Purpose**: Verify FR-001, FR-002 -- project registration and agent session on desktop.

**Steps**:
1. In the desktop IDE, click "Register Project" and select a sample project folder (e.g., a simple Node.js or Rust project).
2. Configure a model provider in Settings (enter API key for OpenAI or set local Ollama URL).
3. Select the registered project and type a prompt: `"add input validation to the signup form"`.
4. Submit the prompt.

**Expected**:
- Agent begins running; desktop shows streamed reasoning output, file changes, and command execution in real time (FR-003).
- File change proposals appear with approve/reject buttons.
- Command execution requests trigger approval dialogs (FR-005).
- Session transcript records every step (FR-007).

**Pass criteria**: A file is created or modified only after explicit developer approval; rejection leaves the file unchanged.

---

### VS-002: Pair a Mobile Device with the Computer

**Purpose**: Verify FR-008, FR-009 -- pairing code flow with expiry and attempt limits.

**Steps**:
1. In the desktop IDE home screen, note the displayed 6-digit pairing code.
2. On the mobile app, enter the pairing code (or scan it via QR if implemented).
3. Wait for confirmation on both devices.
4. Verify the device appears in the desktop IDE trusted devices list with its name and connection time.

**Expected**:
- Both devices show "paired" status simultaneously.
- Device appears in desktop's paired device list (FR-010).
- Attempt to use an expired code is rejected with a message to generate a new one (FR-009, 5-minute expiry).
- After 5 failed attempts, further attempts are refused until a new code is generated (FR-009, attempt limit).

**Pass criteria**: Pairing succeeds within 2 minutes without documentation (SC-001); expired codes and excessive attempts are properly rejected.

---

### VS-003: Send a Prompt from Mobile and Watch Streamed Output

**Purpose**: Verify FR-014, FR-015, FR-003 -- mobile prompt submission with real-time streaming (User Story 3).

**Steps**:
1. On the mobile app, confirm the paired computer is shown as "connected".
2. Select one of the registered projects from the list (FR-014).
3. Type a prompt and send it.
4. Observe the streamed transcript on the phone.
5. Simultaneously open the desktop IDE -- verify it shows the same live session (FR-021, User Story 3 acceptance scenario 2).

**Expected**:
- Prompt begins producing visible output on the phone within 3 seconds of sending (SC-002).
- 95% of streamed output appears on the phone within 1 second of being produced (SC-003).
- Desktop IDE shows the same session with identical ordered transcript entries.

**Pass criteria**: Mobile prompt runs on the computer; both devices display synchronized, real-time streaming output.

---

### VS-004: Approve Risky Actions from the Phone

**Purpose**: Verify FR-018 -- mobile approval workflow (User Story 4).

**Steps**:
1. Send a prompt from the phone that triggers an agent action requiring approval (e.g., file write or command execution).
2. Observe the approval request on the phone with full details (command text or affected file paths).
3. Approve the request from the phone.
4. Verify the action executes on the computer.

**Expected**:
- Phone shows exact action details: command text, target paths, diff preview (FR-018).
- Approval is processed; action executes on the computer.
- If the desktop IDE is also viewing the session, it immediately reflects the approval decision and withdraws its own approval prompt (FR-005a -- first response wins).

**Pass criteria**: Remote round trip (send prompt -> review changes -> approve -> confirm applied) completes entirely from the phone in 90% of attempts (SC-007).

---

### VS-005: Session History Across Devices

**Purpose**: Verify FR-021, FR-007g -- consistent session history on both devices.

**Steps**:
1. Run multiple prompts from both the desktop IDE and the mobile app (create several sessions).
2. On each device, open the session history view.
3. Compare the session lists and individual transcript entries between devices.

**Expected**:
- Both devices show the same sessions in the same order with identical content (User Story 5 acceptance scenario 1).
- Each transcript entry shows which device originated it (acceptance scenario 2).
- When the computer is unreachable, the phone states that history is unavailable rather than showing stale data (FR-017b, User Story 5 acceptance scenario 3).

**Pass criteria**: Every session transcript viewed on the phone matches the desktop entry-for-entry with no missing or reordered entries (SC-008); transcripts survive T-ide restarts in 100% of cases (SC-008a).

---

### VS-006: Network Interruption Reconnection

**Purpose**: Verify FR-017 -- automatic reconnection and session resumption.

**Steps**:
1. Start a prompt from the mobile app that produces extended output.
2. Turn off Wi-Fi on the phone (simulate network loss).
3. Wait up to 2 minutes, then turn Wi-Fi back on.
4. Observe the phone rejoin the session and display missed output.

**Expected**:
- Phone shows "reconnecting" state during disconnection.
- Upon reconnection, phone rejoins the same session (not a new one).
- All output produced while disconnected is displayed (FR-017).
- 99% of reconnections within 2 minutes show complete missed output (SC-004).

**Pass criteria**: Phone seamlessly reconnects and catches up on missed transcript entries.

---

### VS-007: Security -- Unpaired Device Cannot Access Agent Capabilities

**Purpose**: Verify FR-012, FR-013, SC-006 -- security isolation for unpaired devices.

**Steps**:
1. Attempt to connect a device that has not completed pairing.
2. Try to list projects, send prompts, or view session data from the unpaired device.

**Expected**:
- WSS connection is rejected with HTTP 401 (unauthenticated) or 403 (revoked).
- No project information is exposed to the unpaired device.
- No prompts can be submitted by an unpaired device.

**Pass criteria**: Unpaired devices fail to obtain any project information or run any prompt in 100% of attempts (SC-006).

---

### VS-008: Credential Isolation

**Purpose**: Verify FR-007c, FR-007d, SC-006a -- provider credentials never leave the computer.

**Steps**:
1. Configure an external model provider with real API credentials in the desktop IDE.
2. Send a prompt from the mobile app that uses this provider.
3. Inspect all messages transmitted over WSS (use browser DevTools or proxy).
4. Inspect transcript entries stored on disk and displayed on both devices.

**Expected**:
- No API keys, tokens, or credential fragments appear in any WSS message.
- No credentials appear in transcript entries, logs, or error messages on either device.
- Prompts and project content are sent only to the configured model provider (not to any other destination).

**Pass criteria**: Automated tests confirm zero credential leakage across all transmission paths (SC-006a).

---

### VS-009: Single Session Enforcement

**Purpose**: Verify FR-022 -- exactly one agent session at a time.

**Steps**:
1. Start a prompt from the desktop IDE on Project A.
2. While it is running, attempt to start a prompt on Project B (from desktop or mobile).

**Expected**:
- The second prompt is refused with a clear message naming Project A as busy.
- Only one agent session runs at a time across all registered projects.

**Pass criteria**: Second prompts are queued or refused with a clear reason naming the busy project (FR-022).

---

### VS-010: Revoking a Paired Device

**Purpose**: Verify FR-011 -- device revocation terminates active sessions immediately.

**Steps**:
1. Pair a mobile device and start an active session visible on both devices.
2. Revoke the paired device from the desktop IDE while the session is active.

**Expected**:
- The mobile app's WSS connection is terminated immediately.
- The phone shows "device revoked" state; cannot reconnect without re-pairing.
- The desktop IDE removes the device from its trusted list (FR-011).

**Pass criteria**: Revocation terminates active sessions and prevents reconnection until re-paired.

---

## Automated Test Commands

```bash
# Rust backend unit tests
cd src-tauri && cargo test

# Rust backend integration tests
cargo test --test integration

# Desktop frontend tests (Vitest)
npm run test:frontend

# Contract tests for WebSocket protocol
cargo test --test contract-ws

# Mobile E2E tests (requires device/emulator)
cd mobile/ && npx detox test --configuration ios.sim.debug
```

All tests must pass as a merge gate per constitution Principle II.
