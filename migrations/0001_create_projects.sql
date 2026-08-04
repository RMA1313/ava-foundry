create table if not exists projects (
    id uuid primary key,
    name text not null,
    description text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists projects_created_at_idx
    on projects (created_at asc, id asc);
