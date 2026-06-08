# Token Insight 自动升级与关于页面设计规范

本文档定义了本地 AI 用量统计仪表盘（Token Insight）的自动升级与“关于”界面的详细设计，涵盖后端 Tauri 2.0 插件集成、前端状态管理上下文以及 UI 层的玻璃拟态设计与兜底交互。

---

## 1. 架构概述

为了在保证安全与稳定的前提下为用户提供极致的更新体验，我们采用 **混合双轨制（双通道）升级架构**：
* **自动通道**：使用 Tauri 2.0 官方提供的 `tauri-plugin-updater` 与 `tauri-plugin-process` 插件，在应用内部实现检查更新、一键下载、自动安装和热重启。
* **兜底通道**：若自动通道遭遇证书签名缺失（Portable 便携版或未配置签名的本地构建）、网络超时等异常，系统将优雅捕获并降级为“调用系统默认浏览器打开 GitHub Releases 页面”的手动下载升级形式。

---

## 2. 后端设计与依赖配置

### 2.1 Cargo 依赖调整
在 `src-tauri/Cargo.toml` 的 `[dependencies]` 区域添加如下依赖：
```toml
tauri-plugin-process = "2"
tauri-plugin-updater = "2"
```

### 2.2 插件注册
在 `src-tauri/src/main.rs` 中，在 `tauri::Builder` 链式调用中注册这两个插件的 Rust 端初始化函数：
```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // ...
        }))
```

### 2.3 自动更新配置
在 `src-tauri/tauri.conf.json` 中的 `plugins` 下追加 `updater` 配置：
```json
  "plugins": {
    "updater": {
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEM4MDI4QzlBNTczOTI4RTMKUldUaktEbFhtb3dDeUM5US9kT0FmdGR5Ti9vQzcwa2dTMlpibDVDUmQ2M0VGTzVOWnd0SGpFVlEK",
      "endpoints": [
        "https://github.com/ruansheng8/token-insight/releases/latest/download/latest.json"
      ]
    }
  }
```
* **注意**：`pubkey` 为用于校验更新包签名的公钥。在本地调试（Cargo Dev）下不会限制签名，在生产 Release 打包时必须使用 `tauri-action` 传入 `TAURI_SIGNING_PRIVATE_KEY` 环境变量来自动对 MSI 安装包进行签名并生成配套的 `latest.json`。

---

## 3. 前端设计与逻辑集成

### 3.1 Npm 依赖安装
在项目根目录下安装对应的 Tauri 2.0 插件前端 API 支持库：
```bash
npm install @tauri-apps/plugin-updater@2.0.0 @tauri-apps/plugin-process@2.0.0
```

### 3.2 动态更新服务层 (`src/lib/updater.ts`)
为了确保代码的健壮性，使用动态导入来避免在非桌面环境（如 Vite Web 浏览器端调试）下因为插件缺失引发的构建错误。
```typescript
import { getVersion } from "@tauri-apps/api/app";

// 前端对应 Updater 状态机类型定义
export type UpdaterPhase =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "installing"
  | "restarting"
  | "upToDate"
  | "error";

export interface UpdateProgressEvent {
  event: "Started" | "Progress" | "Finished";
  total?: number;
  downloaded?: number;
}

export async function getCurrentVersion(): Promise<string> {
  try {
    return await getVersion();
  } catch {
    return "0.1.0";
  }
}

export async function checkForUpdate() {
  // 动态导入，防范 Web 开发期报错
  const { check } = await import("@tauri-apps/plugin-updater");
  const update = await check({ timeout: 30000 });
  return update;
}

export async function relaunchApp() {
  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}
```

### 3.3 全局更新状态上下文 (`src/contexts/UpdateContext.tsx`)
为应用提供一个全局状态管理环境，方便应用在启动时进行静默自动升级检查，同时为“关于”界面提供交互接口：
* **核心提供状态与方法**：
  * `hasUpdate` (是否有更新)
  * `updateInfo` (新版本信息，如版本号、说明等)
  * `isChecking` (是否在检测更新)
  * `isDownloading` (是否在下载中)
  * `downloadProgress` (下载进度，0 - 100)
  * `error` (错误文本)
  * `phase` (当前更新所处的阶段: `UpdaterPhase`)
  * `checkUpdate()` (触发手动检查)
  * `downloadAndInstall()` (触发下载并安装)
* **静默检查机制**：
  在应用挂载 1 秒后自动延时执行一次静默检测。若有新版，将在内部标记状态，并在“关于”Tab上可以通过醒目徽标提示用户。

---

## 4. UI 界面设计与交互

配置大厅增加为 4 个标签页：
1. `🖥️ 数据源与系统设置`
2. `💵 汇率与模型费率`
3. `⚡ 维护与瘦身`
4. `ℹ️ 关于系统` (新增 Tab，ID 为 `'about'`)

### 4.1 移除原“系统版本”区域
从 `source`（数据源与系统设置）标签页中彻底删除原有的“系统版本 (System Version)”表单区域，减少主设置页面的冗余。

### 4.2 “关于系统” 页面布局与交互逻辑
采用流体渐变玻璃拟态暗黑风格。
1. **应用品牌区**：
   * 居中展示发光机器人图标 🤖（带流体渐变描边动画）。
   * 醒目的应用名称 `Token Insight` 以及当前版本 Badge（如 `v1.0.7`）。
2. **GitHub 卡片链接**：
   * 提供“项目 GitHub 仓库”卡片，点击调用默认浏览器直接打开 GitHub 地址。
3. **升级控制台区域**：
   * **未检测到更新时**：展示“检查更新”按钮。
   * **检测中状态 (`checking`)**：按钮进入 Loading 且不可点击。
   * **已是最新状态 (`upToDate`)**：按钮重置，且在旁边显示绿色勾选标志：`✓ 已是最新版本`。
   * **有新版本可用 (`available`)**：
     * 显示新版本卡片：“发现新版本 `vX.Y.Z`”。
     * 提供“立即下载并安装”按钮。
   * **下载与安装中 (`downloading`)**：
     * 显示下载进度百分比。
     * 提供一个美观的进度条组件展示下载进度。
   * **安装完成，等待重启 (`restarting`)**：
     * 提供“更新已就绪，立即重启生效”按钮，点击调用 `relaunchApp()` 重启。
   * **出错状态 (`error`)**：
     * 显示红色警告标志及友好报错信息。
     * 提供一个降级兜底按钮：`手动去 GitHub 下载更新`。
