# Feature Specification: Agentic IDE with Mobile Remote Control

**Feature Branch**: `001-agentic-ide-mobile-control`

**Created**: 2026-08-31

**Status**: Draft

**Input**: User description: "T-ide is a agentic IDE running on a computer and a mobile app that connect to the IDE in order to run prompt in the computer."

## Clarifications

### Session 2026-08-31

- Q: When the phone and the computer are not on the same Wi-Fi network, how should the phone reach the computer? → A: LAN-only for this feature; internet/remote access is explicitly deferred to a later feature.
- Q: How does the developer supply the AI model that powers the agent? → A: Bring-your-own provider credentials, stored on the computer only, plus support for a locally-running model so the developer can work with no external provider.
- Q: How many projects can a computer expose, and can more than one agent session be active at once? → A: Several registered projects, but only one agent session running at any moment across the whole computer; a second prompt is queued or refused.
- Q: Which device may answer an approval request when both are connected? → A: Either device; the first response wins and the request is withdrawn from the other, showing which device decided.
- Q: Where does a session transcript live, and how long is it kept? → A: The computer stores all transcripts indefinitely until the developer deletes them; the phone caches only the session it is viewing and can browse history only while connected.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run a prompt on the computer from the desktop IDE (Priority: P1)

A developer opens T-ide on their computer, selects a project folder, and types a prompt such as
"add input validation to the signup form". The agent reads and edits files inside that project,
runs commands when needed, and streams its reasoning, file changes, and command output back into
the IDE. The developer reviews the proposed changes and accepts or rejects them.

**Why this priority**: Without a working agent session on the computer there is nothing for a
mobile client to connect to. This story alone is a usable product.

**Independent Test**: Open the desktop IDE on a sample project, submit a prompt that requires
editing a file and running a command, and confirm the change is applied only after explicit
approval and that the transcript records every step.

**Acceptance Scenarios**:

1. **Given** a project folder is open, **When** the developer submits a prompt, **Then** the agent
   produces a streamed response and a set of proposed file changes scoped to that folder.
2. **Given** the agent proposes a file change, **When** the developer rejects it, **Then** the
   file on disk is unchanged and the rejection is recorded in the session transcript.
3. **Given** the agent wants to run a shell command, **When** approval is required, **Then** the
   command does not execute until the developer approves it.
4. **Given** a prompt is running, **When** the developer cancels it, **Then** the agent stops
   within a few seconds and no further changes are applied.

---

### User Story 2 - Pair a mobile app with the computer (Priority: P1)

A developer installs the T-ide mobile app, and pairs it with their computer by scanning a pairing
code displayed by the desktop IDE. Once paired, the phone appears in the desktop IDE's list of
trusted devices and can be revoked at any time.

**Why this priority**: Pairing is the gate for every remote capability, and it is where the
security of the whole product is decided. No remote feature can ship before it.

**Independent Test**: Pair a phone with a computer using a displayed code, confirm the device is
listed as trusted on the desktop, revoke it, and confirm the phone can no longer connect.

**Acceptance Scenarios**:

1. **Given** the desktop IDE shows a pairing code, **When** the mobile app submits the correct
   code, **Then** the devices are paired and both sides show the pairing as active.
2. **Given** a pairing code is displayed, **When** it is not used before it expires, **Then** it
   is rejected and a new code must be generated.
3. **Given** an incorrect pairing code is submitted repeatedly, **When** the attempt limit is
   reached, **Then** further attempts are refused until a new code is generated.
4. **Given** a paired device, **When** the developer revokes it from the desktop IDE, **Then** the
   device's active session ends immediately and it cannot reconnect without pairing again.

---

### User Story 3 - Send a prompt from the phone and watch it run (Priority: P1)

Away from the keyboard but still on the same local network — on the sofa, in a meeting room —
the developer opens the mobile app, picks one of the projects exposed by
their paired computer, types or dictates a prompt, and sends it. The agent runs on the computer.
The phone shows the response streaming in, the files being changed, and any command output.

**Why this priority**: This is the defining capability of the product — the reason the mobile app
exists.

**Independent Test**: With a paired phone, send a prompt from the phone, and verify the agent runs
on the computer and the phone displays the same streamed transcript as the desktop IDE.

**Acceptance Scenarios**:

1. **Given** a paired and connected phone, **When** the developer sends a prompt, **Then** the
   agent begins running on the computer and the phone shows streamed output as it is produced.
2. **Given** a prompt is running from the phone, **When** the developer opens the desktop IDE,
   **Then** the desktop shows the same live session and transcript.
