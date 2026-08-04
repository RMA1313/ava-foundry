@RTK.md
# AGENTS.md

## Project

Local-first Persian Audio Dataset Builder.

The system collects, processes, categorizes, and exports large-scale Persian audio datasets.

Read the documentation under `docs/` before implementing new functionality.

---

# Architecture

The architecture defined in `docs/` is the source of truth.

Do not introduce architectural changes unless explicitly requested.

Current architecture:

- Rust orchestrates the pipeline.
- PostgreSQL is the only database.
- PostgreSQL is the durable job queue.
- React provides the dashboard.
- Python is used only for local GPU inference.
- Videos are temporary.
- Audio is the permanent asset.
- Processing is asynchronous and pipeline-based.

---

# Engineering Principles

Always preserve the following properties:

- Local-first
- Restart-safe
- Idempotent processing
- Incremental processing
- Fault tolerance
- Storage efficiency
- Clear ownership boundaries

Never introduce cloud dependencies.

---

# Scope

Implement only the requested task.

Do not:

- expand the product scope
- redesign the architecture
- introduce speculative abstractions
- implement roadmap items ahead of schedule
- refactor unrelated modules

Keep every change focused.

---

# Documentation

Documentation is part of the implementation.

Whenever system behavior changes:

- update the relevant document under `docs/`
- keep terminology consistent
- avoid duplicating documentation

Do not create new documentation files unless required.

---

# Testing

Every behavior change must include appropriate automated tests.

Do not disable existing tests.

Run all relevant validation before considering a task complete.

---

# Validation

Rust:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Frontend:

```bash
pnpm test
```

Python:

```bash
pytest
```

---

# Pipeline Rules

Pipeline stages are independent.

Every stage must:

- have a single responsibility
- be restart-safe
- be idempotent
- persist its own state

Stages communicate only through persisted state.

Never invoke downstream stages directly.

---

# Storage Rules

Videos are temporary processing artifacts.

Delete video files only after successful audio extraction and validation.

Audio is the permanent business asset.

Never persist unnecessary temporary files.

---

# Duplicate Rules

Duplicate audio must never contribute twice to business statistics.

Supported duplicate detection is limited to the functionality documented in the project.

Do not introduce unsupported duplicate detection strategies.

---

# Dashboard Rules

The dashboard is an operational view of the system.

The primary business KPI is:

- collected audio duration grouped by accounting category

Do not introduce alternative primary metrics.

---

# AI Rules

Python workers perform inference only.

Rust owns:

- orchestration
- scheduling
- persistence
- pipeline state
- business logic

Do not move orchestration responsibilities into Python.

---

# Code Quality

Prefer:

- small modules
- explicit naming
- deterministic behavior
- simple control flow

Avoid:

- unnecessary abstractions
- hidden side effects
- global mutable state
- premature optimization

---

# Commits

Do not create commits.

Do not modify Git history.

Do not create branches.

---

# Task Completion

A task is complete only when:

- requested functionality is implemented
- tests pass
- documentation is updated if required
- unrelated code remains unchanged
- no architectural rules are violated

---

# Output Format

At the end of every implementation task, report:

- Summary
- Files changed
- Tests executed
- Documentation updated
- Remaining limitations