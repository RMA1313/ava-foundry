# Frontend Backlog --- Data Studio

## Goal

Complete the React frontend into a production-ready interface for
managing projects, ingestion, processing, monitoring, and dataset
review.

## Global Constraints

-   React is responsible only for presentation, interaction, and
    client-side state.
-   Business rules remain in the backend.
-   All server communication must use typed API clients.
-   All pages must support loading, empty, error, and success states.
-   Every list must use the shared pagination, filtering, sorting, and
    search components.
-   All UI must be responsive, accessible, and
    internationalization-ready.
-   All real-time updates must use Server-Sent Events.
-   No mock data in production code.
-   No duplicated components or business logic.

------------------------------------------------------------------------

## F-01 --- Application Shell

### Goal

Implement the global application layout.

### In Scope

-   Layout
-   Navigation
-   Sidebar
-   Header
-   Breadcrumb
-   Theme
-   Responsive layout
-   Error boundary
-   Global dialogs
-   Toast system

### Out of Scope

-   Feature pages
-   Authentication

### Acceptance Criteria

-   Layout is shared across all pages.
-   Navigation is responsive.
-   Global dialogs and notifications work everywhere.

### Tests

-   Component tests
-   Responsive tests

### Validation

``` bash
cd packages/frontend
pnpm test
pnpm build
```

------------------------------------------------------------------------

## F-02 --- Dashboard

### Goal

Provide the main operational dashboard.

### In Scope

-   Statistics cards
-   Recent jobs
-   Recent sources
-   Storage usage
-   Processing overview
-   Refresh
-   Loading, empty, and error states

### Acceptance Criteria

-   Dashboard reflects backend data.
-   Components refresh through SSE.

------------------------------------------------------------------------

## F-03 --- Project Management

### Goal

Complete project management UI.

### In Scope

-   Project list
-   Detail
-   Create
-   Edit
-   Delete
-   Search
-   Filters
-   Sorting
-   Pagination
-   Statistics

### Acceptance Criteria

-   CRUD works end-to-end.
-   Search, filters, sorting, and pagination use shared components.

------------------------------------------------------------------------

## F-04 --- Source Management

### Goal

Manage ingestion sources.

### In Scope

-   Source list
-   Detail
-   Retry
-   Cancel
-   Delete
-   Search
-   Filters
-   Pagination
-   Statistics

### Acceptance Criteria

-   Source lifecycle is fully manageable.

------------------------------------------------------------------------

## F-05 --- Upload Experience

### Goal

Provide production-ready upload workflows.

### In Scope

-   Drag & drop
-   Multi-file upload
-   Folder upload
-   Archive upload
-   Dataset upload
-   URL import
-   Progress
-   Queue
-   Retry
-   Cancel

### Acceptance Criteria

-   Uploads are resilient and user feedback is continuous.

------------------------------------------------------------------------

## F-06 --- Job Monitoring

### Goal

Monitor backend processing.

### In Scope

-   Job list
-   Detail
-   Progress
-   Timeline
-   Logs
-   Retry
-   Cancel
-   Search
-   Filters

### Acceptance Criteria

-   Job state updates live through SSE.

------------------------------------------------------------------------

## F-07 --- Media Browser

### Goal

Browse imported media.

### In Scope

-   Media list
-   Detail
-   Preview
-   Metadata
-   Search
-   Filters
-   Pagination
-   Bulk actions

### Acceptance Criteria

-   Media metadata is fully browsable.

------------------------------------------------------------------------

## F-08 --- Audio Browser

### Goal

Browse processed audio.

### In Scope

-   Audio list
-   Detail
-   Metadata
-   Player
-   Download
-   Filters
-   Pagination

### Acceptance Criteria

-   Audio playback and metadata are available.

------------------------------------------------------------------------

## F-09 --- Duplicate Review

### Goal

Review duplicate groups.

### In Scope

-   Duplicate list
-   Detail
-   Compare
-   Confirm
-   Reject
-   Merge
-   Filters
-   Statistics

### Acceptance Criteria

-   Duplicate review actions update backend successfully.

------------------------------------------------------------------------

## F-10 --- Dataset Browser

### Goal

Browse project datasets.

### In Scope

-   Dataset list
-   Detail
-   Filters
-   Search
-   Statistics
-   Export

------------------------------------------------------------------------

## F-11 --- Real-Time Updates

### Goal

Synchronize UI with backend events.

### In Scope

-   SSE client
-   Auto reconnect
-   Event replay
-   Live dashboard
-   Live jobs
-   Live sources
-   Live notifications

### Acceptance Criteria

-   UI updates without manual refresh.

------------------------------------------------------------------------

## F-12 --- Shared Components

### Goal

Build reusable UI primitives.

### In Scope

-   Data table
-   Search
-   Filters
-   Pagination
-   Status badge
-   Progress bar
-   Statistics card
-   Dialogs
-   Skeletons
-   Empty states

------------------------------------------------------------------------

## F-13 --- API Layer

### Goal

Standardize frontend/backend communication.

### In Scope

-   Typed API client
-   Query hooks
-   Mutation hooks
-   Error mapping
-   Retry policy
-   SSE client

------------------------------------------------------------------------

## F-14 --- State Management

### Goal

Centralize client state.

### In Scope

-   Project store
-   Source store
-   Job store
-   Media store
-   Audio store
-   Statistics store
-   UI store

------------------------------------------------------------------------

## F-15 --- Performance

### Goal

Optimize rendering.

### In Scope

-   Virtual lists
-   Lazy loading
-   Memoization
-   Request deduplication
-   Infinite scrolling

------------------------------------------------------------------------

## F-16 --- Accessibility

### Goal

Meet accessibility requirements.

### In Scope

-   Keyboard navigation
-   Focus management
-   ARIA
-   Screen reader support
-   Color contrast

------------------------------------------------------------------------

## F-17 --- Internationalization

### Goal

Support multilingual UI.

### In Scope

-   Complete i18n
-   RTL
-   Localized dates
-   Localized numbers
-   Locale switching

------------------------------------------------------------------------

## F-18 --- Settings

### Goal

Provide system configuration pages.

### In Scope

-   General
-   Storage
-   Processing
-   Workers
-   Import
-   System information

------------------------------------------------------------------------

## F-19 --- Frontend Testing

### Goal

Verify all UI behavior.

### In Scope

-   Component tests
-   Hook tests
-   Integration tests
-   Accessibility tests

### Validation

``` bash
cd packages/frontend
pnpm test
pnpm build
```

------------------------------------------------------------------------

## F-20 --- End-to-End Frontend Validation

### Goal

Validate complete user workflows.

### In Scope

-   Create project
-   Upload source
-   Monitor job
-   Browse media
-   Browse audio
-   Review duplicates
-   Dashboard updates
-   SSE recovery

### Acceptance Criteria

-   All primary workflows complete successfully.
-   No manual refresh required.

### Final Validation

``` bash
cd packages/frontend
pnpm test
pnpm build
```

------------------------------------------------------------------------

## Frontend Completion

The frontend backlog is complete when:

-   F-01 through F-20 are complete.
-   All pages use shared components.
-   All backend APIs are integrated.
-   Real-time updates use SSE.
-   All primary workflows function end-to-end.
-   All frontend tests pass.
-   Production build succeeds.
