# 设计文档：AI Token Monitor 增加“最近30天”统计项并设置为默认

本设计文档旨在 AI Token Monitor 的前端监控面板中增加“最近30天”的时间区间选项，并优化系统加载时的初始渲染，将其作为默认选中的统计时间区间。

## 目标与背景

当前 AI Token Monitor 的时间筛选控制栏支持以下区间：
- 全部时间 (默认)
- 今日
- 最近7天
- 本月
- 本季度
- 自定义

为了让用户能够更便捷地查看中长周期的 Token 消耗状况，我们将在时间区间选择器中新增“最近30天”选项，并优化首屏加载体验，让页面在首次渲染时默认加载最近30天的数据。同时，通过合理重构规避组件挂载时的重复 API 调用。

---

## 方案设计与变更细节

为了保证应用在启动加载时的极致性能与视觉平滑度，我们选择**方案 A**进行开发：

### 1. 核心状态与计算逻辑重构 (组件外部纯函数)
在 `src/App.tsx` 中，原有的 `getDateBounds` 函数是在 `App` 组件内部声明的。为了让 `useState` 在初始化阶段可以直接获取最近30天的起止时间，我们将 `getDateBounds` 提取到 `App` 组件外部，封装为纯函数。

### 2. 增加最近30天的日期计算规则
在 `getDateBounds` 纯函数中新增 `'30days'` 分支：
- **计算逻辑**：起始日期为当前日期向前推 29 天，截止日期为今天（共计 30 天，含今天）。
```typescript
case '30days': {
  const past = new Date(now.getTime() - 29 * 24 * 60 * 60 * 1000);
  return { start: formatDateStr(past), end: formatDateStr(now) };
}
```

### 3. 时间区间默认值及状态初始化优化
为了杜绝组件在挂载时产生二次 API 数据请求（即首次加载空状态请求一次，随后 `useEffect` 侦测到 `'30days'` 再请求一次），我们将 `useState` 的初始状态一次性设为 `'30days'` 的精确区间值：
- `timeRange` 默认值设为 `'30days'`：
  ```typescript
  const [timeRange, setTimeRange] = useState<'all' | 'today' | 'week' | '30days' | 'month' | 'quarter' | 'custom'>('30days');
  ```
- `startDate` 和 `endDate` 初始值直接通过调用 `getDateBounds('30days')` 获取：
  ```typescript
  const initialBounds = getDateBounds('30days');
  const [startDate, setStartDate] = useState<string>(initialBounds.start);
  const [endDate, setEndDate] = useState<string>(initialBounds.end);
  ```

### 4. 界面 (UI) 按钮呈现
在 `src/App.tsx` 的时间区间渲染配置数组中，于“最近7天”与“本月”之间插入“最近30天”统计项：
```typescript
{ key: '30days', label: '最近30天' }
```

---

## 影响评估与兼容性

1. **后端兼容性**：
   - 后端路由 `/api/metrics` 已经完备支持 `start_date` 和 `end_date` 参数并从 SQLite 数据库筛选，无需任何修改。
2. **图表自适应**：
   - 数据加载后，前端的 ECharts 日走势图（DailyTrendChart）及各维度分析图表会自然平滑地呈现过去 30 天内的每日趋势，完全自适应。
3. **用户体验提升**：
   - 首次加载仅产生一次网络请求，页面加载数据无抖动，速度更快。

---

## 验证计划

### 1. 手动验证
- **首屏默认加载验证**：刷新应用，验证页面加载时，“最近30天”按钮是否高亮，并且 KPI 区域和图表数据展示的是最近 30 天的数据（可查看浏览器的 Network 控制台，确认首次请求 `/api/metrics` 携带的 `start_date` 和 `end_date` 刚好是过去 30 天的范围且**仅发出一次请求**）。
- **时间切换验证**：点击“全部时间”、“今日”、“最近7天”等，查看数据是否对应更新；再次点击“最近30天”按钮，检查数据是否正确还原为最近30天。
- **自定义区间兼容性验证**：切换到“自定义”，选择特定时间，再切回“最近30天”，验证是否能再次正确加载最近30天数据。
