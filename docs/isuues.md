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

# Phase 1 — Ingestion

## P1-01 — Add Project Persistence

### Goal

Create the persistent `Project` entity as the top-level ownership boundary.

### In Scope

* Add the projects migration.
* Add the Rust project domain model.
* Add repository methods to create, read, list, and update projects.
* Store name, description, and timestamps.

### Out of Scope

* Project deletion
* Project settings
* Project dashboard metrics
* Sources
* Frontend UI

### Dependencies

* P0-12

### Acceptance Criteria

* A project can be created.
* A project can be retrieved by identifier.
* Projects can be listed deterministically.
* Project names are validated.
* Persistence tests pass.

### Required Tests

* Repository integration tests
* Domain validation unit tests

### Documentation Updates

* Update `docs/domain-model.md` only if implemented behavior differs from the current model.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-02 — Add Project API

### Goal

Expose project creation, retrieval, listing, and update through the Rust API.

### In Scope

* Add project endpoints.
* Add request validation.
* Add stable response contracts.
* Add not-found and conflict error handling.
* Add pagination for project listing.

### Out of Scope

* Project deletion
* Authentication
* Project dashboard
* Source management
* Frontend project pages

### Dependencies

* P1-01

### Acceptance Criteria

* Project creation endpoint works.
* Project retrieval endpoint works.
* Project listing is paginated.
* Project update endpoint works.
* Invalid requests return deterministic errors.

### Required Tests

* API integration tests
* Validation tests

### Documentation Updates

None unless domain behavior changes.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-03 — Add Project Dashboard Pages

### Goal

Allow users to create and select projects through the React dashboard.

### In Scope

* Add project list page.
* Add project creation form.
* Add project detail shell.
* Display API errors and loading states.
* Persist the selected project in the route.

### Out of Scope

* Source import
* Project metrics
* Project deletion
* Advanced project settings

### Dependencies

* P1-02

### Acceptance Criteria

* Users can create a project.
* Users can view the project list.
* Users can open a project detail route.
* Validation and API errors are visible.
* Frontend tests cover the main interactions.

### Required Tests

* Component tests
* API integration mocks

### Documentation Updates

None.

### Validation Commands

```bash
pnpm test
pnpm build
```

---

## P1-04 — Add Source Persistence

### Goal

Create the persistent `Source` entity owned by a project.

### In Scope

* Add the sources migration.
* Add source type representation.
* Store source identity and import metadata.
* Link each source to one project.
* Add repository methods for creation, lookup, and listing.

### Out of Scope

* Media downloading
* Media assets
* File inspection
* Source status UI
* Source deletion

### Dependencies

* P1-01

### Acceptance Criteria

* A source belongs to exactly one project.
* Supported source types can be persisted.
* Source records can be retrieved and listed.
* Invalid project relationships are rejected.
* Repository tests pass.

### Required Tests

* Repository integration tests
* Source type unit tests

### Documentation Updates

* Update `docs/domain-model.md` only if required by actual behavior.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-05 — Normalize External Source Identity

### Goal

Produce deterministic source identities that prevent repeated registration of the same external item.

### In Scope

* Define normalized identities for supported URL-based sources.
* Normalize URL formatting where safe.
* Extract supported platform identifiers.
* Enforce project-level source uniqueness.
* Preserve the original submitted URL.

### Out of Scope

* Audio-content deduplication
* Downloading
* Redirect resolution
* Partial-content similarity
* Cross-project deduplication

### Dependencies

* P1-04

### Acceptance Criteria

* Equivalent supported source URLs produce the same normalized identity.
* The original URL remains stored.
* Duplicate source registration in one project is rejected or returns the existing source deterministically.
* Unrecognized URLs retain a stable generic identity.
* Tests cover supported URL forms.

### Required Tests

* Source normalization unit tests
* Database uniqueness integration test

### Documentation Updates

* Update `docs/pipeline.md` only if source-registration behavior needs clarification.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-06 — Add Persistent Job Schema

### Goal

Create the durable PostgreSQL job representation used by ingestion work.

### In Scope

* Add the jobs migration.
* Define job type and job status.
* Store project, source, priority, attempt count, availability, lease, and timestamps.
* Add required indexes.
* Add uniqueness support for idempotent job creation.

### Out of Scope

