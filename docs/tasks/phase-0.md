# Phase 0 — Foundation

## P0-01 — Initialize Repository Structure

### Goal

Create the initial repository layout for the Rust backend, React dashboard, Python inference worker, documentation, migrations, tests, and local tooling.

### In Scope

* Create the top-level project structure.
* Initialize the Rust workspace.
* Initialize the frontend package.
* Initialize the Python inference worker package.
* Add placeholder directories for migrations, fixtures, scripts, and tests.
* Add root `.gitignore` and basic editor configuration.

### Out of Scope

* Product features
* Database tables
* API endpoints
* Worker logic
* Docker services
* CI configuration

### Dependencies

None.

### Acceptance Criteria

* The Rust workspace is recognized by Cargo.
* The frontend package installs successfully.
* The Python package environment initializes successfully.
* All documented top-level directories exist.
* No product functionality is implemented.

### Required Tests

* Rust workspace metadata check
* Frontend package configuration validation
* Python package import smoke test

### Documentation Updates

* Update `AGENTS.md` only if actual validation commands differ from the documented commands.

### Validation Commands

```bash
cargo metadata --no-deps
pnpm install --frozen-lockfile
pytest
```

---

## P0-02 — Add Local Configuration Model

### Goal

Provide deterministic local configuration for the backend, frontend, inference worker, database connection, and filesystem paths.

### In Scope

* Define environment variables.
* Add checked-in example environment files.
* Implement Rust configuration loading.
* Implement Python configuration loading.
* Validate required settings during startup.
* Define local storage and temporary staging paths.

### Out of Scope

* Secret management services
* Runtime settings UI
* Database persistence for configuration
* Per-project configuration

### Dependencies

* P0-01

### Acceptance Criteria

* Backend startup fails clearly when required configuration is missing.
* Python worker startup fails clearly when required configuration is missing.
* Default local paths are deterministic.
* Secrets and machine-specific values are not committed.
* Configuration behavior has automated tests.

### Required Tests

* Rust configuration unit tests
* Python configuration unit tests

### Documentation Updates

