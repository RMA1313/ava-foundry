# Backend Backlog — Data Studio

## Goal

Complete the Data Studio backend from project management through ingestion, durable jobs, worker coordination, media and audio records, live events, statistics, security, recovery, maintenance, and production readiness.

## Global Constraints

- Rust owns HTTP APIs, orchestration, domain state, and transactions.
- PostgreSQL is the source of truth and durable job queue.
- Local filesystem storage remains the default storage backend.
- All multi-step operations must be restart-safe and idempotent.
- Applied migrations must never be edited in place.
- Managed files must never exist permanently without database ownership.
- Database records must never reference missing managed files without a recoverable error state.
- User-owned local source files must never be deleted.
- List APIs must share one pagination, filtering, sorting, and search contract.
- Internal paths, credentials, stack traces, and infrastructure details must not leak through APIs.
- Backend scope excludes transcription, segmentation, quality scoring, language models, and other AI inference logic.
- Each task must include tests appropriate to its risk and behavior.

---

## B-01 — Backend Baseline Audit

### Goal

Establish the real implementation status before adding new backend work.

### In Scope

- Inspect current models, repositories, migrations, routes, tests, and worker contracts
- Map implemented, partial, missing, and conflicting behavior
- Detect dead endpoints, duplicate logic, schema drift, and unsafe migration edits
- Produce a short backend gap report

### Out of Scope

- Feature implementation
- Frontend review
- Worker model implementation

### Dependencies

- None

### Acceptance Criteria

- Every backend module is classified as complete, partial, missing, or obsolete.
- Existing migrations are checked for unsafe in-place edits.
- Current API and database contracts are documented.
- The final backlog is updated to avoid duplicate or obsolete work.

### Tests

- No new tests required

### Validation

```bash
rtk cargo test -p backend --no-run
```

---

## B-02 — Shared Backend Contracts

### Goal

Define consistent contracts for identifiers, timestamps, statuses, list queries, and error responses.

### In Scope

- Shared identifier and timestamp types
- Shared pagination metadata
- Shared sorting and filtering conventions
- Shared API error envelope
- Stable error codes
- Request correlation ID

### Out of Scope

- Authentication
- Domain-specific validation
- Frontend types

### Dependencies

- B-01

### Acceptance Criteria

- All new list endpoints use the same response metadata.
- Invalid sort or filter fields return deterministic validation errors.
- All API errors use one documented structure.
- Internal errors expose only safe public details.

### Tests

- Serialization tests
- Validation tests
- Error-mapping tests

### Validation

```bash
rtk cargo test -p backend
```

---

## B-03 — Migration Safety and Schema Baseline

### Goal

Make schema evolution safe and reproducible.

### In Scope

- Review all existing migrations
- Add forward-only migrations for missing changes
- Validate up and down behavior
- Add required foreign keys, unique constraints, checks, and indexes
- Record schema version

### Out of Scope

- Production backup automation
- Cross-database support

### Dependencies

- B-01

### Acceptance Criteria

- No previously applied migration is modified.
- A clean database reaches the latest schema.
- An existing database upgrades without data loss.
- Required constraints reject invalid state.
- Migration tests run in CI.

### Tests

- Fresh migration test
- Upgrade migration test
- Down migration test where supported
- Constraint tests

### Validation

```bash
rtk cargo test -p backend migration
```

---

## B-04 — Project Management Completion

### Goal

Complete production-ready project CRUD and lifecycle behavior.

### In Scope

- Create, read, list, update, and delete projects
- Project status lifecycle
- Search, status filtering, sorting, and pagination
- Case-insensitive unique project names
- Transactional hard-delete cleanup
- Project summary statistics

### Out of Scope

- User permissions
- Frontend pages
- Dataset export

### Dependencies

- B-02
- B-03

### Acceptance Criteria

- CRUD works end to end.
- Duplicate names return a deterministic conflict.
- Invalid statuses are rejected.
- Delete removes all project-owned records in one transaction.
- List results are stable and paginated.
- Project statistics match stored data.

### Tests

- Repository tests
- API tests
- Duplicate-name tests
- Delete rollback tests
- Query contract tests

### Validation

```bash
rtk cargo test -p backend project
```

---

## B-05 — Source Domain Model

### Goal

Define the durable model for every ingestion source.

### In Scope

- Source types: file, folder, archive, dataset, URL, media upload
- Source status lifecycle
- Source metadata and configuration
- File count, byte count, timestamps, and error state
- Project ownership
- Idempotent creation keys

