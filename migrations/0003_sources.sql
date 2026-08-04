create table if not exists sources (
    id uuid primary key,
    project_id uuid not null references projects(id) on delete restrict,
    source_type text not null,
    original_url text,
    normalized_identity text not null,
    import_metadata text,
    ingestion_status text not null default 'pending',
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (project_id, normalized_identity)
);

create index if not exists sources_project_idx
    on sources (project_id, created_at asc, id asc);

create table if not exists media_assets (
    id uuid primary key,
    project_id uuid not null references projects(id) on delete restrict,
    source_id uuid not null unique references sources(id) on delete restrict,
    media_kind text not null,
    staged_path text not null,
    original_filename text,
    byte_size bigint not null,
    ingestion_status text not null,
    extraction_status text,
    extraction_progress text,
    extraction_tmp_path text,
    extracted_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists media_assets_project_idx
    on media_assets (project_id, created_at asc, id asc);
