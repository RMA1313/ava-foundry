# Phase 3 — Audio Deduplication

## Goal

Identify duplicate audio assets, group equivalent recordings, and ensure that downstream AI processing operates only on canonical audio.

## Phase Constraints

- Audio Assets are immutable.
- Duplicate detection never modifies audio content.
- Duplicate decisions are deterministic.
- Every operation is restart-safe and idempotent.
- Canonical Audio is selected exactly once per duplicate group.
- Phase 3 excludes language detection, classification, transcription, segmentation, quality scoring, and AI inference.

---

## P3-01 — Audio Fingerprint Model

### Goal

Persist fingerprints and their processing state.

### Acceptance Criteria

- Each Audio Fingerprint belongs to exactly one Audio Asset.
- Fingerprint algorithm, version, status, duration, storage path, and timestamps are persisted.
- Duplicate fingerprints for the same Audio Asset, algorithm, and version are rejected.

---

## P3-02 — Fingerprint Storage

### Goal

Store fingerprint files and metadata.

### Acceptance Criteria

- Fingerprint storage paths are deterministic and local.
- Storage paths remain within the configured audio roots.
- Fingerprint finalization is atomic.

---

## P3-03 — Fingerprint Worker

### Goal

Generate fingerprints for finalized Audio Assets.

---

## P3-04 — Fingerprint Index

### Goal

Maintain the searchable fingerprint index.

---

## P3-05 — Duplicate Search

### Goal

Find nearest fingerprint candidates.

Duplicate search only considers completed fingerprints from the indexed search set and narrows candidates by deterministic metadata filters before scoring.

---

## P3-06 — Similarity Evaluation

### Goal

Compute similarity scores and determine duplicate confidence.

Similarity scoring is deterministic and based on persisted fingerprint metadata.

---

## P3-07 — Duplicate Decision

### Goal

Persist deterministic duplicate decisions.

Duplicate decisions are idempotent and survive retries without changing prior outcomes.

---

## P3-08 — Duplicate Groups

### Goal

Maintain duplicate groups and their membership.

Duplicate group membership is created and merged idempotently without selecting a canonical asset yet.

---

## P3-09 — Canonical Audio Selection

### Goal

Select one canonical Audio Asset for each duplicate group.

Canonical selection is persisted on the duplicate group itself and is chosen deterministically from the sorted group members.

---

## P3-10 — Recovery

### Goal

Recover interrupted deduplication safely.

Recovery replays missing canonical selection from persisted duplicate-group membership without changing existing canonical assignments.

---

## P3-11 — Duplicate API

### Goal

Expose duplicate groups and canonical relationships.

The API returns duplicate groups, members, decisions, and canonical Audio Asset relationships with project-scoped, deterministic ordering.

---

## P3-12 — Duplicate Dashboard

### Goal

Display duplicate processing and review information.

The dashboard renders duplicate-group loading, empty, error, and polling states alongside canonical membership details.

---

## P3-13 — Deduplication Metrics

### Goal

Measure deduplication performance and effectiveness.

### Acceptance Criteria

- The backend exposes project-scoped deduplication metrics alongside stage runs.
- Metrics include throughput, candidate counts, match rate, duplicate ratio, unique duration, and processing failures.
- Unique duration excludes duplicate-only audio duration from project totals.
- Benchmark coverage exists for core pipeline stage persistence paths, with regression checks where practical.

---

## P3-14 — End-to-End Tests

### Goal

Validate the complete deduplication pipeline.

### Acceptance Criteria

- Phase 3 is covered by local-fixture end-to-end tests.
- Tests verify fingerprint generation, indexing, candidate search, similarity evaluation, duplicate decisions, grouping, canonical selection, recovery, API responses, and dashboard-visible state.
- Tests verify repeated execution and restart recovery are idempotent.

---

## Phase Completion

Phase 3 is complete when:

- P3-01 through P3-14 are complete.
- Every Audio Asset is fingerprinted exactly once.
- Duplicate detection is deterministic.
- Canonical Audio is assigned exactly once.
- Duplicate groups are stable across restarts.
- All validation commands pass.
- No Phase 4 functionality has been introduced.
