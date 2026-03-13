-- Track provisioning details on machines.

ALTER TABLE "machines"
    ADD COLUMN provisioned       BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN provisioner       VARCHAR,
    ADD COLUMN provision_output  JSONB;
