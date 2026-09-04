# t-ide

Agentic IDE that runs on your computer with a companion mobile app for remote
control. See `specs/001-agentic-ide-mobile-control/` for the specification,
plan, and task list.

## Development

| Area | Location | Commands |
|------|----------|----------|
| Backend (Rust) | `src-tauri/` | `cargo build`, `cargo test`, `cargo clippy --all-targets`, `cargo fmt --all` |
| Desktop frontend (React + TS) | `src/` | `npm install`, `npm run dev`, `npm run build`, `npm test`, `npm run lint` |
| Mobile app (Expo / React Native) | `mobile/` | `npm install`, `npm start`, `npm run typecheck` |

Backend unit tests live next to the code; the contract and integration suites
live in `tests/` (registered as `[[test]]` targets in `src-tauri/Cargo.toml`)
and desktop component tests in `tests/frontend/`.

The backend builds headless by default. Two integrations are opt-in because they
need platform toolkits that are not present everywhere:

- `--features desktop` compiles the Tauri shell (needs WebKitGTK on Linux).
- `--features os-keychain` stores model provider credentials in the OS keychain
  instead of the in-memory store used by tests.

Runtime configuration lives in `<data dir>/config.json` next to the SQLite
database; `T_IDE_DATA_DIR` and `T_IDE_PORT` override it. TLS material for the
WSS endpoint is read from `<data dir>/tls/cert.pem` and `<data dir>/tls/key.pem`.

## Spec Kit

This repository is initialized with [GitHub Spec Kit](https://github.com/github/spec-kit)
(GitHub Copilot integration, `sh` scripts). Spec Kit assets live in `.specify/`
(templates, scripts, memory/constitution) and `.github/skills/` (agent skills).

Common workflow with your coding agent:

1. `/speckit-constitution` — establish project principles
2. `/speckit-specify` — create a baseline specification
3. `/speckit-plan` — create an implementation plan
4. `/speckit-tasks` — generate actionable tasks
5. `/speckit-implement` — execute the implementation

Optional helpers: `/speckit-clarify`, `/speckit-analyze`, `/speckit-checklist`, `/speckit-converge`.

To update Spec Kit later:

```bash
uvx --from git+https://github.com/github/spec-kit.git specify init --here --integration copilot --script sh --force
```
