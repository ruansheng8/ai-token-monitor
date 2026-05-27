---
name: dashboard-ui-style
description: 智业 AI 治理平台的数据看板 UI 设计规范与 ECharts 图表视觉样式。当用户要求创建、修改、美化任何数据看板页面、ECharts 图表组件、指标卡片、统计面板或数据可视化界面时，务必使用此技能。即使用户没有显式提及"样式规范"，只要涉及到看板 UI、图表颜色、卡片布局、tooltip 样式等，都应参考此技能确保视觉一致性。
---

# 数据看板 UI 视觉规范技能

本技能封装了智业 AI 治理平台中数据看板的完整 UI 设计语言，包括 ECharts 图表样式、卡片组件风格、交互控件规范等。所有新建或修改的看板类页面和图表组件，都应遵循本技能中的规范。

## 技术栈上下文

- **框架**：Next.js App Router + React + TypeScript
- **样式**：Tailwind CSS v4
- **图表库**：ECharts / echarts-for-react
- **UI 组件**：shadcn/ui + Radix UI
- **组件路径**：`src/components/charts/` 放置图表组件，`src/components/` 放置通用 UI 组件

---

## 一、设计调性

整体风格为 **现代高端 SaaS / 微光渐变与毛玻璃拟态（Glassmorphic & Sleek Modernism）**。

核心原则：
- 避开生硬的扁平纯色块和高对比边框
- 利用微渐变流光、超大圆角、透光半玻璃质感描边和细腻漫反射阴影
- 给用户"专业、尊贵、舒适"的观感体验

---

## 二、八色调色板（全局统一）

所有 ECharts 图表的数据系列均使用此调色板，这是全局唯一的颜色来源，保持色彩统一：

```typescript
const PALETTE_COLORS = [
  "#3b82f6", // 1. 活力蓝
  "#06b6d4", // 2. 明亮青
  "#14b8a6", // 3. 薄荷绿
  "#6366f1", // 4. 睿智靛蓝
  "#8b5cf6", // 5. 科技紫
  "#ec4899", // 6. 柔和粉
  "#f59e0b", // 7. 琥珀黄
  "#10b981", // 8. 翠绿
]
```

在柱状排行图中按数据索引轮换颜色：`PALETTE_COLORS[params.dataIndex % PALETTE_COLORS.length]`

---

## 三、辅助函数

每个需要渐变面积的图表文件应包含此辅助函数：

```typescript
function hexToRgba(hex: string, alpha: number) {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}
```

---

## 四、ECharts 图表通用配置

### 1. Tooltip（高端卡片式提示框）

所有图表 tooltip 统一采用以下拟态白卡片设计：

```typescript
tooltip: {
  trigger: "axis", // 饼图用 "item"
  backgroundColor: "rgba(255, 255, 255, 0.96)",
  borderColor: "#e2e8f0",
  borderWidth: 1,
  textStyle: { color: "#0f172a", fontSize: 11 },
  extraCssText: "box-shadow: 0 10px 30px -5px rgba(0, 0, 0, 0.08); border-radius: 12px; padding: 10px;",
  formatter: (params) => {
    // 标题使用 font-weight:600, color:#0f172a
    // 标签文字使用 color:#64748b
    // 数值使用 font-weight:600, color:#0f172a, font-family:monospace
    // 使用 flex 布局实现双栏对齐
  }
}
```

**Tooltip formatter 模板（多系列）**：
```typescript
formatter: (params: unknown) => {
  const list = params as Array<{
    seriesName: string; value: number; marker: string
  }>
  if (!list || list.length === 0) return ""
  
  let html = `<span style="font-weight:600;color:#0f172a;display:block;margin-bottom:6px;">${(params as [{ name: string }])[0].name}</span>`
  list.forEach((item) => {
    html += `<div style="display:flex;align-items:center;justify-content:between;gap:20px;margin-bottom:3px;">
      <span style="display:inline-flex;align-items:center;gap:4px;color:#64748b;">
        ${item.marker} ${item.seriesName}
      </span>
      <span style="font-weight:600;color:#0f172a;font-family:monospace;margin-left:auto;">
        ${item.value.toLocaleString()}
      </span>
    </div>`
  })
  return html
}
```

### 2. Legend（图例）

```typescript
legend: {
  type: "scroll",
  top: 0,
  icon: "circle",
  itemGap: 16,
  textStyle: { color: "#64748b", fontSize: 10 },
}
```

### 3. Grid（网格间距）

```typescript
grid: { left: 42, right: 18, top: 40, bottom: 32 }
```

### 4. X 轴

```typescript
xAxis: {
  type: "category",
  axisLabel: { color: "#64748b", fontSize: 10 },
  axisLine: { lineStyle: { color: "#e2e8f0" } },
  axisTick: { show: false },
}
```

### 5. Y 轴

```typescript
yAxis: {
  type: "value",
  axisLabel: { color: "#64748b", fontSize: 10 },
  splitLine: { lineStyle: { color: "#f1f5f9" } },
}
```

---

## 五、各图表类型的 Series 样式

### 折线图（Line Chart）

```typescript
{
  type: "line",
  smooth: true,           // 必须开启平滑曲线
  showSymbol: true,
  symbol: "circle",
  symbolSize: 6,
  itemStyle: {
    borderWidth: 2,
    borderColor: "#fff",   // 白色描边
  },
  areaStyle: {             // 柔和渐变面积填充
    color: {
      type: "linear",
      x: 0, y: 0, x2: 0, y2: 1,
      colorStops: [
        { offset: 0, color: hexToRgba(color, 0.16) },
        { offset: 1, color: hexToRgba(color, 0.01) },
      ],
    },
  },
}
```

