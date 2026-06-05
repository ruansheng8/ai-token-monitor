# NSIS 安装程序性能优化设计规约

优化 Tauri 打包的 NSIS 安装程序在 Windows 下安装时卡顿、缓慢的问题。

## 背景与问题分析

用户在 Windows 上运行 `token-insight` 安装程序时，安装界面经常长时间卡在 `状态：` 进度条初始位置。

### 根本原因
1. **WebView2 在线引导程序阻塞**：Tauri 默认的 `webviewInstallMode` 为 `downloadBootstrapper`。安装程序在释放文件前，会尝试静默运行微软的 WebView2 在线引导程序。在网络不佳、无法连接微软服务器或存在安全策略拦截的环境下，该引导程序会同步阻塞等待，导致安装程序看起来卡死。
2. **LZMA 压缩与杀软扫描延迟**：NSIS 默认采用 LZMA 压缩，解压时 CPU 负载高，且容易引起 Windows Defender 等安全软件的实时启发式扫描，导致解包过程产生可感知的停顿。

## 设计目标

- **消除联网检测开销**：完全避免在安装时联网请求微软 WebView2 引导程序。
- **提升解包速度与兼容性**：使用更轻量、对安全软件更友好的压缩格式，确保解包在一瞬间完成。
- **极速安装体验**：使得整个安装过程在 1-2 秒内完成。

## 优化方案

在 `src-tauri/tauri.conf.json` 的 `bundle.windows.nsis` 配置项中：

1. **设置 `webviewInstallMode` 为 `"skip"`**：
   - 跳过 WebView2 运行时的检测与在线下载安装。
   - 现代 Windows 10/11 已内置 WebView2，且大多数日常开发机和办公机已安装过 Edge/WebView2，跳过检测完全可行。
2. **设置 `compression` 为 `"zlib"`**：
   - 弃用对 CPU 消耗高且容易触发杀软深层扫描的 `"lzma"`。
   - 改用结构简单、解压效率高且内存占用小的 `"zlib"`。

### 配置变更示例

```diff
       "nsis": {
         "languages": [
           "SimpChinese"
         ],
-        "displayLanguageSelector": false
+        "displayLanguageSelector": false,
+        "webviewInstallMode": "skip",
+        "compression": "zlib"
       }
```

## 验证计划

1. **配置验证**：确认 `tauri.conf.json` 格式正确且符合 Tauri 2.x Schema。
2. **构建验证**：
   - 执行构建命令验证打包成功：
     ```powershell
     pnpm build
     cd src-tauri
     cargo tauri build --bundles nsis
     ```
3. **安装测试**：
   - 在测试机上运行生成的安装包，观察是否能秒装通过，且安装后程序能正常运行。
