# Development Roadmap

## Overview

This roadmap defines the incremental development plan for the system.

Each phase delivers a complete, usable improvement to the product.

Every phase builds on the previous one without requiring architectural changes.

---

# Phase 0 — Foundation

## Goal

Establish the project foundation.

## Deliverables

- Repository structure
- Configuration management
- Local development environment
- PostgreSQL integration
- Database migrations
- Logging
- Dashboard shell
- Testing infrastructure

## Completion Criteria

The system can be built, started, tested, and developed locally.

---

# Phase 1 — Ingestion

## Goal

Allow media to enter the system reliably.

## Deliverables

- Project management
- Source registration
- Local file import
- Folder import
- URL import
- Dataset import
- Download queue
- Download workers
- Job management
- Progress tracking

## Completion Criteria

Media can be imported and tracked through the ingestion pipeline.

---

# Phase 2 — Audio Processing

## Goal

Convert imported media into permanent audio assets.

## Deliverables

- Media inspection
- Audio extraction
- Audio normalization
- Metadata extraction
- Audio validation
- Temporary video cleanup
- Permanent audio storage

## Completion Criteria

Every supported media asset produces a validated normalized audio asset.

---

# Phase 3 — Duplicate Detection

## Goal

Prevent duplicate audio from contributing to the dataset.

## Deliverables

- Source duplicate detection
- Exact duplicate detection
- Audio fingerprint detection
- Duplicate grouping
- Canonical asset selection

## Completion Criteria

Supported duplicate types are detected and excluded from duration accounting.

---

# Phase 4 — Operational Dashboard

## Goal

Provide visibility into system operation and dataset growth.

## Deliverables

- Processing statistics
- Queue monitoring
- Worker activity
- Storage usage
- Processing throughput
- Duration statistics
- Category statistics

## Completion Criteria

The dashboard reports the operational state of the pipeline and collected dataset.

---

# Phase 5 — AI Analysis

## Goal

Automatically classify collected audio.

## Deliverables

- Language detection
- Audio categorization
- Environment classification
- Audio event detection
- AI result persistence

## Completion Criteria

Every processed audio asset contains automatic classification results.

---

# Phase 6 — Target Interval Mining

## Goal

Extract useful intervals from long audio assets.

## Deliverables

- Window analysis
- Interval generation
- Interval merging
- Parent interval relationships

## Completion Criteria

Relevant intervals are extracted and linked to their parent audio assets.

---

# Phase 7 — Manual Review

## Goal

Allow human validation of generated results.

## Deliverables

- Audio playback
- Interval review
- Category correction
- Approval workflow
- Rejection workflow

## Completion Criteria

Users can review and validate extracted intervals before export.

---

# Phase 8 — Dataset Export

## Goal

Produce curated datasets.

## Deliverables

- Dataset manifests
- CSV export
- JSONL export
- Parquet export
- Export metadata

## Completion Criteria

Approved assets and intervals can be exported as portable datasets.

---

# Phase Dependencies

```text
Phase 0
    ↓
Phase 1
    ↓
Phase 2
    ↓
Phase 3
    ↓
Phase 4
    ↓
Phase 5
    ↓
Phase 6
    ↓
Phase 7
    ↓
Phase 8
```

Each phase depends only on the successful completion of the previous phase.

---

# Development Principles

All development phases follow these rules:

- Deliver working software.
- Preserve architectural consistency.
- Keep features incremental.
- Maintain restart safety.
- Maintain idempotent processing.
- Preserve backward compatibility within the project.
- Update tests with every completed feature.
- Update documentation when behavior changes.

---

# Definition of Completion

A phase is considered complete when:

- All planned deliverables are implemented.
- Automated tests pass.
- Documentation reflects the implemented behavior.
- The delivered functionality is usable independently.
- No architectural changes are required by subsequent phases.