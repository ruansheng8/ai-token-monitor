CREATE TABLE IF NOT EXISTS sessions (
    source TEXT NOT NULL,
    uuid TEXT NOT NULL,
    title TEXT,
    created_at TEXT,
    last_parsed_idx INTEGER DEFAULT -1,
    last_mtime REAL DEFAULT 0.0,
    project_path TEXT,
    PRIMARY KEY (source, uuid)
);

CREATE TABLE IF NOT EXISTS turns (
    source TEXT NOT NULL,
    uuid TEXT NOT NULL,
    idx INTEGER NOT NULL,
    model TEXT,
    input_tokens INTEGER DEFAULT 0,
    cached_input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    thinking_tokens INTEGER DEFAULT 0,
    cost_usd REAL DEFAULT 0.0,
    message_id TEXT,
    request_id TEXT,
    timestamp TEXT,
    latency REAL DEFAULT 0.0,
    tps REAL DEFAULT 0.0,
    PRIMARY KEY (source, uuid, idx),
    FOREIGN KEY(source, uuid) REFERENCES sessions(source, uuid) ON DELETE CASCADE
);

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

CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at);
CREATE INDEX IF NOT EXISTS idx_sessions_source_created ON sessions(source, created_at);
CREATE INDEX IF NOT EXISTS idx_turns_model ON turns(model);
CREATE INDEX IF NOT EXISTS idx_turns_latency ON turns(latency);
