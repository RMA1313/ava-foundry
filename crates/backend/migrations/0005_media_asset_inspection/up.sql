alter table media_assets
    add column if not exists inspection_status text,
    add column if not exists container_format text,
    add column if not exists duration_seconds double precision,
    add column if not exists codecs text,
    add column if not exists sample_rate_hz integer,
    add column if not exists channels integer,
    add column if not exists inspected_at timestamptz;
