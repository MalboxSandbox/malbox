-- Queryable report metadata, extracted at write time.
--
-- plugin_reports stores per-plugin verdict and IOC data so that listing,
-- filtering, and aggregating never require re-reading the immutable
-- report JSON from disk.
--
-- sample_verdicts stores a pre-computed aggregate for each sample so the
-- default question -- "what do we know about this file?" -- is a single
-- row lookup.

-- The classification and confidence enums live in 2_types.sql.

create table plugin_reports (
    id              serial primary key,
    task_id         integer not null references tasks(id) on delete cascade,
    artifact_id     bigint not null references task_artifacts(id) on delete cascade,
    job_id          bigint references jobs(id) on delete set null,
    report_path     text not null,
    plugin_name     text not null,
    display_name    text,
    plugin_version  text not null default '',
    classification  classification,
    score           smallint check (score is null or (score >= 0 and score <= 100)),
    confidence      confidence,
    labels          text[] not null default '{}',
    indicators      jsonb not null default '[]',
    ttps            jsonb not null default '[]',
    summary         text,
    section_count   integer not null default 0,
    artifact_count  integer not null default 0,
    created_on      timestamptz not null default now(),

    unique (artifact_id, plugin_name)
);

create index idx_plugin_reports_task_id on plugin_reports(task_id);
create index idx_plugin_reports_artifact on plugin_reports(artifact_id);

create table sample_verdicts (
    sample_id       bigint primary key references samples(id) on delete cascade,
    classification  classification,
    score           smallint check (score is null or (score >= 0 and score <= 100)),
    indicator_count integer not null default 0,
    ttp_count       integer not null default 0,
    plugin_names    text[] not null default '{}',
    task_count      integer not null default 0,
    last_task_id    integer references tasks(id),
    updated_on      timestamptz not null default now()
);
