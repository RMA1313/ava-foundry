@RTK.mduop


## Project

Local-first Persian Audio Dataset Builder.

Read the relevant files under `docs/` before implementing changes.

## Architecture

The documentation under `docs/` is the source of truth.

Core constraints:

- Rust owns orchestration and business logic.
- PostgreSQL is the only database and durable job queue.
- React provides the dashboard.
- Python is used only for local inference.
- Audio is the permanent asset.
- Videos are temporary.
- Processing is asynchronous, restart-safe, and idempotent.
- No cloud services.

## Development Rules

- Implement only the requested task.
- Do not redesign the architecture.
- Do not expand the project scope.
- Do not refactor unrelated code.
- Keep changes small and reviewable.

## Documentation

If behavior changes:

- update the relevant existing document
- do not duplicate documentation
- do not create new documents unless required

## Testing

Every completed task must include appropriate automated tests.

Run relevant validation before considering a task complete.

Default validation:

```bash
rtk cargo fmt --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test --workspace
pytest
pnpm test
```

## Completion Criteria

A task is complete only if:

- acceptance criteria are satisfied
- relevant tests pass
- documentation is updated when required
- unrelated code is unchanged

## Final Report

Report only:

- summary
- files changed
- validation results
- documentation updated
- remaining limitations