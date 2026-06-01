# 项目消耗大盘走势仅展示 Top 10 项目设计文档

此文档定义了“项目消耗大盘走势”仅展示消耗量最大的前 10 个项目的技术设计。

## 1. 背景与目标
在 AI Token Monitor 中，随着用户开发项目的增多，“项目消耗大盘走势”折线图展示的项目线段会变得非常繁杂，导致图表视觉混乱且严重影响 ECharts 的渲染性能。
为了优化用户体验和看板性能，本方案计划在前端对项目数据进行过滤，仅渲染当前时间范围内 Token 消耗量最大的前 10 个项目，同时在标题上进行明确的文案提示。

## 2. 变更范围与方案设计

### 2.1. 图表数据过滤逻辑设计
修改文件：[ProjectTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/ProjectTrendChart.tsx)

在 `ProjectTrendChart` 的 `useMemo` 数据处理流程中：
1. **聚合计算每个项目的总 Token 消耗量**：
   遍历 `data` 数组，以 `project_name` (若为空则归为 `unknown-project`) 为键，累加其 `tokens` 值。
2. **排序与截取**：
   将所有项目按 Token 累计消耗量从大到小（降序）进行排序，截取前 10 个项目，形成 `topProjects` 数组。
3. **保留 Top 10 绘制数据**：
   - 数据映射 `projectDataMap` 仅对 `topProjects` 包含的项目进行填充。
   - `series` 的渲染数组仅基于 `topProjects` 数组进行 `map` 映射生成。
   - 调色板颜色分配也仅针对 Top 10 项目。

具体过滤代码片段设计：
```typescript
// 1. 计算各项目的总 tokens 消耗，以进行 Top 10 过滤
const projectTotals = new Map<string, number>();
data.forEach(t => {
  const pName = t.project_name || 'unknown-project';
  projectTotals.set(pName, (projectTotals.get(pName) || 0) + (t.tokens || 0));
});

// 2. 排序并获取前 10 个项目
const topProjects = Array.from(projectTotals.entries())
  .sort((a, b) => b[1] - a[1])
  .slice(0, 10)
  .map(entry => entry[0]);
```

### 2.2. 图表标题文案更新设计
修改文件：[App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)

修改第 1650 行的标题文案，以明确告知用户当前只展示了前 10 个项目：
* 修改前：`项目消耗大盘走势 (Token 折线图)`
* 修改后：`项目消耗大盘走势 (Token 折线图 - Top 10)`

---

## 3. 验证计划

### 3.1. 静态验证
- 运行 `npx tsc -b --noEmit`，确保无 TypeScript 类型错误。
- 运行 `npm run lint`，确保无 ESLint 静态代码检查错误。

### 3.2. 动态验证
- 启动本地开发服务，检查项目大盘折线图：
  1. 确认折线的最大数量不超过 10 条。
  2. 确认图表中的 Legend（图例）仅展示排名前 10 的项目。
  3. 确认图表标题已更新为 `项目消耗大盘走势 (Token 折线图 - Top 10)`。
  4. 确认折线图数据准确，且当存在少于 10 个项目时能够正确渲染所有项目。
