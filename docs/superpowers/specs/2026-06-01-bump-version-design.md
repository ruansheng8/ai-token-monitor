# 自动版本管理与展示设计说明书

本项目旨在通过预构建脚本实现 `pnpm tauri build` 构建时的自动版本号管理，并在系统配置界面中优雅地展示当前项目的版本号。

## 需求概述

1. **版本号格式**：使用业界标准的 `x.y.z`（主版本.次版本.修订号）SemVer 格式。
2. **构建时自动递增**：每次运行 `pnpm tauri build` 构建时，若未手动修改版本号，自动将修订号（补丁版本 `z`）递增。
3. **版本号起点**：以当前代码库中的版本 `0.1.0` 作为初始起点。
4. **支持手动修改**：如果用户手动在 `package.json` 中修改了版本号，构建时直接采用用户修改的版本，并不进行额外自增，此后基于该版本继续自动递增。
5. **系统设置展示**：在软件的系统设置面板中，可以看到项目的当前版本号。
6. **多端与 CI 一致性**：无论是在不同开发者的电脑上，还是在远程 CI 上，构建出来的版本号必须是绝对统一且连续的。

---

## 解决方案：基于 Git Commit 数的自动版本机制

为了解决多台电脑及远程 CI 环境下状态共享的问题，避免因本地缓存文件未提交导致的版本号不统一与冲突，我们采用基于 Git 提交历史数量（Commit Count）作为自动递增补丁版本的方法。

### 1. 基准配置文件 `version-config.json`
在项目根目录下创建一个由 Git 追踪的配置文件 `version-config.json`，用于记录用户指定的版本基准和对应时刻的 Commit 数量，同时也用于记录上一次计算出来的版本以识别手动修改：

```json
{
  "base_version": "0.1.0",
  "base_commit_count": 65,
  "last_calculated_version": "0.1.0"
}
```

### 2. 预构建脚本 `scripts/bump-version.js`
构建执行前，该脚本会自动执行，核心算法逻辑如下：
1. **获取当前提交数**：执行 `git rev-list --count HEAD` 获取当前的总提交次数，记为 `commitCount`。若不在 Git 环境中，默认回退为 `0`。
2. **读取配置与状态**：
   * 读取 `package.json` 中的当前版本 `currentPackageVersion`。
   * 读取 `version-config.json` 配置。
3. **识别手动修改**：
   * 比对 `currentPackageVersion` 与 `version-config.json` 的 `last_calculated_version`：
     * **如果不一致**：说明自上一次构建以来，用户手动修改了 `package.json` 里的版本。脚本更新 `version-config.json` 的 `base_version = currentPackageVersion`，并将 `base_commit_count = commitCount`。目标版本 `targetVersion` 即为 `currentPackageVersion`。
     * **如果一致**：说明用户未曾改动。脚本计算 Commit 的增量 `diff = commitCount - base_commit_count`。解析 `base_version` 为 `major.minor.patch`，最终生成的修订号为 `newPatch = patch + diff`。目标版本 `targetVersion` 即为 `${major}.${minor}.${newPatch}`。
4. **写回各配置文件**：
   * 将 `targetVersion` 写回 `version-config.json` 的 `last_calculated_version`。
   * 将 `targetVersion` 写回：
     * `package.json` 中的 `version`
     * `src-tauri/tauri.conf.json` 中的 `version`
     * `src-tauri/Cargo.toml` 中的 `version`

### 3. 构建触发整合
将 `src-tauri/tauri.conf.json` 中的 `beforeBuildCommand` 修改为：
```json
"beforeBuildCommand": "node scripts/bump-version.js && npm run build"
```
这样在每次运行 `pnpm tauri build` 时都会首先执行版本校对和自增。

---

## 接口设计与前端展示

### 1. 后端接口支持
* **数据结构调整**：在 `src-tauri/src/server.rs` 的 `ConfigReq` 结构体中添加只读字段：
  ```rust
  pub app_version: Option<String>
  ```
* **数据接口返回**：在 `/api/config` 路由的 handler `handle_config_get` 中返回：
  ```rust
  app_version: Some(env!("CARGO_PKG_VERSION").to_string())
  ```
  该宏在 Rust 编译阶段会将当前 `Cargo.toml` 里的版本注入，实现向前端的只读输出。

### 2. 前端界面展示
* **状态管理**：在 `src/App.tsx` 中引入 `appVersion` 的 state。
* **数据拉取**：在应用加载 `performInitialSync` 以及进入配置中心 `loadConfig` 时，通过 API 填充 `appVersion`。
* **界面渲染**：在“数据源与系统设置”板块的关闭行为配置下方，加入流体渐变暗黑风格的版本卡片：
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

## 验证方案

1. **本地模拟构建验证**：
   * 提交一次代码，运行 `node scripts/bump-version.js`，验证三个配置文件的版本号是否正确加 1。
   * 手动将 `package.json` 中的版本修改为 `0.2.0`，运行脚本，验证三个配置文件版本号是否被强制重置为 `0.2.0`。
   * 再次提交代码，再次运行脚本，验证版本号是否转为基于 `0.2.0` 继续递增。
2. **前后端接口验证**：
   * 运行 `pnpm dev`（或开发服务），打开设置面板，验证是否能够显示当前正确的版本号。