3. **Given** a prompt is running, **When** the developer cancels it from the phone, **Then** the
   run stops on the computer.
4. **Given** the phone loses network connectivity mid-run, **When** it reconnects, **Then** it
   rejoins the same session and shows everything it missed.
5. **Given** the computer is asleep or T-ide is not running, **When** the phone tries to send a
   prompt, **Then** the phone shows a clear "computer unavailable" state and does not silently
   queue the prompt without telling the developer.

---

### User Story 4 - Approve risky actions from the phone (Priority: P2)

While a remote prompt is running, the agent asks to run a command or write to a file. The
developer receives the request on their phone with enough context to judge it, and approves or
denies it without going back to the computer.

**Why this priority**: Remote prompts are of limited use if every risky step blocks until the
developer returns to their desk, but the core remote flow (P1) is still demonstrable without it.

**Independent Test**: Trigger a prompt that requires an approval, confirm the request appears on
the phone with the command and target paths, approve it, and verify the action then executes on
the computer.

**Acceptance Scenarios**:

1. **Given** the agent requests approval, **When** the developer is using the phone, **Then** the
   phone shows the exact action requested, including command text or affected file paths.
2. **Given** an approval request, **When** the developer denies it, **Then** the action is not
   performed and the agent continues or stops with the denial recorded.
3. **Given** an approval request, **When** neither device responds within the configured window,
   **Then** the request expires as a denial and the session records the timeout.
4. **Given** an approval request visible on both devices, **When** it is answered on one of them,
   **Then** it disappears from the other device and shows which device decided and how.

---

### User Story 5 - Review session history across devices (Priority: P3)

The developer opens a past session on either device and reads the full transcript: the prompts
sent, the agent's responses, the files changed, the commands run, and the approval decisions —
along with which device initiated each action.

**Why this priority**: Valuable for auditability and for resuming work, but the product is usable
without it.

**Independent Test**: Run prompts from both the desktop and the phone, then open the session
history on each device and confirm both show the same ordered, attributed transcript.

**Acceptance Scenarios**:

1. **Given** completed sessions, **When** the developer opens history on either device, **Then**
   both show the same sessions in the same order with the same content.
2. **Given** a transcript entry, **When** the developer inspects it, **Then** it shows which
   device originated the prompt or approval and when.
3. **Given** the computer is unreachable, **When** the developer opens history on the phone,
   **Then** the phone states that history is unavailable instead of showing stale data.

---

### Edge Cases

- The phone leaves the computer's local network — the phone shows an explicit "not on the same
  network as your computer" state rather than an unexplained connection failure.
- Two devices submit prompts at the same time, whether to the same project or to two different
  registered projects — the second is queued or refused with a clear message naming the busy
  project, rather than interleaving two agent runs.
- A registered project folder is renamed, moved, or deleted on disk — the project is shown as
  unavailable and cannot be used to start a session until the developer repairs or removes it.
- The agent proposes a change to a file that was edited on disk since the prompt started — the
  developer is warned about the conflict before the change is applied.
- The agent tries to read or write outside the selected project folder — the action is refused
  and recorded.
- The computer goes to sleep or loses power mid-run — the session is marked interrupted, the
  transcript written so far is preserved, and no partially applied change is presented as complete.
- Stored history grows large over many sessions — the developer can see how much space it uses and
  delete sessions to reclaim it.
- A prompt produces a very large amount of output — the phone remains responsive and does not run
  out of memory.
- The pairing between phone and computer is revoked while a prompt is running — the run stops
  being visible to the phone; the developer chooses on the desktop whether it continues.
- The developer sends a prompt from the phone while the same session is being driven from the
  desktop — both devices see a single consistent conversation.

## Requirements *(mandatory)*

### Functional Requirements

**Agent sessions on the computer**

- **FR-001**: The system MUST let a developer register one or more project folders on the computer
  and start an agent session scoped to a single chosen folder.
- **FR-001a**: The system MUST let the developer add, rename, and remove registered projects, and
  MUST make the registered list visible to paired devices.
- **FR-002**: The system MUST accept a natural-language prompt and run an agent that can read
  files, propose file changes, and request command execution within the session's project.
- **FR-003**: The system MUST stream agent output — reasoning summary, file changes, and command
  output — to every connected client as it is produced, not only on completion.
- **FR-004**: The system MUST refuse any agent read or write outside the open project folder and
  record the refusal in the session transcript.
