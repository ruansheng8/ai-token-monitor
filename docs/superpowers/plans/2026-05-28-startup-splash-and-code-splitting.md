# Startup Splash and Code Splitting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the desktop window show an initialization card immediately and reduce the React startup bundle so chart dependencies do not block first visual feedback.

**Architecture:** Add a static fallback splash directly inside `index.html` so Tauri displays useful content before React loads. Convert chart component imports in `src/App.tsx` to lazy imports with a small chart fallback so ECharts-heavy code is split out of the initial bundle.

**Tech Stack:** Tauri v2, Vite, React 19, TypeScript, Tailwind CSS, ECharts.

---

## File Structure

- Modify `index.html`: add inline minimal CSS and static splash markup inside `#root`; keep existing theme preflight script unchanged.
- Modify `src/App.tsx`: import `lazy` and `Suspense` from React; replace direct chart imports with lazy imports; wrap chart render sites in Suspense with a small visual fallback.
- Verify with `npm run build`: confirm build succeeds and chart chunks split out of the main bundle.

---

### Task 1: Static HTML Startup Card

**Files:**
- Modify: `index.html:3-29`

- [ ] **Step 1: Add inline startup splash styles**

Replace the `<head>` section in `index.html` with:

```html
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>AI 用量统计仪表盘</title>
    <!-- Google Fonts -->
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;600&family=Outfit:wght@300;400;500;600;700&display=swap" rel="stylesheet">
    <style>
      :root {
        --startup-bg: #f8fafc;
        --startup-card: rgba(255, 255, 255, 0.78);
        --startup-border: rgba(148, 163, 184, 0.18);
        --startup-text: #0f172a;
        --startup-muted: #64748b;
        --startup-cyan: #0891b2;
        --startup-purple: #9333ea;
      }
      .dark {
        --startup-bg: #030712;
        --startup-card: rgba(11, 19, 36, 0.72);
        --startup-border: rgba(255, 255, 255, 0.06);
        --startup-text: #f3f4f6;
        --startup-muted: #9ca3af;
        --startup-cyan: #06b6d4;
        --startup-purple: #a855f7;
      }
      html,
      body,
      #root {
        min-height: 100%;
        margin: 0;
      }
      body {
        background: var(--startup-bg);
        font-family: 'Outfit', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      }
      .startup-splash {
        position: fixed;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
        color: var(--startup-text);
        overflow: hidden;
      }
      .startup-splash::before,
      .startup-splash::after {
        content: "";
        position: absolute;
        width: 520px;
        height: 520px;
        border-radius: 9999px;
        filter: blur(80px);
        opacity: 0.16;
      }
      .startup-splash::before {
        top: -220px;
        left: -120px;
        background: var(--startup-cyan);
      }
      .startup-splash::after {
        right: -160px;
        bottom: -260px;
        background: var(--startup-purple);
      }
      .startup-card {
        position: relative;
        z-index: 1;
        width: min(400px, 100%);
        padding: 32px;
        border: 1px solid var(--startup-border);
        border-radius: 32px;
        background: var(--startup-card);
        box-shadow: 0 20px 50px rgba(15, 23, 42, 0.14);
        text-align: center;
        backdrop-filter: blur(20px) saturate(180%);
        -webkit-backdrop-filter: blur(20px) saturate(180%);
      }
      .startup-spinner {
        width: 64px;
        height: 64px;
        margin: 0 auto 24px;
        animation: startup-spin 0.9s linear infinite;
      }
      .startup-title {
        margin: 0 0 8px;
        font-size: 20px;
        font-weight: 700;
        letter-spacing: -0.02em;
        background: linear-gradient(90deg, var(--startup-cyan), var(--startup-purple));
        -webkit-background-clip: text;
        background-clip: text;
        color: transparent;
      }
      .startup-copy {
        margin: 0;
        color: var(--startup-muted);
        font-size: 12px;
        line-height: 1.7;
      }
      @keyframes startup-spin {
        to { transform: rotate(360deg); }
      }
    </style>
    <!-- 原生主题检测脚本，在 React 加载前立即应用 dark 类名以防闪白 -->
    <script>
      (function() {
        try {
          const saved = localStorage.getItem('theme');
          if (saved === 'dark') {
            document.documentElement.classList.add('dark');
          } else {
            document.documentElement.classList.remove('dark');
          }
        } catch (e) {
          console.error(e);
        }
      })();
    </script>
  </head>
```

- [ ] **Step 2: Add static splash markup inside root**

Replace the root element in `index.html` with:

```html
    <div id="root">
      <div class="startup-splash" aria-live="polite" aria-label="AI Token Monitor 正在初始化">
        <div class="startup-card">
          <svg class="startup-spinner" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M12 2C6.47715 2 2 6.47715 2 12C2 17.5228 6.47715 22 12 22C17.5228 22 22 17.5228 22 12" stroke="url(#startup-spinner-grad)" stroke-width="3" stroke-linecap="round"/>
            <defs>
              <linearGradient id="startup-spinner-grad" x1="2" y1="2" x2="22" y2="22" gradientUnits="userSpaceOnUse">
                <stop stop-color="#06b6d4" />
                <stop offset="1" stop-color="#a855f7" />
              </linearGradient>
            </defs>
          </svg>
          <h1 class="startup-title">AI Token Monitor</h1>
          <p class="startup-copy">正在初始化后台服务并同步数据，请稍候...</p>
        </div>
      </div>
    </div>
```

