# 多源 AI Token 监控系统设计规范 (Multi-Source AI Token Monitor Design Spec)

该设计文档描述了如何将本地的 **Claude Code** 和 **Codex CLI** 产生的 Token 使用日志接入到现有的 **AI Token Monitor** (基于 Antigravity) 的增量缓存和数据大盘中。

## 1. 目标与背景 (Goal & Background)

目前，AI Token Monitor 仅支持监控本地的 `antigravity` 会话（通过扫描 `.gemini/antigravity` 下的 conversations 数据库及 transcript.jsonl 日志）。
为了向开发者提供更全面的本地 AI 辅助开发用量大盘，我们需要整合以下两种主流本地 AI 助手的 Token 使用日志：
- **Claude Code**：读取用户家目录下 `~/.claude/projects/` 的 `.jsonl` 文件。
- **Codex CLI**：读取用户家目录下 `~/.codex/sessions/` 的 `rollout-*.jsonl` 或其他 `.jsonl` 文件。

为了支持这种多数据源的高效接入与统一展示，需要对底层的缓存数据库（SQLite）进行重构，引入来源标识 `source`，并统一数据的统计与聚合方式。

## 2. 数据库表结构重构 (Database Schema Design)

为了实现高性能的数据处理并利用 SQLite 的原生聚合能力，我们对 `sessions` 和 `turns` 表进行通用化升级，通过 `source` 列将不同的数据来源进行区隔。

### 2.1 会话表 `sessions`

```sql
CREATE TABLE IF NOT EXISTS sessions (
    source TEXT NOT NULL,                  -- 来源: 'antigravity', 'claude_code', 'codex'
    uuid TEXT NOT NULL,                    -- 来源内部的会话唯一标识 (对于 Claude 为项目名/日志文件名)
    title TEXT,                            -- 会话/项目标题
    created_at TEXT,                       -- 会话创建时间 (ISO 8601 格式，如 YYYY-MM-DDTHH:MM:SSZ)
    last_parsed_idx INTEGER DEFAULT -1,    -- 增量解析偏移量 (字节数/行号/Turn索引)
    last_mtime REAL DEFAULT 0.0,           -- 物理日志文件的最后修改时间戳
    project_path TEXT,                     -- 仅对 Claude/Codex 记录本地项目根路径或日志文件路径
    PRIMARY KEY (source, uuid)
);
```

### 2.2 交互轮次表 `turns`

```sql
CREATE TABLE IF NOT EXISTS turns (
    source TEXT NOT NULL,                  -- 来源: 'antigravity', 'claude_code', 'codex'
    uuid TEXT NOT NULL,                    -- 对应 sessions.uuid
    idx INTEGER NOT NULL,                  -- 会话内的轮次序号 (0, 1, 2...)
    model TEXT,                            -- 使用的模型 (如 claude-3-5-sonnet, gemini-3.5-flash)
    input_tokens INTEGER DEFAULT 0,        -- 输入 tokens 总数
    cached_input_tokens INTEGER DEFAULT 0, -- 命中缓存的输入 tokens
    output_tokens INTEGER DEFAULT 0,       -- 输出 tokens
    thinking_tokens INTEGER DEFAULT 0,     -- 深度思考/推理 tokens
    cost_usd REAL DEFAULT 0.0,             -- 本次交互产生的估算费用 (USD)
    message_id TEXT,                       -- 消息去重 ID
    request_id TEXT,                       -- 请求去重 ID
    timestamp TEXT,                        -- 本次交互发生的精确时间 (ISO 8601 格式)
    PRIMARY KEY (source, uuid, idx),
    FOREIGN KEY(source, uuid) REFERENCES sessions(source, uuid) ON DELETE CASCADE
);
```

---

## 3. 多源数据同步与解析机制 (Data Synchronization)

### 3.1 目录探测与检测
- **Antigravity**：
  - 扫描路径：`~/.gemini/antigravity/conversations/` 下的 `.db` 文件。
  - 会话标题和时间戳：读取 `~/.gemini/antigravity/brain/<uuid>/.system_generated/logs/transcript.jsonl`。
- **Claude Code**：
  - 扫描路径：`~/.claude/projects/` 递归搜索所有 `.jsonl` 文件。
- **Codex CLI**：
  - 扫描路径：`~/.codex/sessions/` 搜索所有 `rollout-*.jsonl` 或其他 `.jsonl` 文件。

### 3.2 增量解析与更新流程
后台扫描线程 `start_background_scan` 启动后，分别执行三个独立的同步器：
1. **Antigravity 同步器**：沿用原有的解析逻辑，解密和提取 Protobuf 块，写入 `sessions` 和 `turns` 时强制设置 `source = 'antigravity'`。
2. **Claude Code 同步器**：
   - 扫描 `~/.claude/projects/` 下的 `.jsonl`。每个 `.jsonl` 文件（或根据项目分组的文件）映射为一个 `session`（`uuid` 可直接使用文件名或相对路径）。
   - 检测文件修改时间 `last_mtime`。如果无变化，则跳过解析。
   - 逐行读取文件，通过 `message_id` & `request_id` 去重。
   - 解析出 `timestamp`、`model`、输入/输出/缓存 token 数量。
   - 按时间升序给每一条数据生成 `idx` (0, 1, 2...)。
   - 使用内置的费率计算器计算 `cost_usd`。
3. **Codex CLI 同步器**：
   - 扫描 `~/.codex/sessions/` 下的 `.jsonl`。每个文件代表一个 `session`。
   - 原理与 Claude Code 同步器类似，逐行增量解析、按时间排序分配 `idx`，计算 `cost_usd` 后入库。

---

## 4. 大盘数据聚合接口重隔 (API & SQL Aggregation)

由于表结构中统一了字段，原有大盘的 SQL 聚合逻辑可以无缝扩展。我们将修改 Rust 后端的 API 和 SQL 查询：

1. **大盘全局指标**：
   - 修改 `SUM(uncached_input + cached_input)` 为 `SUM(input_tokens)`，并对新字段如 `cost_usd` 等直接求和。
2. **支持数据源筛选**：
   - 为 `/api/metrics` 支持接收可选的 `source` 参数（`?source=all`、`?source=claude_code` 等），在 SQL 查询时动态添加 `WHERE source = ?` 过滤。
3. **前端看板展示**：
   - 前端可在右上角或顶部新增一个“数据源切换器”下拉菜单（All / Antigravity / Claude Code / Codex）。
   - 看板的 ECharts 图表和数据卡片根据所选的数据源进行重载，并以统一的视觉系统呈现。

---

## 5. 验证与测试计划 (Verification Plan)

### 5.1 数据库结构升级测试
- 模拟老版本应用（只有旧 `turns`/`sessions` 表），启动新程序，验证数据库是否能平滑升级，且历史的 antigravity 数据完好无损。

### 5.2 模拟日志解析测试
- 在临时目录下创建模拟的 `claude` 和 `codex` 的 `.jsonl` 日志，测试扫描模块是否能正确探测、增量解析、生成 `cost_usd` 并完成落库。
- 模拟文件修改（追加写入新行），验证 `last_mtime` 触发的增量解析是否只读取新追加的行，而不会重复插入已有记录。

### 5.3 接口联调测试
- 通过调用 `/api/metrics?source=claude_code`，验证返回的 JSON 结构是否完整，各项指标（输入/输出/缓存 token，总费用）计算是否准确。
