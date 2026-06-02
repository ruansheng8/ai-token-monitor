# Token Insight - GitHub Actions 自动构建与发布设计规格书

## 1. 目标与背景
实现当开发人员在 GitHub 仓库推送类似 `v*` 的 Git Tag（例如 `v1.0.1`）时，自动触发 GitHub Actions 编译 React 前端和 Rust 后端，并仅打包出 Windows 平台的 `.msi` 格式安装包，最终自动创建 GitHub Release 并将该安装包挂载到 Release 资产列表中提供给用户下载。

## 2. 关键设计

### 2.1 触发器 (Trigger)
* 触发事件：推送 Git Tag。
* 过滤规则：`v*`（以 `v` 开头，如 `v1.0.1`, `v2.0.0-rc1`）。

### 2.2 构建环境与平台 (Build Environment)
* 操作系统：`windows-latest`
* Node.js：v20
* Rust：最新稳定版 (stable)
* 包管理工具：`npm` (对应项目中的 `package-lock.json`)

### 2.3 仅生成 MSI 格式 (MSI Only Build)
在 Tauri v2 中，虽然本地配置了 `["nsis", "msi"]`，但在 GitHub Actions 中我们可以通过覆盖参数只构建 msi。
* 构建命令参数：`--bundles msi`
* 在 `tauri-apps/tauri-action` 中配置为 `args: --bundles msi`。

### 2.4 GitHub Release 权限与生成
* 使用 `tauri-apps/tauri-action` 完成打包和 Release 创建。
* 权限配置：显式在工作流中声明 `permissions: contents: write`，确保 Actions 有权向仓库写入 Release 资产。
* 运行环境要求：用户需要在 GitHub 仓库 Settings 中将 `Workflow permissions` 更改为 `Read and write permissions`。

## 3. 新增文件
* [release.yml](file:///d:/VibeCoding/ai-token-monitor/.github/workflows/release.yml): GitHub Actions 工作流文件。

## 4. 验证方案
* 运行静态检查（如工作流语法）。
* 建议用户在配置完成后推送一个测试 Tag（如 `v0.0.0-test`）来验证整个自动构建和上传流程。