- [ ] **Step 3: Build to verify HTML remains valid**

Run: `npm run build`

Expected: command exits successfully. The build may still warn about large chunks until Task 2 is implemented.

- [ ] **Step 4: Commit**

```bash
git add index.html
git commit -m "fix: show static startup splash before react loads"
```

---

### Task 2: Lazy Load Chart Components

**Files:**
- Modify: `src/App.tsx:1-28`
- Modify: `src/App.tsx:1179-1197`
- Modify: `src/App.tsx:1298-1300`
- Modify: `src/App.tsx:1578-1581`

- [ ] **Step 1: Replace direct chart imports with lazy imports**

Change the first import line and chart imports in `src/App.tsx` to:

```tsx
import { useState, useEffect, useMemo, useRef, lazy, Suspense } from 'react';
import { listen } from '@tauri-apps/api/event';
import {
  Cpu,
  ArrowDown,
  ArrowUp,
  Database,
  Brain,
  Hash,
  RefreshCw,
  Search,
  MessageSquare,
  ChevronsUpDown,
  Compass,
  Sun,
  Moon,
  ChevronLeft,
  ChevronRight,
  Globe,
  ChevronDown,
  Settings,
  Terminal,
  Monitor
} from 'lucide-react';

const DailyTrendChart = lazy(() => import('./components/charts/DailyTrendChart').then((module) => ({ default: module.DailyTrendChart })));
const SourceTrendChart = lazy(() => import('./components/charts/SourceTrendChart').then((module) => ({ default: module.SourceTrendChart })));
const PerformanceChart = lazy(() => import('./components/charts/PerformanceChart').then((module) => ({ default: module.PerformanceChart })));
const CalendarHeatmap = lazy(() => import('./components/charts/CalendarHeatmap').then((module) => ({ default: module.CalendarHeatmap })));
```

- [ ] **Step 2: Add a chart fallback component**

Insert after the chart lazy imports in `src/App.tsx`:

```tsx
const ChartFallback = ({ label = '正在加载图表...' }: { label?: string }) => (
  <div className="h-[300px] flex items-center justify-center text-text-muted text-xs italic">
    {label}
  </div>
);
```

- [ ] **Step 3: Wrap daily/source chart rendering with Suspense**

Replace the chart render block inside the daily trend section with:

```tsx
          <div className="w-full">
            <Suspense fallback={<ChartFallback />}>
              {source === 'all' && chartDimension === 'source' ? (
                data?.source_trends && data.source_trends.length > 0 ? (
                  <SourceTrendChart data={data.source_trends} theme={theme} />
                ) : (
                  <div className="h-[300px] flex items-center justify-center text-text-muted italic">暂无趋势图表数据</div>
                )
              ) : chartDimension === 'device' ? (
                data?.device_trends && data.device_trends.length > 0 ? (
                  <DailyTrendChart data={data.daily_trends} deviceTrends={data.device_trends} dimension="device" theme={theme} />
                ) : (
                  <div className="h-[300px] flex items-center justify-center text-text-muted italic">暂无趋势图表数据</div>
                )
              ) : (
                data?.daily_trends && data.daily_trends.length > 0 ? (
                  <DailyTrendChart data={data.daily_trends} dimension="type" theme={theme} />
                ) : (
                  <div className="h-[300px] flex items-center justify-center text-text-muted italic">暂无趋势图表数据</div>
                )
              )}
            </Suspense>
          </div>
```

- [ ] **Step 4: Wrap PerformanceChart with Suspense**

Replace the chart component in the performance panel with:

```tsx
                  <Suspense fallback={<ChartFallback label="正在加载效能图表..." />}>
                    <PerformanceChart data={data.performance_trends} theme={theme} />
                  </Suspense>
```

- [ ] **Step 5: Wrap CalendarHeatmap with Suspense**

Replace the heatmap render with:

```tsx
            <Suspense fallback={<ChartFallback label="正在加载日历热力图..." />}>
              <CalendarHeatmap data={data.daily_trends} theme={theme} />
            </Suspense>
```

- [ ] **Step 6: Build and confirm chunk split**

Run: `npm run build`

Expected: command exits successfully and output includes multiple JS assets under `dist/assets/`, with the main `index-*.js` smaller than the previous 1,413.52 kB bundle.

- [ ] **Step 7: Commit**

```bash
git add src/App.tsx
git commit -m "perf: lazy load dashboard charts"
```

---

### Task 3: Manual Startup Verification

**Files:**
- No code changes expected.

- [ ] **Step 1: Start the desktop app**

Run: `npm run tauri dev`

Expected: Tauri opens the desktop window and the static startup card appears immediately before React finishes loading.

- [ ] **Step 2: Verify React takes over without visual regression**

Observe the window after startup.

Expected: the static card is replaced by the React initialization card or dashboard; scanning progress still appears when `scanStatus.is_scanning` is true; no long blank white screen appears.

- [ ] **Step 3: Verify chart lazy loading**

Navigate the initialized dashboard.

Expected: daily trend, source/device trend, performance chart, and calendar heatmap render correctly. During slow chunk load, the small chart fallback appears instead of a blank chart area.

- [ ] **Step 4: Final build verification**

Run: `npm run build`

Expected: command exits successfully.

---

## Self-Review

- Spec coverage: covers static startup card and chart code splitting, matching the approved approach.
- Placeholder scan: no TBD/TODO/placeholders remain.
- Type consistency: lazy import names match existing named chart exports used by `App.tsx`.