### Out of Scope

- Actual upload or scanning
- Worker execution
- Crawler sources

### Dependencies

- B-03
- B-04

### Acceptance Criteria

- Every source belongs to one project.
- Source type and status values are constrained.
- Repeated equivalent creation can be detected.
- Source configuration and failure state are persisted.
- Invalid transitions are rejected.

### Tests

- Migration tests
- Repository tests
- Lifecycle tests
- Idempotency tests

### Validation

```bash
rtk cargo test -p backend source
```

---

## B-06 — Source Management API

### Goal

Expose complete source management behavior.

### In Scope

- Source create, list, detail, update, delete
- Cancel and retry actions
- Search, filtering, sorting, and pagination
- Source summary statistics
- Project isolation

### Out of Scope

- Upload transport
- Folder scanning
- Dataset parsing
- Worker internals

### Dependencies

- B-05

### Acceptance Criteria

- Sources can be queried only within their project.
- Retry is allowed only from retryable states.
- Cancel is allowed only from active states.
- Delete cleans owned managed artifacts safely.
- List results use shared query contracts.

### Tests

- API integration tests
- Transition tests
- Project-isolation tests
- Delete tests

### Validation

```bash
rtk cargo test -p backend source_api
```

---

## B-07 — Storage Abstraction

### Goal

Define safe storage behavior independent of ingestion type.

### In Scope

- Storage trait
- Local filesystem implementation
- Managed root configuration
- Temporary and permanent namespaces
- Safe path generation
- Read, write, move, delete, stat, and existence operations
- Range reads
- Atomic finalization support

### Out of Scope

- Object storage implementation
- Deduplication
- Encryption at rest

### Dependencies

- B-03

### Acceptance Criteria

- All managed paths remain inside configured roots.
- Temporary and permanent files are separated.
- Final paths are deterministic.
- Atomic move is supported on the configured filesystem.
- Path traversal is rejected.
- Missing-file deletion is idempotent.

### Tests

- Path containment tests
- Storage integration tests
- Atomic move tests
- Range read tests

### Validation

```bash
rtk cargo test -p backend storage
```

---

## B-08 — Upload API

### Goal

Accept one or more local file uploads safely.

### In Scope

- Single and multi-file upload
- Streaming request handling
- File count and size limits
- Filename sanitization
- MIME and extension validation
- Temporary write and atomic finalize
- Hash calculation during upload
- Transactional Source and Job creation
- Incomplete upload cleanup

### Out of Scope

- Resumable multipart protocol
- Browser upload UI
- Media processing

### Dependencies

- B-05
- B-07
- B-15

### Acceptance Criteria

- Large files are streamed without full memory buffering.
- Interrupted uploads do not create permanent orphan files.
- Successful upload creates Source and initial Job atomically.
- Hash and byte size are persisted.
- Unsafe names and paths are rejected.
- Partial files are cleaned.

### Tests

- Upload API tests
- Large-file tests
- Interrupted-upload tests
- Multi-file transaction tests
- Malicious filename tests

### Validation

```bash
rtk cargo test -p backend upload
```

---

## B-09 — Local Folder Scan

### Goal

Register media from an approved local folder.

### In Scope

- Allowed-root validation
- Recursive scanning
- Include and exclude patterns
- Hidden-file behavior
- Symbolic-link policy
- Loop prevention
- New, changed, and removed file detection
- Source scan summary

### Out of Scope

- Remote folders
- Network filesystems
- File processing

### Dependencies

- B-05
- B-07
- B-15

### Acceptance Criteria

- Paths outside approved roots are rejected.
- Symlink loops cannot cause infinite traversal.
- Repeated scans do not duplicate unchanged files.
- Changed and removed files are reported deterministically.
- Discovered files create idempotent ingestion jobs.

### Tests

- Scan integration tests
- Symlink loop tests
- Include/exclude tests
- Large-directory tests
- Repeat-scan tests

### Validation

```bash
rtk cargo test -p backend folder_scan
```

---

## B-10 — Archive Import

### Goal

Extract supported archives safely and register their contents.

### In Scope

- ZIP, TAR, and TAR.GZ
- Archive type detection
- Safe extraction
- Zip Slip prevention
- Extracted size and file count limits
- Corrupt and encrypted archive handling
- Temporary extraction cleanup
- Child file registration

### Out of Scope

- RAR and 7z
- Password recovery
- Media processing

### Dependencies

- B-07
- B-15

### Acceptance Criteria

