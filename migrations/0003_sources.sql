create table if not exists sources (
    id uuid primary key,
    project_id uuid not null references projects(id) on delete restrict,
    source_type text not null,
    original_url text,
    normalized_identity text not null,
    import_metadata text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (project_id, normalized_identity)
);

create index if not exists sources_project_idx
    on sources (project_id, created_at asc, id asc);
