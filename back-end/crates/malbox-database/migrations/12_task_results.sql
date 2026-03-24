CREATE TYPE result_format AS ENUM (
    'json',
    'bytes'
);

CREATE TABLE task_results (
    id          SERIAL      PRIMARY KEY,
    task_id     INTEGER     NOT NULL REFERENCES tasks(id),
    plugin_name TEXT        NOT NULL,
    result_name TEXT        NOT NULL,
    format      result_format NOT NULL,
    size_bytes  BIGINT      NOT NULL,
    file_path   TEXT        NOT NULL,
    created_on  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_task_results_task_id ON task_results(task_id);
