check:
  cargo fmt --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cd services/inference-worker && pytest
  cd packages/frontend && pnpm test
  cd packages/frontend && pnpm build

