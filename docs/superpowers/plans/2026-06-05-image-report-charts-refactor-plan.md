# 生成图片报表图表拓展与排版自适应优化实现计划

本计划旨在将“生成图片报表”功能升级为基于离屏专用模板的超清导出方案，确保导出的报表图片包含：头部元数据、KPI 网格、每日用量走势（类型）、各引擎每日用量走势（工具）、项目消耗折线图、底层模型分布（完全平铺展平）以及底部签名，并在排版上进行自适应优化。

## User Review Required

> [!IMPORTANT]
> - **截图方式变更**：报表截图的底层逻辑从“截取主页面可见大盘并裁剪高度”重构为“截取绝对定位在负位移处的专用 1200px 宽度报表模板 `#image-report-template`”。
> - **排版调整**：“每日用量走势（类型维度）”与“各引擎每日用量对比走势（工具维度）”均横跨整行以充分展示日期跨度；“项目消耗大盘（折线图）”与“底层模型消耗占比”采用 1.2fr : 0.8fr 并排双列显示。
> - **模型渲染优化**：在导出的图片中，底层模型列表不再显示纵向滚动条，而是物理平铺展开所有模型，避免信息被隐藏。

## Open Questions

无。

## Proposed Changes

### 前端看板组件

#### [MODIFY] [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)
1. **重构 `generateReportImage` 函数**（第 365~533 行）：
   - 将截图元素从主容器 `document.getElementById('report-container')` 改为获取报表专用隐藏模板 `document.getElementById('image-report-template')`。
   - 移除原来的裁剪高度计算逻辑（`screenshotHeight` 动态计算），因为隐藏模板拥有专属排版且高度自适应，无需裁剪。
   - 在调用 `html2canvas` 截图前，增加大约 `200ms` 的微小延迟以确保离屏 ECharts 渲染实例有充足的时间在微任务中完成 Canvas 图表绘制。
   - 截图时的 `height` 和 `windowHeight` 直接绑定为隐藏模板的 `scrollHeight`。

2. **渲染离屏报表专用模板**：
   - 在 `return` 语句中（例如在 `!appInitializing && !initError && data` 下面），在最外层 `div` 内加挂隐藏模板 DOM。
   - 使用 CSS 类定位在视口外：
     ```tsx
     absolute left-[-9999px] top-0 w-[1200px] p-8 flex flex-col gap-6 rounded-3xl
     ```
   - 配合主页面主题 `theme` 映射模板的背景色 and 文本色，使其黑白背景高保真匹配当前配色。
   - 在模板内完整声明：
     - **元数据头部**：展示分析区间、生成日期和设备名。
     - **KPI Grid**：5 列并排，提取 totals 数据。
     - **趋势图一（类型）**：横跨整行，调用 `<DailyTrendChart>`。
     - **趋势图二（工具）**：横跨整行，调用 `<SourceTrendChart>`。
     - **双列组合排版**：
       - 左列（1.2fr）：顶层项目消耗 `<ProjectTrendChart>`。
       - 右列（0.8fr）：底层模型消耗占比，取消 `max-h-[350px] overflow-y-auto`，自适应拉伸。
     - **页脚签名**：居中灰色小字。

## Verification Plan

### Automated Tests
- 执行前端类型检查：
  ```powershell
  npx tsc -b --noEmit
  ```
- 执行前端 Lint 检查：
  ```powershell
  npm run lint
  ```

### Manual Verification
1. 启动项目本地开发服务，确认主界面显示一切正常。
2. 点击“📸 生成图片报表”按钮，检查遮罩 Loading。
3. 检查预览弹出 Modal，验证：
   - 报表顶部是否显示了生成设备名称、分析日期区间。
   - 是否包含每日走势图（类型）与各引擎走势图（工具），且各自独占一行展现。
   - 底层模型消耗占比与项目折线图是否并排陈列，且没有纵向滚动条，各模型占比完整平铺展开。
   - 页面背景主题配对无明显色差。
4. 点击保存，确认 PNG 文件能成功下载并显示高清无暇内容。
