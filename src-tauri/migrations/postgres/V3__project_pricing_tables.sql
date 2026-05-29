ALTER TABLE sessions ADD COLUMN IF NOT EXISTS project_name VARCHAR(255) DEFAULT 'unknown-project';

UPDATE sessions
SET project_name = COALESCE(NULLIF(project_name, ''), 'unknown-project')
WHERE project_name IS NULL OR btrim(project_name) = '';

CREATE TABLE IF NOT EXISTS project_daily_stats (
    date VARCHAR(50) NOT NULL,
    project_name VARCHAR(255) NOT NULL,
    total_tokens BIGINT DEFAULT 0,
    total_cost_usd DOUBLE PRECISION DEFAULT 0.0,
    sessions_count BIGINT DEFAULT 0,
    PRIMARY KEY (date, project_name)
);

CREATE TABLE IF NOT EXISTS model_pricing (
    id BIGSERIAL PRIMARY KEY,
    model_pattern VARCHAR(255) NOT NULL UNIQUE,
    input_price_per_million DOUBLE PRECISION NOT NULL,
    cached_input_price_per_million DOUBLE PRECISION NOT NULL,
    output_price_per_million DOUBLE PRECISION NOT NULL,
    priority BIGINT NOT NULL DEFAULT 100,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at VARCHAR(50) NOT NULL
);

CREATE TABLE IF NOT EXISTS exchange_rates (
    currency_code VARCHAR(16) PRIMARY KEY,
    rate_from_usd DOUBLE PRECISION NOT NULL,
    updated_at VARCHAR(50) NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_project_daily_stats_date ON project_daily_stats(date);
CREATE INDEX IF NOT EXISTS idx_sessions_project_name ON sessions(project_name);
