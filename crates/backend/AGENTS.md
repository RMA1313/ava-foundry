# Backend agent rules

Primary paths:
- src/api/
- src/domain/
- src/repository/
- migrations/
- tests/

Do not inspect frontend or inference-worker unless the task explicitly requires it.

Validation:
- rtk cargo fmt --check
- rtk cargo clippy --workspace --all-targets -- -D warnings
- rtk cargo test -p backend