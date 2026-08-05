alter table if exists sources
    drop constraint if exists sources_ingestion_status_check,
    drop constraint if exists sources_source_type_check,
    drop column if exists byte_count,
    drop column if exists file_count,
    drop column if exists failure_reason,
    drop column if exists source_config;
