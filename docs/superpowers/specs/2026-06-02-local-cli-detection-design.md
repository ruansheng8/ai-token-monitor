# 本机 CLI 识别与测试检测规范设计 (迁移自 open-design)

本设计文档旨在将 `open-design` 中已验证、成熟的“本机 CLI 诊断与检测逻辑”迁移至 `token-insight` 的 AI 复盘与治理中心中，重点解决当前分析引擎检测不到、Ghost 幽灵 CLI（已卸载但残留环境变量/代理软链）以及 GUI 进程中 PATH 被裁剪导致的引擎诊断崩溃问题。

## 1. 痛点分析与改进目标

1. **GUI 进程 PATH 丢失问题**：在 Windows / macOS 上通过桌面快捷方式或 IDE 插件拉起的 Tauri 后端，系统环境变量 `PATH` 常常缺少用户在交互式终端中配置的 Node 版本管理器（如 fnm、nvm）、Cargo 或 npm 全局 bin 路径。导致原本安装好的 CLI（如 Claude Code 或 agy）无法被 `which` 命中。
   * **解决方案**：主动扫描用户 well-known 工具链目录，并在检索可执行文件时将其与 `PATH` 进行合并去重。
2. **幽灵 CLI (Ghost Shim) 误报问题**：很多全局 Node 包被卸载或升级后，在系统 `npm` 或 `fnm` 全局目录下仍会残留 `.cmd` / `.bat` / 软链文件。目前 `token-insight` 仅凭 `which` 命中或文件存在就将 available 置为 `true`。结果运行分析时子进程执行报错崩溃。
   * **解决方案**：引入 **Not Invocable** 判定分类。利用 `--version` 的运行状态判定可执行程序是否可启动。
3. **故障隔离缺陷**：单个 CLI 引擎在探测时发生阻塞（如 `--version` 挂起）可能导致整个检测大盘崩溃或卡死。
   * **解决方案**：引入 `tokio::time::timeout` 异步超时处理以及 `try-catch` (Rust 中为 `Result` 隔离) 保护，确保单个引擎检测挂起不波及其他引擎。

## 2. 详细设计

### 2.1 目录发现算法

在后端 Rust 中新增 `get_well_known_toolchain_dirs()`，针对不同操作系统，生成以下目录列表：

```rust
// 1. Cargo 二进制目录: ~/.cargo/bin
// 2. Bun 二进制目录: ~/.bun/bin
// 3. Volta 二进制目录: ~/.volta/bin
// 4. NPM 全局默认目录: ~/.npm-global/bin, ~/.npm-packages/bin
// 5. 本地标准目录: ~/.local/bin
// 6. fnm 与 nvm 版本管理器的多版本 node 安装目录 (遍历其子目录下的 bin)
//    - Windows: LOCALAPPDATA/fnm/node-versions/*/installation/bin
//    - Windows: APPDATA/npm
//    - Unix: ~/.nvm/versions/node/*/bin
//    - Unix: ~/.local/share/fnm/node-versions/*/installation/bin
//    - Unix: ~/.fnm/node-versions/*/installation/bin
// 7. 环境变量 NPM_CONFIG_PREFIX 指定的路径/bin
// 8. 默认系统 bin (Unix 专享): /opt/homebrew/bin, /usr/local/bin
```

### 2.2 Not Invocable 判定分类

在运行探测探针时，执行：
`Command::new(exe_path).arg("--version")`

我们将其执行状态进行以下划分：

| 执行表现 | 分类结果 | 原因分析 |
| :--- | :--- | :--- |
| 返回 `ErrorKind::NotFound` 或 `ErrorKind::PermissionDenied` | **Not Invocable** (`available: false`) | 底层可执行文件缺失、软链接断开或无权限 |
| 进程退出状态码为 `126` 或 `127` | **Not Invocable** (`available: false`) | CMD / NPM 代理残留（shim 指向的 target 已不存在） |
| 输出内容中包含 "系统找不到指定的路径" 或 "找不到文件" | **Not Invocable** (`available: false`) | Windows 下残留的 broken .cmd 脚本报错 |
| 运行超时 (timeout 5s) | **Invocable but version null** (`available: true`, `version: None`) | 进程虽然挂起，但它被成功拉起并执行了 |
| 运行正常，但返回非 0 码，或输出空内容 | **Invocable but version null** (`available: true`, `version: None`) | 命令可用，但不兼容 `--version` 参数 |
| 运行正常，返回 0 码且有版本输出 | **Invocable** (`available: true`, `version: "xxx"`) | 完美可用，解析首行作为版本 |

## 3. 验证方案

1. **编译检查**：确保后端 Rust 经过 `cargo check` 无任何编译警报与错误。
2. **测试幽灵代理**：在测试机上人工创建一个 Broken Shim 代理（创建一个 `claude.cmd` 写入 `@"%~dp0\node.exe" @"%~dp0\node_modules\claude\index.js" %*`，但不提供实际的 `node.exe`），确认检测结果将其标记为 `available: false`。
3. **真实环境测试**：
   - 验证 `agy.exe` (新版 Antigravity CLI) 可被正常探测。
   - 验证 `claude-code` 可以从 nvm/fnm 安装的深层路径中被自动发现并提取出正确的版本号。
