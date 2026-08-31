# t-ide
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
