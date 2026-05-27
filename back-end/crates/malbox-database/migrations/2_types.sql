create type task_state as enum (
    'pending',
    'initializing',
    'preparing_resources',
    'running',
    'stopping',
    'completed',
    'failed',
    'canceled'
);

create type machine_arch as enum (
    'x86',
    'x64'
);

create type machine_platform as enum (
    'windows',
    'linux'
);

create type machine_status as enum (
    'creating',
    'provisioning',
    'ready',
    'assigned',
    'reverting',
    'deleting',
    'failed'
);

create type provision_run_status as enum (
    'running',
    'success',
    'failed'
);

create type result_format as enum (
    'json',
    'bytes'
);

create type task_result_role as enum (
    'report',
    'artifact'
);

create type recipe_scope as enum (
    'personal',
    'shared'
);

create type transform_kind as enum (
    'yaml',
    'js',
    'wasm'
);