- **FR-005**: The system MUST require explicit developer approval before applying a file change or
  executing a command, with a per-project setting to auto-approve read-only actions.
- **FR-005a**: An approval request MUST be answerable from any connected device; the first response
  received MUST decide the outcome, and every other device MUST immediately withdraw the request
  and show which device decided and how.
- **FR-005b**: The system MUST accept exactly one decision per approval request; a decision
  arriving after the request is already resolved MUST be ignored and MUST NOT cause the action to
  run twice.
- **FR-006**: Developers MUST be able to cancel a running prompt from any connected device and the
  agent MUST stop within 5 seconds.
- **FR-007**: The system MUST record every prompt, response, file change, command, and approval
  decision in a persistent session transcript, attributed to the originating device.
- **FR-007f**: The computer MUST be the single source of truth for transcripts, storing them on
  local disk so they survive restarting T-ide or the machine.
- **FR-007g**: The system MUST retain transcripts indefinitely and MUST NOT delete them
  automatically; the developer MUST be able to delete an individual session or all history.

**Model provider configuration**

- **FR-007a**: The developer MUST be able to configure at least one model provider on the computer
  by supplying their own credentials, and to choose which configured provider a session uses.
- **FR-007b**: The system MUST support using a model running on the developer's own machine or
  local network, so that a session can run with no external provider configured.
- **FR-007c**: Provider credentials MUST be stored only on the computer, MUST never be sent to the
  mobile app, and MUST never appear in transcripts, logs, or error messages.
- **FR-007d**: The system MUST NOT send prompts or project content to any destination other than
  the model provider the developer configured for that session.
- **FR-007e**: When no provider is configured, or the configured provider rejects the request, the
  system MUST surface an actionable error on every connected device rather than failing silently.

**Pairing and trust**

- **FR-008**: The system MUST allow a mobile device to be paired with a computer using a
  single-use code generated on and displayed by the computer.
- **FR-009**: Pairing codes MUST expire within 5 minutes and MUST be rejected after 5 failed
  attempts.
- **FR-010**: The computer MUST maintain a list of paired devices showing device name and last
  connection time, and MUST allow any device to be revoked.
- **FR-011**: Revoking a device MUST terminate its active session immediately and prevent
  reconnection until it is paired again.
- **FR-012**: All communication between the mobile app and the computer MUST be encrypted in
  transit and mutually authenticated, so that an unpaired device cannot issue prompts.
- **FR-013**: The system MUST NOT expose the computer's agent capabilities to any device that has
  not completed pairing.
- **FR-013a**: Pairing and all subsequent communication MUST operate over the local network only;
  the system MUST NOT route prompts, transcripts, or approvals through any third-party relay.
- **FR-013b**: The mobile app MUST let a developer find their paired computer on the local network
  without typing an address by hand.

**Mobile client**

- **FR-014**: The mobile app MUST list the projects the paired computer has made available and
  let the developer choose one before prompting.
- **FR-015**: The mobile app MUST let the developer submit a prompt as text and MUST display the
  resulting streamed transcript.
- **FR-016**: The mobile app MUST show the current connection state (connected, reconnecting,
  computer unavailable, not on the same network) and MUST NOT accept a prompt silently while
  disconnected.
- **FR-017**: The mobile app MUST reconnect automatically after a network interruption and rejoin
  the session it was viewing, showing the output produced while it was away.
- **FR-017a**: The mobile app MUST retain only the session it is currently viewing and MUST fetch
  all other history from the computer on demand.
- **FR-017b**: When the computer is unreachable, the mobile app MUST state that history is
  unavailable rather than presenting a partial or stale transcript as complete.
- **FR-018**: The mobile app MUST present pending approval requests with the full text of the
  command or the list of affected file paths, and MUST let the developer approve or deny.
- **FR-018a**: When a pending request is resolved on another device, the mobile app MUST remove it
  from the pending list within 1 second and show the recorded outcome.
- **FR-019**: Approval requests MUST expire after a configurable window (default 5 minutes) and
  MUST be treated as a denial on expiry.
- **FR-020**: The mobile app MUST notify the developer when a prompt finishes, fails, or needs
  approval while the app is not in the foreground.

**Multi-device consistency**

- **FR-021**: A session MUST be viewable and controllable from the desktop IDE and the mobile app
  at the same time, with both showing the same ordered transcript.
- **FR-022**: The computer MUST run at most one agent session at a time, across all registered
  projects; a prompt submitted while another session is running MUST be queued or refused with a
  clear reason naming the project that is busy.
