# 开发者调试模式实现计划

此文档定义了在 Token Insight 项目中引入“开发者调试模式”的实现步骤与验证方法。

## 1. 目标
通过在前端 UI 中增加开发者调试模式开关，并在后端同步流程中应用最大扫描 20 个会话的限制，提升本地开发和调试速度。

## 2. 计划变更的文件与步骤

---

### 第一步：后端配置模块升级 (Rust)

#### 修改 [config.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/config.rs)
1. 升级 `AppConfig` 结构体：
   ```rust
   #[serde(default)]
   pub developer_mode: bool,
   ```
2. 升级 `impl Default for AppConfig`：
   ```rust
   developer_mode: false,
   ```
3. 在 `sync_to_env` 中同步：
   ```rust
   std::env::set_var("DEVELOPER_MODE", if config.developer_mode { "true" } else { "false" });
   ```

#### 修改 [server.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/server.rs)
1. 升级 `ConfigReq` 传输结构体以包含：
   ```rust
   pub developer_mode: Option<bool>,
   ```
2. 在 `handle_config_get` 中返回配置：
   ```rust
   developer_mode: Some(config.developer_mode),
   ```
3. 在 `handle_config_save` 中读取并保存配置：
   ```rust
   let new_developer_mode = req.developer_mode.unwrap_or(false);
   // ... 构造 AppConfig
   developer_mode: new_developer_mode,
   ```

---

### 第二步：后端同步核心逻辑升级 (Rust)

#### 修改 [db.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/db.rs)
1. 修改同步函数签名以传入 `remaining_limit: &mut Option<usize>`:
   - `pub fn sync_claude_code`
   - `pub fn sync_codex`
   - `pub fn sync_cursor`
   - `pub fn sync_trae`
   - `pub fn sync_trae_cn`
   - `fn sync_trae_common`
2. 在 `sync_cache_db_with_progress` 主函数中：
   - 从 `load_config` 获取 `developer_mode`。
   - 初始化限制计数器：
     ```rust
     let mut remaining_limit = if config.developer_mode { Some(20) } else { None };
     ```
   - 在 Antigravity 会话文件的 `for` 循环顶部添加限制校验并递减：
     ```rust
     if let Some(ref mut rem) = remaining_limit {
         if *rem == 0 {
             break;
         }
         *rem -= 1;
     }
     ```
   - 在调用 `sync_claude_code`、`sync_codex`、`sync_cursor`、`sync_trae`、`sync_trae_cn` 时，传入 `&mut remaining_limit`。
3. 在各子数据源遍历循环顶部，同样添加校验和递减：
   ```rust
   if let Some(ref mut rem) = remaining_limit {
       if *rem == 0 {
           break;
       }
       *rem -= 1;
   }
   ```

---

### 第三步：前端页面升级 (React + TS)

#### 修改 [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)
1. 声明 `developerMode` 状态变量：
   ```typescript
   const [developerMode, setDeveloperMode] = useState(false);
   ```
2. 升级拉取配置（`loadConfig` 内）以设置状态：
   ```typescript
   setDeveloperMode(!!data.developer_mode);
   ```
3. 升级保存配置（`config/save` 的 fetch 请求体）以传递状态：
   ```typescript
   developer_mode: developerMode
   ```
4. 在“🖥️ 数据源与系统设置”设置区域内渲染 Toggle 控制及黄色警告提示框。

---

## 3. 验证计划

### 3.1 静态与自动化校验
1. 运行前端类型检查：
   ```powershell
   npx tsc -b --noEmit
   ```
2. 运行前端 Lint 校验：
   ```powershell
   npm run lint
   ```
3. 运行 Rust 后端编译校验：
   ```powershell
   cd src-tauri; cargo check
   ```

### 3.2 手动功能测试
1. 在前端设置中打开“开发者模式”，并保存。
2. 触发一次数据重新扫描同步，查看并确认扫描状态的文件总数及实际扫描数。在日志台确认处理 20 个会话后即停止。
3. 关闭该开关并保存，再次执行扫描，确认恢复全量扫描以确保功能恢复能力。
