CREATE TABLE IF NOT EXISTS sessions (
    source VARCHAR(50) NOT NULL,
    uuid VARCHAR(255) NOT NULL,
    title TEXT,
    created_at VARCHAR(100),
    last_parsed_idx BIGINT DEFAULT -1,
    last_mtime DOUBLE PRECISION DEFAULT 0.0,
    project_path TEXT,
    PRIMARY KEY (source, uuid)
);

CREATE TABLE IF NOT EXISTS turns (
    source VARCHAR(50) NOT NULL,
    uuid VARCHAR(255) NOT NULL,
    idx BIGINT NOT NULL,
    model VARCHAR(255),
    input_tokens BIGINT DEFAULT 0,
    cached_input_tokens BIGINT DEFAULT 0,
    output_tokens BIGINT DEFAULT 0,
    thinking_tokens BIGINT DEFAULT 0,
    cost_usd DOUBLE PRECISION DEFAULT 0.0,
    message_id VARCHAR(255),
    request_id VARCHAR(255),
    timestamp VARCHAR(100),
    latency DOUBLE PRECISION DEFAULT 0.0,
    tps DOUBLE PRECISION DEFAULT 0.0,
    PRIMARY KEY (source, uuid, idx),
    FOREIGN KEY(source, uuid) REFERENCES sessions(source, uuid) ON DELETE CASCADE
);

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

CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at);
CREATE INDEX IF NOT EXISTS idx_sessions_source_created ON sessions(source, created_at);
CREATE INDEX IF NOT EXISTS idx_turns_model ON turns(model);
CREATE INDEX IF NOT EXISTS idx_turns_latency ON turns(latency);