- **FR-022a**: Every connected device MUST be able to see which project currently holds the
  running session.
- **FR-023**: The system MUST detect that a file changed on disk after a prompt started and warn
  before applying a conflicting change.
- **FR-024**: The system MUST mark a session as interrupted, rather than complete, when the
  computer shuts down or the agent terminates unexpectedly mid-run.

**Documentation and verification (constitution)**

- **FR-025**: Every capability above MUST be described in committed documentation before it is
  implemented, and that documentation MUST be updated in the same change that alters behaviour.
- **FR-026**: Every functional requirement MUST have at least one automated test that fails when
  the requirement is violated.

### Key Entities

- **Computer (IDE host)**: The machine running T-ide. Owns projects, agent sessions, the paired
  device list, and all file and command execution.
- **Project**: A registered folder on the computer that bounds what an agent session may read or
  write. Has a name, a path, per-project approval settings, and an available/unavailable state.
  A computer may have many; only one may host a running session at a time.
- **Model Provider**: A configured source of model capability — either an external service the
  developer holds credentials for, or a model running on the developer's own machine or local
  network. Has a display name, a location, credentials held only on the computer, and a
  local/external kind.
- **Agent Session**: A conversation bound to one project. Has a status (idle, running, waiting for
  approval, interrupted, complete), an ordered transcript, and a set of connected clients.
- **Prompt Run**: A single prompt and everything it produced — response text, proposed file
  changes, commands, approvals, and final outcome. Attributed to the device that sent it.
- **Paired Device**: A mobile device trusted by a computer. Has a display name, a pairing date,
  a last-seen time, and a revoked/active state.
- **Approval Request**: A pending risky action (file write or command) with its full details, an
  expiry time, and its resolution. Answerable from any connected device; records which device
  decided and when. Resolves exactly once.
- **Transcript Entry**: One ordered, timestamped record in a session, attributed to a device and
  typed (prompt, response, file change, command, approval decision, error). Stored on the computer;
  held on the phone only for the session currently being viewed.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can pair a phone with their computer for the first time in under
  2 minutes without reading documentation.
- **SC-002**: A prompt sent from the phone begins producing visible output on the phone within
  3 seconds of being sent, on a typical home or office Wi-Fi network.
- **SC-003**: 95% of streamed output produced on the computer appears on the phone within
  1 second of being produced.
- **SC-004**: After a network interruption of up to 2 minutes, the phone rejoins the session and
  shows the complete missed output in 99% of cases.
- **SC-005**: No file change or command is ever applied without a recorded approval — verified by
  an automated test suite covering every approval path, with zero exceptions.
- **SC-006**: An unpaired device fails to obtain any project information or run any prompt in 100%
  of attempts.
- **SC-006a**: Provider credentials never leave the computer — verified by automated tests
  asserting that no transcript, log, error message, or message sent to the phone contains them.
- **SC-007**: A developer can complete a full remote round trip — send prompt, review changes,
  approve, confirm applied — entirely from the phone in 90% of attempts.
- **SC-008**: Every session transcript viewed on the phone matches the desktop transcript entry
  for entry, with no missing or reordered entries.
- **SC-008a**: A transcript written before a restart of T-ide or the computer is fully readable
  afterwards in 100% of cases.
- **SC-009**: Every functional requirement in this specification maps to at least one automated
  test, verified as a merge gate.

## Assumptions

- The computer and the phone belong to the same developer; T-ide is a single-user tool and multi
  user or team collaboration is out of scope for this feature.
- The developer's computer is a normal desktop or laptop that they control and can install
  software on.
- The agent's model capability is provided either by an external provider the developer holds
  credentials for, or by a model running locally; paying for and operating that provider is the
  developer's responsibility and out of scope here. T-ide ships no accounts and no billing.
- Prompts and generated content are transmitted only between the developer's own devices and the
  model provider the developer configures.
- The mobile app is a companion, not a full editor: browsing and editing arbitrary files by hand
  on the phone is out of scope for this feature.
- Offline queuing of prompts on the phone is out of scope; the phone must be able to reach the
  computer to submit a prompt.
- Version control operations (commits, branches, pull requests) are out of scope for this feature
  and will be specified separately.
- Connectivity is local-network only for this feature: the phone must be on the same local network
  as the computer to pair or send prompts. Reaching the computer over the internet (relay or
  peer-to-peer) is explicitly deferred to a later feature.
- The desktop IDE displays the pairing code; the mobile app may enter it manually or scan it, and
  both entry methods are considered equivalent.
