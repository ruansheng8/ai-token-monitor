-- SQLite migration: V2__add_latency_and_tps.sql
ALTER TABLE turns ADD COLUMN latency REAL DEFAULT 0.0;
ALTER TABLE turns ADD COLUMN tps REAL DEFAULT 0.0;