- Extracted paths remain inside the temporary root.
- Corrupt or unsupported archives fail deterministically.
- Limits prevent archive bombs.
- Extracted files preserve lineage to the source archive.
- Retry does not duplicate child records.

### Tests

- Archive integration tests
- Zip Slip tests
- Archive bomb limit tests
- Corrupt archive tests
- Idempotency tests

### Validation

```bash
rtk cargo test -p backend archive
```

---

## B-11 — Dataset Import

### Goal

Import datasets from CSV manifests.

### In Scope

- Local CSV and CSV URL sources
- Column validation and mapping
- Local path and remote URL rows
- Preview endpoint
- Row-level error reporting
- Partial success
- Import summary
- Idempotent retry

### Out of Scope

- Spreadsheet formats
- Annotation import
- Dataset export

### Dependencies

- B-05
- B-12
- B-15

### Acceptance Criteria

- Required columns are validated before execution.
- Invalid rows do not block valid rows.
- Row errors are persisted and queryable.
- Success and failure totals are accurate.
- Re-running the same manifest does not duplicate accepted rows.

### Tests

- Parser tests
- Preview API tests
- Large-manifest tests
- Partial-success tests
- Retry tests

### Validation

```bash
rtk cargo test -p backend dataset_import
```

---

## B-12 — URL Import

### Goal

Download remote media safely from approved HTTP sources.

### In Scope

- HTTP and HTTPS validation
- SSRF protection
- DNS and IP checks
- Redirect limits
- Timeout and size limits
- Content-Type validation
- Filename resolution
- Hashing and storage
- Retryable download failures

### Out of Scope

- Website crawling
- Authentication flows
- Streaming sites
- Browser automation

### Dependencies

- B-07
- B-15

### Acceptance Criteria

- Private, loopback, link-local, and disallowed network targets are blocked.
- Redirects are revalidated.
- Downloads stop at configured limits.
- Successful downloads create managed files and ingestion jobs.
- Retry behavior is deterministic.

### Tests

- SSRF tests
- Redirect tests
- Timeout tests
- Size-limit tests
- Content-Type tests

### Validation

```bash
rtk cargo test -p backend url_import
```

---

## B-13 — Media Asset Domain Model

### Goal

Persist every ingested media file before processing.

### In Scope

- Media record and repository
- Project and Source lineage
- Original name, path, MIME, extension, size, and hash
- Processing status and error state
- Raw technical metadata
- Idempotent creation constraints

### Out of Scope

- FFprobe execution
- Audio extraction
- Duplicate grouping

### Dependencies

- B-03
- B-05
- B-07

### Acceptance Criteria

- Every Media Asset belongs to one Source and one project.
- Repeated registration does not duplicate the same managed file.
- File metadata and processing state are persisted.
- Invalid lineage is rejected.

### Tests

- Migration tests
- Repository tests
- Idempotency tests
- Relationship tests

### Validation

```bash
rtk cargo test -p backend media_repository
```

---

## B-14 — Media Asset API

### Goal

Expose media records for browsing and management.

### In Scope

- Project media listing
- Media detail
- Search, filtering, sorting, and pagination
- Source and Job lineage
- Technical metadata
- Single and bulk delete

### Out of Scope

- Media editing
- Audio playback
- Processing implementation

### Dependencies

- B-13
- B-02

### Acceptance Criteria

- Results are isolated by project.
- Filters support type, status, source, and processing state.
- Bulk delete is transactional at the database level.
- Managed files follow the configured deletion policy.
- Missing assets return deterministic errors.

### Tests

- API tests
- Filtering tests
- Pagination tests
- Bulk-delete tests
- Project-isolation tests

### Validation

```bash
rtk cargo test -p backend media_api
```

---

## B-15 — Durable Job Model

### Goal

Provide the canonical durable job lifecycle.

### In Scope

- Job types and statuses
- Parent and child jobs
- Batch jobs
- Progress and counters
- Attempts, retry limits, and next-run time
- Lease owner and lease expiry
- Error and result payloads
- Cancellation state

### Out of Scope

- Worker implementation
- SSE delivery
- Frontend controls

### Dependencies

- B-03
- B-02

### Acceptance Criteria

- Invalid transitions are rejected.
- Active jobs can be leased by only one worker.
- Retry scheduling is persisted.
- Progress is monotonic unless explicitly reset by retry.
- Parent and batch aggregates are deterministic.
- Job creation is idempotent where required.

### Tests

- Repository tests
- Lifecycle tests
- Lease tests
- Retry tests
- Concurrency tests

