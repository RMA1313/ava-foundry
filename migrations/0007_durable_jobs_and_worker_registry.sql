alter table if exists jobs
    add column if not exists parent_job_id uuid references jobs(id) on delete set null,
    add column if not exists batch_id uuid,
    add column if not exists max_attempts integer not null default 3,
    add column if not exists retry_count integer not null default 0,
    add column if not exists progress_current bigint not null default 0,
    add column if not exists progress_total bigint not null default 0,
    add column if not exists result_payload jsonb,
    add column if not exists failure_payload jsonb,
    add column if not exists cancel_requested_at timestamptz,
    add column if not exists cancelled_at timestamptz;

update jobs
set max_attempts = coalesce(max_attempts, 3),
    retry_count = coalesce(retry_count, 0),
    progress_current = coalesce(progress_current, 0),
    progress_total = coalesce(progress_total, 0);

create index if not exists jobs_parent_job_idx
    on jobs (parent_job_id, created_at asc, id asc);

create index if not exists jobs_batch_idx
    on jobs (batch_id, created_at asc, id asc);

create table if not exists worker_registry (
    id uuid primary key,
    worker_id text not null unique,
    version text not null,
    capabilities text[] not null default '{}',
    active_lease_count integer not null default 0,
    last_heartbeat_at timestamptz not null default now(),
    expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists worker_registry_expires_idx
    on worker_registry (expires_at asc, last_heartbeat_at desc, worker_id asc);
