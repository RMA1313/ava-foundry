# Phase 1 — Ingestion



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
