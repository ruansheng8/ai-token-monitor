-- Postgres migration: V2__add_latency_and_tps.sql
ALTER TABLE turns ADD COLUMN latency DOUBLE PRECISION DEFAULT 0.0;
ALTER TABLE turns ADD COLUMN tps DOUBLE PRECISION DEFAULT 0.0;
