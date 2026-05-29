create table recipes (
    id              uuid primary key default gen_random_uuid(),
    name            varchar not null,
    description     text,
    author          varchar not null,
    scope           recipe_scope not null default 'personal',
    tags            varchar[] not null default '{}',
    steps           jsonb not null,
    created_on      timestamp without time zone not null default now(),
    updated_at      timestamp without time zone
);

select trigger_updated_on('recipes');
create index idx_recipes_author on recipes(author);
create index idx_recipes_scope on recipes(scope);

create table custom_transforms (
    id              uuid primary key default gen_random_uuid(),
    transform_id    varchar not null unique,
    name            varchar not null,
    category        varchar not null,
    kind            transform_kind not null,
    content         text not null,
    enabled         boolean not null default true,
    git_synced      boolean not null default false,
    created_on      timestamp without time zone not null default now(),
    updated_at      timestamp without time zone
);

select trigger_updated_on('custom_transforms');
create index idx_custom_transforms_kind on custom_transforms(kind);
