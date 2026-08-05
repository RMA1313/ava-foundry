# Product Scope

## Overview

This project is a local-first platform for collecting, processing,
categorizing and exporting large-scale Persian audio datasets.

The primary goal is to maximize the amount of unique, categorized Persian
audio while minimizing manual effort, storage usage and processing cost.

The system processes media incrementally: newly downloaded files can be
converted, deduplicated and analyzed while other files are still downloading.
Temporary video files are deleted after successful audio extraction and
validation.

# Goals

The system shall:

- Collect audio from multiple public sources.
- Support importing existing datasets.
- Automatically process incoming media.
- Detect duplicate content.
- Categorize audio into predefined domains.
- Extract target audio intervals from long recordings.
- Provide accurate statistics about collected audio.
- Export curated datasets in standard formats.

The entire system must run locally without requiring cloud services.

---

# Non-Goals

The following capabilities are intentionally out of scope.

- Speech-to-text transcription
- Speaker diarization
- Speaker identification
- Age or gender estimation
- Emotion recognition
- Noise removal
- Voice enhancement
- Model training
- Cloud storage
- Cloud inference APIs
- Distributed cluster deployment
- Kubernetes
- Multi-machine orchestration
- Audio quality scoring
- License tracking or rights management
- Partial-content duplicate detection
- Semantic audio similarity detection

These may be added in future versions but are not part of the MVP.

---

# Supported Inputs

The platform accepts:

- Local audio files
- Local video files
- Local folders
- CSV files containing media URLs
- Public media URLs
- Existing audio datasets
- Hugging Face datasets
- Archive formats (zip, tar, tar.gz)

---

# Supported Outputs

The platform produces:

- Normalized audio
- Target audio intervals
- Structured metadata
- Categorized audio assets
- Dataset manifests
- CSV exports
- JSONL exports
- Parquet exports

---

# Core Functionalities

The MVP includes:

- Project management
- Media ingestion
- Download management
- Audio extraction
- Audio normalization
- Duplicate detection
- Language detection
- Audio categorization
- Target interval extraction
- Manual review
- Search and filtering
- Dataset export
- Operational dashboard

---

# Dataset Categories

The primary accounting categories are configurable.

Typical categories include:

- Meetings
- Public environments
- Telephone conversations
- Personal speech
- Acted speech
- Dialects
- Other

Each audio asset or interval belongs to one accounting category while allowing multiple secondary labels.

---

# Dashboard Metrics

The dashboard is a core feature of the product.

The most important metrics are:

- Total collected duration
- Total unique duration
- Total approved duration
- Duration per category
- Number of processed assets
- Duplicate ratio
- Processing throughput
- Queue status
- Storage usage
- GPU utilization
- Pipeline bottlenecks

The current frontend only renders metrics that are directly backed by the backend response shape.
Unsupported operational signals are omitted from the UI rather than approximated with placeholders.

Category durations are considered the primary business KPI.

---

# Design Principles

The system follows these principles.

- Local-first
- Automation-first
- Incremental processing
- Fault tolerant
- Restart safe
- Idempotent processing
- Storage efficient
- Reproducible outputs
- Extensible architecture
- Human review only when necessary

---

# Success Criteria

The MVP is considered complete when it can:

1. Import media from multiple sources.
2. Process media automatically.
3. Detect supported duplicate types and prevent duplicate duration from being
counted in dataset totals.
4. Categorize collected audio.
5. Extract target intervals.
6. Display collection statistics.
7. Export production-ready datasets.
