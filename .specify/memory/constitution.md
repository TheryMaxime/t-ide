<!--
Sync Impact Report
Version change: (unversioned template) → 1.0.0
Modified principles:
  - [PRINCIPLE_1_NAME] → I. Documentation-First (NON-NEGOTIABLE)
  - [PRINCIPLE_2_NAME] → II. Tests Are Mandatory (NON-NEGOTIABLE)
  - [PRINCIPLE_3_NAME] → III. Specification-Driven Change Flow
  - [PRINCIPLE_4_NAME] → IV. Living Documentation
  - [PRINCIPLE_5_NAME] → V. Simplicity and Explicitness
Added sections:
  - Documentation and Testing Standards (was [SECTION_2_NAME])
  - Development Workflow and Quality Gates (was [SECTION_3_NAME])
Removed sections: none
Follow-up TODOs: none
-->

# t-ide Constitution

## Core Principles

### I. Documentation-First (NON-NEGOTIABLE)

Every feature, module, and public interface MUST be documented before its implementation
is written. Documentation of record lives in the repository as Markdown: feature specs and
plans under `specs/`, project-wide guidance in `README.md` and `docs/`. A change MUST NOT
be merged if it introduces behaviour that no committed document describes. When intent and
code disagree, the documented intent is authoritative until the document is amended.

Rationale: This project is documented first. Written intent is the shared contract between
humans and coding agents, and it is what makes generated code reviewable.

### II. Tests Are Mandatory (NON-NEGOTIABLE)

All code MUST be covered by automated tests that run in CI. Each documented requirement MUST
map to at least one test that fails when the requirement is violated. Bug fixes MUST add a
regression test that fails before the fix. Code MUST NOT be merged with failing or skipped
tests; a skip requires an inline justification and a tracked follow-up. Tests SHOULD be
written before or alongside the implementation, never deferred to a later change.

Rationale: Code needs to be tested. Untested code cannot be verified against the documents
that define it.

### III. Specification-Driven Change Flow

Non-trivial work MUST follow the Spec Kit flow: specification, then plan, then tasks, then
implementation. The specification MUST state observable behaviour and acceptance criteria
without prescribing implementation detail. Ambiguities MUST be resolved in the specification
before implementation begins, rather than settled implicitly in code.

Rationale: A stable, explicit specification is what allows implementation to be delegated,
reviewed, or regenerated without loss of intent.

### IV. Living Documentation

Documentation MUST be updated in the same change that alters the behaviour it describes.
Stale documentation is treated as a defect of equal severity to a failing test. Removed
features MUST have their documentation removed or explicitly marked obsolete in the same
change.

Rationale: Documentation only remains authoritative if it can never lag behind the code.

### V. Simplicity and Explicitness

The simplest solution that satisfies the documented requirements MUST be preferred. New
dependencies, abstractions, or configuration surfaces MUST be justified in the plan of the
change that introduces them. Speculative generality (YAGNI) MUST be rejected in review.

Rationale: Simple, explicit designs are cheaper to document, cheaper to test, and safer to
change.

## Documentation and Testing Standards

- Every specification MUST contain: purpose, scope, user-visible behaviour, acceptance
  criteria, and explicit out-of-scope items.
- Every public function, module, CLI command, and API endpoint MUST have a description of
  its inputs, outputs, and error behaviour.
- Tests MUST be runnable with a single documented command, and that command MUST be recorded
  in `README.md`.
- Test suites MUST include unit tests for logic and integration tests for cross-component
  contracts and external interfaces.
- Tests MUST be deterministic; flaky tests MUST be fixed or quarantined with a tracked issue,
  never ignored silently.
- Documentation and code examples MUST be verified against the current behaviour before merge.

## Development Workflow and Quality Gates

- Work proceeds as: `/speckit-specify` → `/speckit-plan` → `/speckit-tasks` →
  `/speckit-implement`, with `/speckit-clarify`, `/speckit-analyze`, and `/speckit-checklist`
  used as needed.
- Each change is delivered as a pull request that states which document it implements.
- Merge gates, all of which MUST pass: (1) referenced documentation exists and is current,
  (2) the full test suite passes in CI, (3) new or changed behaviour has corresponding tests,
  (4) at least one reviewer approves.
- Changes that only touch documentation still require review, but are exempt from the new-test
  gate.
- Reviewers MUST reject changes that add behaviour without documentation or tests.

## Governance

This constitution supersedes all other development practices in this repository. Where any
guide, template, or habit conflicts with it, this document wins.

Amendments MUST be made by a pull request that edits this file, states the rationale, and
records the version bump. Versioning follows semantic versioning: MAJOR for removing or
redefining a principle in a backward-incompatible way, MINOR for adding a principle or
materially expanding guidance, PATCH for clarifications and wording fixes.

Compliance is reviewed on every pull request. Any deviation MUST be documented in the pull
request description with an explicit justification and a plan to return to compliance.

**Version**: 1.0.0 | **Ratified**: 2026-08-31 | **Last Amended**: 2026-08-31
