# AI 复盘与治理中心移除打印与导出 PDF 功能设计规范

## 1. 背景与目标
为了精简“AI 复盘与治理中心”的导出与分享体验，避免由于窗口重构后默认打印版式可能带来的格式兼容问题，决定完全移除该模块下的“打印/导出 PDF”功能。

---

## 2. 修改范围

### 2.1 FullscreenReportViewer.tsx
- **位置**：[FullscreenReportViewer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/FullscreenReportViewer.tsx)
- **变动**：
  1. 移除 `lucide-react` 中导入的 `Printer` 图标。
  2. 移除 `handlePrint` 事件处理函数（`const handlePrint = () => window.print();`）。
  3. 从顶栏的按钮操作组中删除 `打印/PDF` 按钮。
  4. 完全删除底部的 `<footer>` 节点及其所包含的关于 Ctrl+P/打印的提示信息。

### 2.2 ReviewDrawer.tsx
- **位置**：[ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx)
- **变动**：
  1. 移除控制栏中当 `activeTask.status === 'succeeded'` 时展示的 `🖨️ 打印/导出 PDF` 按钮。

---

## 3. 保留与兼容性设计
- **`@media print` 与 `.no-print`**：
  在 [index.css](file:///d:/VibeCoding/ai-token-monitor/src/index.css) 中的 `@media print` 打印媒体查询样式，以及在 [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx) 中用于 html2canvas 财务报表图片导出的 `.no-print` 排除规则将**全部保留**。这些机制是系统生成财务报表图片的关键依赖，不应随着此复盘页面按钮的删除而被移除。

---

## 4. 验证方案
- **人工验证**：
  1. 打开 AI 复盘与治理中心，点击进入任意已完成的复盘报告详情抽屉，确认右上方没有 `🖨️ 打印/导出 PDF` 按钮。
  2. 点击“全屏查看”进入报告的全屏阅读模式，确认顶栏没有 `打印/PDF` 按钮，且底部没有关于 Ctrl+P 的页脚提示。
  3. 尝试在监控大盘点击“保存财务报表图片”，验证生成的 PNG 图片导出功能仍然完好，且不含大盘上的操作按钮。
