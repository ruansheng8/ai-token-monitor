# 智能复盘报告当前窗口全屏展示设计方案 (Fullscreen Report In-Place Design)

本设计方案旨在解决“AI 复盘与治理中心”全屏查看报告无数据、独立弹出窗口退出程序后残留且无法关闭的问题。我们将放弃 Tauri 独立弹出窗口方案，改为在当前窗口通过 Fixed 浮层全屏展示，并支持 ESC 退出和暗黑/亮色主题完美切换，同时清理废弃的 Tauri 后端及前端代码。

---

## 1. 变更点概述

### 1.1 前端变更
1. **`src/App.tsx`**：
   * 移除顶层检测 `detectFullscreenWindow()` 和 `IS_FULLSCREEN_WINDOW` 逻辑。
   * 新增 `fullscreenTaskId` 状态 (`string | null`)，用于控制全屏报告组件的展示。
   * 为 `<ReviewPage>` 传入 `onFullscreenView` 回调，激活全屏查看时设置 `fullscreenTaskId`。
   * 在页面最外层以 `fixed inset-0 z-[9999]` 定位条件渲染 `<FullscreenReportViewer taskId={fullscreenTaskId} onClose={() => setFullscreenTaskId(null)} />`，实现无感知覆盖并保留底层大盘状态。

2. **`src/components/ReviewDrawer.tsx`**：
   * 在 `ReviewPageProps` 接口中新增 `onFullscreenView?: (taskId: string) => void` 属性。
   * 修改“全屏查看 ↗”按钮的点击事件，由调用 Tauri 命令 `open_fullscreen_window` 改为调用 `onFullscreenView(activeTask.id)`。

3. **`src/components/FullscreenReportViewer.tsx`**：
   * 在 `FullscreenReportViewerProps` 接口中新增 `onClose: () => void` 属性。
   * 移除 Tauri 事件监听 (`fullscreen-task-id`) 和 `localStorage` 缓存读取逻辑，直接基于传入的 `taskId` 获取报告数据。
   * 注册 `keydown` 监听器，在捕获到 `Escape` (ESC) 键时调用 `onClose()` 关闭全屏。
   * 将顶栏的“关闭”按钮事件改为触发 `onClose()`。
   * **主题平滑恢复机制**：在 mount 时判断当前系统是否启用暗黑模式（检测 `document.documentElement` 是否包含 `dark` 类）。若有，则暂时移出以展示亮色纯净版报告，并在 unmount 时恢复 `dark` 类，防止破坏底层大盘的暗黑主题状态。

### 1.2 后端变更 (`src-tauri/src/main.rs`)
* 移除 `open_fullscreen_window` command 的定义。
* 在 `tauri::generate_handler!` 中移除对 `open_fullscreen_window` 的注册。

---

## 2. 详细设计与代码改动

### 2.1 Rust 后端清理 (`src-tauri/src/main.rs`)
* 废弃 `open_fullscreen_window` 命令：
  ```rust
  // 移除此段代码
  #[tauri::command]
  fn open_fullscreen_window(app_handle: tauri::AppHandle, task_id: String) -> Result<(), String> {
      ...
  }
  ```
* 移除注册：
  ```rust
  // 修改前
  .invoke_handler(tauri::generate_handler![exit_app, hide_window, open_fullscreen_window])
  // 修改后
  .invoke_handler(tauri::generate_handler![exit_app, hide_window])
  ```

### 2.2 前端 `FullscreenReportViewer.tsx` 修改
* **Props 接口更新**：
  ```typescript
  interface FullscreenReportViewerProps {
    taskId: string;
    onClose: () => void;
  }
  ```
* **键盘 ESC 退出与主题恢复实现**：
  ```typescript
  export function FullscreenReportViewer({ taskId, onClose }: FullscreenReportViewerProps) {
    const [task, setTask] = useState<ReviewTask | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [copied, setCopied] = useState(false);

    // 主题切换与平滑恢复
    useEffect(() => {
      const isDark = document.documentElement.classList.contains('dark');
      if (isDark) {
        document.documentElement.classList.remove('dark');
      }
      return () => {
        if (isDark) {
          document.documentElement.classList.add('dark');
        }
      };
    }, []);

    // 监听 ESC 键退出
    useEffect(() => {
      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
          onClose();
        }
      };
      window.addEventListener('keydown', handleKeyDown);
      return () => {
        window.removeEventListener('keydown', handleKeyDown);
      };
    }, [onClose]);

    // 报告数据直接根据 props.taskId 进行加载
    useEffect(() => {
      if (!taskId) return;
      let cancelled = false;
      setLoading(true);
      setError(null);

      const loadTask = async () => {
        try {
          const res = await fetch(apiUrl(`/review/tasks/${taskId}`));
          if (!res.ok) {
            throw new Error(`加载报告详情失败 (状态码: ${res.status})`);
          }
          const data = await readJsonResponse<ReviewTask>(res);
          if (!cancelled) setTask(data);
        } catch (err: any) {
          console.error('加载全屏报告错误:', err);
          if (!cancelled) setError(err.message || '加载报告时发生未知错误');
        } finally {
          if (!cancelled) setLoading(false);
        }
      };

      loadTask();
      return () => { cancelled = true; };
    }, [taskId]);

    // 顶栏关闭按钮修改
    // onClick={onClose}
    ...
  }
  ```

---

## 3. 验证方案

### 3.1 自动化编译与类型检查
* 运行前端类型检查：`npx tsc -b --noEmit`
* 运行前端 Lint 检查：`npm run lint`
* 运行 Rust 后端编译：`cd src-tauri; cargo check`

### 3.2 手动功能测试
1. 打开“AI复盘与治理中心”，在“历史复盘记录”中选择一份报告，点击进入详情页。
2. 点击“全屏查看 ↗”按钮，验证页面是否瞬间无缝在当前窗口全屏展示报告，且报告包含正确的文本和数据（不再是空白）。
3. 验证键盘按下 `ESC` 键是否能立即退出全屏，并完美回到刚才的大盘复盘详情页，大盘之前的滚动位置和选择等状态应完全被保留。
4. 验证点击全屏报告顶栏的“关闭”按钮是否能正常退出全屏。
5. 验证如果是暗黑模式下点击全屏查看，全屏报告展示为纯白背景，退出全屏后大盘自动恢复为原有的暗黑模式背景。
6. 验证完全关闭并退出 Token Insight 桌面客户端，不再产生残留的未关闭窗口进程。
