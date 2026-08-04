create table if not exists stage_runs (
    id uuid primary key,
    project_id uuid not null references projects(id) on delete restrict,
    media_asset_id uuid references media_assets(id) on delete restrict,
    audio_asset_id uuid references audio_assets(id) on delete restrict,
    job_id uuid,
    stage_name text not null,
    status text not null,
    queue_wait_ms bigint not null default 0,
    wall_time_ms bigint not null default 0,
    input_bytes bigint not null default 0,
    output_bytes bigint not null default 0,
    input_duration_seconds double precision not null default 0,
    throughput double precision,
    failure_reason text,
    cpu_percent double precision,
    memory_bytes bigint,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists stage_runs_project_idx
    on stage_runs (project_id, created_at desc, id desc);
