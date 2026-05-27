import { useMemo } from 'react';
import { ECharts } from '../ECharts';

interface SourceTrendItem {
  date: string;
  source: string;
  tokens: number;
  cost: number;
}

interface SourceTrendChartProps {
  data: SourceTrendItem[];
  theme: 'light' | 'dark';
}

const ENGINE_COLORS = {
  antigravity: '#8b5cf6', // 科技紫
  claude_code: '#f59e0b', // 橙黄
  codex: '#06b6d4',       // 明亮青
};

const SOURCE_LABELS: Record<string, string> = {
  antigravity: 'Antigravity (Gemini)',
  claude_code: 'Claude Code',
  codex: 'Codex CLI',
};

function hexToRgba(hex: string, alpha: number) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function SourceTrendChart({ data = [], theme }: SourceTrendChartProps) {
  const isDark = theme === 'dark';

  const chartOption = useMemo(() => {
    // 提取所有不重复的日期并排序
    const dates = Array.from(new Set(data.map(item => item.date))).sort();

    // 针对每个来源，按日期对齐数据
    const sources = ['antigravity', 'claude_code', 'codex'];
    const seriesData = sources.map(src => {
      const srcData = dates.map(d => {
        const found = data.find(item => item.date === d && item.source === src);
        return found ? found.tokens : 0;
      });
      return {
        name: SOURCE_LABELS[src],
        type: 'line',
        data: srcData,
        smooth: true,
        showSymbol: true,
        symbol: 'circle',
        symbolSize: 6,
        itemStyle: {
          color: ENGINE_COLORS[src as keyof typeof ENGINE_COLORS],
          borderWidth: 2,
          borderColor: isDark ? '#0b1528' : '#ffffff',
        },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: hexToRgba(ENGINE_COLORS[src as keyof typeof ENGINE_COLORS], 0.12) },
              { offset: 1, color: hexToRgba(ENGINE_COLORS[src as keyof typeof ENGINE_COLORS], 0.01) },
            ],
          },
        },
      };
    });

    return {
      tooltip: {
        trigger: 'axis',
        backgroundColor: isDark ? 'rgba(11, 21, 40, 0.94)' : 'rgba(255, 255, 255, 0.96)',
        borderColor: isDark ? 'rgba(255, 255, 255, 0.08)' : '#e2e8f0',
        borderWidth: 1,
        textStyle: { color: isDark ? '#f3f4f6' : '#0f172a', fontSize: 11 },
        extraCssText: `box-shadow: 0 10px 30px -5px rgba(0, 0, 0, ${isDark ? '0.35' : '0.08'}); border-radius: 12px; padding: 10px; backdrop-filter: blur(8px);`,
        formatter: (params: any) => {
          if (!params || params.length === 0) return '';
          const dateStr = params[0].name;
          let html = `<span style="font-weight:600;color:${isDark ? '#f3f4f6' : '#0f172a'};display:block;margin-bottom:6px;font-size:12px;">${dateStr}</span>`;
          
          let totalTokens = 0;
          params.forEach((item: any) => {
            const val = item.value || 0;
            totalTokens += val;
            
            html += `<div style="display:flex;align-items:center;justify-content:space-between;gap:20px;margin-bottom:4px;font-size:11px;">
              <span style="display:inline-flex;align-items:center;gap:4px;color:${isDark ? '#9ca3af' : '#64748b'};">
                ${item.marker} ${item.seriesName}
              </span>
              <span style="font-weight:600;color:${isDark ? '#f3f4f6' : '#0f172a'};font-family:'JetBrains Mono', monospace;margin-left:auto;">
                ${val.toLocaleString('zh-CN')} Tokens
              </span>
            </div>`;
          });
          
          // 总计
          html += `<div style="margin-top:6px;padding-top:6px;border-top:1px solid ${isDark ? 'rgba(255,255,255,0.08)' : '#e2e8f0'};display:flex;align-items:center;justify-content:space-between;gap:20px;font-size:11px;">
            <span style="font-weight:600;color:${isDark ? '#06b6d4' : '#0891b2'};">总消耗 TOKEN</span>
            <span style="font-weight:700;color:${isDark ? '#06b6d4' : '#0891b2'};font-family:'JetBrains Mono', monospace;margin-left:auto;">
              ${totalTokens.toLocaleString('zh-CN')} Tokens
            </span>
          </div>`;
          
          return html;
        }
      },
      legend: {
        type: 'scroll',
        top: 0,
        right: 'center',
        icon: 'circle',
        itemGap: 16,
        textStyle: { color: isDark ? '#9ca3af' : '#64748b', fontSize: 10, fontFamily: 'Outfit' },
      },
      grid: { left: 45, right: 18, top: 40, bottom: 25 },
      xAxis: {
        type: 'category',
        data: dates,
        axisLabel: { color: isDark ? '#9ca3af' : '#64748b', fontSize: 10, fontFamily: 'Outfit' },
        axisLine: { lineStyle: { color: isDark ? 'rgba(255,255,255,0.08)' : '#e2e8f0' } },
        axisTick: { show: false },
      },
      yAxis: {
        type: 'value',
        axisLabel: {
          color: isDark ? '#9ca3af' : '#64748b',
          fontSize: 10,
          fontFamily: 'JetBrains Mono',
          formatter: (value: number) => {
            if (value >= 1000000) return (value / 1000000).toFixed(1) + 'M';
            if (value >= 1000) return (value / 1000).toFixed(0) + 'K';
            return value.toString();
          }
        },
        splitLine: { lineStyle: { color: isDark ? 'rgba(255,255,255,0.04)' : '#f1f5f9' } },
      },
      series: seriesData
    };
  }, [data, isDark]);

  return (
    <div style={{ height: '350px', width: '100%' }}>
      <ECharts option={chartOption as any} />
    </div>
  );
}
