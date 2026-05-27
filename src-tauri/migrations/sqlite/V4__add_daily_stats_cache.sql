-- SQLite migration: V4__add_daily_stats_cache.sql
CREATE TABLE IF NOT EXISTS daily_stats (
    date TEXT NOT NULL,
    source TEXT NOT NULL,
    input_tokens INTEGER DEFAULT 0,
    cached_input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    thinking_tokens INTEGER DEFAULT 0,
    sessions_count INTEGER DEFAULT 0,
    cost_usd REAL DEFAULT 0.0,
    PRIMARY KEY (date, source)
);
