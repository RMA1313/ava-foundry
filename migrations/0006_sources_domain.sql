alter table if exists sources
    add column if not exists source_config text,
    add column if not exists failure_reason text,
    add column if not exists file_count bigint not null default 0,
    add column if not exists byte_count bigint not null default 0;

do $$
begin
    if not exists (
        select 1
        from pg_constraint
        where conname = 'sources_source_type_check'
    ) then
        alter table sources
            add constraint sources_source_type_check
            check (source_type in (
                'local_file',
                'local_folder',
                'public_url',
                'csv_import',
                'existing_dataset',
                'hugging_face_dataset',
                'archive'
            ));
    end if;
    if not exists (
        select 1
        from pg_constraint
        where conname = 'sources_ingestion_status_check'
    ) then
        alter table sources
            add constraint sources_ingestion_status_check
            check (ingestion_status in ('pending', 'staged', 'completed', 'failed'));
    end if;
end $$;

update sources
set file_count = coalesce(file_count, 0),
    byte_count = coalesce(byte_count, 0),
    ingestion_status = coalesce(ingestion_status, 'pending');
