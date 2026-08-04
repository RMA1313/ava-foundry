create table if not exists projects (
    id uuid primary key,
    name text not null,
    description text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

alter table projects
    alter column created_at set default now(),
    alter column updated_at set default now();