* Job claiming
* Retry calculation
* Job workers
* Media downloading
* Generic workflow engine

### Dependencies

* P1-04

### Acceptance Criteria

* Jobs can be persisted.
* Job type and status values are validated.
* Required queue indexes exist.
* Duplicate logical jobs are prevented.
* Migration and repository tests pass.

### Required Tests

* Migration integration tests
* Repository integration tests
* State validation unit tests

### Documentation Updates

* Update `docs/domain-model.md` only if required.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-07 — Implement Atomic Job Claiming

### Goal

Allow concurrent workers to claim available jobs without processing the same job twice.

### In Scope

* Claim the highest-priority available job.
* Use PostgreSQL row locking with skip-locked behavior.
* Transition the job to running atomically.
* Record worker identity and lease expiration.
* Return no job when none is available.

### Out of Scope

* Retry policy
* Job completion
* Lease recovery
* Actual worker execution
* External message brokers

### Dependencies

* P1-06

### Acceptance Criteria

* Two concurrent workers never claim the same job.
* Only queued and available jobs are claimable.
* Priority and creation order are respected.
* Claim state is persisted atomically.
* Concurrency integration tests pass.

### Required Tests

* PostgreSQL concurrent-claim integration test
* Queue ordering integration test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-08 — Implement Job Completion and Failure

### Goal

Persist successful and failed ingestion job outcomes safely.

### In Scope

* Mark a running job completed.
* Mark a running job failed.
* Store concise failure information.
* Reject invalid transitions.
* Make repeated completion calls idempotent.

### Out of Scope

* Automatic retries
* Lease recovery
* Worker loops
* Failure dashboards

### Dependencies

* P1-07

### Acceptance Criteria

* Valid completion transitions succeed.
* Valid failure transitions succeed.
* Invalid transitions are rejected.
* Repeated terminal updates do not duplicate effects.
* Transition tests pass.

### Required Tests

* Job state unit tests
* Repository integration tests

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-09 — Implement Retry Scheduling

### Goal

Reschedule retryable ingestion failures using bounded, persisted retry behavior.

### In Scope

* Define maximum attempts.
* Compute retry availability using backoff.
* Increment attempt count.
* Preserve the latest failure reason.
* Move exhausted jobs to a terminal failed state.

### Out of Scope

* Failure classification based on downloader details
* User-configurable retry policies
* Manual retry API
* Lease recovery

### Dependencies

* P1-08

### Acceptance Criteria

* Retryable failures return to queued state.
* `available_at` is set in the future.
* Attempts are bounded.
* Exhausted jobs become terminally failed.
* Repeated retry scheduling is safe.

### Required Tests

* Backoff unit tests
* Retry transition integration tests

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-10 — Implement Job Lease Recovery

### Goal

Recover jobs abandoned by crashed or terminated workers.

### In Scope

* Detect expired running-job leases.
* Requeue recoverable jobs.
* Preserve attempts and failure history.
* Prevent recovery of actively leased jobs.
* Provide an explicit recovery operation.

### Out of Scope

* Background scheduler loop
* Process supervision
* Worker heartbeats
* Distributed coordination

### Dependencies

* P1-09

### Acceptance Criteria

* Expired jobs are recoverable.
* Non-expired jobs remain untouched.
* Recovery is idempotent.
* Concurrent recovery does not duplicate jobs.
* Integration tests cover crash-recovery behavior.

### Required Tests

* Lease-expiration integration tests
* Concurrent recovery integration test

### Documentation Updates

* Update `docs/pipeline.md` only if recovery semantics require clarification.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-11 — Add Worker Runtime Framework

### Goal

Create the Rust worker loop that continuously claims and executes jobs through registered handlers.

### In Scope

* Poll and claim jobs.
* Dispatch by job type.
* Complete, fail, or reschedule jobs.
* Support graceful shutdown.
* Limit concurrent jobs.
* Generate worker identifiers.

### Out of Scope

* Download implementation
* Generic plugin SDK
* Cross-machine workers
* Python inference work
* Dashboard controls

### Dependencies

* P1-10

### Acceptance Criteria

* Worker processes available jobs continuously.
* Concurrency limits are respected.
* Graceful shutdown stops new claims and finishes or releases current work safely.
* Unknown job types fail deterministically.
* Worker behavior is testable with fake handlers.

