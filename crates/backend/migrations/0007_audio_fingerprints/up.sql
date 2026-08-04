create table if not exists audio_fingerprints (
    id uuid primary key,
    audio_asset_id uuid not null references audio_assets(id) on delete restrict,
    algorithm text not null,
    algorithm_version text not null,
    processing_status text not null,
    duration_seconds double precision not null,
    storage_path text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (audio_asset_id, algorithm, algorithm_version)
);

create index if not exists audio_fingerprints_audio_asset_idx
    on audio_fingerprints (audio_asset_id, created_at asc, id asc);
