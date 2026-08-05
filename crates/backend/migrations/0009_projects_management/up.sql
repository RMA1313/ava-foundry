alter table if exists projects
    add column if not exists status text not null default 'active';

update projects
set status = 'active'
where status is null;

create unique index if not exists projects_name_unique_idx
    on projects (lower(name));