### Validation

```bash
rtk cargo test -p backend job
```

---

## B-16 — Job Management API

### Goal

Expose complete job inspection and control.

### In Scope

- Job list and detail
- Search, filtering, sorting, and pagination
- Cancel, retry, and recover actions
- Job history and attempt data
- Job logs endpoint
- Job statistics

### Out of Scope

- Worker claim endpoint
- SSE
- Manual job payload editing

### Dependencies

- B-15

### Acceptance Criteria

- Only valid actions are accepted for the current state.
- Retry creates or resets work without duplicate active execution.
- Recovery is limited to stale or recoverable jobs.
- Attempt history remains queryable.
- Job list metadata is stable.

### Tests

- API tests
- Action-transition tests
- History tests
- Query contract tests

### Validation

```bash
rtk cargo test -p backend job_api
```

---

## B-17 — Worker Registration and Heartbeat

### Goal

Track worker availability and capabilities.

### In Scope

- Worker registration
- Worker identity and version
- Supported job types
- Heartbeat timestamp
- Active lease count
- Worker status and expiry

### Out of Scope

- Worker process implementation
- Distributed discovery
- Autoscaling

### Dependencies

- B-15

### Acceptance Criteria

- Workers can register idempotently.
- Missing heartbeats mark workers unavailable.
- Backend can query worker capabilities.
- Stale worker state does not block job recovery.

### Tests

- Registration tests
- Heartbeat tests
- Expiry tests
- Capability tests

### Validation

```bash
rtk cargo test -p backend worker_registry
```

---

## B-18 — Job Claim and Lease Protocol

### Goal

Allow workers to claim jobs safely.

### In Scope

- Atomic claim query
- Capability matching
- Priority ordering
- Lease duration
- Lease renewal
- Claim limits
- Pause and resume state
- Duplicate-delivery protection

### Out of Scope

- External message broker
- Worker execution logic
- Cross-region scheduling

### Dependencies

- B-15
- B-17

### Acceptance Criteria

- One active lease exists per job.
- Concurrent workers cannot claim the same job.
- Lease renewal is ownership-checked.
- Expired leases become recoverable.
- Priority and availability rules are deterministic.

### Tests

- Concurrent-claim tests
- Lease-renewal tests
- Expiry tests
- Capability-matching tests
- Queue pause tests

### Validation

```bash
rtk cargo test -p backend job_claim
```

---

## B-19 — Worker Progress and Completion Protocol

### Goal

Accept progress, success, and failure reports from workers.

### In Scope

- Progress updates
- Stage counters
- Structured results
- Retryable and terminal failures
- Cancellation acknowledgment
- Artifact registration
- Ownership validation

### Out of Scope

- SSE broadcast implementation
- Worker-side processing
- Manual result editing

### Dependencies

- B-18
- B-13
- B-20

### Acceptance Criteria

- Only the lease owner can update an active job.
- Late updates from expired leases are rejected.
- Completion is transactional with required output records.
- Failure preserves useful structured context.
- Repeated completion requests are idempotent.

### Tests

- Ownership tests
- Completion transaction tests
- Late-update tests
- Duplicate-completion tests
- Failure tests

### Validation

```bash
rtk cargo test -p backend worker_updates
```

---

## B-20 — Audio Asset Domain Model

### Goal

Persist normalized audio outputs and their lineage.

### In Scope

- Audio Asset migration and repository
- Media relationship
- Duration, codec, format, sample rate, channels, size, hash, and path
- Processing and cleanup state
- Normalization profile
- Idempotent creation constraints

### Out of Scope

- Audio extraction
- Streaming API
- AI metadata

### Dependencies

- B-03
- B-13

### Acceptance Criteria

- One finalized normalized output cannot create duplicate Audio Assets.
- Technical metadata is persisted.
- Media lineage is mandatory.
- Invalid or incomplete finalized records are rejected.

### Tests

- Migration tests
- Repository tests
- Idempotency tests
- Metadata constraint tests

### Validation

```bash
rtk cargo test -p backend audio_asset
```

---

## B-21 — Audio Asset API and Range Streaming

### Goal

Expose audio metadata and controlled file access.

### In Scope

- Audio Asset list and detail
- Search, filtering, sorting, and pagination
- Media and Source lineage
- Processing and cleanup state
- HTTP Range requests
- Content headers and file existence checks
- Single and bulk delete

### Out of Scope

- Waveforms
- Metadata editing
- Transcoding on request

### Dependencies

- B-07
- B-20