### Required Tests

* Worker-loop unit tests
* Graceful shutdown test
* Handler dispatch tests

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-12 — Add Local File Source Registration API

### Goal

Allow a local audio or video file to be registered as an ingestion source.

### In Scope

* Add an API request containing a local path and project.
* Validate that the path exists and references a regular file.
* Register the source.
* Create an import job idempotently.
* Return source and job identifiers.

### Out of Scope

* Copying the file
* Media inspection
* Folder import
* Archive extraction
* Browser file upload

### Dependencies

* P1-05
* P1-06

### Acceptance Criteria

* Existing files can be registered.
* Missing or non-file paths are rejected.
* Repeated registration does not create duplicate jobs.
* The source is linked to the selected project.
* API tests pass.

### Required Tests

* API tests
* Filesystem validation tests
* Idempotent job-creation integration test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-13 — Implement Local File Import Worker

### Goal

Import a registered local media file into controlled staging storage.

### In Scope

* Handle local-file import jobs.
* Copy the source file into staging.
* Use a temporary destination followed by atomic rename.
* Record imported byte size and path.
* Avoid duplicate copies on retry.

### Out of Scope

* Media inspection
* Audio extraction
* File hashing for content deduplication
* Deleting the user's source file
* Folder import

### Dependencies

* P1-11
* P1-12

### Acceptance Criteria

* A registered local file is copied to staging.
* Partial copies are never finalized.
* Retrying a completed import does not create another copy.
* The original local file remains unchanged.
* Import failures are persisted through the job framework.

### Required Tests

* Worker integration tests with temporary files
* Interrupted-copy recovery test
* Idempotency test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-14 — Add Folder Scan API

### Goal

Discover supported media files under a local folder and register each as an independent source.

### In Scope

* Accept a project and folder path.
* Scan files recursively.
* Filter supported media extensions.
* Register sources idempotently.
* Create local-file import jobs.
* Return discovered, created, and skipped counts.

### Out of Scope

* Watching folders for future changes
* Archive extraction
* Media inspection
* Symlink traversal outside the selected folder
* Uploading files through the browser

### Dependencies

* P1-12

### Acceptance Criteria

* Supported files are discovered recursively.
* Unsupported files are skipped.
* Repeated scans do not duplicate sources or jobs.
* Symlink behavior is deterministic and safe.
* Summary counts are accurate.

### Required Tests

* Folder scanning unit tests
* API integration tests
* Repeated-scan idempotency test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-15 — Add CSV URL Import

### Goal

Register media URLs from a CSV file as project sources and enqueue download jobs.

### In Scope

* Accept a local CSV path.
* Support a documented URL column.
* Parse rows incrementally.
* Normalize and register each source.
* Create download jobs idempotently.
* Return imported, duplicate, invalid, and failed row counts.

### Out of Scope

* Download execution
* Arbitrary CSV schema mapping
* CSV editing
* License metadata
* Category assignment

### Dependencies

* P1-05
* P1-06

### Acceptance Criteria

* Valid URLs create sources and download jobs.
* Invalid rows are reported without aborting the complete import.
* Duplicate rows do not create duplicate sources or jobs.
* Large CSV files are not loaded entirely into memory.
* Import summary counts are correct.

### Required Tests

* CSV parser tests
* API integration tests
* Large fixture streaming test
* Duplicate-row test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-16 — Add Generic URL Registration API

### Goal

Allow a single public media URL to be registered and queued for download.

### In Scope

* Accept project and URL.
* Normalize source identity.
* Register the source.
* Create a download job idempotently.
* Return an existing source when already registered.

### Out of Scope

* Download execution
* URL discovery
* Playlist expansion
* Authentication-protected media
* Browser extension support

### Dependencies

* P1-05
* P1-06

### Acceptance Criteria

* A valid URL creates a source and job.
* Invalid URLs are rejected.
* Repeated submission is idempotent.
* Project ownership is enforced.
* API tests pass.

### Required Tests

* URL validation tests
* API integration tests
* Idempotency integration test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-17 — Implement Download Command Adapter

### Goal

Provide a Rust-controlled adapter for invoking the local media downloader.

### In Scope

* Invoke `yt-dlp` as a subprocess.
* Build deterministic command arguments.
* Capture progress and errors.
* Support a temporary output path.
* Support process cancellation.
* Verify that the required executable exists.

