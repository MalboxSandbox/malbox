-- Machine snapshots: tracks provisioning steps and their snapshots.
-- Each row represents a provider-level snapshot taken after a provisioning step.
-- The is_active flag determines which snapshot is used for revert.

CREATE TABLE "machine_snapshots" (
    id                    UUID PRIMARY KEY DEFAULT uuid_generate_v1mc(),
    machine_id            INTEGER NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    name                  VARCHAR NOT NULL,
    provider_snapshot_id  VARCHAR NOT NULL,
    provisioner           VARCHAR,
    provision_output      JSONB,
    description           TEXT,
    tags                  VARCHAR[],
    guest_plugins         JSONB DEFAULT '[]',
    is_active             BOOLEAN NOT NULL DEFAULT false,
    created_at            TIMESTAMPTZ DEFAULT now(),
    updated_at            TIMESTAMPTZ DEFAULT now(),
    UNIQUE(machine_id, name)
);

SELECT trigger_updated_on('"machine_snapshots"');
