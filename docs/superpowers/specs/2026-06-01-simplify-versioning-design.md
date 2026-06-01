# 简化版本管理设计说明书 (Simplify Versioning Design)

本项目旨在简化多端版本号管理，将 Rust 后端的 `Cargo.toml` 作为整个项目版本号的唯一源，避免每次构建发行版本时需要在多个配置文件中手动或自动修改版本号，从而保持 Git 状态的干净。

## 方案设计

### 1. 核心设计思路
* **唯一版本源**：以 `src-tauri/Cargo.toml` 中的 `version` 字段为整个软件版本的唯一手动修改源。
* **取消构建时自增**：完全废除基于 Git Commit 数量自动修改文件的 `bump-version.cjs` 脚本和 `version-config.json` 配置文件。
* **固定前端版本**：将根目录下的 `package.json` 中的 `version` 字段永久固定为 `"1.0.0"`，不再由脚本自动同步或修改，减少 Git 变更。
* **利用 Tauri v2 的回退机制**：移除 `src-tauri/tauri.conf.json` 中的 `version` 字段。在 Tauri v2 中，如果配置文件省略 `version` 字段，构建打包系统会自动读取并使用 `src-tauri/Cargo.toml` 中的版本号。
* **前后端数据连通**：
  * **后端**：在编译时使用 `env!("CARGO_PKG_VERSION")` 获取 `Cargo.toml` 的版本并注入配置结构体。
  * **前端**：加载时通过 `/api/config` 接口拉取配置数据（包含后端动态获取的版本号）并完成界面渲染。

---

## 拟修改/删除的文件列表

### 1. [MODIFY] [tauri.conf.json](file:///d:/VibeCoding/ai-token-monitor/src-tauri/tauri.conf.json)
* 移除顶层的 `"version"` 属性，使 Tauri 自动回退至 Cargo.toml 的版本。
* 将 `"build.beforeBuildCommand"` 从 `"node scripts/bump-version.cjs && npm run build"` 简化为 `"npm run build"`。

### 2. [MODIFY] [package.json](file:///d:/VibeCoding/ai-token-monitor/package.json)
* 将 `"version"` 修改并固定为 `"1.0.0"`。之后无需再手动或自动修改该文件。

### 3. [DELETE] [bump-version.cjs](file:///d:/VibeCoding/ai-token-monitor/scripts/bump-version.cjs)
* 彻底删除此辅助脚本。

### 4. [DELETE] [version-config.json](file:///d:/VibeCoding/ai-token-monitor/version-config.json)
* 彻底删除该版本跟踪配置文件。

---

## 验证方案

1. **构建流程验证**：
   * 运行 `pnpm build` 和 `cd src-tauri; cargo check`，确保编译正常。
2. **前后端接口验证**：
   * 启动程序后，在设置面板中检查版本号是否正确显示为 `Cargo.toml` 中配置的版本号。
