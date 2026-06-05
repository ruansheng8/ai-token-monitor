# 配置 AI CLI 引擎白色简约风格设计方案

本文档定义了将“Token Insight”的 `配置 AI CLI 引擎` 弹窗（`CliConfigModal.tsx`）重构为**亮白简约风格**的视觉设计与实现方案。该弹窗在保留完整配置功能与接口逻辑的基础上，重构其 CSS 类与行内样式，实现更加现代、克制且具科技感的高端亮色视觉。

---

## 1. 视觉设计调性定义

*   **遮罩蒙层 (Overlay)**：采用亮色白毛玻璃蒙砂效果（`rgba(255, 255, 255, 0.45)` + `backdrop-filter: blur(16px)`），弱化弹窗下方的深色首页内容，使整体界面呈现通透感。
*   **弹窗容器 (Container)**：采用纯白背景（`#ffffff`），配以极细的浅灰色边框（`#e2e8f0`）和淡雅的灰色弥散投影，移除原有的深色或发紫色阴影。
*   **核心品牌点缀 (Accent Colors)**：引入首页代表性的**霓虹青 (`#06b6d4`)** 与**霓虹紫 (`#8b5cf6`)** 作为微量的点缀色：
    *   **标题文字**：使用青紫双色渐变字。
    *   **激活状态**：左侧选中 CLI 的边框线使用霓虹青。
    *   **输入框聚焦**：聚焦边框使用霓虹青/霓虹紫过渡的聚焦环效果。
    *   **主要操作按钮**：保存并应用按钮采用精美的青紫渐变背景，作为白色页面上的操作焦点。
*   **辅助元素 (Subtle Grays)**：左侧列表背景、输入框背景、环境变量 Key 标签等，由原本的黄底、彩底或亮彩色块统一规范为冷灰色系（`Slate`，如 `#f8fafc`、`#f1f5f9`、`#e2e8f0`）。

---

## 2. 弹窗结构具体样式重构设计

以下是对 [CliConfigModal.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/CliConfigModal.tsx) 具体的重构细节方案：

### 2.1. 外部蒙层与弹窗容器
*   **遮罩层**：`style` 中的 `backgroundColor` 从 `rgba(15, 15, 35, 0.55)` 修改为 `rgba(255, 255, 255, 0.45)`。
*   **弹窗容器**：
    *   `style` 中 `background` 修改为 `#ffffff`。
    *   `boxShadow` 替换为 `0 20px 50px rgba(0, 0, 0, 0.05), 0 4px 12px rgba(0, 0, 0, 0.02)`（移除带有紫色倾向的阴影）。
    *   `border` 调整为 `1px solid #e2e8f0`。

### 2.2. 弹窗头部 (Header)
*   **头部背景**：由原先的高饱和度渐变 `linear-gradient(90deg, #6366f1 0%, #8b5cf6 50%, #06b6d4 100%)` 改为**纯白背景** `background: '#ffffff'`。
*   **底部边框**：增加 `border-bottom: 1px solid #f1f5f9` 与下方内容清晰隔开。
*   **标题文字**：增加 `bg-gradient-to-r from-[#06b6d4] to-[#8b5cf6] bg-clip-text text-transparent`，将标题 `配置 AI CLI 引擎` 重构为高质感的霓虹青紫渐变字。
*   **副标题与关闭按钮**：
    *   副标题文字由 `text-white/75` 修改为 `text-slate-500`。
    *   Terminal 图标的背景框从 `bg-white/20` 修改为 `bg-slate-100`，图标本身添加青紫色。
    *   关闭按钮由原白底 `bg-white/15 hover:bg-white/25 text-white` 重构为 `bg-slate-100 hover:bg-slate-200 text-slate-500 hover:text-slate-700`。

### 2.3. 左侧 CLI 列表 (Sidebar)
*   **背景**：容器 `style` 的 `background` 统一使用干净的冷白灰色 `#f8fafc`。
*   **检测按钮**：重构为纯白底配灰细框：
    *   `background: '#ffffff', border: '1px solid #e2e8f0', color: '#334155'`，hover 时为浅灰。
*   **列表项 (Item)**：
    *   未选中项悬浮背景采用 `bg-slate-100` (`#f1f5f9`)，文字保持深灰 `#374151`。
    *   选中项采用淡淡的冷灰色背景 `#f1f5f9`，右侧或左侧的激活竖线使用青色高亮 `#06b6d4`，选中文字颜色修改为 `#0f172a`。

### 2.4. 右侧编辑区 (Editor Panel)
*   **CLI 头部卡片**：
    *   由紫蓝渐变底改为**纯白细灰框卡片**（`background: '#ffffff', border: '1px solid #e2e8f0'`）。
    *   状态徽章（已检测/未检测）采用温和的小胶囊样式：
        *   已检测：`background: '#f0fdf4', border: '1px solid #bbf7d0', color: '#16a34a'`。
        *   未检测：`background: '#fef2f2', border: '1px solid #fecaca', color: '#dc2626'`。
*   **输入框**：
    *   默认状态：`background: '#f8fafc', border: '1.5px solid #e2e8f0'`。
    *   聚焦状态 (`onFocus`)：聚焦边框高亮改为 `#06b6d4`，背景微带青紫色光泽。
*   **环境变量 Key 标签**：
    *   原先的土黄色（`#fffbeb`, `#fde68a`）改为亮冷灰色：`background: '#f1f5f9', border: '1px solid #e2e8f0', color: '#475569'`。
*   **测试结果卡片**：
    *   连通性测试结果的背景与文字，保持原有的浅绿/浅红主色调，但细化边框线，与右侧整体白净氛围协调。

### 2.5. 底部操作栏 (Footer)
*   **背景与边框**：使用干净的 `#f8fafc` 底色，顶部带 `1px solid #e8eaf0` 的细灰线。
*   **测试连通性按钮**：改为 `background: '#ffffff', border: '1.5px solid #e2e8f0', color: '#334155'`，仅在 hover 时展现出浅青紫的文字光泽。
*   **关闭按钮**：保持 `bg-slate-100` 或白底灰框，保证纯净。
*   **保存并应用按钮**：依然使用首页品牌色的渐变色背景（`background: 'linear-gradient(90deg, #06b6d4 0%, #8b5cf6 100%)'`），为整个白色极简界面点睛。

---

## 3. 验证与回归测试

1.  **UI 视觉还原度**：启动开发服务器，检查弹窗的整体背景是否为无边界的通透白色毛玻璃。检查头部标题是否渐变自然，各列表项、卡片和环境变量的排布是否为协调的白色冷灰色系。
2.  **基本功能回归**：
    *   切换不同 CLI，环境变量表单、检测状态以及当前所设路径显示无误。
    *   添加新环境变量、删除环境变量行为正常。
    *   连通性测试功能（调用后台接口并展示结果）以及保存应用功能（数据正常入库并反映在主界面上）完整无阻碍。
