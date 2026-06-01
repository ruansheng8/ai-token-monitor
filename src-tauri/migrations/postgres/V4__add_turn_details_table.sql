CREATE TABLE IF NOT EXISTS turn_details (
    source VARCHAR(50) NOT NULL,
    uuid VARCHAR(100) NOT NULL,
    idx INTEGER NOT NULL,
    user_prompt TEXT,
    executed_commands TEXT,
    failed_commands TEXT,
    modified_files TEXT,
    PRIMARY KEY (source, uuid, idx)
);

CREATE INDEX IF NOT EXISTS idx_turn_details_lookup ON turn_details(source, uuid, idx);
