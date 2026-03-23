-- Separate provisioning tracking from snapshots.
-- machine_snapshots becomes pure snapshot data.
-- provision_runs tracks every provisioning attempt.

ALTER TABLE "machine_snapshots" DROP COLUMN IF EXISTS provisioner;
ALTER TABLE "machine_snapshots" DROP COLUMN IF EXISTS provision_output;

CREATE TYPE provision_run_status AS ENUM ('running', 'success', 'failed');

CREATE TABLE "provision_runs" (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v1mc(),
    machine_id      INTEGER NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    provisioner     VARCHAR NOT NULL,
    status          provision_run_status NOT NULL DEFAULT 'running',
    config          JSONB,
    output          JSONB,
    error_message   TEXT,
    snapshot_id     UUID REFERENCES machine_snapshots(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ DEFAULT now(),
    updated_at      TIMESTAMPTZ DEFAULT now()
);

SELECT trigger_updated_on('"provision_runs"');
