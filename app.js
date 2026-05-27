// ==========================================================================
// Antigravity Token Monitor Front-end Application
// ==========================================================================

document.addEventListener("DOMContentLoaded", () => {
    // 全局数据存储
    let dashboardData = null;
    let currentSortField = "created_at";
    let currentSortOrder = "desc"; // desc | asc
    let trendChart = null;

    // DOM 元素引用
    const loadingOverlay = document.getElementById("loading-overlay");
    const refreshBtn = document.getElementById("refresh-btn");
    const lastUpdateSpan = document.getElementById("last-update");
    const searchInput = document.getElementById("session-search");
    const hideZeroCheckbox = document.getElementById("hide-zero-sessions");

    // KPI 元素
    const kpiTotalTokens = document.getElementById("kpi-total-tokens");
    const kpiTotalInput = document.getElementById("kpi-total-input");
    const kpiTotalOutput = document.getElementById("kpi-total-output");
    const kpiCacheRate = document.getElementById("kpi-cache-rate");
    const kpiThinkingRate = document.getElementById("kpi-thinking-rate");
    const kpiTotalCached = document.getElementById("kpi-total-cached");
    const kpiTotalThinking = document.getElementById("kpi-total-thinking");
    const kpiTotalSessions = document.getElementById("kpi-total-sessions");

    // 表格及容器
    const modelContainer = document.getElementById("model-list-container");
    const monthlyTableBody = document.getElementById("monthly-summary-table");
    const sessionsTableBody = document.getElementById("sessions-detail-table");

    // 格式化数字
    const formatNum = (num) => new Intl.NumberFormat('zh-CN').format(num || 0);
    // 格式化百分比
    const formatPercent = (val) => (val * 100).toFixed(1) + "%";
    // 格式化日期：将 2026-05-25T06:00:20Z 转换为本地更好看的格式
    const formatDate = (isoStr) => {
        if (!isoStr) return "--";
        try {
            const date = new Date(isoStr);
            if (isNaN(date.getTime())) return isoStr;
            const y = date.getFullYear();
            const m = String(date.getMonth() + 1).padStart(2, '0');
            const d = String(date.getDate()).padStart(2, '0');
            const hh = String(date.getHours()).padStart(2, '0');
            const mm = String(date.getMinutes()).padStart(2, '0');
            return `${y}-${m}-${d} ${hh}:${mm}`;
        } catch (e) {
            return isoStr;
        }
    };

    // 1. 数据获取核心函数
    async function fetchDashboardData() {
        showLoading(true);
        refreshBtn.classList.add("spinning");
        try {
            // 添加随机查询参数，防止 HTTP 缓存
            const response = await fetch(`/api/metrics?t=${Date.now()}`);
            if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
            dashboardData = await response.json();
            
            // 更新最后刷新时间
            const now = new Date();
            lastUpdateSpan.textContent = now.toTimeString().split(' ')[0];
            
            // 渲染各项数据
            renderDashboard();
        } catch (error) {
            console.error("Fetch data failed:", error);
            alert("抓取用量统计数据失败，请确认后端服务运行正常。");
        } finally {
            showLoading(false);
            refreshBtn.classList.remove("spinning");
        }
    }

    // 2. 加载状态控制
    function showLoading(show) {
        if (show) {
            loadingOverlay.classList.add("active");
        } else {
            loadingOverlay.classList.remove("active");
        }
    }

    // 3. 渲染主仪表盘
    function renderDashboard() {
        if (!dashboardData) return;

        // A. 填充 KPI
        const t = dashboardData.totals;
        kpiTotalTokens.textContent = formatNum(t.total_tokens);
        kpiTotalInput.textContent = formatNum(t.total_input);
        kpiTotalOutput.textContent = formatNum(t.total_output);
        kpiCacheRate.textContent = formatPercent(t.cache_hit_rate);
        kpiThinkingRate.textContent = formatPercent(t.thinking_ratio);
        kpiTotalCached.textContent = formatNum(t.total_cached);
        kpiTotalThinking.textContent = formatNum(t.total_thinking);
        kpiTotalSessions.textContent = formatNum(t.total_sessions);

        // B. 绘制图表
        renderDailyChart(dashboardData.daily_trends);

        // C. 渲染模型排行进度条
        renderModelDistribution(dashboardData.model_distribution);

        // D. 渲染按月用量表格
        renderMonthlyTable(dashboardData.monthly_summary);

        // E. 渲染会话明细列表
        renderSessionsTable();
    }

    // 4. 绘制 Chart.js 图表
    function renderDailyChart(trends) {
        const ctx = document.getElementById("daily-trend-chart").getContext("2d");
        
        if (trendChart) {
            trendChart.destroy();
        }

        if (!trends || trends.length === 0) {
            ctx.clearRect(0, 0, 400, 400);
            return;
        }

        const dates = trends.map(t => t.date);
        const cachedData = trends.map(t => t.cached);
        // 未缓存 = 总输入 - 缓存
        const uncachedData = trends.map(t => Math.max(0, t.input - t.cached));
        const outputData = trends.map(t => t.output);
        const thinkingData = trends.map(t => t.thinking);

        trendChart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: dates,
                datasets: [
                    {
                        label: '缓存输入 Token',
                        data: cachedData,
                        backgroundColor: 'rgba(20, 184, 166, 0.4)',
                        borderColor: 'rgba(20, 184, 166, 0.8)',
                        borderWidth: 1,
                        stack: 'stack0',
                        order: 2
                    },
                    {
                        label: '未缓存输入 Token',
                        data: uncachedData,
                        backgroundColor: 'rgba(6, 182, 212, 0.65)',
                        borderColor: 'rgba(6, 182, 212, 0.95)',
                        borderWidth: 1,
                        stack: 'stack0',
                        order: 2
                    },
                    {
                        label: '输出 Token',
                        data: outputData,
                        backgroundColor: 'rgba(236, 72, 153, 0.65)',
                        borderColor: 'rgba(236, 72, 153, 0.95)',
                        borderWidth: 1,
                        stack: 'stack0',
                        order: 2
                    },
                    {
                        label: '推理 Token',
                        data: thinkingData,
                        type: 'line',
                        borderColor: '#a855f7',
                        borderWidth: 2,
                        pointBackgroundColor: '#a855f7',
                        pointBorderColor: '#ffffff',
                        pointHoverRadius: 6,
                        tension: 0.35,
                        fill: false,
                        order: 1
                    }
                ]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        display: false // 自定义 HTML 图例以支持更高端视觉效果
                    },
                    tooltip: {
                        mode: 'index',
                        intersect: false,
                        backgroundColor: 'rgba(15, 23, 42, 0.85)',
                        titleColor: '#fff',
                        bodyColor: '#e2e8f0',
                        borderColor: 'rgba(255,255,255,0.1)',
                        borderWidth: 1,
                        padding: 12,
                        callbacks: {
                            label: function(context) {
                                let label = context.dataset.label || '';
                                if (label) {
                                    label += ': ';
                                }
                                if (context.parsed.y !== null) {
                                    label += formatNum(context.parsed.y);
                                }
                                return label;
                            },
                            footer: function(tooltipItems) {
                                let sum = 0;
                                tooltipItems.forEach(function(tooltipItem) {
                                    // 0: 缓存输入, 1: 未缓存输入, 2: 输出
                                    // 仅对柱状图这三个堆叠层累加，排除第 4 个数据集折线图（推理已包含在输出中）
                                    if (tooltipItem.datasetIndex >= 0 && tooltipItem.datasetIndex <= 2) {
                                        sum += tooltipItem.parsed.y;
                                    }
                                });
                                return '总消耗 TOKEN: ' + formatNum(sum);
                            }
                        },
                        footerColor: '#06b6d4',
                        footerFont: { family: 'Outfit', weight: 'bold', size: 13 },
                        footerMarginTop: 8
                    }
                },
                scales: {
                    x: {
                        stacked: true,
                        grid: {
                            color: 'rgba(255, 255, 255, 0.03)',
                            drawBorder: false
                        },
                        ticks: {
                            color: '#9ca3af',
                            font: { family: 'Outfit' }
                        }
                    },
                    y: {
                        stacked: true,
                        grid: {
                            color: 'rgba(255, 255, 255, 0.05)',
                            drawBorder: false
                        },
                        ticks: {
                            color: '#9ca3af',
                            font: { family: 'JetBrains Mono' },
                            callback: function(value) {
                                if (value >= 1000000) return (value / 1000000).toFixed(1) + 'M';
                                if (value >= 1000) return (value / 1000).toFixed(0) + 'K';
                                return value;
                            }
                        }
                    }
                }
            }
        });
    }

    // 5. 渲染模型使用进度条列表
    function renderModelDistribution(models) {
        modelContainer.innerHTML = "";
        if (!models || models.length === 0) {
            modelContainer.innerHTML = '<div class="no-data">暂无模型数据</div>';
            return;
        }

        // 以消耗最多的模型为 100% 进度基准
        const maxTokens = Math.max(...models.map(m => m.total_tokens));

        models.forEach(m => {
            const pct = maxTokens > 0 ? (m.total_tokens / maxTokens) * 100 : 0;
            const modelDiv = document.createElement("div");
            modelDiv.className = "model-item";
            modelDiv.innerHTML = `
                <div class="model-info-row">
                    <span class="model-name">${m.model}</span>
                    <span class="model-tokens number-font">${formatNum(m.total_tokens)} Tokens</span>
                </div>
                <div class="progress-bar-bg">
                    <div class="progress-bar-fill" style="width: ${pct}%"></div>
                </div>
            `;
            modelContainer.appendChild(modelDiv);
        });
    }

    // 6. 渲染月度汇总表格
    function renderMonthlyTable(summary) {
        monthlyTableBody.innerHTML = "";
        if (!summary || summary.length === 0) {
            monthlyTableBody.innerHTML = '<tr><td colspan="6" class="text-center">暂无月度数据</td></tr>';
            return;
        }

        summary.forEach(row => {
            const tr = document.createElement("tr");
            tr.innerHTML = `
                <td class="number-font">${row.month}</td>
                <td class="number-font text-right">${formatNum(row.sessions)}</td>
                <td class="number-font text-right">${formatNum(row.input)}</td>
                <td class="number-font text-right">${formatNum(row.output)}</td>
                <td class="number-font text-right">${formatNum(row.cached)}</td>
                <td class="number-font text-right">${formatNum(row.thinking)}</td>
            `;
            monthlyTableBody.appendChild(tr);
        });
    }

    // 7. 渲染/搜索/排序会话明细表格
    function renderSessionsTable() {
        sessionsTableBody.innerHTML = "";
        if (!dashboardData || !dashboardData.sessions || dashboardData.sessions.length === 0) {
            sessionsTableBody.innerHTML = '<tr><td colspan="8" class="text-center">暂无会话数据</td></tr>';
            return;
        }

        // A. 搜索过滤
        const keyword = searchInput.value.toLowerCase().trim();
        const hideZero = hideZeroCheckbox ? hideZeroCheckbox.checked : false;
        
        let filtered = dashboardData.sessions.filter(s => {
            // 如果勾选了隐藏0消耗，则过滤掉总消耗（输入+输出）为0的会话
            if (hideZero && (s.input + s.output) === 0) {
                return false;
            }
            return s.title.toLowerCase().includes(keyword) || 
                   s.uuid.toLowerCase().includes(keyword) ||
                   s.models.some(m => m.toLowerCase().includes(keyword));
        });

        // B. 数据排序
        filtered.sort((a, b) => {
            let valA, valB;
            if (currentSortField === "title") {
                valA = a.title;
                valB = b.title;
            } else if (currentSortField === "created_at") {
                valA = new Date(a.created_at).getTime();
                valB = new Date(b.created_at).getTime();
            } else if (currentSortField === "models") {
                valA = a.models.join(",");
                valB = b.models.join(",");
            } else if (currentSortField === "total") {
                valA = a.input + a.output;
                valB = b.input + b.output;
            } else {
                valA = a[currentSortField] || 0;
                valB = b[currentSortField] || 0;
            }

            if (valA < valB) return currentSortOrder === "asc" ? -1 : 1;
            if (valA > valB) return currentSortOrder === "asc" ? 1 : -1;
            return 0;
        });

        if (filtered.length === 0) {
            sessionsTableBody.innerHTML = '<tr><td colspan="8" class="text-center">没有符合条件的会话记录</td></tr>';
            return;
        }

        // C. 动态渲染 DOM
        filtered.forEach(s => {
            const totalTokens = s.input + s.output;
            const tr = document.createElement("tr");
            
            // 构造模型 Tag
            const tagsHtml = s.models.map(m => `<span class="model-tag">${m}</span>`).join("");

            tr.innerHTML = `
                <td class="session-title-cell">
                    ${s.title}
                    <span class="uuid-sub">${s.uuid}</span>
                </td>
                <td>${formatDate(s.created_at)}</td>
                <td>${tagsHtml ? tagsHtml : '<span class="model-tag">unknown</span>'}</td>
                <td class="number-font text-right">${formatNum(s.input)}</td>
                <td class="number-font text-right">${formatNum(s.output)}</td>
                <td class="number-font text-right">${formatNum(s.cached)}</td>
                <td class="number-font text-right">${formatNum(s.thinking)}</td>
                <td class="number-font text-right" style="font-weight: 700; color: var(--cyan);">${formatNum(totalTokens)}</td>
            `;
            sessionsTableBody.appendChild(tr);
        });
    }

    // 8. 表头点击排序逻辑绑定
    document.querySelectorAll("th.sortable").forEach(th => {
        th.addEventListener("click", () => {
            const field = th.getAttribute("data-sort");
            if (currentSortField === field) {
                // 相同字段反转顺序
                currentSortOrder = currentSortOrder === "desc" ? "asc" : "desc";
            } else {
                // 不同字段默认设为降序
                currentSortField = field;
                currentSortOrder = "desc";
            }
            
            // 更新排序小图标的视觉反馈 (通过重绘表格)
            renderSessionsTable();
        });
    });

    // 9. 输入框及复选框搜索监听
    searchInput.addEventListener("input", renderSessionsTable);
    if (hideZeroCheckbox) {
        hideZeroCheckbox.addEventListener("change", renderSessionsTable);
    }

    // 10. 刷新按钮事件
    refreshBtn.addEventListener("click", fetchDashboardData);

    // 初始化自动加载
    fetchDashboardData();
});
