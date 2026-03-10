-- Image registry and pool state persistence

CREATE TYPE provision_status AS ENUM (
    'unprovisioned',
    'provisioning',
    'provisioned',
    'failed'
);

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

-- Extend machines table for pool persistence
ALTER TABLE "machines"
    ADD COLUMN image_id          UUID REFERENCES images(id),
    ADD COLUMN provision_status  provision_status NOT NULL DEFAULT 'unprovisioned',
    ADD COLUMN clean_snapshot    VARCHAR,
    ADD COLUMN pool_member       BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN provider          VARCHAR,
    ADD COLUMN provider_id       VARCHAR,
    ADD COLUMN last_seen         TIMESTAMPTZ;
