-- Image registry

CREATE TABLE "images" (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v1mc(),
    name        TEXT UNIQUE NOT NULL,
    platform    machine_platform NOT NULL,
    arch        machine_arch NOT NULL,
    format      TEXT NOT NULL DEFAULT 'qcow2',
    description TEXT,
    path        TEXT NOT NULL,
    available   BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT trigger_updated_on('"images"');

-- Add foreign keys on machines that reference tables created in later migrations
ALTER TABLE "machines"
    ADD CONSTRAINT machines_image_id_fkey FOREIGN KEY (image_id) REFERENCES images(id),
    ADD CONSTRAINT machines_current_task_id_fkey FOREIGN KEY (current_task_id) REFERENCES tasks(id);
