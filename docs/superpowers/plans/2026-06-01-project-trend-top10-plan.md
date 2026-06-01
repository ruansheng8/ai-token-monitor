# 项目消耗大盘走势展示 Top 10 项目实现计划

过滤并限制“项目消耗大盘走势”折线图，仅展示 Token 累计消耗量前 10 名的项目，并更新相应的图表标题，以优化性能和图表可读性。

## 1. 拟修改文件

### 前端图表组件
#### [MODIFY] [ProjectTrendChart.tsx](file:///d:/VibeCoding/ai-token-monitor/src/components/charts/ProjectTrendChart.tsx)
- 在 `useMemo` 数据处理中：
  - 累加统计各项目的 `tokens` 消耗。
  - 降序排序并截取 Top 10 项目。
  - 限制 `projects`、`projectDataMap`、`series` 的处理范围为 Top 10 项目。

### 前端主页面
#### [MODIFY] [App.tsx](file:///d:/VibeCoding/ai-token-monitor/src/App.tsx)
- 将大盘走势标题文案由 `项目消耗大盘走势 (Token 折线图)` 改为 `项目消耗大盘走势 (Token 折线图 - Top 10)`。

---

## 2. 验证计划

### 静态验证
- 执行前端类型检查：
  ```powershell
  npx tsc -b --noEmit
  ```
- 执行前端 Linter 检查：
  ```powershell
  npm run lint
  ```

### 动态验证
- 启动本地开发服务，确认折线图仅渲染排名前 10 的项目，且标题正确显示。
