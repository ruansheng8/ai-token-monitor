# 智能复盘分析诊断报告 Markdown 可视化实现计划 (Plan)

本项目将在前端引入 `react-markdown` 及其依赖插件 `remark-gfm`，以提升“智能复盘分析诊断报告”的可视化效果与安全性。

## 用户评审要求

- 本次修改将引入第三方 npm 包 `react-markdown` 与 `remark-gfm`。在 React 19 下，可能需要使用 `--legacy-peer-deps` 来安装，以规避某些旧库导致的 Peer Dependency 冲突。
- 引入超链接跳转的安全策略：在桌面应用（Tauri v2）中点击 Markdown 生成的超链接会调用系统的默认浏览器打开，而不会在 Webview 内直接跳转。

---

## 计划变更内容

### 前端依赖

#### [MODIFY] [package.json](file:///d:/VibeCoding/ai-token-monitor/package.json)
- 新增 `react-markdown` 依赖
- 新增 `remark-gfm` 依赖

### 前端组件

#### [NEW] [Markdown.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/Markdown.tsx)
- 新增独立的 Markdown 渲染组件，封装 `ReactMarkdown` 与 `remark-gfm`。
- 配置 `components` 自定义映射，通过 Tailwind CSS v4 样式支持玻璃拟态表格、等宽字体对齐、引用块、代码块以及 `a` 标签的拦截机制。

#### [MODIFY] [ReviewDrawer.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/ReviewDrawer.tsx)
- 引入 `<Markdown />` 组件。
- 替换智能诊断报告处的 `dangerouslySetInnerHTML` 渲染方式。
- 移除遗留的 `renderMarkdown` 简易正则工具函数及其相关引用。

---

## 验证计划

### 自动化验证与构建
- 在项目根目录下执行 `npx tsc -b --noEmit` 进行 TypeScript 类型无误性验证。
- 执行 `npm run build` 验证 Vite 前端打包流程是否完全正常。

### 手动功能验证
- 启动本地开发服务，生成或查看历史诊断报告，确认原本为纯文本格式的表格是否以精美的“半透明边框+交替行高亮”表格进行可视化呈现。
- 在报告中点击随机超链接，验证系统是否成功在外部默认浏览器中开启对应的超链接，而 Tauri 桌面应用不受影响。
