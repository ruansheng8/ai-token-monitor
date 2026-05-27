# 数据库连接测试与 Refinery 迁移方案设计

本文档详细描述了如何优化数据库配置中的连接测试逻辑（避免在测试时触发数据库表结构初始化），以及如何引入 Rust 业界常用的数据库迁移工具 `refinery`，以规范管理 SQLite 与 PostgreSQL 的版本化表结构。

---

## 1. 现状与痛点

1. **测试连接触发 DDL**
   目前在 `/api/config/test` 接口（对应 [server.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/server.rs) 中的 `handle_config_test`）中，建立物理连接后会立即调用 `crate::db_adapter::init_tables` 方法。这导致仅仅在“测试连接”阶段就会强制在目标数据库中创建表并打印表结构变更日志，干扰了用户的测试意图。

2. **缺少版本化迁移工具**
   当前 SQLite 本地缓存和 PostgreSQL 远程库的表结构初始化是硬编码在 Rust 代码中的 DDL 字符串。如果后续有表结构的更改，依赖手工检测字段是否存在并手动运行 SQL，这种方式容易出错且难以维护。

---

## 2. 方案设计

### 2.1 优化连接测试 (Ping)
在 `handle_config_test` 接口中，移除 `init_tables` 调用，改为使用简单的连接活性检验（Ping Query）：
* **SQLite**：执行 `SELECT 1` 查询。
* **PostgreSQL**：执行 `SELECT 1` 查询。

这样只需验证物理链路及凭证的有效性，不需要读写任何业务表或创建任何结构。

### 2.2 引入 Refinery 迁移框架
在 `Cargo.toml` 中添加 `refinery` 的依赖：
```toml
refinery = { version = "0.8", features = ["rusqlite", "postgres"] }
```

### 2.3 设计两套独立的迁移 SQL 文件
由于不同数据库驱动（SQLite 和 PostgreSQL）的 SQL 语法与类型略有差异，在 `src-tauri/migrations` 路径下设立 `sqlite` 和 `postgres` 两个独立文件夹：

* **本地 SQLite 迁移文件 (`src-tauri/migrations/sqlite/V1__initial_sqlite_schema.sql`)**：
  ```sql
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
      PRIMARY KEY (source, uuid, idx),
      FOREIGN KEY(source, uuid) REFERENCES sessions(source, uuid) ON DELETE CASCADE
  );
  ```

* **远程 PostgreSQL 迁移文件 (`src-tauri/migrations/postgres/V1__initial_postgres_schema.sql`)**：
  ```sql
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
      PRIMARY KEY (source, uuid, idx),
      FOREIGN KEY(source, uuid) REFERENCES sessions(source, uuid) ON DELETE CASCADE
  );
  ```

### 2.4 在运行时执行迁移
1. 在 `db_adapter.rs` 中引入宏嵌入：
   ```rust
   mod sqlite_migrations {
       refinery::embed_migrations!("migrations/sqlite");
   }
   mod postgres_migrations {
       refinery::embed_migrations!("migrations/postgres");
   }
   ```
2. 将 `db_adapter::init_tables` 方法的内部实现重构为使用 Refinery 的 runner：
   * 对 SQLite 物理连接调用：
     ```rust
     sqlite_migrations::migrations::runner()
         .run(&mut *conn_lock.lock().unwrap())
         .map_err(|e| e.to_string())?;
     ```
   * 对 Postgres 物理连接调用：
     ```rust
     postgres_migrations::migrations::runner()
         .run(&mut *client_lock.lock().unwrap())
         .map_err(|e| e.to_string())?;
     ```

### 2.5 兼容旧版本 SQLite
由于本地可能已存在包含旧数据的 SQLite 缓存，并且没有 `refinery_schema_history` 迁移表：
* 我们将保留 [db.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/db.rs#L111) 中已有的 `init_cache_db` 检测和迁移逻辑（检测到旧版结构，先对其进行联合主键平滑转换升级）。
* 平滑迁移完成后再调用 `run_migrations`，此时 Refinery 会安全地在本地库中创建 `refinery_schema_history` 表并记录 V1 迁移已执行（因为建表语句使用 `CREATE TABLE IF NOT EXISTS`）。

---

## 3. 自检与测试计划

1. **连接测试验证**：
   在前端配置界面，点击 SQLite 或 PostgreSQL 的“测试连接”按钮，观察终端日志，应仅输出网络/物理连接成功信息，绝不能输出创表或结构变更的日志。
2. **全新初始化验证**：
   在清空本地 SQLite 数据库或新建 PostgreSQL 空数据库后，启动程序，应能正确生成 `refinery_schema_history` 以及业务表，确保没有报错。
3. **老数据平滑过渡验证**：
   模拟旧版没有 `source` 字段的 SQLite 数据，启动程序，检查是否先成功升级了老结构，后被 Refinery 正确接管并写入 `refinery_schema_history`。