### Acceptance Criteria

- Project isolation is enforced.
- Range requests support audio playback.
- Missing managed files produce a recoverable storage error.
- Delete respects lineage and retention rules.
- List queries are deterministic.

### Tests

- API tests
- Range request tests
- Missing-file tests
- Filtering tests
- Bulk-delete tests

### Validation

```bash
rtk cargo test -p backend audio_api
```

---

## B-22 — Transactional Ingestion Flow

### Goal

Guarantee consistent Source, Media, and Job creation.

### In Scope

- Source creation transaction
- Managed file registration
- Media creation
- Initial Job creation
- Database rollback
- Storage compensation on failure
- Idempotency key handling

### Out of Scope

- Worker processing
- Audio finalization
- Duplicate grouping

### Dependencies

- B-05
- B-07
- B-13
- B-15

### Acceptance Criteria

- Successful ingestion creates all required records.
- Database failure does not leave an unmanaged permanent file.
- Storage failure does not leave committed processing records.
- Repeated requests return the same logical result.
- Partial failure is recoverable.

### Tests

- End-to-end ingestion tests
- Database rollback tests
- Storage compensation tests
- Idempotency tests

### Validation

```bash
rtk cargo test -p backend ingestion_flow
```

---

## B-23 — Event Model and Outbox

### Goal

Persist backend events reliably before live delivery.

### In Scope

- Event types and versioned payloads
- Project-scoped event stream
- Transactional outbox
- Event sequence or cursor
- Retention policy
- Event replay query

### Out of Scope

- SSE endpoint
- External event broker
- Long-term analytics

### Dependencies

- B-03
- B-02

### Acceptance Criteria

- Domain changes and events commit atomically.
- Events have stable IDs and project scope.
- Consumers can replay events after a cursor.
- Old events are removed only by retention policy.
- Payload versions are explicit.

### Tests

- Outbox transaction tests
- Ordering tests
- Replay tests
- Retention tests

### Validation

```bash
rtk cargo test -p backend event_outbox
```

---

## B-24 — Server-Sent Events

### Goal

Deliver live project updates to the frontend.

### In Scope

- Project-scoped SSE endpoint
- Source, Job, Media, Audio Asset, Duplicate, and statistics events
- Heartbeat frames
- Last-Event-ID reconnect
- Replay from outbox
- Slow-client handling
- Connection cleanup

### Out of Scope

- WebSocket
- Cross-instance pub/sub
- Browser implementation

### Dependencies

- B-23

### Acceptance Criteria

- Clients receive ordered project events.
- Reconnect resumes after the last confirmed event.
- Slow or disconnected clients do not block backend work.
- Heartbeats keep idle connections alive.
- Project isolation is enforced.

### Tests

- SSE integration tests
- Reconnect tests
- Ordering tests
- Slow-client tests
- Isolation tests

### Validation

```bash
rtk cargo test -p backend sse
```

---

## B-25 — Duplicate Data Model

### Goal

Represent exact duplicate files and review state.

### In Scope

- Duplicate groups and members
- Exact hash-based grouping
- Canonical member selection
- Confirmed and rejected states
- Merge and remove operations
- Project and Source scope

### Out of Scope

- Acoustic fingerprinting
- Near-duplicate detection
- AI similarity

### Dependencies

- B-13
- B-20

### Acceptance Criteria

- Equal hashes can be grouped idempotently.
- A member belongs to at most one active exact-duplicate group.
- Canonical selection is deterministic.
- Review actions are persisted.
- Group statistics are accurate.

### Tests

- Repository tests
- Grouping tests
- Canonical-selection tests
- Review-state tests
- Merge tests

### Validation

```bash
rtk cargo test -p backend duplicate
```

---

## B-26 — Duplicate API

### Goal

Expose exact duplicate groups for review and action.

### In Scope

- Group list and detail
- Filtering by project, Source, type, and review state
- Set canonical member
- Confirm, reject, merge, and remove member
- Bulk actions
- Duplicate statistics

### Out of Scope

- File preview UI
- Near-duplicate scoring
- Automatic deletion

### Dependencies

- B-25

### Acceptance Criteria

- All actions preserve group consistency.
- Project isolation is enforced.
- Invalid group operations return conflicts.
- Bulk actions are transactional where possible.
- Statistics reflect current group state.

### Tests

- API tests
- Action tests
- Bulk-operation tests
- Isolation tests

### Validation

```bash
rtk cargo test -p backend duplicate_api
```

---

## B-27 — Statistics Service

