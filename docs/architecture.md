# Architecture

## Overview

The system is a local-first, asynchronous pipeline for building large-scale Persian audio datasets.

Its primary responsibility is to ingest media, convert it into normalized audio, remove supported forms of duplication, classify audio content, extract target intervals, and export curated datasets.

The architecture is optimized for continuous processing, restart safety, storage efficiency, and high throughput on a single workstation.

---

# Architectural Principles

The system follows these principles:

- Local-first operation
- Pipeline-based processing
- Asynchronous execution
- Fault tolerance
- Idempotent processing
- Incremental data processing
- Storage efficiency
- Horizontal worker scalability on a single machine
- Clear separation between orchestration and AI inference

---

# High-Level Architecture

```text
                        React Dashboard
                               │
                               ▼
                         Rust Backend API
                               │
                               ▼
                 PostgreSQL + Durable Job Queue
                               │
      ┌──────────────┬──────────┴──────────┬──────────────┐
      ▼              ▼                     ▼              ▼
 Download      Media Processing      Deduplication   AI Analysis
 Workers           Workers             Workers         Worker
      │              │                     │              │
      └──────────────┴──────────┬──────────┴──────────────┘
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

# System Components

## Rust Backend

The Rust backend is the orchestration layer of the entire system.

Responsibilities:

- Project management
- Source registration
- Job scheduling
- Worker coordination
- Pipeline orchestration
- Progress tracking
- Dashboard aggregation
- Dataset export
- System configuration

The backend owns all pipeline state.

---

## PostgreSQL

PostgreSQL is the only persistent storage layer.

Responsibilities:

- Application data
- Metadata
- Job queue
- Pipeline state
- Processing history
- Dashboard statistics

The database is the single source of truth.

---

## Download Workers

Download workers retrieve media from supported sources.

Responsibilities:

- Download media
- Resume interrupted downloads
- Retry failed downloads
- Report download progress

Downloaded videos are stored temporarily.

---

## Media Processing Workers

Media processing workers transform media into permanent audio assets.

Responsibilities:

- Media inspection
- Audio extraction
- Audio normalization
- Metadata extraction
- Audio validation
- Hash generation
- Temporary file cleanup

Video files are deleted immediately after successful audio validation.

Audio becomes the permanent asset.

---

## Deduplication Workers

Deduplication workers detect supported duplicate types.

Responsibilities:

- Source identity lookup
- Exact hash comparison
- Audio fingerprint comparison
- Duplicate grouping

Only supported duplicate types are detected.

---

## AI Analysis Worker

AI inference is isolated from the orchestration layer.

Responsibilities:

- Language detection
- Audio categorization
- Environment classification
- Audio event detection

The AI worker never owns pipeline state.

---

## Target Interval Mining

This stage analyzes long audio assets and extracts target intervals.

Responsibilities:

- Window-based analysis
- Interval generation
- Interval merging
- Parent-child relationship preservation

Extracted intervals remain linked to the original audio asset.

---

## Manual Review

Manual review validates automatically generated results.

Responsibilities:

- Review extracted intervals
- Correct categories
- Approve or reject assets

Human review is performed only when required.

---

## Dataset Export

The export layer generates portable datasets.

Responsibilities:

- Metadata generation
- Manifest generation
- Dataset packaging
- Standard format export

---

# Processing Model

Every media asset progresses independently through the pipeline.

Multiple pipeline stages execute concurrently.

The system never waits for all assets to complete one stage before starting the next.

Each stage processes available work immediately.

---

# Pipeline Ownership

Each processing stage owns only its own responsibility.

Stages communicate exclusively through persisted pipeline state.

No stage directly controls another stage.

---

# Storage Architecture

The system maintains two storage classes.

## Temporary Storage

Contains:

- Downloaded video files
- Temporary processing files

Temporary storage is automatically cleaned after successful processing.

---

## Permanent Storage

Contains:

- Normalized audio
- Extracted intervals
- Exported datasets

Only permanent assets contribute to the dataset.

---

# AI Boundary

AI models are not part of the orchestration layer.

Rust is responsible for:

- Scheduling
- State management
- Coordination
- Persistence

Python is responsible only for inference.

This separation allows AI models to evolve independently from the pipeline.

---

# Dashboard Architecture

The dashboard is a read-only operational view of the system.

It visualizes:

- Pipeline state
- Processing progress
- Worker activity
- Storage usage
- Processing throughput
- Dataset statistics

The primary business metric is collected audio duration grouped by accounting category.

## Observability

Operational logging is structured and correlation-friendly.

Requirements:

- Backend and worker logs include request, project, source, job, and stage correlation identifiers when available.
- Log output can be rendered as human-readable console text or JSON for machine ingestion.
- Pipeline stages record execution timing and outcome details in structured logs.
- Benchmark runs capture stage latency, throughput, CPU, memory, and storage metrics where available.

---

# Fault Tolerance

The system is designed to recover safely from failures.

Requirements:

- Every processing stage is restart-safe.
- Every job is idempotent.
- Interrupted processing can resume.
- Pipeline state is always recoverable from PostgreSQL.

Unexpected termination must never corrupt permanent assets.

---

# Scalability

The architecture is designed to scale by increasing worker concurrency.

Independent worker pools allow multiple assets to be processed simultaneously.

Each processing stage can scale independently without changing the overall architecture.

---

# Architectural Boundaries

The architecture intentionally excludes:

- Cloud services
- Distributed clusters
- Kubernetes
- External message brokers
- Cloud object storage
- Cloud AI inference
- Multi-machine orchestration

The system is designed to operate entirely on a single local workstation.
