# 简化版本管理实现计划 (Simplify Versioning Plan)

## 拟更改文件

### 配置文件修改

#### [MODIFY] [tauri.conf.json](file:///d:/VibeCoding/ai-token-monitor/src-tauri/tauri.conf.json)
* 移除第 3 行的 `"version": "0.2.10",`。
* 将第 8 行的 `"beforeBuildCommand": "node scripts/bump-version.cjs && npm run build"` 修改为 `"beforeBuildCommand": "npm run build"`。

#### [MODIFY] [package.json](file:///d:/VibeCoding/ai-token-monitor/package.json)
* 将第 4 行的 `"version": "0.2.10",` 修改为 `"version": "1.0.0",`。

### 清理废弃文件

#### [DELETE] [bump-version.cjs](file:///d:/VibeCoding/ai-token-monitor/scripts/bump-version.cjs)
* 彻底删除此版本自增脚本。

#### [DELETE] [version-config.json](file:///d:/VibeCoding/ai-token-monitor/version-config.json)
* 彻底删除该版本跟踪配置文件。

---

## 验证计划

1. **静态代码检查**：
   * 执行 `npx tsc -b --noEmit` 验证前端类型。
   * 执行 `npm run lint` 验证前端代码规范。
2. **Rust 后端编译**：
   * 在 `src-tauri` 目录下运行 `cargo check` 验证编译。
3. **前端预览与启动验证**：
   * 运行 `pnpm build` 进行完整的前端打包，并在 Tauri 编译后启动软件以确认版本号显示完全正确（来自 `Cargo.toml`）。