### Goal

Provide accurate project and system statistics.

### In Scope

- Source counts and bytes
- Media counts and bytes
- Audio counts, bytes, and duration
- Job counts by state
- Duplicate counts
- Error counts
- Daily and weekly ingestion trends
- Duration by category when category data exists
- Query optimization

### Out of Scope

- AI quality metrics
- Billing
- Long-term business intelligence

### Dependencies

- B-04
- B-05
- B-13
- B-15
- B-20
- B-25

### Acceptance Criteria

- Statistics match source-of-truth records.
- Deleted or failed records follow documented inclusion rules.
- Expensive queries have suitable indexes or cached summaries.
- Empty projects return zero-valued results.
- Time-bucket boundaries are deterministic.

### Tests

- Repository tests
- Accuracy fixtures
- Empty-state tests
- Time-bucket tests
- Query performance tests

### Validation

```bash
rtk cargo test -p backend statistics
```

---

## B-28 — Statistics API and Cache Invalidation

### Goal

Expose dashboard data without stale results.

### In Scope

- Project summary endpoint
- Trend endpoints
- Source and Job breakdowns
- Cache layer for expensive summaries
- Event-driven or transactional invalidation
- Cache bypass for tests and administration

### Out of Scope

- Frontend charts
- Distributed cache requirement
- Historical warehouse

### Dependencies

- B-23
- B-27

### Acceptance Criteria

- API results are consistent with repository calculations.
- Relevant domain changes invalidate affected cache entries.
- Missing cache never breaks the request.
- Cached responses remain project-scoped.
- Response contracts are documented.

### Tests

- API tests
- Cache hit and miss tests
- Invalidation tests
- Isolation tests

### Validation

```bash
rtk cargo test -p backend statistics_api
```

---

## B-29 — Shared Search, Filter, Sort, and Pagination Layer

### Goal

Remove inconsistent list-query behavior across APIs.

### In Scope

- Offset pagination contract
- Cursor pagination support for large tables
- Stable tie-break sorting
- Allowed sort fields
- Typed filter parsing
- Search normalization
- Maximum page size
- Common response metadata

### Out of Scope

- Full-text search engine
- Cross-project global search
- Frontend query state

### Dependencies

- B-02

### Acceptance Criteria

- Project, Source, Media, Audio Asset, Job, and Duplicate lists use shared behavior.
- Invalid query fields fail before database execution.
- Pagination is stable under equal primary sort values.
- Maximum limits are enforced.
- Index requirements are documented.

### Tests

- Parser tests
- Stability tests
- Invalid-field tests
- Cursor tests
- Combined-query tests

### Validation

```bash
rtk cargo test -p backend query
```

---

## B-30 — Transaction and Data Integrity Hardening

### Goal

Prevent orphan records and partial domain transitions.

### In Scope

- Review all create, update, complete, and delete transactions
- Foreign keys and cascade rules
- Check constraints
- Storage compensation
- Orphan detection queries
- Concurrency conflict handling

### Out of Scope

- Distributed transactions
- External storage transactions
- User-facing repair UI

### Dependencies

- B-03
- B-22

### Acceptance Criteria

- Critical multi-record updates are transactional.
- Orphan records are prevented by schema or detected by reconciliation.
- Storage failures leave recoverable database state.
- Concurrent conflicting operations return deterministic errors.
- Rollback behavior is tested.

### Tests

- Rollback tests
- Foreign-key tests
- Concurrent update tests
- Storage failure tests
- Orphan detection tests

### Validation

```bash
rtk cargo test -p backend integrity
```

---

## B-31 — Recovery and Stale Work Handling

### Goal

Recover safely after backend or worker interruption.

### In Scope

- Expired lease recovery
- Stale Job detection
- Abandoned temporary file detection
- Safe requeue
- Finalized artifact protection
- Recovery audit records
- Concurrent recovery protection

### Out of Scope

- Manual repair UI
- Permanent artifact deletion
- Cross-region failover

### Dependencies

- B-18
- B-19
- B-30

### Acceptance Criteria

- Expired active jobs become recoverable.
- Finalized files and completed outputs are never removed.
- Repeated recovery is idempotent.
- Multiple recovery processes cannot duplicate active work.
- Recovery actions remain auditable.

### Tests

- Crash recovery tests
- Concurrent recovery tests
- Finalized-file protection tests
- Stale temporary file tests

### Validation

```bash
rtk cargo test -p backend recovery
```

---

## B-32 — Queue Backpressure and Resource Limits

### Goal

