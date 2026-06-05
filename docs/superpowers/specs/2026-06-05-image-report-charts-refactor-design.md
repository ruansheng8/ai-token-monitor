# 生成图片报表图表拓展与排版自适应优化设计规范

该设计规范旨在扩展“生成图片报表”功能，在导出的报表图片中增加“各引擎每日用量对比走势（工具维度）”和“底层模型消耗占比”两块核心图表，并通过“专用离屏高保真报表模板”对整体图片报表的排版进行重新组织和自适应美化，从而输出高保真、排版工整且内容完备的图片报表。

## 1. 业务背景与问题分析

当前的“生成图片报表”通过 `html2canvas` 截取主页面中的 `#report-container` 元素，存在以下局限性：
1. **排版对用户状态敏感**：如果用户当前折叠了某些区域，或者切换了每日趋势图的维度（如切换到了“设备维度”），导出的图片报表就无法包含默认的“类型维度”或“工具维度”。
2. **缺失关键图表**：用户无法同时在图片报表中看到“工具维度（各引擎用量）”和“模型占比”。
3. **滚动条与杂质污染**：“底层模型消耗占比”在 UI 上有 `max-h-[350px] overflow-y-auto` 限制，直接截图会带有滚动条，且周围有许多交互控件（如按钮、下拉框）污染了报表视觉。
4. **自适应布局局限**：难以让多张图表在不同分辨率的屏幕下均能稳定、整齐地呈现。

## 2. 解决方案：专用离屏高保真报表模板 (方案 A)

在主页面 DOM 中，渲染一个绝对定位在视口之外（`left: -9999px`）的专用报表容器 `#image-report-template`。该容器拥有固定的宽度（`1200px`），采用高档的暗黑/明亮双态玻璃拟态风格，在后台自动绑定状态刷新。

当用户点击“📸 生成图片报表”时，`html2canvas` 直接对该隐藏容器进行截图。这样，无论用户在前端界面上怎么操作、折叠或滚动，生成的图片报表内容和排版都始终保持一致、精美。

## 3. 图片报表排版规范

在固定 `1200px` 宽度的容器中，页面元素由上至下规划如下：

### 3.1 报表头部 (Header Area)
* **左侧**：主标题“📸 Token Insight 数据分析报表”，字体大小设为 `text-2xl`，使用霓虹渐变（`bg-gradient-to-r from-neon-cyan to-neon-purple`）。
* **右侧**：小字展示生成时间（格式 `YYYY-MM-DD HH:mm`）、统计时间范围、分析设备名称，体现专业报表元数据。

### 3.2 核心 KPI 板块 (KPIs Grid)
* 横向排列的 5 列网格卡片（`grid grid-cols-5 gap-4`）。
* 包含：**总 Token 消耗**、**总费用 (USD)**、**缓存命中率**、**推理 Token 占比**、**总会话数**。
* 样式为半透明玻璃底色（`bg-white/3 dark:bg-white/3`，配细微边框 `border border-white/10`），保留精致的小图标和单卡片微弱发光阴影。

### 3.3 每日用量走势 (Daily Trend - Type)
* 横跨整行，高度固定为 `300px`。
* 独立展示“每日用量走势 (Token 堆叠柱状图) - 类型维度”。

### 3.4 各引擎每日用量对比走势 (Daily Trend - Source)
* 横跨整行，高度固定为 `300px`。
* 独立展示“各引擎每日用量对比走势 (Token 堆叠柱状图) - 工具维度”。

### 3.5 项目大盘与底层模型分布双列板块 (Projects & Models Row)
* 采用双列非等宽网格排版（`grid grid-cols-[1.2fr_0.8fr] gap-6`）。
* **左侧 (1.2fr)**：项目消耗大盘走势 (Token 折线图 - Top 10)，高度固定为 `300px`。
* **右侧 (0.8fr)**：底层模型消耗占比。
  * **自适应优化**：在报表模板中，底层模型消耗占比模块将**彻底移除最大高度限制和滚动条**（去掉 `max-h-[350px] overflow-y-auto`），完全垂直铺开所有模型。
  * 精致的渐变色进度条和百分比，与左侧的折线图在高度上实现自适应对齐。

### 3.6 底部签名 (Footer)
* 居中浅色文字：`由 Token Insight 智能体治理中心自动生成 • 零外部依赖本地分析`。

## 4. 技术实现要点

### 4.1 DOM 结构定义
在 `src/App.tsx` 最外层结构中，渲染该离屏 DOM：
```tsx
<div 
  id="image-report-template" 
  className="absolute left-[-9999px] top-0 w-[1200px] p-8 flex flex-col gap-6 bg-slate-950 text-white rounded-3xl"
  style={{ pointerEvents: 'none' }}
>
  {/* 各组件实现，配合主题进行类名映射 */}
</div>
```
为了配合 Light/Dark 主题，可以在该容器外围附加类名。如果当前 `theme === 'dark'` 则加上 `dark` 及背景色，保持与当前主题一致的高清配色。

### 4.2 ECharts 离屏渲染与同步
由于离屏容器不在可视区域，ECharts 图表需要能够成功渲染并保持与数据的同步。
* 引入独立的 ECharts 渲染实例或在模板中复用现有的 Chart 组件，传入相同的 `data`。
* 在报表专用模板中渲染：
  1. `<DailyTrendChart data={data.daily_trends} dimension="type" theme={theme} />`
  2. `<SourceTrendChart data={data.source_trends} theme={theme} />`
  3. `<ProjectTrendChart data={data.project_trends} theme={theme} ... />`
* 确保模板中的 ECharts 图表拥有固定的宽高属性（可通过 `className="w-full h-[300px]"` 或者配置项显式声明），避免 ECharts 自动计算宽度为 0。

### 4.3 截图与导出函数重构
重构 `generateReportImage` 函数：
1. 原来是查找 `#report-container`，现在改为获取隐藏的报表模板元素：
   ```javascript
   const element = document.getElementById('image-report-template');
   ```
2. 移除原有的高度计算裁剪逻辑（因为 `#image-report-template` 是完全为报表定制的，它的总高度就是截图需要的高度，不需要单独计算裁剪点）。
3. 调用 `html2canvas` 截图时，`height` 和 `windowHeight` 直接使用 `element.scrollHeight` 即可：
   ```javascript
   const canvas = await html2canvas(element, {
     useCORS: true,
     allowTaint: false,
     backgroundColor: theme === 'dark' ? '#030712' : '#f8fafc',
     scale: 1.5,
     logging: false,
     height: element.scrollHeight,
     windowHeight: element.scrollHeight
   });
   ```

## 5. 效果自检与测试用例

1. **渲染内容完整性**：点击“📸 生成图片报表”后，确认生成的报表图片包含：元数据头部、5个核心 KPI、类型维度每日趋势图、工具维度每日趋势图、Top 10 项目折线图以及完整的底层模型占比进度条列表。
2. **滚动条排查**：报表图片中，“底层模型消耗占比”不应出现纵向滚动条，而是将所有模型列表完全垂直平铺展开。
3. **主界面隔离**：不管用户在主界面如何折叠面板或切换图表维度，图片报表的内容及排版样式始终恒定，且不受影响。
