# 数据看板 UI 视觉与图表设计规范（完整版）

此文件是 `docs/design/dashboard-ui-style.md` 的完整参考副本，包含更详细的设计意图说明。
当 SKILL.md 中的简化规范不足以回答具体设计问题时，查阅本文件获取完整上下文。

---

## 一、 核心设计理念与视觉调性

智业 AI 治理平台的数据看板致力于给用户以"专业、尊贵、舒适"的观感体验。整体设计避开了传统管理后台生硬、冰冷的扁平纯色块或高对比边框，转而利用**微渐变流光**、**超大圆角**、**透光半玻璃质感描边**以及**细腻的漫反射阴影**，呈现出一种现代高档 SaaS 应用的数据美学。

---

## 二、 基础视觉规范 (Base UI Specification)

### 1. 复合流光渐变背景 (Glow / Liquid Gradient Background)
看板的核心分析表面（如 `DashboardSurface` 头部运营面板）采用复合径向与线性渐变色，营造呼吸感与纵深感：
* **CSS 样式代码**：
  ```css
  background: 
    radial-gradient(circle at top left, rgba(56, 189, 248, 0.18), transparent 28%),
    radial-gradient(circle at 85% 10%, rgba(249, 115, 22, 0.16), transparent 18%),
    linear-gradient(180deg, #f8fbff 0%, #fffaf3 100%);
  ```
* **设计意图**：左上微蓝，右侧微橙，底色由淡蓝向淡暖过渡，多色融汇，赋予首屏极强的视觉冲击力与高端感。

### 2. 圆角与玻璃拟态 (Rounded & Glassmorphism)
* **大圆角结构**：核心组件与图表外层容器均使用超大圆角以弱化界面的机械感，规格统一在 **`rounded-[24px]`（24px）** 至 **`rounded-3xl`（24px）**。
* **玻璃半透明质感**：背景采用高透明度白底，如 `bg-white/80`、`bg-slate-50/60`，并辅以极其轻薄的白色或灰色描边（如 `border-white/70` 或 `border-slate-200/70`），模拟精致的轻透玻璃质感。
* **漫反射阴影与微交互**：
  ```css
  box-shadow: 0 18px 48px rgba(15, 23, 42, 0.05);
  transition: all 0.2s ease-in-out;
  
  /* Hover 动效 */
  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 22px 56px rgba(15, 23, 42, 0.10);
  }
  ```
  通过极柔和的大跨度阴影，加上悬浮微位移，在操作时提供灵动的回馈。

### 3. 指标卡分色微渐变 (Theme-colored Micro-gradients)
指标卡片根据不同数据维度，精细化定制底色渐变，避免视觉单一性：
* **蓝色系 (`blue`)**：`from-blue-50 via-white to-cyan-50/70` —— 用于通用基础核心数据。
* **青色系 (`cyan`)**：`from-cyan-50 via-white to-sky-50/70` —— 用于系统性能、健康度或效率。
* **橙色系 (`orange`)**：`from-orange-50 via-white to-amber-50/70` —— 用于高价值指标或消耗额度。
* **灰色系 (`slate`)**：`from-slate-50 via-white to-slate-100/80` —— 用于次级辅助或普通信息展示。

---

## 三、 交互控件规范 (Interactive Elements)

### 1. 药丸形时间范围切换器
采用全圆角胶囊状结构，带来平滑温润的手感：
* **基础容器**：`rounded-full border border-slate-200/80 bg-white/80 p-1 shadow-[0_12px_32px_rgba(15,23,42,0.06)]`
* **未激活按钮**：`text-slate-600 hover:bg-slate-50`
* **激活状态按钮**：采用高饱和双色渐变与发光微投影，引导视觉焦点：
  ```css
  background: linear-gradient(to right, #2563eb, #06b6d4); /* from-blue-600 to-cyan-500 */
  color: #ffffff;
  box-shadow: 0 10px 24px rgba(37, 99, 235, 0.22);
  ```

