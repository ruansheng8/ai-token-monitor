-- Postgres migration: V4__add_daily_stats_cache.sql
CREATE TABLE IF NOT EXISTS daily_stats (
    date VARCHAR(50) NOT NULL,
    source VARCHAR(50) NOT NULL,
    input_tokens BIGINT DEFAULT 0,
    cached_input_tokens BIGINT DEFAULT 0,
    output_tokens BIGINT DEFAULT 0,
    thinking_tokens BIGINT DEFAULT 0,
    sessions_count BIGINT DEFAULT 0,
    cost_usd DOUBLE PRECISION DEFAULT 0.0,
    PRIMARY KEY (date, source)
);
