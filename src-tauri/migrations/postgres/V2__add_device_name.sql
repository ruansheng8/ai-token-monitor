-- 1. 为 sessions 表添加 device_name 字段
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS device_name VARCHAR(100) DEFAULT 'unknown';

-- 2. 重构 daily_stats 缓存表以支持设备维度
DROP TABLE IF EXISTS daily_stats;

CREATE TABLE daily_stats (
    date VARCHAR(50) NOT NULL,
    source VARCHAR(50) NOT NULL,
    device_name VARCHAR(100) NOT NULL DEFAULT 'unknown',
    input_tokens BIGINT DEFAULT 0,
    cached_input_tokens BIGINT DEFAULT 0,
    output_tokens BIGINT DEFAULT 0,
    thinking_tokens BIGINT DEFAULT 0,
    sessions_count BIGINT DEFAULT 0,
    cost_usd DOUBLE PRECISION DEFAULT 0.0,
    PRIMARY KEY (date, source, device_name)
);
