create table images (
    id          uuid primary key default uuid_generate_v1mc(),
    name        text unique not null,
    platform    machine_platform not null,
    arch        machine_arch not null,
    format      text not null default 'qcow2',
    description text,
    path        text not null,
    available   boolean not null default true,
    created_at  timestamptz not null default now(),
    updated_at  timestamptz not null default now()
);

select trigger_updated_on('images');