### 柱状图（Bar Chart）

```typescript
{
  type: "bar",
  itemStyle: {
    borderRadius: 4,       // 堆叠柱状图用 4
    // 排行柱状图用 [6, 6, 0, 0] (仅顶端圆角)
    borderColor: "#fff",
    borderWidth: 2,        // 排行图可用 1
  },
  // 排行图额外的 emphasis:
  emphasis: {
    itemStyle: {
      shadowBlur: 10,
      shadowOffsetX: 0,
      shadowColor: "rgba(0, 0, 0, 0.12)",
    },
  },
}
```

### 饼图 / 环形图（Pie / Donut Chart）

```typescript
{
  type: "pie",
  itemStyle: {
    borderRadius: 6,       // 扇区圆角
    borderColor: "#fff",
    borderWidth: 2,        // 白色切割线
  },
}
```

---

## 六、图表容器规范

```tsx
<ReactECharts
  style={{ height: 320, width: "100%" }}
  option={chartOption}
  notMerge
  lazyUpdate
/>
```

- 默认高度 `320px`，宽度 `100%`
- 始终使用 `notMerge` 和 `lazyUpdate`
- 图表选项使用 `useMemo` 缓存

---

## 七、卡片与容器样式（Tailwind CSS）

### 1. 流光渐变背景（看板头部）

用于看板核心分析面板的头部区域：

```
background:
  radial-gradient(circle at top left, rgba(56, 189, 248, 0.18), transparent 28%),
  radial-gradient(circle at 85% 10%, rgba(249, 115, 22, 0.16), transparent 18%),
  linear-gradient(180deg, #f8fbff 0%, #fffaf3 100%);
```

### 2. 玻璃拟态卡片

```tsx
className="rounded-[24px] bg-white/80 border border-white/70 shadow-[0_18px_48px_rgba(15,23,42,0.05)] transition-all duration-200 hover:-translate-y-0.5 hover:shadow-[0_22px_56px_rgba(15,23,42,0.10)]"
```

核心要素：
- 圆角 ≥ 24px
- 背景 `bg-white/80`（半透明白底）
- 描边 `border-white/70` 或 `border-slate-200/70`
- 漫反射阴影 `shadow-[0_18px_48px_rgba(15,23,42,0.05)]`
- Hover 微位移 `hover:-translate-y-0.5`

### 3. 指标卡分色微渐变

根据数据维度使用不同底色：

| 色系 | Tailwind class | 适用场景 |
|------|---------------|---------|
| 蓝色 | `from-blue-50 via-white to-cyan-50/70` | 核心基础数据 |
| 青色 | `from-cyan-50 via-white to-sky-50/70` | 系统性能/健康度 |
| 橙色 | `from-orange-50 via-white to-amber-50/70` | 高价值/消耗指标 |
| 灰色 | `from-slate-50 via-white to-slate-100/80` | 次级辅助信息 |

### 4. 药丸形时间范围切换器

```tsx
// 容器
className="rounded-full border border-slate-200/80 bg-white/80 p-1 shadow-[0_12px_32px_rgba(15,23,42,0.06)]"

// 未激活按钮
className="text-slate-600 hover:bg-slate-50"

// 激活状态
style={{
  background: "linear-gradient(to right, #2563eb, #06b6d4)",
  color: "#ffffff",
  boxShadow: "0 10px 24px rgba(37, 99, 235, 0.22)",
}}
```

---

## 八、组件编写模板

新建图表组件时，参照以下模板结构：

```tsx
"use client"

import React, { useMemo } from "react"
import ReactECharts from "echarts-for-react"

// 如果需要渐变面积填充
function hexToRgba(hex: string, alpha: number) {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

const PALETTE_COLORS = [
  "#3b82f6", "#06b6d4", "#14b8a6", "#6366f1",
  "#8b5cf6", "#ec4899", "#f59e0b", "#10b981",
]

export function MyChart({ data }: { data: YourDataType }) {
  const chartOption = useMemo(() => ({
    color: PALETTE_COLORS,
    tooltip: { /* 参见第四节 tooltip 规范 */ },
    legend: { /* 参见第四节 legend 规范 */ },
    grid: { left: 42, right: 18, top: 40, bottom: 32 },
    xAxis: { /* 参见第四节 X 轴规范 */ },
    yAxis: { /* 参见第四节 Y 轴规范 */ },
    series: [ /* 参见第五节对应图表类型 */ ],
  }), [data])

  return (
    <ReactECharts
      style={{ height: 320, width: "100%" }}
      option={chartOption}
      notMerge
      lazyUpdate
    />
  )
}
```

---

## 九、检查清单

创建或修改图表/看板组件后，对照以下清单确认：

- [ ] 使用了统一的八色调色板 `PALETTE_COLORS`
- [ ] tooltip 采用白底卡片式设计，数值使用 monospace 字体
- [ ] 图例使用 `icon: "circle"`，`fontSize: 10`
- [ ] 网格分割线使用 `#f1f5f9`，轴线标签使用 `#64748b`
- [ ] 折线图启用了 `smooth: true` 和渐变面积填充
- [ ] 柱状图/饼图有白色 borderColor 和圆角
- [ ] 卡片容器使用了 ≥24px 圆角和玻璃拟态效果
- [ ] 组件使用 `useMemo` 优化 ECharts option
- [ ] ReactECharts 使用了 `notMerge` 和 `lazyUpdate`
