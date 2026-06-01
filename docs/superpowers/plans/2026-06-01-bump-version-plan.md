# 自动版本管理与展示实施计划

本项目通过在 `pnpm tauri build` 前植入预构建脚本，实现构建时补丁版本号的自动递增；同时升级前后端配置通信，在系统配置界面中展示精美的版本标志。

## 用户评审要求

> [!IMPORTANT]
> 1. 版本号自增将在执行 `pnpm tauri build` 时被触发（通过修改 `tauri.conf.json` 里的 `beforeBuildCommand`）。如果您只是本地开发运行 `pnpm tauri dev`（触发的是 `beforeDevCommand`），版本号不会自增。
> 2. 为确保在不同电脑及 CI 平台上生成一致的版本号，我们将会在项目根目录下新增一个受 Git 追踪的配置文件 `version-config.json`。该文件在您手动修改版本号或每次提交构建时，都会协助同步当前构建的最新版本。

## 开放性问题

目前没有待解决的开放性问题。

## 计划变更内容

---

### 1. 自动版本更新模块

#### [NEW] [version-config.json](file:///d:/VibeCoding/ai-token-monitor/version-config.json)
* 创建基准版本配置文件，用于追踪版本号和 commit 数量。初始内容为：
  ```json
  {
    "base_version": "0.1.0",
    "base_commit_count": 65,
    "last_calculated_version": "0.1.0"
  }
  ```

#### [NEW] [bump-version.js](file:///d:/VibeCoding/ai-token-monitor/scripts/bump-version.js)
* 创建版本自增与同步的预构建脚本：
  1. 通过 `git rev-list --count HEAD` 获取当前的 Commit 数。
  2. 读取 `package.json` 中的 `version` 字段，判断与 `version-config.json` 里的 `last_calculated_version` 是否一致。
  3. 若不一致，说明用户手动改动了版本，更新 `version-config.json` 中的 `base_version` 和 `base_commit_count`。
  4. 若一致，根据 Commit 数 of 增长差值计算最新补丁版本号。
  5. 将计算出的新版本号同步写回 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 和 `version-config.json`。

#### [MODIFY] [tauri.conf.json](file:///d:/VibeCoding/ai-token-monitor/src-tauri/tauri.conf.json)
* 将 `beforeBuildCommand` 更改为 `"node scripts/bump-version.js && npm run build"`。

---

### 2. 后端 API 模块

#### [MODIFY] [server.rs](file:///d:/VibeCoding/ai-token-monitor/src-tauri/src/server.rs)
* 修改 `ConfigReq` 结构体：
  * 引入 `pub app_version: Option<String>` 字段。
* 修改 `handle_config_get` 函数：
  * 在返回的对象中，将 `app_version` 设置为编译期版本号 `Some(env!("CARGO_PKG_VERSION").to_string())`。

---

### 3. 前端界面展示

#### [MODIFY] [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)
* 新增状态变量 `const [appVersion, setAppVersion] = useState('');`。
* 在拉取系统配置的逻辑中增加对 `data.app_version` 的读取 and 赋值。
* 在系统设置组件的关闭行为板块下方，新增版本号卡片 UI，采用玻璃拟态暗黑面板与霓虹青边框（遵循项目设计规范）：
  ```tsx
  <div className="flex flex-col gap-2 animate-fade-in text-left">
    <label className="text-xs font-semibold text-text-secondary">🏷️ 系统版本 (System Version)</label>
    <div className="w-full bg-bg-secondary/40 dark:bg-white/3 border border-card-border rounded-xl px-4 py-3 flex items-center justify-between">
      <span className="text-xs font-medium text-text-primary">AI Token Monitor</span>
      <span className="px-2.5 py-1 text-xs font-mono font-semibold rounded-lg bg-neon-cyan/10 text-neon-cyan border border-neon-cyan/20">
        v{appVersion || "0.1.0"}
      </span>
    </div>
  </div>
  ```

---

## 验证计划

### 自动化与脚本验证
* 运行以下测试命令：
  ```powershell
  # 测试执行版本递增脚本，检查 package.json, tauri.conf.json, Cargo.toml 以及 version-config.json
  node scripts/bump-version.js
  
  # 运行前端 TS 类型校验，确认无 TS 编译报错
  npx tsc -b --noEmit
  
  # 运行 Rust 语法和编译校验
  cd src-tauri; cargo check
  ```

### 手动功能验证
* 启动应用开发服务器：
  ```powershell
  pnpm dev
  ```
* 打开系统设置弹窗的“数据源与系统设置”面板，确认是否正确展现版本号。
* 在 `package.json` 中将版本号修改为 `0.2.0`，运行 `node scripts/bump-version.js`，验证是否检测到手动修改并成功重置版本号基准。