### Out of Scope

* Job handling
* Retry policy
* Source registration
* Media validation
* Playlist imports

### Dependencies

* P0-02

### Acceptance Criteria

* The adapter starts a download process.
* Progress can be consumed by the caller.
* Cancellation terminates the subprocess.
* Missing executable produces a clear error.
* Tests use a fake executable and do not access the internet.

### Required Tests

* Command construction unit tests
* Fake-process integration tests
* Cancellation test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-18 — Implement URL Download Worker

### Goal

Download registered URL sources into staging through the durable job system.

### In Scope

* Handle download jobs.
* Use the download command adapter.
* Persist progress at a bounded frequency.
* Finalize downloads with atomic rename.
* Store staging path and byte size.
* Classify failures as retryable or terminal where deterministic.

### Out of Scope

* Audio extraction
* Media inspection
* Playlist expansion
* Browser cookies UI
* Storage backpressure

### Dependencies

* P1-11
* P1-16
* P1-17

### Acceptance Criteria

* Download jobs produce a finalized staged file.
* Partial downloads are not exposed as completed media.
* Worker retries do not create duplicate finalized files.
* Progress is visible in persisted job state.
* Failures use the existing retry mechanism.

### Required Tests

* Worker tests with fake downloader
* Atomic finalization test
* Retry idempotency test
* Progress persistence test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-19 — Add Media Asset Persistence

### Goal

Create the persistent `Media Asset` entity produced by successful local imports and downloads.

### In Scope

* Add the media-assets migration.
* Link media assets to sources.
* Store controlled staging path, byte size, original filename, and media kind.
* Track ingestion lifecycle.
* Prevent multiple active media assets for the same successful ingestion result.

### Out of Scope

* `ffprobe` inspection
* Audio assets
* Content hashes
* Media deletion
* Deduplication

### Dependencies

* P1-04

### Acceptance Criteria

* A media asset belongs to one source.
* Successfully imported and downloaded files can be represented.
* Retried completion does not create duplicate media assets.
* File paths are not used as entity identifiers.
* Repository tests pass.

### Required Tests

* Migration integration tests
* Repository integration tests
* Idempotency test

### Documentation Updates

* Update `docs/domain-model.md` only if implemented lifecycle semantics differ.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-20 — Finalize Ingestion into Media Assets

### Goal

Connect successful import and download jobs to persistent media-asset creation.

### In Scope

* Create a media asset after successful staging.
* Update source ingestion status.
* Complete the associated job atomically with media-asset persistence.
* Handle repeated finalization safely.
* Preserve failure state when asset creation fails.

### Out of Scope

* Audio extraction
* Media inspection
* Deduplication
* Temporary video deletion
* Dashboard statistics

### Dependencies

* P1-13
* P1-18
* P1-19

### Acceptance Criteria

* Local imports create media assets.
* URL downloads create media assets.
* Job completion and media-asset creation are atomic.
* Repeated finalization does not create duplicate assets.
* Source state reflects successful ingestion.

### Required Tests

* Transaction integration tests
* Import finalization test
* Download finalization test
* Repeated-finalization test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-21 — Add Basic Dataset Manifest Import

### Goal

Import existing local audio datasets described by CSV, JSONL, or Parquet manifests.

### In Scope

* Accept project, manifest path, and audio-path field.
* Read CSV manifests.
* Read JSONL manifests.
* Read Parquet manifests.
* Resolve local audio paths.
* Register each dataset item as a source.
* Preserve unrecognized row metadata as structured JSON.
* Create local import jobs idempotently.

### Out of Scope

* Hugging Face network download
* Archive extraction
* Transcript processing
* Split generation
* Media normalization
* Dataset export

### Dependencies

* P1-12
* P1-19

### Acceptance Criteria

* Supported manifest formats are read.
* Valid audio paths create sources and import jobs.
* Missing files are reported per row.
* Original metadata is preserved.
* Re-importing the same manifest does not duplicate sources or jobs.
* Large manifests are processed incrementally where supported.

### Required Tests

* CSV manifest tests
* JSONL manifest tests
* Parquet manifest tests
* Missing-file tests
* Re-import idempotency test

### Documentation Updates

