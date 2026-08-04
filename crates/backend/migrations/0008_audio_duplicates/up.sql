create table if not exists duplicate_groups (
    id uuid primary key,
    project_id uuid not null references projects(id) on delete restrict,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists duplicate_group_memberships (
    duplicate_group_id uuid not null references duplicate_groups(id) on delete cascade,
    audio_asset_id uuid not null references audio_assets(id) on delete restrict,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (audio_asset_id)
);

create table if not exists audio_duplicate_decisions (
    id uuid primary key,
    audio_asset_low_id uuid not null references audio_assets(id) on delete restrict,
    audio_asset_high_id uuid not null references audio_assets(id) on delete restrict,
    candidate_fingerprint_low_id uuid not null references audio_fingerprints(id) on delete restrict,
    candidate_fingerprint_high_id uuid not null references audio_fingerprints(id) on delete restrict,
    algorithm text not null,
    algorithm_version text not null,
    duration_delta_seconds double precision not null,
    similarity_score double precision not null,
    decision text not null,
    confidence text not null,
    duplicate_group_id uuid references duplicate_groups(id) on delete set null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (audio_asset_low_id, audio_asset_high_id, algorithm, algorithm_version)
);

create index if not exists duplicate_group_memberships_group_idx
    on duplicate_group_memberships (duplicate_group_id, audio_asset_id asc);

create index if not exists audio_duplicate_decisions_lookup_idx
    on audio_duplicate_decisions (algorithm, algorithm_version, decision, similarity_score desc, created_at asc, id asc);
