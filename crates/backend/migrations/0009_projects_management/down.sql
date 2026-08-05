drop index if exists projects_name_unique_idx;
alter table if exists projects drop column if exists status;
