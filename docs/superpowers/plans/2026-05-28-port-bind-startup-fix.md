# Port Bind Startup Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent the desktop app from reporting a successful backend start or launching hot-sync work when port 19362 is already occupied, and make window close exit the process so old instances do not keep the port.

**Architecture:** Add a tiny pure decision function in `src-tauri/src/main.rs` so the bind-success/bind-failure behavior can be tested without opening sockets. Reorder the Axum startup block so `TcpListener::bind` happens before hot-sync startup and success logging. Change the close handler from hide-on-close to process exit.

**Tech Stack:** Rust 2021, Tauri v2, Tokio, Axum, Cargo tests.

---

### Task 1: Add Startup Decision Test

**Files:**
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write the failing test**

Append this test module to `src-tauri/src/main.rs` after `main()`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_failure_does_not_start_watcher_or_log_success() {
        let decision = backend_startup_decision(false);

        assert!(!decision.start_watcher);
        assert!(!decision.log_success);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run from `src-tauri`:

```powershell
cargo test bind_failure_does_not_start_watcher_or_log_success
```

Expected: FAIL with an error like `cannot find function backend_startup_decision in this scope`.

### Task 2: Implement Startup Decision

**Files:**
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Add the minimal decision type and function**

Add this code above `fn start_folder_watcher()`:

```rust
#[derive(Debug, PartialEq, Eq)]
struct BackendStartupDecision {
    start_watcher: bool,
    log_success: bool,
}

fn backend_startup_decision(bind_succeeded: bool) -> BackendStartupDecision {
    BackendStartupDecision {
        start_watcher: bind_succeeded,
        log_success: bind_succeeded,
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

Run from `src-tauri`:

```powershell
cargo test bind_failure_does_not_start_watcher_or_log_success
```

Expected: PASS.

### Task 3: Add Bind Success Test

**Files:**
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Add the second test**

Add this test inside the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn bind_success_starts_watcher_and_logs_success() {
    let decision = backend_startup_decision(true);

    assert!(decision.start_watcher);
    assert!(decision.log_success);
}
```

- [ ] **Step 2: Run test to verify it passes**

Run from `src-tauri`:

```powershell
cargo test backend_startup_decision
```

Expected: both startup decision tests PASS.

### Task 4: Reorder Backend Startup

**Files:**
- Modify: `src-tauri/src/main.rs:124-142`

- [ ] **Step 1: Move bind before watcher and success logging**

Replace the current block that starts with `// 启动文件监测与热同步服务` and ends before `if let Err(e) = axum::serve(listener, app).await {` with this code:

```rust
            // 本地桌面版绑定本地回环地址 127.0.0.1
            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Error binding to port {}: {}", port, e);
                    return;
                }
            };

            let decision = backend_startup_decision(true);
            if decision.start_watcher {
                start_folder_watcher();
            }
            if decision.log_success {
                println!("\n==================================================");
                println!(" AI Token Monitor 极速增量缓存用量统计后台服务已成功启动！");
                println!(" 接口地址: http://127.0.0.1:{}", port);
                println!("==================================================\n");
            }
```

- [ ] **Step 2: Run focused Rust tests**

Run from `src-tauri`:

```powershell
cargo test backend_startup_decision
```

Expected: PASS.

### Task 5: Make Window Close Exit

**Files:**
- Modify: `src-tauri/src/main.rs:179-185`

- [ ] **Step 1: Change the close handler**

Replace the current `on_window_event` close handler body with:

```rust
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                std::process::exit(0);
            }
        })
```

- [ ] **Step 2: Run Rust tests**

Run from `src-tauri`:

```powershell
cargo test backend_startup_decision
```

Expected: PASS.

### Task 6: Verify Build

**Files:**
- No code changes.

- [ ] **Step 1: Run full Rust test suite**

Run from `src-tauri`:

```powershell
cargo test
```

Expected: PASS.

- [ ] **Step 2: Build the frontend**

Run from repo root:

```powershell
npm run build
```

Expected: build completes successfully.

- [ ] **Step 3: Report verification result**

Report whether tests and build passed. If an old `ai-token-monitor.exe` still occupies `19362`, tell the user the code is fixed but the already-running process must be closed once before the fixed binary can bind the port.