---

## 四、 ECharts 统一图表视觉规范 (Chart Styles Specification)

为保持看板整体视觉的严肃性与统一性，ECharts 图表需要严格执行以下视觉配置。

### 1. 优雅八色调色板 (Harmony Palette)
所有图表（折线、柱状、饼图）的数据系列（Series）均以此调色板进行着色，保持全局统一的色彩观感：
```javascript
const PALETTE_COLORS = [
  "#3b82f6", // 1. 活力蓝
  "#06b6d4", // 2. 明亮青
  "#14b8a6", // 3. 薄荷绿
  "#6366f1", // 4. 睿智靛蓝
  "#8b5cf6", // 5. 科技紫
  "#ec4899", // 6. 柔和粉
  "#f59e0b", // 7. 琥珀黄
  "#10b981", // 8. 翠绿
];
```

### 2. 图例设计与网格 (Legend & Grid)
* **图例**：统一使用圆形标识，字体小而柔和，并在需要时支持滚动图例。
  ```javascript
  legend: {
    type: "scroll",
    icon: "circle",
    itemGap: 16,
    textStyle: { color: "#64748b", fontSize: 10 }
  }
  ```
* **网格间距**：左、右、下均预留合理的透气感，X/Y 轴网格线使用极淡的 `#f1f5f9`。
  ```javascript
  grid: { left: 42, right: 18, top: 40, bottom: 32 }
  ```

### 3. 数据系列美化 (Series Style)

#### A. 折线图 (Line Chart)
* **平滑曲线**：强制开启 `smooth: true`。
* **数据点样式**：显示圆形数据点，直径 6px，拥有白边描边：
  ```javascript
  symbol: "circle",
  symbolSize: 6,
  itemStyle: { borderWidth: 2, borderColor: "#fff" }
  ```
* **柔和渐变面积填充 (Area Gradient)**：面积填充透明度从顶部的 `16%` 缓慢淡出至底部的 `1%`。
  ```javascript
  areaStyle: {
    color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
      { offset: 0, color: hexToRgba(color, 0.16) },
      { offset: 1, color: hexToRgba(color, 0.01) }
    ])
  }
  ```

#### B. 饼图 / 环形图 (Pie / Donut Chart)
* **圆角与间隙**：扇区之间必须带有 2px 的白色切割线与圆角过渡，增加精致感与透气感：
  ```javascript
  itemStyle: {
    borderRadius: 6,
    borderColor: "#fff",
    borderWidth: 2
  }
  ```

#### C. 柱状图 (Bar Chart)
* **顶端圆角**：柱体顶端设置适当的圆角，同样保留白边：
  ```javascript
  itemStyle: {
    borderRadius: 4,
    borderColor: "#fff",
    borderWidth: 2
  }
  ```

### 4. 高端卡片式提示框 (Premium Tooltip)
图表的 `tooltip` 采用拟态白卡片设计，并对内部的数值格式进行高精度对齐排版：
```javascript
tooltip: {
  trigger: "axis",
  backgroundColor: "rgba(255, 255, 255, 0.96)",
  borderColor: "#e2e8f0",
  borderWidth: 1,
  textStyle: { color: "#0f172a", fontSize: 11 },
  extraCssText: "box-shadow: 0 10px 30px -5px rgba(0, 0, 0, 0.08); border-radius: 12px; padding: 10px;",
  formatter: (params) => {
    // 强制使用等宽字体对齐，保证数据清晰易读
    // 内容排版使用简洁的双栏分布，值使用 Monospace 字体展示
  }
}
```

---

## 五、 小结

在今后扩展或新建任何数据看板页面时，开发团队应遵循本规范中所定义的**大圆角 (>=24px)**、**玻璃质感半透边框**、**微渐变背景**与**八色 ECharts 视觉样式**。通过严格遵循这些细节，能够让整个平台的 UI 设计保持高度统一，维持智业 AI 治理平台的现代品牌感。
