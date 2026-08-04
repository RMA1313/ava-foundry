# Phase 4 — Dataset Workspace

## Goal

Provide a complete user workspace for managing projects, sources, jobs, media, audio assets, duplicate groups, and pipeline execution. i need fully RTL and persian Frontend and very biutiful and highly mature UI/UX.

## Phase Constraints

- Backend capabilities from Phases 1–3 are reused.
- No AI inference is introduced.
- No duplicate detection logic is modified.
- The workspace remains local-first.
- Every operation is restart-safe.
- Phase 4 excludes language detection, classification, transcription, segmentation, quality scoring, and dataset export.

---

## P4-01 — Project Workspace

### Goal

Create the primary project workspace.

---

## P4-02 — Source Management

### Goal

Manage ingestion sources.

---

## P4-03 — File Upload

### Goal

Upload local media and archives.

---

## P4-04 — Dataset Import

### Goal

Import supported dataset manifests.

---

## P4-05 — URL Import

### Goal

Create ingestion sources from supported URLs.

---

## P4-06 — Job Monitor

### Goal

Monitor pipeline jobs and execution state.

---

## P4-07 — Media Browser

### Goal

Browse imported media assets.

---

## P4-08 — Audio Browser

### Goal

Browse finalized audio assets.

---

## P4-09 — Duplicate Browser

### Goal

Browse duplicate groups and canonical audio.

### Status

Implemented in the workspace with duplicate-group cards, members, canonical audio, similarity, and load/empty/error states.

---

## P4-10 — Search & Filters

### Goal

Provide unified filtering and search across workspace resources.

### Status

Implemented with project-scoped, URL-backed search and filters across sources, jobs, media, audio, and duplicate groups.

---

## P4-11 — Project Statistics

### Goal

Display project progress and storage statistics.

### Status

Implemented with source counts, job states, media/audio counts, duplicate ratio, unique duration, storage usage, and throughput.

---

## P4-12 — Pipeline Controls

### Goal

Control pipeline execution and failed-job recovery.

### Status

Implemented with confirmed retry, recovery, pause, and resume controls for supported backend actions.

---

## P4-13 — Workspace Settings

### Goal

Manage project-level processing settings.

### Status

Implemented with project metadata editing, validation, success and error feedback, and project-scoped pipeline state visibility using only supported backend contracts.

---

## P4-14 — End-to-End Tests

### Goal

Validate the complete user workflow.

### Status

Implemented with local-fixture frontend coverage for project selection, URL import, dataset import, job monitoring, media/audio browsing, duplicate browsing, search and filters, statistics, pipeline controls, and settings.

---

## Phase Completion

Phase 4 is complete when:

- P4-01 through P4-14 are complete.
- Users can create projects, import data, monitor processing, browse media, audio, and duplicate groups.
- Users can search, filter, and manage pipeline execution.
- All validation commands pass.
- No Phase 5 AI functionality has been introduced.
- Phase 4 UI polish is complete and applied consistently across the workspace.
