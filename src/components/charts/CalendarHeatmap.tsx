import { useMemo, useState } from 'react';
import { ECharts } from '../ECharts';

interface DailyTrend {
  date: string;
  input: number;
  output: number;
  cached: number;
  thinking: number;
  sessions: number;
}

interface CalendarHeatmapProps {
  data: DailyTrend[];
  theme: 'light' | 'dark';
}

// 格式化数字，带中文大数单位（万、亿），保留最多1位有效小数
const formatValueWithUnit = (val: number) => {
  const precise = val.toLocaleString('zh-CN');
  if (val >= 10000) {
    let unit = '';
    let formatted = val;
    if (val >= 1e8) {
      formatted = val / 1e8;
      unit = '亿';
    } else if (val >= 1e4) {
      formatted = val / 1e4;
      unit = '万';
    }
    const trimmed = parseFloat(formatted.toFixed(1)).toString();
    return `${precise} (${trimmed}${unit})`;
  }
  return precise;
};

export function CalendarHeatmap({ data = [], theme }: CalendarHeatmapProps) {
  const isDark = theme === 'dark';
  const [dimension, setDimension] = useState<'tokens' | 'sessions'>('tokens');

  // 1. 计算以最晚数据时间（或今天）为基准的滚动一年的起止范围
  const today = useMemo(() => {
    if (data && data.length > 0) {
      // 找到最近的日期，防止因为离线演示数据导致无法渲染
      const lastItem = data[data.length - 1];
      return new Date(lastItem.date);
    }
    return new Date();
  }, [data]);

  // 2. 生成最近 365 天的所有连续日期序列 (YYYY-MM-DD)
  const rollingYearDates = useMemo(() => {
    const dates: string[] = [];
    for (let i = 364; i >= 0; i--) {
      const d = new Date(today.getFullYear(), today.getMonth(), today.getDate() - i);
      const y = d.getFullYear();
      const m = String(d.getMonth() + 1).padStart(2, '0');
      const day = String(d.getDate()).padStart(2, '0');
      dates.push(`${y}-${m}-${day}`);
    }
    return dates;
  }, [today]);

  // 3. 将现有的 trends 存入 Map 以便极速检索与缺省补零
  const dataMap = useMemo(() => {
    const map = new Map<string, DailyTrend>();
    data.forEach(item => {
      map.set(item.date, item);
    });
    return map;
  }, [data]);

  // 4. 根据当前维度组装 365 天数据列表，并动态计算区间最大值
  const chartData = useMemo(() => {
    let maxVal = 0;
    const list = rollingYearDates.map(dateStr => {
      const item = dataMap.get(dateStr);
      let val = 0;
      if (item) {
        val = dimension === 'tokens' ? (item.input + item.output) : item.sessions;
      }
      if (val > maxVal) maxVal = val;
      return [dateStr, val] as [string, number];
    });
    return { list, maxVal };
  }, [rollingYearDates, dataMap, dimension]);

  // 5. 翠绿 HSL 调色步长 (Emerald step palette)
  const emptyColor = isDark ? '#1b2330' : '#ebedf0'; // 暗色自适应暗蓝灰，明亮自适应淡灰
  const colorSteps = useMemo(() => {
    return isDark
      ? [emptyColor, '#065f46', '#047857', '#059669', '#10b981'] // 暗黑模式下从深翠绿到亮翠绿
      : [emptyColor, '#d1fae5', '#a7f3d0', '#34d399', '#059669']; // 明亮模式下由浅入深
  }, [isDark, emptyColor]);

  // 6. ECharts 核心配置参数构建
  const chartOption = useMemo(() => {
    const startDateStr = rollingYearDates[0];
    const endDateStr = rollingYearDates[rollingYearDates.length - 1];

    return {
      tooltip: {
        position: 'top',
        confine: true,
        backgroundColor: isDark ? 'rgba(11, 21, 40, 0.94)' : 'rgba(255, 255, 255, 0.96)',
        borderColor: isDark ? 'rgba(255, 255, 255, 0.08)' : '#e2e8f0',
        borderWidth: 1,
        textStyle: { color: isDark ? '#f3f4f6' : '#0f172a', fontSize: 11 },
        extraCssText: `box-shadow: 0 10px 30px -5px rgba(0, 0, 0, ${isDark ? '0.35' : '0.08'}); border-radius: 12px; padding: 10px; backdrop-filter: blur(8px);`,
        formatter: (params: any) => {
          const date = params.value[0];
          const val = params.value[1];

          let displayVal = '';
          if (dimension === 'tokens') {
            displayVal = `${formatValueWithUnit(val)} Tokens`;
          } else {
            displayVal = `${val.toLocaleString()} 个会话`;
          }

          return `<div style="font-size: 11px;">
            <span style="font-weight: 600; color: ${isDark ? '#f3f4f6' : '#0f172a'}; display: block; margin-bottom: 5px;">${date}</span>
            <div style="display: flex; align-items: center; justify-content: space-between; gap: 15px;">
              <span style="color: ${isDark ? '#9ca3af' : '#64748b'};">${dimension === 'tokens' ? 'Token 总消耗' : '会话总数'}:</span>
              <span style="font-weight: 700; color: ${isDark ? '#10b981' : '#059669'}; font-family: 'JetBrains Mono', monospace; margin-left: auto;">${displayVal}</span>
            </div>
          </div>`;
        }
      },
      visualMap: {
        show: false, // 隐藏控制条
        min: 0,
        max: chartData.maxVal || 1,
        calculable: true,
        inRange: {
          color: colorSteps
        }
      },
      calendar: {
        top: 25,
        bottom: 5,
        left: 30,
        right: 15,
        cellSize: ['auto', 13],
        range: [startDateStr, endDateStr],
        itemStyle: {
          borderWidth: 2,
          borderColor: isDark ? '#0b1528' : '#ffffff',
          borderRadius: 2
        },
        splitLine: {
          show: false
        },
        yearLabel: { show: false },
        dayLabel: {
          firstDay: 1, // 星期一作为每周首日
          nameMap: 'cn',
          color: isDark ? '#9ca3af' : '#64748b',
          fontSize: 9,
          fontFamily: 'Outfit, sans-serif'
        },
        monthLabel: {
          nameMap: 'cn',
          color: isDark ? '#9ca3af' : '#64748b',
          fontSize: 9,
          fontFamily: 'Outfit, sans-serif'
        }
      },
      series: {
        type: 'heatmap',
        coordinateSystem: 'calendar',
        data: chartData.list
      }
    };
  }, [isDark, rollingYearDates, chartData, dimension, colorSteps]);

  return (
    <div className="flex flex-col gap-4">
      {/* 头部区：包含标题和右上角的药丸形维度切换器 */}
      <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center pb-2 border-b border-card-border/50 gap-2">
        <div>
          <h2 className="text-base font-semibold text-text-primary flex items-center gap-2">
            📅 每日活跃度日历热力图 (Activity Heatmap)
          </h2>
          <p className="text-xs text-text-secondary mt-1">
            滚动展示最近 365 天的历史活跃频度。非选定范围内的日期自动淡化。
          </p>
        </div>

        {/* 药丸形维度切换按钮 */}
        <div className="rounded-full border border-slate-200/80 dark:border-slate-800 bg-white/80 dark:bg-[#0b1528] p-0.5 shadow-[0_12px_32px_rgba(15,23,42,0.06)] flex items-center gap-0.5">
          <button
            onClick={() => setDimension('tokens')}
            style={
              dimension === 'tokens'
                ? {
                    background: "linear-gradient(to right, #10b981, #059669)",
                    color: "#ffffff",
                    boxShadow: "0 4px 12px rgba(16, 185, 129, 0.25)",
                  }
                : undefined
            }
            className={`px-3.5 py-1.5 rounded-full text-xs font-semibold hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer ${
              dimension === 'tokens'
                ? 'text-white'
                : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100/50 dark:hover:bg-white/5'
            }`}
          >
            📊 TOKEN 总数
          </button>
          <button
            onClick={() => setDimension('sessions')}
            style={
              dimension === 'sessions'
                ? {
                    background: "linear-gradient(to right, #10b981, #059669)",
                    color: "#ffffff",
                    boxShadow: "0 4px 12px rgba(16, 185, 129, 0.25)",
                  }
                : undefined
            }
            className={`px-3.5 py-1.5 rounded-full text-xs font-semibold hover:scale-105 active:scale-100 transition-all duration-200 cursor-pointer ${
              dimension === 'sessions'
                ? 'text-white'
                : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100/50 dark:hover:bg-white/5'
            }`}
          >
            💬 会话总数
          </button>
        </div>
      </div>

      {/* 热力图容器 */}
      <div style={{ height: '170px', width: '100%' }} className="relative">
        <ECharts option={chartOption as any} />
      </div>

      {/* 底部区：色系图例说明 */}
      <div className="flex justify-between sm:justify-end items-center gap-4 text-[10px] text-text-secondary pr-4 select-none">
        <span>少 (Less)</span>
        <div className="flex gap-0.5">
          {colorSteps.map((c, i) => (
            <div
              key={i}
              style={{ backgroundColor: c }}
              className="w-3 h-3 rounded-[2px] border border-black/5 dark:border-white/5"
            />
          ))}
        </div>
        <span>多 (More)</span>
      </div>
    </div>
  );
}
