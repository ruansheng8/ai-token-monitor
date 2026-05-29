# 恢复“财务报表”功能实现计划

该计划旨在把先前被隐藏的“财务报表”功能恢复，包括解除逻辑代码注释并加回触发按钮。

## 用户评审要求

- **功能恢复**：利用 `html2canvas` 导出页面高清长图的财务报表功能。点击后会显示重绘 Loading，并弹出高清财务报表图片预览 Modal，支持一键下载保存。
- **按钮位置**：将放置在“会话用量明细”卡片右上角“导出 CSV 账单”按钮的右侧，方便用户集中操作。
- **按钮样式**：使用符合看板 UI 风格的微光渐变圆角按钮。

## 待讨论问题

无。先前功能的 Loading 遮罩、图片预览 Modal、截图库 `html2canvas` 依赖以及相关的 CSS 保证均已存在，仅需恢复逻辑与触发入口。

## 提议的变更

---

### 前端组件与页面

#### [MODIFY] [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)

1. **状态定义恢复**：
   将 `_setIsGeneratingReport` 和 `_setReportImgUrl` 修改为 `setIsGeneratingReport` 和 `setReportImgUrl`，使更新状态的函数能够被正常使用。
   ```diff
   -  const [isGeneratingReport, _setIsGeneratingReport] = useState(false);
   +  const [isGeneratingReport, setIsGeneratingReport] = useState(false);
      const [isReportModalOpen, setIsReportModalOpen] = useState(false);
   -  const [reportImgUrl, _setReportImgUrl] = useState('');
   +  const [reportImgUrl, setReportImgUrl] = useState('');
   ```

2. **核心生成逻辑恢复**：
   将 `_generateReportImage` 解除注释，并重命名为 `generateReportImage`。

3. **操作按钮加回**：
   将 1644 行左右的 `{/* 生成财务报表按钮 - 暂时隐藏 */}` 替换为可点击的按钮组件，当点击时触发 `generateReportImage`。按钮带 `no-print` 类，保证截图本身不包含此按钮。
   ```tsx
   <button
     onClick={generateReportImage}
     className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-gradient-to-r from-neon-cyan to-neon-purple hover:from-neon-cyan/90 hover:to-neon-purple/90 text-white cursor-pointer transition-all duration-200 flex items-center gap-1 shadow-sm no-print"
     title="将当前大盘数据重绘为超清财务报表图片"
   >
     🧾 生成财务报表
   </button>
   ```

## 验证计划

### 编译构建验证
- 在根目录执行 `npx tsc -b --noEmit` 确保无 TypeScript 类型错误。
- 运行 `npm run build` 确保能够成功构建前端静态资源。

### 手动功能验证
- 启动项目，在“会话用量明细”卡片上方找到“🧾 生成财务报表”按钮。
- 点击按钮，验证是否显示“正在生成财务报表...”的加载遮罩。
- 确认加载后是否弹出“🧾 财务报表生成成功”的高清图片预览 Modal。
- 测试点击“📥 保存财务报表图片”是否能够成功下载带有当前大盘图表和汇总的 PNG 图片，且图片内没有操作按钮等交互元素（由于 `no-print` 排除了它们）。
