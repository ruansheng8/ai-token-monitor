# 生成图片报表与项目走势加入截图设计规范

## 1. 背景与目标
Token Insight 目前提供了“生成财务报表”功能，但该功能在生成图片时存在以下局限：
1. **高度截断**：由于没有指定截图高度，导出的图片只到“每日用量走势”即被视口高度截断，未能包含更下方的图表板块。
2. **命名偏向财务**：“财务报表”的概念对于纯 Token 统计仪表盘偏重，更准确的概念应为“图片报表”。
3. **内容缺失**：“项目消耗大盘走势”图表是用户非常关注的维度，但在目前的图片导出中因为高度截断而不可见。

本规范旨在将“生成财务报表”重命名为“生成图片报表”，同时优化截图逻辑，确保图片报表自适应延展至包含“项目消耗大盘走势”，并排除更下方的模型分布、效能诊断和会话明细等非核心板块，输出精炼、聚焦的核心图片报表。

---

## 2. 修改范围

### 2.1 按钮与界面文案重命名
修改 [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx) 中的相关文案：
- **操作按钮**（约 1836-1842 行）：
  - 文本由 `🧾 生成财务报表` 变更为 `📸 生成图片报表`
  - 鼠标悬停提示 `title` 由 `将当前大盘数据重绘为超清财务报表图片` 变更为 `将当前大盘数据重绘为超清图片报表`
- **生成遮罩提示**（约 3210-3215 行）：
  - 提示标题由 `正在生成财务报表...` 变更为 `正在生成图片报表...`
  - 提示描述由 `系统正在使用 2x 超清高保真模式为您重绘大盘走势图并渲染财务账单，请稍候` 变更为 `系统正在使用 2x 超清高保真模式为您重绘大盘走势并渲染图片报表，请稍候`
- **预览 Modal 标题与说明**（约 3230-3235 行）：
  - 标题由 `🧾 财务报表生成成功` 变更为 `📸 图片报表生成成功`
  - 描述由 `已自动为您生成高清财务报表图片（已忽略交互控件，保留核心账单细节）` 变更为 `已自动为您生成高清图片报表（已忽略交互控件，保留核心账单与趋势大盘）`
- **图片 alt 属性**（约 3251 行）：
  - 由 `alt="Token Insight 财务报表"` 变更为 `alt="Token Insight 图片报表"`
- **一键下载文件名**（约 3291 行）：
  - 默认下载文件名由 `Token_Insight_财务报表_${new Date().toISOString().split('T')[0]}.png` 变更为 `Token_Insight_图片报表_${new Date().toISOString().split('T')[0]}.png`
- **错误警告弹窗**（约 516 行）：
  - 由 `生成财务报表图片失败` 变更为 `生成图片报表失败`

### 2.2 截图高度自适应与动态截断逻辑
修改 [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx) 中 `generateReportImage` 方法中的 `html2canvas` 参数。
由于后面的版块被加了 `no-print` 隐藏，如果继续使用 `element.scrollHeight`，会导致隐藏版块所占据的物理高度依然被截取为大片空白区域。
为了移除这些下方无用的留白，我们需要给“项目消耗大盘走势”的 `<section>` 板块赋予特定 ID `project-trend-section`，并在截图时通过 `getBoundingClientRect` 计算从容器顶部到折线图底部的精确像素高度：
```typescript
      const element = document.getElementById('report-container');
      if (!element) {
        throw new Error('未找到报表容器 #report-container');
      }

      // 动态计算截图边界高度，避免 no-print 隐藏后留下的大片空白
      let screenshotHeight = element.scrollHeight;
      const projectSection = document.getElementById('project-trend-section');
      if (projectSection) {
        const elementRect = element.getBoundingClientRect();
        const projectRect = projectSection.getBoundingClientRect();
        screenshotHeight = projectRect.bottom - elementRect.top + 24; // 增加 24px 内边距保证美观
      }

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

### 2.3 隐藏非核心板块
在 [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx) 中，为以下非核心大盘板块的 `<section>` 或外层容器容器添加 `no-print` 类，使其被 `html2canvas` 自动忽略，不显示在导出的图片中：
1. **分布与汇总** 板块（底层模型消耗占比 & 按月汇总）：
   - 将 `<section className="grid grid-cols-1 lg:grid-cols-[1fr_1.5fr] gap-6">` 修改为 `<section className="grid grid-cols-1 lg:grid-cols-[1fr_1.5fr] gap-6 no-print">`。
2. **深度效能诊断中心** 板块：
   - 将 `<section className="glass-card p-6 flex flex-col gap-6">` 修改为 `<section className="glass-card p-6 flex flex-col gap-6 no-print">`。
3. **会话用量明细** 列表板块：
   - 将 `<section className="glass-card p-6">` 修改为 `<section className="glass-card p-6 no-print">`。

---

## 3. 验证方案

### 3.1 自动构建与验证
- 运行前端 TypeScript 类型验证：`npx tsc -b --noEmit`
- 运行代码规范验证：`npm run lint`
- 运行打包编译：`npm run build`

### 3.2 手动验证步骤
1. 启动本地开发服务器。
2. 在“会话用量明细”卡片上方确认操作按钮已更新为 `📸 生成图片报表`，且悬停提示正常。
3. 点击按钮，检查生成中的加载遮罩中文案是否已正确重命名。
4. 等待图片重绘生成，确认弹出的 Modal 标题为 `📸 图片报表生成成功`。
5. 仔细观察生成的图片：
   - 确认图片内容**包含**了“看板头部”、“核心 KPI 卡片”、“每日用量走势图” 以及 “项目消耗大盘走势图”。
   - 确认图片内容**不包含**后面的“模型分布占比”、“月度汇总表”、“效能诊断中心”和“会话用量明细”。
6. 点击 Modal 底部的“保存图片”按钮，保存生成的图片，确认默认图片文件名称为 `Token_Insight_图片报表_YYYY-MM-DD.png`。
