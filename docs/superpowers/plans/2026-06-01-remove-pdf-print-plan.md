# 移除 AI 复盘打印与导出 PDF 功能实现计划

## 1. 目标描述
从 `AI 复盘与治理中心` 前端界面中移除所有直接提供给用户的 `打印/导出 PDF` 操作入口。

---

## 2. 拟修改内容

### 前端组件

#### [MODIFY] [FullscreenReportViewer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/FullscreenReportViewer.tsx)
- 移除 `Printer` 图标的导入。
- 移除 `handlePrint` 函数（`const handlePrint = () => window.print();`）。
- 移除顶栏操作按钮组中的 `打印/PDF` 按钮。
- 完全删除底部的 `<footer>` 节点（包含 Ctrl+P 的页脚提示）。

#### [MODIFY] [ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx)
- 移除历史记录详情中，成功状态下的 `🖨️ 打印/导出 PDF` 按钮。

---

## 3. 验证方案

### 自动验证 (构建与类型检查)
- `npx tsc -b --noEmit` (TypeScript 类型验证)
- `npm run lint` (代码规范验证)
- `npm run build` (打包验证)

### 手动验证
- 验证已生成的复盘报告详情抽屉中，不含“打印/导出 PDF”按钮。
- 验证报告“全屏查看”时，顶栏没有“打印/PDF”按钮且底栏提示不存在。
- 验证监控大盘“保存财务报表图片”仍然能够正常渲染。