* Update `docs/product-scope.md` only if supported manifest behavior needs clarification.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-22 — Add Archive Dataset Import

### Goal

Import local datasets packaged as supported archive files.

### In Scope

* Accept ZIP, TAR, and TAR.GZ files.
* Extract into controlled temporary storage.
* Prevent path traversal.
* Locate a selected or supported manifest.
* Delegate records to the dataset manifest importer.
* Clean temporary extraction files after registration.

### Out of Scope

* Password-protected archives
* Nested archives
* Remote archive download
* Automatic manifest inference without a manifest
* Audio processing

### Dependencies

* P1-21

### Acceptance Criteria

* Supported archives are extracted safely.
* Path traversal entries are rejected.
* Manifest records are imported through the existing path.
* Temporary extracted files are cleaned after successful registration.
* Retry does not duplicate dataset items.

### Required Tests

* Archive extraction tests
* Path traversal security tests
* Import integration tests
* Cleanup tests

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-23 — Add Hugging Face Dataset Adapter

### Goal

Allow a locally accessible Hugging Face dataset to be registered through a controlled Python adapter.

### In Scope

* Define a narrow Rust-to-Python adapter contract.
* Accept dataset identifier, configuration, split, and audio column.
* Stream dataset records.
* Materialize audio files into controlled local staging.
* Preserve original dataset metadata.
* Register sources and import jobs idempotently.

### Out of Scope

* GPU inference
* Arbitrary dataset transformation
* Transcript normalization
* Dataset publishing
* Authentication UI
* Cloud inference services

### Dependencies

* P1-21
* P0-10

### Acceptance Criteria

* A supported dataset split can be enumerated.
* Audio items become registered sources and import jobs.
* Records are streamed rather than loaded entirely into memory.
* Existing local cache can be reused.
* Interrupted imports can resume without duplicating records.
* Tests use a local fixture dataset and require no internet.

### Required Tests