Prevent local resource exhaustion.

### In Scope

- Global and per-job-type concurrency limits
- Queue depth limits
- Temporary storage thresholds
- Permanent storage thresholds
- Pause and resume state
- Hysteresis
- Safe behavior when resource checks fail

### Out of Scope

- GPU scheduling
- Cluster autoscaling
- Network bandwidth shaping

### Dependencies

- B-18
- B-07

### Acceptance Criteria

- New claims pause when configured limits are reached.
- Running work is not terminated by backpressure.
- Claims resume after recovery thresholds are met.
- Hysteresis prevents rapid state changes.
- Resource-check failure defaults to safe behavior.

### Tests

- Concurrency tests
- Storage-threshold tests
- Queue-limit tests
- Hysteresis tests
- Failure-mode tests

### Validation

```bash
rtk cargo test -p backend backpressure
```

---

## B-33 — API Validation and Security Hardening

### Goal

Protect backend entry points from unsafe input and abuse.

### In Scope

- Body, query, path, and header validation
- Request size limits
- Upload limits
- CORS configuration
- Security headers
- Rate limiting
- Path traversal protection
- SSRF protection
- Archive extraction protection
- Sensitive error redaction

### Out of Scope

- User authentication
- WAF integration
- Network firewall management

### Dependencies

- B-02
- B-07
- B-08
- B-10
- B-12

### Acceptance Criteria

- Known malicious path, URL, and archive inputs are rejected.
- Oversized requests fail before resource exhaustion.
- Public errors reveal no local paths or internal stack traces.
- Rate limits are configurable.
- Security behavior is covered by tests.

### Tests

- Malicious input tests
- Size-limit tests
- Rate-limit tests
- Error-redaction tests
- CORS tests

### Validation

```bash
rtk cargo test -p backend security
```

---

## B-34 — Authentication and Authorization

### Goal

Add multi-user access control when the product moves beyond trusted local use.

### In Scope

- User identity model
- Session or token validation
- Project ownership
- Roles and permissions
- Authorization middleware
- SSE authorization
- Audit attribution

### Out of Scope

- External enterprise SSO unless separately approved
- Billing roles
- Organization hierarchy

### Dependencies

- B-04
- B-24
- B-33

### Acceptance Criteria

- Unauthenticated requests are rejected where required.
- Users cannot access another user's projects.
- Destructive actions require explicit permission.
- SSE streams enforce the same project permissions as REST APIs.
- Audit records identify the acting user.

### Tests

- Authentication tests
- Role tests
- Cross-user isolation tests
- SSE authorization tests

### Validation

```bash
rtk cargo test -p backend auth
```

---

## B-35 — Structured Logging and Audit Trail

### Goal

Make backend behavior observable and destructive actions traceable.

### In Scope

- Structured logs
- Request and correlation IDs
- Request duration
- Database and storage error context
- Job transition logs
- Import, retry, recovery, and delete audit events
- Sensitive-field redaction
- Audit query API

### Out of Scope

- External log platform
- Long-term log warehouse
- Frontend audit viewer

### Dependencies

- B-02
- B-16
- B-31

### Acceptance Criteria

- Every request has a correlation ID.
- Job transitions and destructive actions are auditable.
- Logs contain useful context without secrets or raw credentials.
- Audit records are project-scoped.
- Logging failure does not break business operations.

### Tests

- Log context tests
- Redaction tests
- Audit repository tests
- Audit API tests

### Validation

```bash
rtk cargo test -p backend audit
```

---

## B-36 — Health, Readiness, and Graceful Shutdown

### Goal

Expose reliable service health and shutdown behavior.

### In Scope

- Liveness endpoint
- Readiness endpoint
- PostgreSQL check
- Storage check
- Queue state check
- Worker availability summary
- Backend and schema version
- Graceful shutdown
- Stop new claims during shutdown

### Out of Scope

- External monitoring stack
- Automatic restart policy
- Worker shutdown implementation

### Dependencies

- B-07
- B-17
- B-18

### Acceptance Criteria

- Liveness does not depend on optional external systems.
- Readiness fails when critical dependencies are unavailable.
- Shutdown stops accepting new work before process exit.
- Active HTTP requests receive a bounded drain period.
- Version information is available safely.

### Tests

- Health endpoint tests
- Dependency failure tests
- Shutdown tests
- Queue-drain tests

### Validation

```bash
rtk cargo test -p backend health
```

---

## B-37 — Cleanup and Reconciliation

### Goal

