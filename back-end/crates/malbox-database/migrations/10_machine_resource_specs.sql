-- Add resource spec columns for config reconciliation.
-- These store the declared specs from config so that reconcile_config()
-- can detect spec drift without re-reading the config.
ALTER TABLE "machines" ADD COLUMN cpus INTEGER;
ALTER TABLE "machines" ADD COLUMN memory_mb INTEGER;
ALTER TABLE "machines" ADD COLUMN disk_size_mb BIGINT;
ALTER TABLE "machines" ADD COLUMN image_name VARCHAR(255);
ALTER TABLE "machines" ADD COLUMN provider_config_hash VARCHAR(64);
