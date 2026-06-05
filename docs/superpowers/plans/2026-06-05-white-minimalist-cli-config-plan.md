# 配置 AI CLI 引擎白色简约风格实现计划

本文档提供了将“Token Insight”的 `配置 AI CLI 引擎` 弹窗（`CliConfigModal.tsx`）重构为白色简约风格的详细执行步骤、代码变更描述和测试验证方法。

---

## 1. 拟修改的文件列表

*   **`src/components/CliConfigModal.tsx`**：负责弹窗界面的整体样式与交互样式覆写。

---

## 2. 详细变更步骤

### 第一步：遮罩蒙层与弹窗边框/阴影重构
*   修改第 395 行的 Overlay 背景及高斯模糊：
    *   将 `rgba(15, 15, 35, 0.55)` 改为 `rgba(255, 255, 255, 0.45)`。
    *   保持 `backdropFilter: 'blur(16px)'`（或从 12px 提高到 16px 提升朦胧感）。
*   修改第 402-408 行的弹窗容器样式：
    *   `background: '#ffffff'`（保持）。
    *   `boxShadow` 改为 `'0 20px 50px rgba(0,0,0,0.05), 0 4px 12px rgba(0,0,0,0.02)'`。
    *   `border` 改为 `'1px solid #e2e8f0'`。

### 第二步：头部 (Header) 元素白色极简重构
*   修改第 413 行的 Header 背景样式：
    *   将渐变 `linear-gradient(...)` 改为纯白 `background: '#ffffff'`。
    *   新增底部细线：在 `style` 字典中增加 `borderBottom: '1px solid #f1f5f9'`。
*   修改第 422 行的标题 `<h2>` 文字：
    *   `className` 从 `text-sm font-bold text-white drop-shadow-sm` 改为 `text-sm font-bold bg-gradient-to-r from-[#06b6d4] to-[#8b5cf6] bg-clip-text text-transparent`。
*   修改第 423 行的副标题 `<p>`：
    *   `className` 从 `text-xs text-white/75 mt-0.5` 改为 `text-xs text-slate-500 mt-0.5`。
*   修改第 418 行的 Terminal 图标容器：
    *   `className` 中的 `bg-white/20` 替换为 `bg-slate-100`。
    *   `Terminal` 图标的颜色由 `text-white` 改为渐变双色，或直接使用紫色点缀 `className="w-4 h-4 text-[#8b5cf6]"`。
*   修改第 426 行的关闭按钮：
    *   `className` 从 `bg-white/15 hover:bg-white/25 flex ...` 改为 `bg-slate-100 hover:bg-slate-200 flex ...`。
    *   `X` 图标颜色由 `text-white` 改为 `text-slate-500 hover:text-slate-700`。

### 第三步：左侧 CLI 列表重构
*   修改第 441 行的左侧容器背景：
    *   将 `background: '#f8f9fc'` 保持，其右侧边框 `borderRight` 调整为 `1px solid #e2e8f0`（替换原有的 `#e8eaf0`）。
*   修改第 446 行和第 451 行的“重新检测所有 CLI”按钮：
    *   `borderBottom` 替换为 `1px solid #e2e8f0`。
    *   按钮 `style` 变更为：`color: '#334155', background: '#ffffff', border: '1px solid #e2e8f0'`。
    *   去除行内 hover 覆盖，直接依靠干净的白底灰框样式，或追加浅灰 hover。
*   修改第 475-480 行的列表项 `isSelected` 选中态和未选中态样式：
    *   选中态 `style`：`background: '#f1f5f9', borderRight: '3px solid #06b6d4'`。
    *   未选中项悬浮背景采用 `#f1f5f9` 浅灰色（对应 mouse enter/leave）。
*   修改第 492 行的选中文字颜色：
    *   选中文字：`style={{ color: isSelected ? '#0f172a' : '#475569' }}`。

### 第四步：右侧编辑与配置面板重构
*   修改第 528 行的 CLI 头部卡片背景：
    *   由 `linear-gradient(...)` 改为 `background: '#ffffff', border: '1px solid #e2e8f0'`。
    *   标题文字改为 `#0f172a`。
*   修改第 538-546 行的状态徽章：
    *   已检测：`background: '#f0fdf4', border: '1px solid #bbf7d0', color: '#16a34a'`。
    *   未检测：`background: '#fef2f2', border: '1px solid #fecaca', color: '#dc2626'`。
*   修改第 570-582 行的可执行路径输入框：
    *   默认状态：`background: '#ffffff', border: '1.5px solid #e2e8f0'`。
    *   Focus 状态下的边框颜色改为 `#06b6d4`，背景改为 `#fafdfd`。
*   修改第 628-632 行的环境变量 Key 标签：
    *   将原本刺眼的黄色背景改为：`background: '#f1f5f9', border: '1px solid #e2e8f0', color: '#475569'`。
*   修改第 642-647 行的环境变量 Value 输入框：
    *   默认状态：`background: '#ffffff', border: '1.5px solid #e2e8f0'`。
    *   Focus 状态下的边框颜色改为 `#06b6d4`。
*   修改第 838-842 行的“添加变量”按钮：
    *   改为白底灰框：`background: '#ffffff', border: '1.5px solid #e2e8f0', color: '#334155'`，hover 背景为 `#f1f5f9`。
*   修改第 855-857 行的添加环境变量浮动面板：
    *   背景 `#ffffff`，边框 `1.5px solid #e2e8f0`，阴影改为亮色浅灰投影。
*   修改第 869-874 行的添加面板内输入框聚焦：
    *   Focus 边框变更为 `#06b6d4`。

### 第五步：底部操作区重构
*   修改第 742 行的底部容器背景和边框：
    *   `borderTop` 调整为 `1px solid #e2e8f0`。
    *   `background` 调整为 `#f8fafc`。
*   修改第 749-753 行的“测试连通性”按钮：
    *   改为白底灰框：`background: '#ffffff', border: '1.5px solid #e2e8f0', color: '#334155'`。
    *   在非测试运行时，hover 时展现淡青色或紫色文字。
*   修改第 779-783 行的“关闭”按钮：
    *   改为白底灰框：`background: '#ffffff', border: '1.5px solid #e2e8f0', color: '#334155'`，hover 为 `#f1f5f9`。
*   修改第 796 行的“保存并应用”按钮：
    *   采用首页同款霓虹渐变色：`background: 'linear-gradient(90deg, #06b6d4 0%, #8b5cf6 100%)'`，文字白色保持。

---

## 3. 验证步骤

1.  运行 `pnpm build` 或通过开发服务确认 `src-tauri` 编译构建和前端构建无报错。
2.  进入 Token Insight，点击 CLI 引擎配置，验证以下项：
    *   [ ] 背景是否有白毛玻璃亮色朦胧质感。
    *   [ ] 弹窗头部是否为白色底、具有霓虹渐变的“配置 AI CLI 引擎”标题字。
    *   [ ] 左侧列表选中时是否有霓虹青色边框，底色是否为淡灰。
    *   [ ] 环境变量的 Key 是否为冷灰色，与整体色调协调。
    *   [ ] 连通测试和保存按钮是否显示符合设计。
3.  操作流验证：
    *   [ ] 点击“重新检测所有 CLI”动作正常。
    *   [ ] 测试“连通测试”反馈正常。
    *   [ ] 添加、修改、删除环境变量及路径并保存，验证数据更新无误。
