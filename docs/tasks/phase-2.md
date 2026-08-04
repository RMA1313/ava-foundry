# Phase 2 — Audio Processing

## Goal

Convert ingested media assets into validated permanent audio assets, then safely remove temporary video files.

## Phase Constraints

- Rust owns orchestration and processing state.
- PostgreSQL remains the durable job queue.
- `ffprobe` and `FFmpeg` run as local subprocesses.
- Audio is stored on the local filesystem.
- Video is deleted only after audio is validated and finalized.
- Every operation is restart-safe and idempotent.
- Phase 2 excludes deduplication, AI analysis, transcription, quality scoring, and interval mining.

---

## P2-01 — Audio Asset Model

### Goal

Add the persistent domain model for normalized audio.

### In Scope

- `audio_assets` migration
- Rust domain model and repository
- Media Asset relationship
- Processing status
- Duration, format, codec, sample rate, channels, byte size, and storage path
- Created and updated timestamps
- Idempotent creation constraints

### Out of Scope

- Audio extraction
- Fingerprints
- Classification
- Dashboard

### Dependencies

- Phase 1 complete

### Acceptance Criteria

- An Audio Asset belongs to exactly one Media Asset.
- Repeated creation cannot produce duplicate Audio Assets.
- Technical metadata is persisted.
- Repository create, read, and list operations work.
- Invalid relationships are rejected.

### Tests

- Migration tests
- Repository integration tests
- Idempotency tests

### Validation