Remove abandoned temporary data and detect storage/database drift.

### In Scope

- Incomplete upload cleanup
- Temporary archive cleanup
- Stale processing output cleanup
- Old Job and Event retention
- Orphan file detection
- Orphan record detection
- Database/storage reconciliation
- Dry-run mode
- Cleanup report

### Out of Scope

- Permanent audio deletion without policy
- User-owned source deletion
- Backup pruning

### Dependencies

- B-07
- B-23
- B-30
- B-31

### Acceptance Criteria

- Cleanup never deletes user-owned files.
- Dry-run reports exactly what would change.
- Missing temporary files are handled idempotently.
- Reconciliation identifies both file and record orphans.
- Every destructive cleanup action is audited.

### Tests

- Cleanup tests
- Dry-run tests
- User-file protection tests
- Orphan fixture tests
- Idempotency tests

### Validation

```bash
rtk cargo test -p backend cleanup
```

---

## B-38 — Backend Administration Commands

### Goal

Provide safe operational commands for maintenance and recovery.

### In Scope

- Run migrations
- Create and inspect projects
- Import local files
- Retry and recover Jobs
- Recalculate statistics
- Rebuild exact duplicate groups
- Reconcile storage
- Run cleanup
- Inspect queue and workers

### Out of Scope

- Interactive web admin panel
- Direct arbitrary SQL
- AI worker controls

### Dependencies

- B-04
- B-16
- B-25
- B-27
- B-31
- B-37

### Acceptance Criteria

- Commands reuse domain services instead of bypassing invariants.
- Destructive commands support confirmation or explicit force flags.
- Cleanup and reconciliation support dry-run.
- Commands return non-zero exit codes on failure.
- Output is concise and script-friendly.

### Tests

- Command integration tests
- Exit-code tests
- Dry-run tests
- Destructive-operation safety tests

### Validation

```bash
rtk cargo test -p backend cli
```

---

## B-39 — OpenAPI and Backend Documentation

### Goal

Document the final backend contracts and operational behavior.

### In Scope

- OpenAPI for all public endpoints
- Request and response examples
- Error code catalog
- Pagination and filtering contract
- Job lifecycle
- Worker protocol
- SSE protocol
- Storage policy
- Migration policy
- Recovery and cleanup procedures

### Out of Scope

- End-user frontend documentation
- Deployment runbook
- Model documentation

### Dependencies

- All public backend APIs

### Acceptance Criteria

- OpenAPI matches implemented routes.
- Every endpoint includes success and error responses.
- Job and Source state transitions are documented.
- Worker and SSE payload versions are documented.
- Migration and cleanup rules are explicit.

### Tests

- OpenAPI generation test
- Route/spec consistency test where practical

### Validation

```bash
rtk cargo test -p backend openapi
```

---

## B-40 — Backend End-to-End Lifecycle

### Goal

Prove the complete backend lifecycle from project creation to finalized audio visibility.

### In Scope

- Create project
- Upload file
- Create Source
- Create Media Asset
- Create and claim Job
- Accept worker progress
- Register Audio Asset
- Complete Job
- Update statistics
- Publish SSE events
- Retry after interruption
- Delete project and verify cleanup

### Out of Scope

- Real AI inference
- Browser automation
- External crawlers
- Frontend rendering

### Dependencies

- B-04 through B-39 as applicable

### Acceptance Criteria

- One uploaded fixture produces one visible finalized Audio Asset.
- Source, Media, Job, and Audio lineage is complete.
- Repeated execution creates no duplicates.
- Interruption and retry recover safely.
- Statistics reflect the final state.
- Expected SSE events are replayable.
- Project deletion removes owned records and managed files safely.

### Tests

- Backend end-to-end integration tests
- Restart and idempotency tests
- Failure and retry tests
- Delete cleanup tests
- SSE replay tests

### Final Validation

```bash
rtk cargo fmt --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test --workspace
rtk pytest
```

---

## Backend Completion

The backend backlog is complete when:

- B-01 through B-40 are complete or explicitly marked not applicable.
- All final validation commands pass.
- The full ingestion-to-audio lifecycle is restart-safe and idempotent.
- No managed file or project-owned record is orphaned after success, failure, retry, recovery, or deletion.
- Worker claims, progress, completion, retry, and recovery are concurrency-safe.
- REST and SSE project isolation is enforced.
- Statistics are accurate and update after domain changes.
- Security tests cover upload, URL import, archive extraction, path handling, and error redaction.
- OpenAPI and backend operational documentation match the implementation.
