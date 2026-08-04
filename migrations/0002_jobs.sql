create table if not exists jobs (
    id uuid primary key,
    project_id uuid not null references projects(id) on delete restrict,
    source_id uuid,
    job_type text not null,
    status text not null,
    priority integer not null,
    attempts integer not null default 0,
    available_at timestamptz not null,
    lease_expires_at timestamptz,
    worker_id text,
    failure_reason text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (project_id, source_id, job_type)
);

create index if not exists jobs_queue_idx
    on jobs (status asc, priority desc, available_at asc, created_at asc, id asc);

create index if not exists jobs_project_idx
    on jobs (project_id, created_at asc, id asc);
