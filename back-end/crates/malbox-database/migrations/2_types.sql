create type task_state as enum (
    'pending',
    'running',
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

create type recipe_scope as enum (
    'personal',
    'shared'
);

create type transform_kind as enum (
    'yaml',
    'js',
    'wasm'
);

-- Ordered by severity so max(classification) = worst-wins.
create type classification as enum ('clean', 'unknown', 'suspicious', 'malicious');
create type confidence as enum ('low', 'medium', 'high');

create type artifact_relation as enum (
    'root', 'extracted', 'dropped', 'unpacked', 'memdump', 'downloaded', 'pcap', 'output'
);

create type artifact_state as enum (
    'stored', 'dropped', 'pending', 'running', 'done', 'canceled'
);

create type analysis_stage as enum (
    'filter', 'extract', 'static', 'dynamic', 'post', 'review'
);

create type job_kind as enum ('host', 'detonation');

create type job_state as enum (
    'blocked', 'ready', 'running', 'completed', 'failed', 'skipped', 'cached', 'canceled'
);

create type job_error_class as enum (
    'recoverable', 'nonrecoverable', 'timeout', 'machine_lost'
);
