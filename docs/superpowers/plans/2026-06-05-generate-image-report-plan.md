# 生成图片报表与项目走势加入截图实现计划

该实现计划旨在将仪表盘的“生成财务报表”功能重命名为“生成图片报表”，并扩展截图的渲染高度以包含“项目消耗大盘走势”折线图，同时排除其下方的非核心调试和明细板块，生成精炼的图片报表。

## 1. 拟修改内容

### 前端页面与组件
#### [MODIFY] [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)

1. **重命名所有相关文案**：
   - 替换 `🧾 生成财务报表` 按钮文案为 `📸 生成图片报表`，修改其 `title` 属性为 `将当前大盘数据重绘为超清图片报表`。
   - 替换 `正在生成财务报表...` 的 Loading 标题为 `正在生成图片报表...`。
   - 替换 `系统正在使用 2x 超清高保真模式为您重绘大盘走势图并渲染财务账单，请稍候` 为 `系统正在使用 2x 超清高保真模式为您重绘大盘走势并渲染图片报表，请稍候`。
   - 替换 `🧾 财务报表生成成功` 的 Modal 标题为 `📸 图片报表生成成功`。
   - 替换 `已自动为您生成高清财务报表图片（已忽略交互控件，保留核心账单细节）` 为 `已自动为您生成高清图片报表（已忽略交互控件，保留核心账单与趋势大盘）`。
   - 替换 `alt="Token Insight 财务报表"` 为 `alt="Token Insight 图片报表"`。
   - 替换下载文件的默认名称 `Token_Insight_财务报表_${new Date().toISOString().split('T')[0]}.png` 为 `Token_Insight_图片报表_${new Date().toISOString().split('T')[0]}.png`。
   - 将 catch 块中的错误提示 `生成财务报表图片失败` 修改为 `生成图片报表失败`。

2. **自适应截图高度与截断参数**：
   - 为“项目消耗大盘走势”板块 `<section className="animate-fade-in">` 赋予 `id="project-trend-section"`。
   - 在 `generateReportImage` 中计算 `screenshotHeight`，避免被隐藏版块在底部留下空白：
     ```typescript
     let screenshotHeight = element.scrollHeight;
     const projectSection = document.getElementById('project-trend-section');
     if (projectSection) {
       const elementRect = element.getBoundingClientRect();
       const projectRect = projectSection.getBoundingClientRect();
       screenshotHeight = projectRect.bottom - elementRect.top + 24; // 24px padding
     }
     ```
   - 在调用 `html2canvas` 导出时，配置 `height` 和 `windowHeight` 为 `screenshotHeight`：
     ```typescript
     const canvas = await html2canvas(element, {
       useCORS: true,
       allowTaint: false,
       backgroundColor: theme === 'dark' ? '#030712' : '#f8fafc',
       scale: 1.5,
       logging: false,
       height: screenshotHeight,
       windowHeight: screenshotHeight,
       ignoreElements: (el: Element) => {
         if (el.tagName.toLowerCase() === 'canvas') {
           const canvasEl = el as HTMLCanvasElement;
           if (canvasEl.width === 0 || canvasEl.height === 0) {
             return true;
           }
         }
         return el.classList.contains('no-print');
       }
     });
     ```

3. **对非核心板块加上 `no-print` 进行排除**：
   - 将分布与汇总板块的 `<section className="grid grid-cols-1 lg:grid-cols-[1fr_1.5fr] gap-6">` 修改为 `<section className="grid grid-cols-1 lg:grid-cols-[1fr_1.5fr] gap-6 no-print">`。
   - 将深度效能诊断面板的 `<section className="glass-card p-6 flex flex-col gap-6">` 修改为 `<section className="glass-card p-6 flex flex-col gap-6 no-print">`。
   - 将会话明细部分的 `<section className="glass-card p-6">` 修改为 `<section className="glass-card p-6 no-print">`。

---

## 2. 验证计划

### 编译构建与规范验证
- 运行类型检查：在 `src-tauri` 的上一级目录下执行 `npx tsc -b --noEmit`，确保无类型报错。
- 运行代码规范：`npm run lint`，确保无 ESLint 语法警告/错误。
- 运行打包测试：`npm run build`，确保前端项目可以正常构建。

### 手动功能验证
1. 启动项目，切换至监控大盘。
2. 确认会话用量明细头部的按钮文本变更为 `📸 生成图片报表`，且悬停提示已改变。
3. 点击“📸 生成图片报表”按钮，确认出现的重绘遮罩文字中没有包含“财务报表”字样，取而代之的是“图片报表”。
4. 检查生成的图片报表预览 Modal：
   - 确认图片报表只显示到“项目消耗大盘走势 (Token 折线图 - Top 10)”，其底部的模型分布、月汇总、效能诊断和会话明细列表均已完美隐藏，使图片不过长。
   - 确认折线图内容已成功截取进去，且未由于 ECharts 重绘导致空白。
5. 点击下载保存，确认导出的 PNG 文件名为 `Token_Insight_图片报表_YYYY-MM-DD.png`。