* Python adapter unit tests
* Rust adapter contract tests
* Local fixture dataset integration test
* Resume idempotency test

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pytest
```

---

## P1-24 — Add Source and Job Listing APIs

### Goal

Expose ingestion state for project sources and jobs.

### In Scope

* List project sources with pagination.
* Filter sources by type and ingestion state.
* List project jobs with pagination.
* Filter jobs by type and status.
* Return current progress and failure summaries.

### Out of Scope

* Manual retry
* Cancellation
* Job editing
* Dashboard aggregation
* Live streaming updates

### Dependencies

* P1-08
* P1-20

### Acceptance Criteria

* Sources can be listed and filtered.
* Jobs can be listed and filtered.
* Pagination is deterministic.
* Project isolation is enforced.
* Failure details are bounded and safe to display.

### Required Tests

* API integration tests
* Filter tests
* Pagination tests
* Project isolation tests

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-25 — Add Ingestion Dashboard

### Goal

Provide a project-level dashboard view for registering inputs and monitoring ingestion.

### In Scope

* Display source list.
* Display job list and progress.
* Add single URL registration form.
* Add local file and folder path forms.
* Add CSV and dataset manifest import forms.
* Display import summaries and failures.
* Poll for updated ingestion state.

### Out of Scope

* Audio processing statistics
* Dataset duration KPI
* Manual job retry
* Audio playback
* WebSocket or SSE updates

### Dependencies

* P1-03
* P1-14
* P1-15
* P1-16
* P1-21
* P1-24

### Acceptance Criteria

* Users can submit all supported Phase 1 input types.
* Sources and jobs are visible by project.
* Progress and failures update through polling.
* Duplicate submissions show deterministic outcomes.
* Frontend tests cover primary workflows.

### Required Tests

* Component tests
* Form validation tests
* API interaction tests
* Polling behavior test

### Documentation Updates

None.

### Validation Commands

```bash
pnpm test
pnpm build
```

---

## P1-26 — Add Manual Job Retry API

### Goal

Allow terminally failed ingestion jobs to be explicitly requeued.

### In Scope

* Add a retry endpoint.
* Restrict retry to eligible failed jobs.
* Clear active lease fields.
* Preserve attempt and failure history.
* Set a new availability time.
* Keep the operation idempotent.

### Out of Scope

* Bulk retry
* Retry policy editing
* Automatic source modification
* Cancelling active jobs
* Frontend control

### Dependencies

* P1-09
* P1-24

### Acceptance Criteria

* Eligible failed jobs can be requeued.
* Completed and active jobs cannot be retried.
* Project ownership is enforced.
* Repeated retry requests do not create duplicate jobs.
* API tests pass.

### Required Tests

* API tests
* State transition tests
* Project isolation tests

### Documentation Updates

None.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-27 — Add Manual Retry Dashboard Control

### Goal

Allow users to retry eligible failed ingestion jobs from the dashboard.

### In Scope

* Display retry eligibility.
* Add retry action.
* Show retry success and failure feedback.
* Refresh job state after retry.

### Out of Scope

* Bulk actions
* Retry-policy configuration
* Job cancellation
* Job priority editing

### Dependencies

* P1-25
* P1-26

### Acceptance Criteria

* Eligible failed jobs show a retry action.
* Ineligible jobs do not show an active retry action.
* Successful retry updates the visible status.
* API errors are displayed.
* Frontend tests pass.

### Required Tests

* Component tests
* Retry interaction test
* Error-state test

### Documentation Updates

None.

### Validation Commands

```bash
pnpm test
pnpm build
```

---

## P1-28 — Add Ingestion Recovery Startup Task

### Goal

Recover abandoned ingestion jobs automatically when the Rust backend or worker runtime starts.

### In Scope

* Run lease recovery during controlled startup.
* Record the number of recovered jobs.
* Avoid delaying liveness unnecessarily.
* Ensure repeated startup recovery is safe.
* Log recovery outcomes.

### Out of Scope

* Recurring background recovery
* Worker heartbeat
* Multi-machine coordination
* Dashboard alerts

### Dependencies

* P1-10
* P1-11

### Acceptance Criteria

* Expired running jobs are requeued on startup.
* Active leases remain untouched.
* Repeated startup does not alter healthy jobs.
* Recovery outcome is logged.
* Startup integration tests pass.

### Required Tests

* Startup recovery integration test
* Repeated-startup idempotency test

### Documentation Updates

* Update `docs/pipeline.md` if startup recovery behavior is not already represented.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-29 — Add Staging Storage Capacity Guard

### Goal

Prevent ingestion workers from filling the local disk faster than downstream processing can consume staged media.

### In Scope

* Measure configured staging filesystem capacity.
* Define configurable pause and resume thresholds.
* Prevent new download and local-import claims when storage is constrained.
* Resume claims after capacity recovers.
* Expose the guard state through the backend.

### Out of Scope

* Audio-storage capacity management
* Automatic file deletion
* Phase 2 media cleanup
* Per-source quotas
* Cloud storage

### Dependencies

* P1-11
* P1-13
* P1-18

### Acceptance Criteria

* New ingestion claims pause below the configured free-space threshold.
* Running jobs are not killed.
* Claims resume above the configured recovery threshold.
* Threshold hysteresis prevents rapid toggling.
* Filesystem errors fail safely.
* Tests use injectable filesystem metrics.

### Required Tests

* Capacity-guard unit tests
* Worker-claim integration test
* Hysteresis test

### Documentation Updates

* Update `docs/architecture.md` only if the operational boundary requires clarification.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## P1-30 — Complete Phase 1 End-to-End Ingestion Test

### Goal

Prove that all supported Phase 1 inputs produce persistent media assets through the asynchronous ingestion pipeline.

### In Scope

* Test local file registration and import.
* Test folder discovery and import.
* Test CSV URL registration with a fake downloader.
* Test local dataset manifest import.
* Test retry and restart recovery.
* Verify project, source, job, and media-asset relationships.

### Out of Scope

* Internet access
* Audio extraction
* Deduplication
* AI analysis
* Frontend browser automation

### Dependencies

* P1-20
* P1-22
* P1-23
* P1-28
* P1-29

### Acceptance Criteria

* Each supported input path produces the expected persistent entities.
* Processing remains idempotent across repeated execution.
* Failed jobs recover according to documented behavior.
* No external network or cloud service is required.
* The full Phase 1 backend test suite passes.

### Required Tests

* End-to-end backend integration tests

### Documentation Updates

* Update `docs/development-roadmap.md` only to mark Phase 1 complete after all criteria pass.

### Validation Commands

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pytest
pnpm test
pnpm build
```
