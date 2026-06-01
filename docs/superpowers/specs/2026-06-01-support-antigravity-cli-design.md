# 2026-06-01 支持 Antigravity CLI (agy.exe) 设计规范

随着 Gemini 2.0 的升级，旧版 `gemini.exe` 命令行客户端即将停服，全面被新版 `agy.exe`（Antigravity CLI）替代。本设计规范旨在为 Token Insight 系统新增对新版 `agy` 的原生支持。

## 目标与改动范围

1. **后端 (Rust)**：
   - 增加探测 `agy` 可执行文件。
   - 在 CLI 显示名称映射中将 `gemini` 标记为“旧版”，新增 `agy` 显示为“Antigravity CLI (新版)”。
   - 适配 `agy` 的执行参数（新版通过标准输入接收 Prompt，不附带 `-p` 参数）。

2. **前端 (React)**：
   - 增加 `agy` 的 CLI 映射名称。
   - 更新未安装或未登录时的引导提示命令（安装：`npm install -g @google/agy`，登录：`agy login`）。
   - 更新无可用 CLI 时的安全警告卡片，加入 `agy` 说明。

## 详细设计

### 后端 `src-tauri/src/review.rs`

#### 1. 扩展 CLI 探测
在 `probe_cli` 中的 `candidate_bins` 列表中追加 `"agy"`。
```rust
let candidate_bins = ["claude", "codex", "gemini", "agy"];
```

#### 2. 显示名称映射
在 `get_cli_display_name` 函数中：
```rust
fn get_cli_display_name(bin: &str) -> &'static str {
    match bin {
        "claude" => "Claude Code",
        "codex" => "Codex CLI",
        "gemini" => "Gemini CLI (旧版)",
        "agy" => "Antigravity CLI (新版)",
        _ => "AI CLI",
    }
}
```

#### 3. 进程拉起参数
在 `run_cli_task_background` 中，针对 `agy` 的启动参数做特殊判断：
```rust
    let mut cmd = Command::new(&exe_path);
    if cli_name.starts_with("claude") {
        cmd.args([
            "-p",
            "--output-format",
            "text",
            "--permission-mode",
            "bypassPermissions",
        ]);
    } else if cli_name.starts_with("codex") {
        cmd.args(["--full-auto", "-q"]);
    } else if cli_name.starts_with("agy") {
        // agy.exe 新版 CLI 不带 -p 参数，通过 stdin 接收输入
    } else {
        cmd.arg("-p");
    }
```

### 前端 `src/components/ReviewDrawer.tsx`

#### 1. 前端名称映射
在 `getCliDisplayName` 函数中：
```typescript
  const getCliDisplayName = (name: string) => {
    switch (name) {
      case 'claude':
        return 'Claude Code';
      case 'codex':
        return 'Codex CLI';
      case 'gemini':
        return 'Gemini CLI (旧版)';
      case 'agy':
        return 'Antigravity CLI (新版)';
      default:
        return 'AI CLI';
    }
  };
```

#### 2. 引导文案更新
在未安装/未登录 CLI 时的引导命令行：
```typescript
activeTask.cli_name === 'claude' 
  ? 'npm install -g @anthropic-ai/claude-code' 
  : activeTask.cli_name === 'agy'
    ? 'npm install -g @google/agy'
    : activeTask.cli_name === 'gemini'
      ? 'npm install -g @google/gemini-cli'
      : 'npm install -g codex-cli'
```
```typescript
activeTask.cli_name === 'claude' 
  ? 'claude login' 
  : activeTask.cli_name === 'agy'
    ? 'agy login'
    : activeTask.cli_name === 'gemini'
      ? 'gemini login'
      : 'codex login'
```

## 验证与测试方案

### 1. Mock 探测测试
在 PATH 环境的任一目录下放置一个 `agy.cmd` 脚本，内容为：
```cmd
@echo off
if "%1"=="--version" (
    echo agy version 1.0.0
)
```
并在前端点击“重新检测”按钮，验证：
- 是否能成功探测到 `agy`，并且正确显示版本号 `(1.0.0)`。
- 下拉框中是否出现 `Antigravity CLI (新版)`。

### 2. Mock 执行测试
将 `agy.cmd` 替换为如下内容以模拟 AI 返回：
```cmd
@echo off
echo [Stage] 正在分析大盘数据...
echo 诊断结果：
echo 1. 您的 Token 消耗整体正常。
echo 2. 建议针对特定 IDE 进行用量调优。
```
点击“开始智能效能分析报告”按钮，验证：
- 任务能够正常流式接收 `agy.cmd` 的输出并渲染在报告详情中。
- 日志框正常打印执行控制台输出。