```bash
rtk cargo fmt --check
rtk cargo clippy -p backend --all-targets -- -D warnings
rtk cargo test -p backend
P2-02 — Audio Storage
Goal

Define deterministic permanent and temporary audio storage behavior.

In Scope
Configurable permanent audio root
Temporary extraction directory
Content-independent temporary paths
Deterministic final paths based on Audio Asset identity
Directory creation
Path containment checks
Atomic rename support
Out of Scope
FFmpeg execution
File deletion policy
Object storage
Content-addressed deduplication
Dependencies
P2-01
Acceptance Criteria
Temporary and permanent paths remain inside configured roots.
Final paths are deterministic.
Missing directories are created safely.
Atomic finalization is supported on the configured filesystem.
Retrying path generation produces the same result.
Tests
Path-generation unit tests
Path-containment tests
Atomic-move integration tests
Validation
rtk cargo test -p backend
P2-03 — FFprobe Adapter
Goal

Inspect local media through a controlled ffprobe subprocess.

In Scope
Executable availability check
Deterministic command construction
JSON output parsing
Timeout and cancellation
Exit-status and malformed-output handling
Fake executable tests
Out of Scope
Metadata persistence
FFmpeg execution
Internet access
Media modification
Dependencies
Phase 1 complete
Acceptance Criteria
The adapter returns container and stream metadata.
Missing executables return clear errors.
Timeout and cancellation terminate the subprocess.
Invalid output fails deterministically.
Tests require no real media tools.
Tests
Command unit tests
Fake-process integration tests
Timeout and cancellation tests
JSON parsing tests
Validation
rtk cargo test -p backend
P2-04 — Media Metadata Inspection
Goal

Inspect Media Assets and persist their technical metadata.

In Scope
Media inspection job type
Worker handler
Audio-stream detection
Duration, container, streams, codecs, sample rates, and channels
Media Asset inspection state
Retry-safe persistence
Out of Scope
Audio extraction
Quality scoring
Content classification
Dependencies
P2-03
Acceptance Criteria
Media with an audio stream is marked inspectable.
Media without audio reaches a deterministic terminal failure.
Metadata is persisted once.
Repeated execution does not duplicate state.
Inspection failures use existing retry behavior.
Tests
Worker tests
Repository integration tests
No-audio-stream test
Idempotency test
Validation
rtk cargo test -p backend
P2-05 — FFmpeg Adapter
Goal

Provide controlled local FFmpeg execution for audio extraction.

In Scope
Executable availability check
Deterministic command construction
Input and output path handling
Progress parsing
Timeout and cancellation
Exit-status handling
Fake executable tests
Out of Scope
Job processing
Output validation
Permanent finalization
Network input
Dependencies
P2-02
Acceptance Criteria
The adapter extracts into a temporary output path.
Partial output is never treated as finalized.
Progress is available to the caller.
Cancellation terminates FFmpeg.
Missing FFmpeg returns a clear error.
Tests
Command unit tests
Fake-process tests
Progress parsing tests
Cancellation tests
Validation
rtk cargo test -p backend
P2-06 — Audio Extraction Worker
Goal

Extract audio asynchronously from inspected Media Assets.

In Scope
Extraction job type
Worker handler
FFmpeg adapter integration
Temporary output creation
Progress persistence
Retry and cancellation behavior
Extraction lifecycle state
Out of Scope
Final Audio Asset creation
Permanent file move
Video deletion
Audio validation
Dependencies
P2-04
P2-05
Acceptance Criteria
Eligible Media Assets create extraction jobs idempotently.
Extraction produces only a temporary audio file.
Retries do not create multiple active outputs.
Partial outputs are not finalized.
Failures integrate with the durable job system.
Tests
Worker integration tests
Retry tests
Cancellation tests
Partial-output tests
Job-creation idempotency tests
Validation
rtk cargo test -p backend
P2-07 — Audio Normalization
Goal

Produce one standard permanent audio representation.

In Scope
FLAC output
16 kHz sample rate
Mono channel layout
Explicit codec arguments
Configurable normalization profile
Preservation of original Media Asset metadata
Out of Scope
Loudness normalization
Noise removal
Quality enhancement
Multiple permanent formats
Dependencies
P2-05
P2-06
Acceptance Criteria
Extracted output uses the configured normalization profile.
Default output is FLAC, 16 kHz, mono.
Command arguments are deterministic.
Existing compliant audio still passes through the same controlled process.
The active profile is recorded with processing state.
Tests
Normalization command tests
Configuration tests
Worker integration tests
Validation
rtk cargo test -p backend
P2-08 — Audio Validation
Goal

Verify extracted audio before permanent finalization.

In Scope
Output existence check
Non-zero size check
FFprobe validation
Audio-stream verification
Duration validation
Normalization-profile verification
Validation failure state
Out of Scope
Perceptual quality scoring
Language detection
Content analysis
Duplicate detection
Dependencies
P2-03
P2-07
Acceptance Criteria
Missing, empty, corrupt, or non-audio output is rejected.
Duration must be positive.
Output format must match the active profile.
Validation results are persisted.
Repeated validation is idempotent.
Tests
Valid-output tests
Empty-file tests
Corrupt-output tests
Profile-mismatch tests
Idempotency tests
Validation
rtk cargo test -p backend
P2-09 — Atomic Audio Finalization
Goal

Move validated audio into permanent storage safely.

In Scope
Temporary-to-permanent atomic rename
Existing-destination handling
File size verification
Final path persistence
Crash-safe retry behavior
Out of Scope
Audio Asset transaction finalization
Video cleanup
Cross-filesystem copy fallback
Dependencies
P2-02
P2-08
Acceptance Criteria
Only validated audio can be finalized.
Permanent paths never expose partial files.
Repeated execution returns the same finalized file.
Existing conflicting files produce a deterministic error.
Recovery after interruption is safe.
Tests
Atomic-finalization tests
Existing-destination tests
Interrupted-operation tests
Idempotency tests
Validation
rtk cargo test -p backend
P2-10 — Transactional Audio Completion
Goal

Atomically create the Audio Asset and complete its processing job.

In Scope
Audio Asset creation
Media Asset lifecycle update
Job completion
Permanent file metadata
Transactional persistence
Repeated-finalization handling
Out of Scope
Video deletion
Deduplication
Classification
Dependencies
P2-01
P2-09
Acceptance Criteria
Audio Asset creation and job completion are atomic.
Failure cannot leave a completed job without an Audio Asset.
Repeated completion does not create duplicates.
Media Asset status reflects successful audio conversion.
Technical metadata matches the permanent file.
Tests
Transaction integration tests
Rollback tests
Repeated-completion tests
Metadata consistency tests
Validation
rtk cargo test -p backend
P2-11 — Temporary Media Cleanup
Goal

Delete temporary video only after successful audio completion.

In Scope
Cleanup job type
Video and temporary-file deletion
Missing-file idempotency
Cleanup lifecycle state
Failure recording and retry
Out of Scope
Deleting user-owned local source files
Deleting permanent audio
Retention settings
General filesystem garbage collection
Dependencies
P2-10
Acceptance Criteria
Video cleanup begins only after Audio Asset completion.
User-owned source files are never deleted.
Missing temporary files count as successful cleanup.
Failed deletion is retryable.
Permanent audio remains untouched.
Tests
Cleanup worker tests
User-file protection tests
Missing-file tests
Retry tests
Validation
rtk cargo test -p backend
P2-12 — Processing Recovery
Goal

Recover incomplete audio-processing operations after interruption.

In Scope
Expired job lease recovery
Stale temporary-output detection
Requeueing safe operations
Cleaning abandoned partial outputs
Preserving finalized audio
Out of Scope
Manual repair UI
Phase 1 ingestion recovery changes
Permanent audio deletion
Dependencies
P2-06
P2-09
P2-11
Acceptance Criteria
Expired processing jobs are safely requeued.
Abandoned partial outputs are detected.
Finalized files are never removed.
Repeated recovery is idempotent.
Recovery does not duplicate Audio Assets.
Tests
Crash-recovery tests
Stale-output tests
Finalized-file protection tests
Concurrent-recovery tests
Validation
rtk cargo test -p backend
P2-13 — Extraction Backpressure
Goal

Prevent extraction from exhausting local CPU, disk, or queue capacity.

In Scope
Configurable extraction concurrency
Temporary-storage thresholds
Pending-output queue limit
Pause and resume behavior
Backpressure state reporting
Out of Scope
GPU resource control
Network download throttling
Permanent storage quotas
Distributed scheduling
Dependencies
P2-06
P2-11
Acceptance Criteria
New extraction claims pause when configured limits are reached.
Running extraction jobs continue.
Claims resume after recovery thresholds are met.
Hysteresis prevents rapid toggling.
Resource-check failures default to safe behavior.
Tests
Concurrency tests
Storage-threshold tests
Queue-limit tests
Hysteresis tests
Validation
rtk cargo test -p backend
P2-14 — Audio Asset API
Goal

Expose Audio Asset metadata and processing state.

In Scope
Project Audio Asset listing
Audio Asset detail
Pagination
Status filtering
Media Asset relationship
Technical metadata
Processing and cleanup status
Out of Scope
File download endpoint
Audio streaming
Editing metadata
Deduplication data
Classification data
Dependencies
P2-10
Acceptance Criteria
Audio Assets can be listed by project.
Detail responses expose technical metadata and lineage.
Pagination is deterministic.
Project isolation is enforced.
Missing assets return deterministic errors.
Tests
API integration tests
Filtering tests
Pagination tests
Project-isolation tests
Validation
rtk cargo test -p backend
P2-15 — Audio Processing Dashboard
Goal

Display audio-processing status in the React dashboard.

In Scope
Audio Asset list
Processing-state indicators
Source and Media Asset lineage
Technical metadata
Failure display
Polling updates
Empty and loading states
Out of Scope
Audio player
Manual retry controls
Waveform
Dataset category statistics
AI results
Dependencies
P2-14
Acceptance Criteria
Users can view Audio Assets by project.
Processing and cleanup states update through polling.
Failures are visible.
Empty, loading, and API-error states are handled.
Frontend tests cover primary flows.
Tests
Component tests
API interaction tests
Polling tests
Error-state tests
Validation
cd packages/frontend
rtk pnpm test
rtk pnpm build
P2-16 — Pipeline Metrics
Goal

Measure processing time and local resource usage per stage.

In Scope
Stage Run persistence for Phase 2
Queue wait time
Wall-clock duration
Input and output bytes
Input media duration
Processing throughput
CPU and memory measurements when locally available
Dashboard summary endpoint
Out of Scope
GPU metrics
Cloud monitoring
Historical analytics UI
Business category duration
Dependencies
P2-06
P2-10
Acceptance Criteria
Extraction, validation, finalization, and cleanup runs are recorded.
Failed runs retain timing and failure state.
Throughput can be calculated as audio duration per wall-clock time.
Missing optional system metrics do not fail processing.
Repeated jobs create distinct Stage Runs without duplicating Audio Assets.
Tests
Stage Run repository tests
Timing calculation tests
Failure-recording tests
Metrics API tests
Validation
rtk cargo test -p backend
P2-17 — Phase 2 End-to-End Tests
Goal

Prove the complete media-to-audio lifecycle.

In Scope
Local fixture video
Media inspection
Audio extraction
Normalization
Validation
Permanent finalization
Audio Asset creation
Temporary-video deletion
Restart and retry behavior
Dashboard/API-visible state
Out of Scope
Internet downloads
Deduplication
AI analysis
Audio quality scoring
Browser end-to-end automation
Dependencies
P2-11
P2-12
P2-13
P2-14
P2-15
P2-16
Acceptance Criteria
A fixture Media Asset produces one permanent Audio Asset.
Temporary video is deleted only after successful completion.
Failed extraction preserves retryable state.
Repeated execution creates no duplicate Audio Asset.
Recovery resumes an interrupted operation.
Full Phase 2 validation passes.
Tests
Backend end-to-end integration tests
Frontend component tests
Restart and idempotency tests
Final Validation
rtk cargo fmt --check
rtk cargo clippy -p backend --all-targets -- -D warnings
rtk cargo test --workspace
rtk pytest
cd packages/frontend && rtk pnpm test
cd packages/frontend && rtk pnpm build
Phase Completion

Phase 2 is complete when:

P2-01 through P2-17 are complete.
All final validation commands pass.
Temporary videos are deleted only after validated audio finalization.
Permanent Audio Assets remain restart-safe and idempotent.
No Phase 3 deduplication functionality has been introduced.