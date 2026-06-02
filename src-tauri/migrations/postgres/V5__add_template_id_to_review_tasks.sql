ALTER TABLE review_tasks ADD COLUMN template_id VARCHAR(255) DEFAULT NULL;

-- 清理掉历史报告
TRUNCATE TABLE review_task_events CASCADE;
TRUNCATE TABLE review_tasks CASCADE;