* Update `AGENTS.md` with actual local environment bootstrap commands if needed.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pytest
```

---

## P0-03 — Add PostgreSQL Local Runtime

### Goal

Provide a reproducible local PostgreSQL service for development and testing.

### In Scope

* Add PostgreSQL to Docker Compose.
* Configure persistent local database storage.
* Add health checks.
* Configure development and test databases.
* Document the minimal startup command through repository tooling.

### Out of Scope

* Application migrations
* Job queue schema
* Backup automation
* Production deployment
* External database hosting

### Dependencies

* P0-02

### Acceptance Criteria

* PostgreSQL starts locally through Docker Compose.
* The health check becomes healthy.
* Backend configuration can connect to the development database.
* Test configuration targets a separate database.
* Restarting PostgreSQL preserves development data.

### Required Tests

* Database connectivity smoke test

### Documentation Updates

* Update `AGENTS.md` with the database startup and shutdown commands.

### Validation Commands

```bash
docker compose config
docker compose up -d postgres
cargo test --workspace
```

---

## P0-04 — Implement Migration Runner

### Goal

Establish versioned database migrations managed by the Rust backend.

### In Scope

* Add migration directory and migration tooling.
* Run pending migrations during an explicit command.
* Record applied migrations.
* Fail safely on migration errors.
* Add an initial empty or infrastructure-only migration.

### Out of Scope

* Business-domain tables
* Automatic destructive rollback
* Application startup migrations unless explicitly configured
* Job queue tables

### Dependencies

* P0-03

### Acceptance Criteria

* Migrations apply to an empty database.
* Running migrations twice is safe.
* Migration failures return a non-zero exit status.
* Applied migration state is inspectable.
* Tests use the same migration mechanism.

### Required Tests

* Migration integration test
* Repeat-execution integration test

### Documentation Updates

* Update `AGENTS.md` with migration commands.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P0-05 — Create Rust Backend Service

### Goal

Create the minimal Rust HTTP service that forms the orchestration boundary.

### In Scope

* Add the backend executable.
* Start an Axum HTTP server.
* Add graceful shutdown.
* Add basic application state.
* Bind using configured host and port.

### Out of Scope

* Authentication
* Product endpoints
* Database-backed features
* Job processing
* Frontend serving

### Dependencies

* P0-02
* P0-04

### Acceptance Criteria

* The backend starts locally.
* The backend shuts down cleanly.
* Invalid configuration prevents startup.
* Startup does not create product data.
* The service has an automated startup smoke test.

### Required Tests

* Backend startup test
* Graceful shutdown test

### Documentation Updates

None unless runtime commands differ from `AGENTS.md`.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P0-06 — Add Health and Readiness Endpoints

### Goal

Expose separate liveness and readiness signals for local operation.

### In Scope

* Add a liveness endpoint.
* Add a readiness endpoint.
* Include PostgreSQL connectivity in readiness.
* Return machine-readable status.
* Keep liveness independent of downstream dependencies.

### Out of Scope

* GPU health
* FFmpeg health
* Worker health
* Detailed system metrics
* Authentication

### Dependencies

* P0-05

### Acceptance Criteria

* Liveness succeeds while the process is running.
* Readiness succeeds when PostgreSQL is available.
* Readiness fails when PostgreSQL is unavailable.
* Responses use stable JSON contracts.
* Endpoint tests cover healthy and unhealthy states.

### Required Tests

* API tests
* Database-unavailable readiness test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P0-07 — Add Structured Logging

### Goal

Provide consistent structured logs across the Rust backend and future workers.

### In Scope

* Configure Rust structured logging.
* Include timestamp, level, target, and correlation fields.
* Add request logging.
* Define environment-based log levels.
* Prevent secrets from being logged.

### Out of Scope

* Centralized log aggregation
* Cloud monitoring
* Log retention management
* Frontend telemetry

### Dependencies

* P0-05

### Acceptance Criteria

* Backend logs are structured and readable.
* Request logs include method, path, status, and duration.
* Log level can be configured.
* Sensitive configuration values are not emitted.
* Logging initialization is covered by tests where practical.

### Required Tests

* Logging configuration unit tests
* Request middleware test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P0-08 — Establish Rust Test Infrastructure

### Goal

Provide reusable infrastructure for unit, API, and PostgreSQL integration tests.

### In Scope

* Add shared test utilities.
* Create isolated test database setup.
* Apply migrations during integration tests.
* Provide application test construction.
* Ensure test data cleanup.

### Out of Scope

* Product behavior tests
* Browser tests
* GPU tests
* Performance benchmarks

### Dependencies

* P0-04
* P0-05

### Acceptance Criteria

* Integration tests can create isolated application state.
* Tests do not depend on existing development data.
* Failed tests do not leave persistent test records.
* Test execution is deterministic.
* Parallel test behavior is documented or safely constrained.

### Required Tests

* Test harness self-test
* Database isolation test

### Documentation Updates

* Update `AGENTS.md` if test setup requires an additional command.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P0-09 — Create React Dashboard Shell

### Goal

Create the minimal dashboard application without product-specific pages.

### In Scope

* Add application bootstrap.
* Add base layout.
* Add routing.
* Add API client configuration.
* Add loading and error boundaries.
* Add a placeholder home page.

### Out of Scope

* Authentication
* Project screens
* Queue screens
* Dataset metrics
* Audio playback
* Visual design system expansion

### Dependencies

* P0-01
* P0-06

### Acceptance Criteria

* The frontend starts locally.
* Routing works.
* The frontend can call the backend liveness endpoint.
* Backend-unavailable state is shown clearly.
* Basic component tests pass.

### Required Tests

* Frontend component tests
* API client unit test

### Documentation Updates

* Update `AGENTS.md` with actual frontend commands if needed.

### Validation Commands

```bash
pnpm test
pnpm build
```

---

## P0-10 — Create Python Inference Worker Skeleton

### Goal

Create the local Python service boundary reserved exclusively for GPU inference.

### In Scope

* Initialize the Python service.
* Add configuration loading.
* Add a minimal health response.
* Add structured logging.
* Add package and test layout.
* Ensure no orchestration logic exists.

### Out of Scope

* Model downloads
* GPU inference
* Database job ownership
* Audio classification
* Language detection

### Dependencies

* P0-01
* P0-02

### Acceptance Criteria

* The Python service starts locally.
* The health response succeeds.
* Configuration validation works.
* The service contains no pipeline scheduling logic.
* Python tests pass without a GPU.

### Required Tests

* Configuration tests
* Health smoke test

### Documentation Updates

None.

### Validation Commands

```bash
pytest
```

---

## P0-11 — Add Local Compose Runtime

### Goal

Allow the complete foundation stack to start through one local command.

### In Scope

* Add backend service to Docker Compose.
* Add frontend service to Docker Compose.
* Add Python worker service to Docker Compose.
* Connect all services to PostgreSQL where applicable.
* Add service health checks and startup dependencies.
* Mount local development storage paths.

### Out of Scope

* Production images
* GPU model execution
* Reverse proxy
* TLS
* Horizontal scaling

### Dependencies

* P0-03
* P0-06
* P0-09
* P0-10

### Acceptance Criteria

* The foundation stack starts through Docker Compose.
* Backend readiness succeeds.
* Frontend is reachable.
* Python health succeeds.
* PostgreSQL data persists across restarts.
* Services restart without manual cleanup.

### Required Tests

* Compose configuration validation
* Local stack smoke test

### Documentation Updates

* Update `AGENTS.md` with the canonical stack commands.

### Validation Commands

```bash
docker compose config
docker compose up -d
cargo test --workspace
pytest
pnpm test
```

---

## P0-12 — Add Unified Validation Command

### Goal

Provide one deterministic command that validates the complete repository.

### In Scope

* Add a root validation script or task runner command.
* Run Rust formatting, linting, and tests.
* Run Python tests.
* Run frontend tests and build.
* Return failure when any step fails.

### Out of Scope

* Deployment
* Performance benchmarks
* GPU integration tests
* External service tests

### Dependencies

* P0-08
* P0-09
* P0-10

### Acceptance Criteria

* One command validates all current components.
* Failures preserve the failing command output.
* The command runs locally without cloud dependencies.
* The command is documented in `AGENTS.md`.

### Required Tests

* Validation command smoke run

### Documentation Updates

* Update `AGENTS.md` with the canonical full-validation command.

### Validation Commands

```bash
just check
```

---
