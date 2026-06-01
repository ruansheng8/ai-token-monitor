# 开发者调试模式技术设计文档

此文档定义了在 Token Insight 项目中引入“开发者调试模式”的技术设计。该模式开启后，系统在扫描各历史数据源会话时会严格限制总处理会话数为 20 个，以显著缩短开发与调试时的性能开销。

## 1. 背景与目标
在 Token Insight 的本地开发或测试过程中，系统默认执行全量会话与轮次数据的扫描同步（包括 Antigravity、Claude Code、Codex、Cursor、Trae、Trae CN 等物理存储源）。若用户的本地开发会话历史非常庞大，每次启动或刷新均执行全量扫描，将导致高额的 I/O 及 CPU 占用，极大地拖慢了开发时的迭代验证速度。
为了解决该痛点，本方案建议在配置系统及前端 UI 中引入“开发者调试模式 (Developer Mode)”。当其开启时，每次扫描逻辑只遍历并同步前 20 个会话便直接退出，以达成极速就绪的效果。

---

## 2. 详细设计与变更范围

### 2.1. 后端配置字段扩展 (Rust)

#### 2.1.1 `AppConfig` 结构体（修改文件：[config.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/config.rs)）
- 扩展 `AppConfig` 结构体，添加 `developer_mode` 布尔值字段。
- 使用 `#[serde(default)]` 保证旧版 `config.json` 文件能够兼容反序列化。
- 在 `sync_to_env` 中同步设置环境变量 `DEVELOPER_MODE`。

#### 2.1.2 API 传输结构 `ConfigReq`（修改文件：[server.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/server.rs)）
- 在 `ConfigReq` 中增加 `developer_mode: Option<bool>` 字段。
- 修改 `handle_config_get`，将配置中的 `developer_mode` 组装返回给前端。
- 修改 `handle_config_save`，接收前端传来的 `developer_mode` 并更新保存至 `config.json`。

### 2.2. 后端同步扫描逻辑改造 (Rust)

修改文件：[db.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/db.rs)

1. **共享计数器的引入**：
   在 `sync_cache_db_with_progress` 入口处载入配置并确定计数上限：
   ```rust
   let config = crate::config::load_config();
   let mut remaining_limit = if config.developer_mode { Some(20) } else { None };
   ```

2. **函数签名变更**：
   将 `remaining_limit: &mut Option<usize>` 传递给所有的子同步函数：
   - `sync_claude_code`
   - `sync_codex`
   - `sync_cursor`
   - `sync_trae` (其内部调用的 `sync_trae_common`)
   - `sync_trae_cn` (其内部调用的 `sync_trae_common`)

3. **迭代循环中的中断逻辑**：
   在遍历会话文件的各个循环顶部，插入剩余限额校验及计数器扣减逻辑。一旦计数器减为 0，直接 `break` 中断该数据源的扫描流程：
   ```rust
   if let Some(ref mut rem) = remaining_limit {
       if *rem == 0 {
           break;
       }
       *rem -= 1;
   }
   ```

### 2.3. 前端 UI 设置与提示语实现 (React + TypeScript)

修改文件：[App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)

1. **设置面板状态新增**：
   增加状态定义：
   ```typescript
   const [developerMode, setDeveloperMode] = useState(false);
   ```

2. **拉取与保存逻辑适配**：
   - 在 `loadConfig` 回调中填充：`setDeveloperMode(!!data.developer_mode);`
   - 在 `config/save` 的 POST 请求体中包含 `developer_mode: developerMode`。

3. **UI 渲染**：
   在配置面板的“🖥️ 数据源与系统设置”标签页中，渲染一个精美的 Toggle 开关以及当开启时的提示：
   ```tsx
   <div className="flex flex-col gap-2 animate-fade-in text-left">
     <div className="flex items-center justify-between bg-bg-secondary/40 dark:bg-white/3 border border-card-border rounded-xl px-4 py-3">
       <div className="flex flex-col">
         <span className="text-xs font-semibold text-text-primary">🛠️ 调试开发者模式 (Developer Mode)</span>
         <span className="text-[10px] text-text-muted">限制扫描量为 20 个会话以提升本地调试速度</span>
       </div>
       <label className="relative inline-flex items-center cursor-pointer">
         <input
           type="checkbox"
           checked={developerMode}
           onChange={(e) => setDeveloperMode(e.target.checked)}
           className="sr-only peer"
         />
         <div className="w-9 h-5 bg-slate-300 dark:bg-white/10 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-slate-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-gradient-to-r peer-checked:from-neon-cyan peer-checked:to-neon-purple"></div>
       </label>
     </div>
     {developerMode && (
       <div className="text-[10px] text-amber-500 bg-amber-500/10 border border-amber-500/20 rounded-xl p-3 leading-relaxed font-semibold animate-fade-in">
         ⚠️ 开发者模式下仅扫描前20个会话，用于调试，不会扫描全量数据
       </div>
     )}
   </div>
   ```

---

## 3. 验证计划

### 3.1. 编译与类型检查
- 运行 `tsc -b --noEmit` 验证前端无类型报错。
- 在 `src-tauri` 下运行 `cargo check` 验证 Rust 后端编译正常。

### 3.2. 功能性验证
1. 打开系统设置面板，确认“🖥️ 数据源与系统设置”下显示新增的“🛠️ 调试开发者模式”。
2. 开启开关，确认黄色警告横幅正常滑入/渐变显示。
3. 点击“保存并应用配置”以应用设置。
4. 触发数据重新同步扫描，在同步控制台日志中查看总文件数。确认扫描量达到上限 20 后即可极速停止扫描。
5. 再次关闭开关并保存，确认能恢复全量历史会话扫描。
