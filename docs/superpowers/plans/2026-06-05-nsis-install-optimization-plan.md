# NSIS 安装程序性能优化实现计划

优化 Tauri 打包的 NSIS 安装程序，通过配置跳过 WebView2 自动联网检测和优化压缩算法，消除安装时的卡顿，实现秒级安装。

## User Review Required

> [!IMPORTANT]
> 调整 `webviewInstallMode` 为 `skip` 后，如果目标用户的 Windows 系统（尤其是极其精简的老旧 Win10 系统）未安装 WebView2，本程序启动时会报错。
> 但现代主流 Windows 10（2004及以后版本）与 Windows 11 已默认内置 WebView2，绝大多数机器不受影响。

## Open Questions

无。

## Proposed Changes

---

### Tauri Build Configuration

修改 Tauri 打包配置，为 Windows NSIS 安装程序增加解压与检测优化。

#### [MODIFY] [tauri.conf.json](file:///d:/VibeCoding/ai-token-monitor/src-tauri/tauri.conf.json)
- 在 `bundle.windows.nsis` 下添加：
  - `"webviewInstallMode": "skip"` (跳过在线 WebView2 检测)
  - `"compression": "zlib"` (优化解压缩速度和杀软扫描兼容性)

---

## Verification Plan

### Automated Tests
无。

### Manual Verification
1. **构建安装包**：
   在命令行运行以下命令，完成前端构建及 Tauri 的 NSIS 打包：
   ```powershell
   pnpm build
   cd src-tauri
   cargo tauri build --bundles nsis
   ```
2. **安装程序测试**：
   - 运行位于 `src-tauri/target/release/bundle/nsis/` 下的安装程序（`.exe` 格式）。
   - 检查进度条是否能直接通过，不再卡在 `状态：`。
   - 安装完成后，在测试机上启动运行 `token-insight` 软件，确保其可以正常打开并呈现 UI 界面。
