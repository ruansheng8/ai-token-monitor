# Token Insight - GitHub Actions 自动发布实现计划

本计划旨在实现当推送符合特定规则的 Git Tag（如 `v1.0.1`）时，自动通过 GitHub Actions 构建 Windows 平台的 `.msi` 安装包并生成 GitHub Release。

## 用户确认事项
* **GitHub 仓库权限设置**：为使构建脚本有权自动创建 Release 并上传打包好的 `.msi` 产物，您需要在 GitHub 仓库中将 **Settings -> Actions -> General -> Workflow permissions** 修改为 **Read and write permissions**。

## 待变更文件

### [NEW] [.github/workflows/release.yml](file:///d:/VibeCoding/ai-token-monitor/.github/workflows/release.yml)
创建 GitHub Actions 工作流配置文件，核心逻辑如下：
1. 监听 `tags: ['v*']` 推送事件。
2. 在 `windows-latest` 虚拟机上拉起构建任务。
3. 配置 Node.js 并自动缓存 `npm` 依赖。
4. 安装 Rust 工具链并使用 `swatinem/rust-cache` 缓存编译产物。
5. 还原前端依赖：`npm ci`。
6. 使用 `tauri-apps/tauri-action` 进行打包发布，并传入 `args: --bundles msi`。

---

## 验证计划

### 1. 语法与配置校验
* 检查 `.github/workflows/release.yml` 的 YAML 语法是否合法。

### 2. 端到端流程测试 (需手动执行)
* 在完成文件创建并推送后，打一个临时标签 `v0.0.0-test` 并推送：
  ```bash
  git tag v0.0.0-test
  git push origin v0.0.0-test
  ```
* 登录 GitHub 仓库的 **Actions** 页面观察构建过程是否成功，并验证 **Releases** 页面是否生成了带有 `.msi` 文件的 Release。
* 验证完毕后，删除该临时标签：
  ```bash
  git tag -d v0.0.0-test
  git push origin :refs/tags/v0.0.0-test
  ```
