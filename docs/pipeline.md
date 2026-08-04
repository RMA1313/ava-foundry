# Pipeline

## Overview

The pipeline defines how media moves through the system from ingestion to dataset export.

Processing is asynchronous, incremental, and stage-based.

Every asset progresses independently through the pipeline.

---

# Pipeline Principles

The pipeline follows these principles:

- Asynchronous execution
- Independent asset processing
- Incremental progress
- Restart-safe execution
- Idempotent stages
- Persistent pipeline state
- Temporary video storage
- Permanent audio storage

Pipeline stages never depend on the completion of other assets.

---

# Processing Flow

```text
Source Registration
        │
        ▼
Media Ingestion
        │
        ▼
Media Download / Import
        │
        ▼
Audio Extraction
        │
        ▼
Audio Validation
        │
        ▼
Video Cleanup
        │
        ▼
Duplicate Detection
        │
        ▼
AI Analysis
        │
        ▼
Target Interval Mining
        │
        ▼
Manual Review
        │
        ▼
Dataset Export
```

---

# Pipeline Stages

## Source Registration

Creates a source record for every imported item.

Responsibilities:

- Register source identity
- Validate source information
- Create processing jobs

Output:

- Source

---

## Media Ingestion

Obtains the original media.

Responsibilities:

- Download media
- Import local files
- Import existing datasets

Output:

- Media Asset

---

## Audio Extraction

Converts media into normalized audio.

Responsibilities:

- Extract audio
- Normalize audio
- Generate media metadata

Output:

- Audio Asset

---

## Audio Validation

Verifies that the extracted audio is usable.

Responsibilities:

- Verify audio integrity
- Verify extraction success
- Finalize permanent audio

Only validated audio becomes a permanent asset.

---

## Video Cleanup

Removes temporary media.

Responsibilities:

- Delete temporary video
- Delete temporary processing files

Video is never retained after successful validation.

---

## Duplicate Detection

Detects supported duplicate types.

Responsibilities:

- Source identity lookup
- Exact duplicate detection
- Audio fingerprint detection
- Duplicate grouping

Duplicate relationships are recorded without removing assets.

---

## AI Analysis

Generates automatic classifications.

Responsibilities:

- Language detection
- Audio categorization
- Environment classification
- Audio event detection

Results are stored independently from manual review.

---

## Target Interval Mining

Extracts relevant intervals from long audio assets.

Responsibilities:

- Analyze audio windows
- Generate intervals
- Merge adjacent intervals
- Preserve parent relationships

Intervals remain linked to the original audio asset.

---

## Manual Review

Allows human validation of generated results.

Responsibilities:

- Approve intervals
- Reject intervals
- Correct classifications

Manual review is optional and does not modify pipeline history.

---

## Dataset Export

Produces curated datasets.

Responsibilities:

- Collect approved assets
- Generate manifests
- Produce export formats

Exports are immutable snapshots.

---

# Pipeline State

Every job transitions through explicit states.

```text
Queued
↓
Running
↓
Completed
```

Failure states:

```text
Failed
Cancelled
```

State transitions are persisted.

---

# Stage Independence

Each pipeline stage:

- Has a single responsibility
- Reads persisted state
- Produces persisted state
- Does not directly invoke other stages

Stages communicate only through persisted data.

---

# Concurrency Model

Multiple assets are processed simultaneously.

Different pipeline stages execute concurrently.

Example:

```text
Asset A → AI Analysis
Asset B → Duplicate Detection
Asset C → Audio Extraction
Asset D → Download
```

The pipeline continuously processes available work.

---

# Scheduling Model

Work is scheduled per stage.

A stage begins processing as soon as its required input becomes available.

Pipeline execution is driven by available jobs rather than global synchronization.

---

# Retry Model

Each stage executes independently.

A failed stage:

- Does not affect completed stages.
- Does not restart unrelated stages.
- Can be executed again safely.

Retry behavior relies on persisted pipeline state.

---

# Idempotency

Every stage must be idempotent.

Executing the same stage multiple times for the same asset must produce the same logical result.

Repeated execution must never duplicate:

- Audio assets
- Intervals
- Classifications
- Duplicate groups
- Dashboard duration

---

# Failure Recovery

Pipeline recovery begins from the last persisted state.

Recovery never requires restarting the entire pipeline.

Previously completed stages are never repeated unless explicitly requested.

---

# Storage Flow

```text
Temporary Video
        │
        ▼
Normalized Audio
        │
        ▼
Extracted Intervals
        │
        ▼
Dataset Export
```

Only normalized audio, extracted intervals, and exported datasets are permanent assets.

---

# Duration Accounting

Business statistics are generated only from permanent assets.

Temporary media never contributes to dashboard metrics.

Duplicate duration is excluded from business totals.

Collected duration is aggregated by accounting category.

---

# Processing Boundaries

The pipeline intentionally excludes:

- Speech transcription
- Speaker diarization
- Audio quality scoring
- Noise reduction
- Cloud inference
- Distributed execution
- Multi-machine orchestration

These responsibilities are outside the current pipeline definition.