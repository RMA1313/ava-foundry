create table if not exists audio_assets (
    id uuid primary key,
    media_asset_id uuid not null unique references media_assets(id) on delete restrict,
    project_id uuid not null references projects(id) on delete restrict,
    processing_status text not null,
    normalization_profile text,
    validation_status text,
    validation_error text,
    validated_at timestamptz,
    duration_seconds double precision not null,
    audio_format text not null,
    codec text not null,
    sample_rate_hz integer not null,
    channels integer not null,
    byte_size bigint not null,
    storage_path text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists audio_assets_project_idx
    on audio_assets (project_id, created_at asc, id asc);
