# Domain Model

## Overview

The domain model defines the core business entities, their responsibilities, and their relationships.

It represents the permanent concepts of the system independently of implementation details.

---

# Design Principles

The domain model follows these principles:

- Every entity has a single responsibility.
- Entities have stable identities.
- Relationships are explicit.
- Processing state is persisted.
- Audio is the primary business asset.
- Video exists only as a temporary processing artifact.

---

# Core Entities

## Project

A logical workspace that groups all processing activities.

Responsibilities:

- Own sources
- Own assets
- Own jobs
- Own exports
- Own dashboard statistics

A project is the top-level ownership boundary.

---

## Source

Represents a discovered origin of media.

Examples:

- Local file
- Local folder
- Public URL
- CSV import
- Existing dataset

Responsibilities:

- Preserve source identity
- Record import information
- Track ingestion status

A source exists independently of whether media processing succeeds.

---

## Media Asset

Represents an imported media object.

A media asset is the original input accepted by the system.

Examples:

- Video file
- Audio file

Responsibilities:

- Preserve source relationship
- Store media metadata
- Track processing lifecycle

Video media is temporary.

Audio media becomes permanent after successful extraction.

---

## Audio Asset

Represents the permanent normalized audio stored by the system.

Responsibilities:

- Store normalized audio
- Preserve media lineage
- Participate in duplicate detection
- Participate in AI analysis
- Participate in dataset export

Audio assets are the primary business objects.

Technical metadata is preserved on each audio asset, including duration, container format,
codec, sample rate, channel count, byte size, and local storage path.

---

## Audio Interval

Represents a target interval extracted from an audio asset.

Responsibilities:

- Preserve parent audio relationship
- Store interval boundaries
- Store classification results
- Participate in dataset export

Intervals never exist without a parent audio asset.

---

## Classification

Represents AI-generated labels attached to an audio asset or interval.

Responsibilities:

- Store predicted categories
- Store confidence values
- Preserve prediction history

Classification results are independent from review decisions.

---

## Fingerprint

Represents identifiers used for duplicate detection.

Responsibilities:

- Exact content identification
- Audio fingerprint identification
- Duplicate lookup

Fingerprints do not own business state.

---

## Duplicate Group

Represents a set of assets determined to contain the same supported audio content.

Responsibilities:

- Group duplicate assets
- Preserve canonical asset relationship
- Prevent duplicate duration accounting

Duplicate groups do not modify the underlying assets.

---

## Job

Represents a unit of pipeline work.

Responsibilities:

- Represent work ownership
- Track execution state
- Track retries
- Track failures

Every processing stage operates through jobs.

---

## Stage Run

Represents a single execution of one pipeline stage.

Responsibilities:

- Record execution history
- Record timing information
- Record execution outcome

Stage runs provide operational observability.

---

## Manual Review

Represents a human validation decision.

Responsibilities:

- Approve assets
- Reject assets
- Correct classifications

Manual review does not modify the original AI predictions.

---

## Export

Represents a generated dataset package.

Responsibilities:

- Preserve export metadata
- Preserve exported asset set
- Preserve export format

Exports are immutable snapshots.

---

# Entity Relationships

```text
Project
│
├── Sources
│     │
│     └── Media Assets
│              │
│              └── Audio Assets
│                      │
│                      ├── Audio Intervals
│                      ├── Classifications
│                      ├── Fingerprints
│                      ├── Duplicate Groups
│                      ├── Manual Reviews
│                      └── Exports
│
└── Jobs
      │
      └── Stage Runs
```

---

# Pipeline State Model

Every processing job progresses through explicit states.

```text
Discovered
↓
Queued
↓
Running
↓
Completed
```

Alternative terminal states:

```text
Failed
Cancelled
```

State transitions are persisted.

---

# Media Lifecycle

Media progresses through the following lifecycle.

```text
Source
↓
Media Asset
↓
Temporary Video (optional)
↓
Normalized Audio Asset
↓
Audio Interval (optional)
↓
Dataset Export
```

Video is removed after successful audio extraction.

Audio remains the permanent asset.

---

# Ownership Rules

Ownership follows a strict hierarchy.

```text
Project
owns Sources

Source
owns Media Assets

Media Asset
owns Audio Asset

Audio Asset
owns Audio Intervals

Project
owns Jobs

Job
owns Stage Runs
```

Ownership is never shared.

---

# Identity Rules

Each entity has a stable identifier.

Identifiers never change during the lifetime of an entity.

Relationships are maintained using entity identifiers.

Business logic never depends on file paths.

---

# Classification Model

Classification is multi-label.

Every audio asset or interval contains:

- One accounting category
- Zero or more secondary labels

The accounting category determines dashboard duration aggregation.

Secondary labels provide additional search and filtering capabilities.

---

# Duplicate Model

Duplicate detection never removes business entities.

Instead:

- Assets remain preserved.
- Duplicate relationships are recorded.
- A canonical asset is identified.
- Duplicate duration is excluded from business statistics.

---

# Operational Model

Pipeline execution is fully asynchronous.

Jobs are independent.

Assets progress independently.

Multiple pipeline stages execute concurrently.

No entity depends on global pipeline completion.

---

# Persistence Rules

The following entities are persistent:

- Project
- Source
- Media Asset
- Audio Asset
- Audio Interval
- Classification
- Fingerprint
- Duplicate Group
- Job
- Stage Run
- Manual Review
- Export

Temporary processing artifacts are not part of the domain model.
