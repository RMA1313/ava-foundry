alter table media_assets
    drop column if exists inspected_at,
    drop column if exists channels,
    drop column if exists sample_rate_hz,
    drop column if exists codecs,
    drop column if exists duration_seconds,
    drop column if exists container_format,
    drop column if exists inspection_status;
