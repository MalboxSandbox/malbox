CREATE TYPE task_result_role AS ENUM (
    'report',
    'artifact'
);

ALTER TABLE task_results
    ADD COLUMN role task_result_role NOT NULL DEFAULT 'artifact';

CREATE INDEX idx_task_results_task_role ON task_results(task_id, role);
