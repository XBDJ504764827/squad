import { useEffect, useRef, useState } from 'react'
import Chart from 'chart.js/auto'
import Icon from './components/Icon'
import { createDefaultDashboardData } from './data/defaultDashboard'
import { dashboardApi } from './services/dashboardApi'

const DEFAULT_DASHBOARD = createDefaultDashboardData()

function getChartColors(theme) {
  const dark = theme === 'dark'
  return {
    text: dark ? '#a1a1aa' : '#94a3b8',
    grid: dark ? '#27272a' : '#f1f5f9',
    bg: dark ? '#111113' : '#ffffff',
  }
}

function formatCurrentTime() {
  return new Date().toLocaleString('zh-CN', {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
}

function Sparkline({ id, data, color }) {
  const canvasRef = useRef(null)

  useEffect(() => {
    if (!canvasRef.current) {
      return undefined
    }

    const chart = new Chart(canvasRef.current, {
      type: 'line',
      data: {
        labels: data.map((_, index) => index),
        datasets: [
          {
            data,
            borderColor: color,
            borderWidth: 2,
            pointRadius: 0,
            tension: 0.4,
            fill: true,
            backgroundColor: (context) => {
              const gradient = context.chart.ctx.createLinearGradient(0, 0, 0, 36)
              gradient.addColorStop(0, `${color}44`)
              gradient.addColorStop(1, `${color}00`)
              return gradient
            },
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: { display: false },
          tooltip: { enabled: false },
        },
        scales: {
          x: { display: false },
          y: { display: false },
        },
        animation: { duration: 0 },
      },
    })

    return () => {
      chart.destroy()
    }
  }, [color, data])

  return <canvas id={id} ref={canvasRef} height="36"></canvas>
}

function PlayersLineChart({ theme, chartData }) {
  const canvasRef = useRef(null)

  useEffect(() => {
    if (!canvasRef.current) {
      return undefined
    }

    const colors = getChartColors(theme)

    const chart = new Chart(canvasRef.current, {
      type: 'line',
      data: {
        labels: chartData.labels,
        datasets: [
          {
            label: '在线玩家',
            data: chartData.data,
            borderColor: '#6366f1',
            borderWidth: 2.5,
            pointRadius: 0,
            pointHoverRadius: 5,
            tension: 0.4,
            fill: true,
            backgroundColor: (context) => {
              const gradient = context.chart.ctx.createLinearGradient(0, 0, 0, 260)
              gradient.addColorStop(0, 'rgba(99,102,241,0.25)')
              gradient.addColorStop(1, 'rgba(99,102,241,0.00)')
              return gradient
            },
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        interaction: { mode: 'index', intersect: false },
        plugins: {
          legend: { display: false },
          tooltip: {
            backgroundColor: colors.bg,
            titleColor: colors.text,
            bodyColor: '#6366f1',
            borderColor: '#e2e8f0',
            borderWidth: 1,
            padding: 10,
            callbacks: {
              label: (item) => `  ${item.raw.toLocaleString()} 人`,
            },
          },
        },
        scales: {
          x: {
            grid: { color: colors.grid },
            ticks: { color: colors.text, font: { size: 11 }, maxTicksLimit: 12 },
          },
          y: {
            grid: { color: colors.grid },
            ticks: {
              color: colors.text,
              font: { size: 11 },
              callback: (value) => (value >= 1000 ? `${value / 1000}千` : value),
            },
          },
        },
      },
    })

    return () => {
      chart.destroy()
    }
  }, [chartData, theme])

  return <canvas id="playersChart" ref={canvasRef}></canvas>
}

function ResourceChart({ theme, chartData }) {
  const canvasRef = useRef(null)

  useEffect(() => {
    if (!canvasRef.current) {
      return undefined
    }

    const colors = getChartColors(theme)

    const chart = new Chart(canvasRef.current, {
      type: 'bar',
      data: {
        labels: chartData.labels,
        datasets: [
          {
            label: 'CPU',
            data: chartData.cpu,
            backgroundColor: 'rgba(99,102,241,0.8)',
            borderRadius: 4,
            borderSkipped: false,
          },
          {
            label: '内存',
            data: chartData.ram,
            backgroundColor: 'rgba(139,92,246,0.6)',
            borderRadius: 4,
            borderSkipped: false,
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: {
            labels: {
              color: colors.text,
              font: { size: 11 },
              boxWidth: 10,
              boxHeight: 10,
            },
          },
          tooltip: {
            backgroundColor: colors.bg,
            titleColor: colors.text,
            bodyColor: colors.text,
            borderColor: '#e2e8f0',
            borderWidth: 1,
          },
        },
        scales: {
          x: {
            grid: { display: false },
            ticks: { color: colors.text, font: { size: 11 } },
          },
          y: {
            grid: { color: colors.grid },
            ticks: {
              color: colors.text,
              font: { size: 11 },
              callback: (value) => `${value}%`,
            },
            max: 100,
          },
        },
      },
    })

    return () => {
      chart.destroy()
    }
  }, [chartData, theme])

  return <canvas id="resourceChart" ref={canvasRef}></canvas>
}

function DonutChart({ theme, chartData }) {
  const canvasRef = useRef(null)

  useEffect(() => {
    if (!canvasRef.current) {
      return undefined
    }

    const colors = getChartColors(theme)

    const chart = new Chart(canvasRef.current, {
      type: 'doughnut',
      data: {
        labels: chartData.labels,
        datasets: [
          {
            data: chartData.values,
            backgroundColor: chartData.colors,
            borderWidth: 3,
            borderColor: colors.bg,
            hoverOffset: 4,
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        cutout: '72%',
        plugins: {
          legend: { display: false },
          tooltip: {
            backgroundColor: colors.bg,
            titleColor: colors.text,
            bodyColor: colors.text,
            borderColor: '#e2e8f0',
            borderWidth: 1,
          },
        },
      },
    })

    return () => {
      chart.destroy()
    }
  }, [chartData, theme])

  return <canvas id="donutChart" ref={canvasRef}></canvas>
}

function BandwidthChart({ theme, chartData }) {
  const canvasRef = useRef(null)

  useEffect(() => {
    if (!canvasRef.current) {
      return undefined
    }

    const colors = getChartColors(theme)

    const chart = new Chart(canvasRef.current, {
      type: 'bar',
      data: {
        labels: chartData.labels,
        datasets: [
          {
            data: chartData.data,
            backgroundColor: chartData.colors,
            borderRadius: 5,
            borderSkipped: false,
          },
        ],
      },
      options: {
        indexAxis: 'y',
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: { display: false },
          tooltip: {
            backgroundColor: colors.bg,
            titleColor: colors.text,
            bodyColor: colors.text,
            borderColor: '#e2e8f0',
            borderWidth: 1,
            callbacks: {
              label: (item) => `${item.raw} GB/天`,
            },
          },
        },
        scales: {
          x: {
            grid: { color: colors.grid },
            ticks: {
              color: colors.text,
              font: { size: 10 },
              callback: (value) => `${value}GB`,
            },
          },
          y: {
            grid: { display: false },
            ticks: { color: colors.text, font: { size: 10 }, maxTicksLimit: 5 },
          },
        },
      },
    })

    return () => {
      chart.destroy()
    }
  }, [chartData, theme])

  return <canvas id="bandwidthChart" ref={canvasRef}></canvas>
}

function ProgressCell({ data }) {
  return (
    <div className="progress-bar-wrap">
      <div className="progress-bar-bg">
        <div className="progress-bar-fill" style={{ width: data.width, background: data.background }}></div>
      </div>
      <span className="progress-val" style={data.muted ? { color: 'var(--text-muted)' } : undefined}>
        {data.value}
      </span>
    </div>
  )
}

function TableActionButtons({ actions, row, index, onConsole, onRestart, onStart }) {
  return (
    <div style={{ display: 'flex', gap: '4px' }}>
      {actions.includes('console') && (
        <button
          className="btn btn-secondary btn-sm btn-icon"
          title={index === 0 ? '控制台' : undefined}
          type="button"
          onClick={() => onConsole(row)}
        >
          <Icon name="terminal" width={12} height={12} />
        </button>
      )}
      {actions.includes('restart') && (
        <button
          className="btn btn-secondary btn-sm btn-icon"
          title={index === 0 ? '重启' : undefined}
          type="button"
          onClick={() => onRestart(row)}
        >
          <Icon name="refresh" width={12} height={12} />
        </button>
      )}
      {actions.includes('start') && (
        <button
          className="btn btn-primary btn-sm"
          style={{ fontSize: '11px', padding: '4px 10px' }}
          type="button"
          onClick={() => onStart(row)}
        >
          启动
        </button>
      )}
    </div>
  )
}

function App() {
  const [dashboard, setDashboard] = useState(DEFAULT_DASHBOARD)
  const [theme, setTheme] = useState('light')
  const [collapsed, setCollapsed] = useState(false)
  const [currentTime, setCurrentTime] = useState(() => formatCurrentTime())
  const [globalSearch, setGlobalSearch] = useState('')
  const [tableSearch, setTableSearch] = useState('')
  const [gameFilter, setGameFilter] = useState(DEFAULT_DASHBOARD.table.gameOptions[0])
  const [statusFilter, setStatusFilter] = useState(DEFAULT_DASHBOARD.table.statusOptions[0])
  const [activeTab, setActiveTab] = useState(DEFAULT_DASHBOARD.playersOverview.activeTab)
  const [livePlayers, setLivePlayers] = useState(
    DEFAULT_DASHBOARD.stats.find((stat) => stat.label === '在线玩家')?.value ?? '4,892',
  )

  useEffect(() => {
    let cancelled = false

    async function loadDashboard() {
      const payload = await dashboardApi.getDashboardData()
      if (!cancelled) {
        setDashboard(payload)
        setActiveTab(payload.playersOverview.activeTab)
        setGameFilter(payload.table.gameOptions[0])
        setStatusFilter(payload.table.statusOptions[0])
        const onlinePlayersStat = payload.stats.find((stat) => stat.label === '在线玩家')
        if (onlinePlayersStat) {
          setLivePlayers(onlinePlayersStat.value)
        }
      }
    }

    loadDashboard()

    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme)
    document.title = 'GamePanel — 游戏服务器管理面板'
  }, [theme])

  useEffect(() => {
    const timer = window.setInterval(() => {
      setCurrentTime(formatCurrentTime())
    }, 1000)

    return () => {
      window.clearInterval(timer)
    }
  }, [])

  useEffect(() => {
    const timer = window.setInterval(() => {
      setLivePlayers((currentValue) => {
        const current = Number.parseInt(currentValue.replace(',', ''), 10)
        const delta = Math.floor(Math.random() * 20) - 8
        const next = Math.max(4600, Math.min(5200, current + delta))
        return next.toLocaleString()
      })
    }, 3000)

    return () => {
      window.clearInterval(timer)
    }
  }, [])

  const displayedStats = dashboard.stats.map((stat) =>
    stat.label === '在线玩家' ? { ...stat, value: livePlayers } : stat,
  )

  const handleRefresh = async () => {
    await dashboardApi.actions.onRefresh({ dashboard, filters: { globalSearch, tableSearch, gameFilter, statusFilter } })
  }

  const handleAddServer = async () => {
    await dashboardApi.actions.onAddServer()
  }

  const handleExportPlayers = async () => {
    await dashboardApi.actions.onExportPlayers({ range: activeTab })
  }

  const handleViewAllActivity = async () => {
    await dashboardApi.actions.onViewAllActivity()
  }

  const handleServerConsole = async (server) => {
    await dashboardApi.actions.onServerConsole(server)
  }

  const handleServerRestart = async (server) => {
    await dashboardApi.actions.onServerRestart(server)
  }

  const handleServerStart = async (server) => {
    await dashboardApi.actions.onServerStart(server)
  }

  return (
    <>
      <aside className={`sidebar${collapsed ? ' collapsed' : ''}`} id="sidebar">
        <div className="sidebar-logo">
          <div className="logo-icon">
            <Icon name="bolt" width={18} height={18} />
          </div>
          <span className="logo-text">
            Game<span>Panel</span>
          </span>
        </div>

        <nav className="sidebar-nav">
          {dashboard.sidebar.sections.map((section) => (
            <div key={section.label}>
              <div className="nav-section-label">{section.label}</div>
              {section.items.map((item) => (
                <div className={`nav-item${item.active ? ' active' : ''}`} key={item.label}>
                  <div className="nav-icon">
                    <Icon name={item.icon} width={17} height={17} />
                  </div>
                  <span className="nav-label">{item.label}</span>
                  {item.badge ? <span className="nav-badge">{item.badge}</span> : null}
                  <span className="nav-item-tooltip">{item.tooltip || item.label}</span>
                </div>
              ))}
            </div>
          ))}
        </nav>

        <div className="sidebar-footer">
          <div className="sidebar-user">
            <div className="user-avatar">{dashboard.sidebar.user.initials}</div>
            <div className="user-info">
              <div className="user-name">{dashboard.sidebar.user.name}</div>
              <div className="user-role">{dashboard.sidebar.user.role}</div>
            </div>
          </div>
        </div>
      </aside>

      <div className={`main-wrapper${collapsed ? ' expanded' : ''}`} id="mainWrapper">
        <header className="header">
          <button
            className="collapse-btn"
            id="collapseBtn"
            title="切换侧边栏"
            type="button"
            onClick={() => setCollapsed((currentValue) => !currentValue)}
          >
            <Icon name="menu" width={18} height={18} />
          </button>
          <div className="search-box">
            <Icon name="search" width={14} height={14} />
            <input
              type="text"
              placeholder={dashboard.header.searchPlaceholder}
              value={globalSearch}
              onChange={(event) => setGlobalSearch(event.target.value)}
            />
            <span
              style={{
                fontSize: '11px',
                color: 'var(--text-muted)',
                background: 'var(--bg-surface-2)',
                border: '1px solid var(--border)',
                padding: '2px 5px',
                borderRadius: '4px',
                whiteSpace: 'nowrap',
              }}
            >
              {dashboard.header.searchShortcut}
            </span>
          </div>
          <div className="header-right">
            <span className="status-live" style={{ padding: '0 8px' }}>
              <span className="status-live-dot"></span> {dashboard.header.liveLabel}
            </span>
            <div className="divider-v"></div>
            <button className="icon-btn" title="告警" type="button">
              <Icon name="bell" width={16} height={16} />
              <span className="notif-dot"></span>
            </button>
            <button className="icon-btn" title="终端" type="button">
              <Icon name="terminal" width={16} height={16} />
            </button>
            <button
              className="theme-btn"
              id="themeBtn"
              title="切换主题"
              type="button"
              onClick={() => setTheme((currentValue) => (currentValue === 'dark' ? 'light' : 'dark'))}
            >
              <span id="sunIcon" style={{ display: theme === 'dark' ? 'none' : undefined }}>
                <Icon name="sun" width={16} height={16} />
              </span>
              <span id="moonIcon" style={{ display: theme === 'dark' ? undefined : 'none' }}>
                <Icon name="moon" width={16} height={16} />
              </span>
            </button>
            <div className="divider-v"></div>
            <div className="header-avatar" title="个人资料">
              {dashboard.header.profileInitials}
            </div>
          </div>
        </header>

        <main className="content">
          <div className="page-header">
            <div>
              <div className="page-title">{dashboard.page.title}</div>
              <div className="page-subtitle">
                {dashboard.page.subtitle} · <span id="currentTime">{currentTime}</span>
              </div>
            </div>
            <div className="page-actions">
              <button className="btn btn-secondary btn-sm" type="button" onClick={handleRefresh}>
                <Icon name="refresh" width={13} height={13} />
                {dashboard.page.refreshLabel}
              </button>
              <button className="btn btn-primary btn-sm" type="button" onClick={handleAddServer}>
                <Icon name="plus" width={13} height={13} />
                {dashboard.page.addServerLabel}
              </button>
            </div>
          </div>

          <div className="stats-grid">
            {displayedStats.map((stat) => (
              <div className={`stat-card ${stat.color}`} key={stat.label}>
                <div className="stat-header">
                  <div className="stat-label">{stat.label}</div>
                  <div className={`stat-icon-wrap ${stat.color}`}>
                    <Icon name={stat.icon} width={16} height={16} />
                  </div>
                </div>
                <div className="stat-value">{stat.value}</div>
                <span className={`stat-change ${stat.changeDirection}`}>{stat.change}</span>
                <div className="stat-trend">{stat.trend}</div>
                <div className="sparkline-wrap">
                  <Sparkline id={stat.sparklineId} data={stat.sparklineData} color={stat.sparklineColor} />
                </div>
              </div>
            ))}
          </div>

          <div className="charts-grid" style={{ marginBottom: '24px' }}>
            <div className="card chart-wide">
              <div className="card-header">
                <div>
                  <div className="card-title">{dashboard.playersOverview.title}</div>
                  <div className="card-subtitle">{dashboard.playersOverview.subtitle}</div>
                </div>
                <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                  <div className="tabs">
                    {dashboard.playersOverview.tabs.map((tab) => (
                      <div
                        className={`tab${activeTab === tab ? ' active' : ''}`}
                        key={tab}
                        onClick={() => setActiveTab(tab)}
                      >
                        {tab}
                      </div>
                    ))}
                  </div>
                  <button className="btn btn-secondary btn-sm" type="button" onClick={handleExportPlayers}>
                    {dashboard.playersOverview.exportLabel}
                  </button>
                </div>
              </div>
              <div className="card-body">
                <div className="chart-container">
                  <PlayersLineChart theme={theme} chartData={dashboard.playersOverview} />
                </div>
              </div>
            </div>

            <div className="card">
              <div className="card-header">
                <div>
                  <div className="card-title">{dashboard.resourceChart.title}</div>
                  <div className="card-subtitle">{dashboard.resourceChart.subtitle}</div>
                </div>
              </div>
              <div className="card-body">
                <div className="chart-container-sm">
                  <ResourceChart theme={theme} chartData={dashboard.resourceChart} />
                </div>
              </div>
            </div>

            <div className="card">
              <div className="card-header">
                <div>
                  <div className="card-title">{dashboard.distribution.title}</div>
                  <div className="card-subtitle">{dashboard.distribution.subtitle}</div>
                </div>
              </div>
              <div className="donut-wrapper">
                <div className="donut-chart">
                  <DonutChart theme={theme} chartData={dashboard.distribution} />
                  <div className="donut-center">
                    <div className="donut-center-val">{dashboard.distribution.total}</div>
                    <div className="donut-center-label">{dashboard.distribution.totalLabel}</div>
                  </div>
                </div>
                <div className="donut-legend">
                  {dashboard.distribution.labels.map((label, index) => (
                    <div className="legend-item" key={label}>
                      <div className="legend-dot" style={{ background: dashboard.distribution.colors[index] }}></div>
                      <span className="legend-label">{label}</span>
                      <span className="legend-val">{dashboard.distribution.values[index]}</span>
                    </div>
                  ))}
                </div>
              </div>
              <div style={{ borderTop: '1px solid var(--border)', paddingTop: '16px' }} className="progress-section">
                {dashboard.nodeProgress.map((item) => (
                  <div className="progress-item" key={item.name}>
                    <div className="progress-meta">
                      <span className="progress-name">{item.name}</span>
                      <span className="progress-stat">{item.value}</span>
                    </div>
                    <div className="progress-track">
                      <div className="progress-fill" style={{ width: item.width, background: item.background }}></div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>

          <div className="bottom-grid">
            <div className="card">
              <div className="table-toolbar">
                <div style={{ flex: 1 }}>
                  <div className="card-title">{dashboard.table.title}</div>
                  <div className="card-subtitle" style={{ marginTop: '2px' }}>
                    {dashboard.table.subtitle}
                  </div>
                </div>
                <div className="table-actions">
                  <div className="table-search">
                    <Icon name="search" width={13} height={13} />
                    <input
                      type="text"
                      placeholder={dashboard.table.searchPlaceholder}
                      value={tableSearch}
                      onChange={(event) => setTableSearch(event.target.value)}
                    />
                  </div>
                  <select className="filter-select" value={gameFilter} onChange={(event) => setGameFilter(event.target.value)}>
                    {dashboard.table.gameOptions.map((option) => (
                      <option key={option}>{option}</option>
                    ))}
                  </select>
                  <select className="filter-select" value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}>
                    {dashboard.table.statusOptions.map((option) => (
                      <option key={option}>{option}</option>
                    ))}
                  </select>
                </div>
              </div>
              <div style={{ overflowX: 'auto' }}>
                <table className="data-table">
                  <thead>
                    <tr>
                      <th>服务器</th>
                      <th>游戏</th>
                      <th>状态</th>
                      <th>玩家</th>
                      <th>CPU</th>
                      <th>RAM</th>
                      <th>区域</th>
                      <th>操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {dashboard.table.rows.map((row, index) => (
                      <tr key={row.name}>
                        <td>
                          <div className="server-name">
                            <span className={`server-dot ${row.dot}`}></span>
                            <div>
                              <div className="server-name-text">{row.name}</div>
                              <div className="server-ip">{row.ip}</div>
                            </div>
                          </div>
                        </td>
                        <td>
                          <span className={row.game.className}>{row.game.label}</span>
                        </td>
                        <td>
                          <span className={row.status.className}>{row.status.label}</span>
                        </td>
                        <td style={{ fontFamily: "'DM Mono',monospace", fontSize: '12.5px' }}>{row.players}</td>
                        <td>
                          <ProgressCell data={row.cpu} />
                        </td>
                        <td>
                          <ProgressCell data={row.ram} />
                        </td>
                        <td>
                          <span className={row.region.className}>{row.region.label}</span>
                        </td>
                        <td>
                          <TableActionButtons
                            actions={row.actions}
                            row={row}
                            index={index}
                            onConsole={handleServerConsole}
                            onRestart={handleServerRestart}
                            onStart={handleServerStart}
                          />
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <div className="table-pagination">
                <span>{dashboard.table.pagination.summary}</span>
                <div className="pagination-pages">
                  {dashboard.table.pagination.pages.map((page) => (
                    <button
                      className={`page-btn${dashboard.table.pagination.active === page ? ' active' : ''}`}
                      key={page}
                      type="button"
                    >
                      {page}
                    </button>
                  ))}
                </div>
              </div>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div className="card">
                <div className="card-header">
                  <div className="card-title">核心指标</div>
                  <span className="badge badge-green pulse">实时</span>
                </div>
                <div className="kpi-grid">
                  {dashboard.quickKpis.map((item) => (
                    <div className="kpi-card" key={item.label}>
                      <div className="kpi-label">{item.label}</div>
                      <div className="kpi-value" style={item.color ? { color: item.color } : undefined}>
                        {item.value}
                      </div>
                      <div className="kpi-sub">{item.sub}</div>
                    </div>
                  ))}
                </div>
              </div>

              <div className="card">
                <div className="card-header">
                  <div className="card-title">{dashboard.nodeLocations.title}</div>
                </div>
                <div className="card-body-sm">
                  <div className="map-placeholder">
                    <div className="map-grid"></div>
                    {dashboard.nodeLocations.nodes.map((node) => (
                      <div
                        className="map-dot"
                        key={node.label}
                        style={{ background: node.background, left: node.left, top: node.top }}
                      >
                        <div className="map-dot-label">{node.label}</div>
                      </div>
                    ))}
                    <div style={{ position: 'absolute', bottom: '8px', left: '12px', fontSize: '11px', color: 'var(--text-muted)' }}>
                      {dashboard.nodeLocations.footer}
                    </div>
                  </div>
                </div>
              </div>

              <div className="card" style={{ flex: 1 }}>
                <div className="card-header" style={{ paddingBottom: '12px' }}>
                  <div className="card-title">最近动态</div>
                  <button className="btn btn-secondary btn-sm" type="button" onClick={handleViewAllActivity}>
                    查看全部
                  </button>
                </div>
                <div className="activity-list">
                  {dashboard.activities.map((activity, index) => (
                    <div className="activity-item" key={`${activity.time}-${index}`}>
                      <div className="activity-icon" style={activity.iconStyle}>
                        <Icon name={activity.icon} width={14} height={14} strokeWidth={2.5} />
                      </div>
                      <div className="activity-content">
                        <div className="activity-text">
                          {activity.text.before}
                          <strong>{activity.text.strong}</strong>
                          {activity.text.after}
                        </div>
                        <div className="activity-meta">
                          <span className={activity.badge.className} style={activity.badge.style}>
                            {activity.badge.label}
                          </span>
                          <span className="activity-time">{activity.time}</span>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>

          <div className="charts-grid" style={{ gridTemplateColumns: '1fr 1fr 1fr', marginBottom: 0 }}>
            <div className="card">
              <div className="card-header">
                <div className="card-title">在线玩家排行</div>
                <span className="status-live">
                  <span className="status-live-dot"></span>实时
                </span>
              </div>
              <div className="players-list">
                {dashboard.topPlayers.map((player) => (
                  <div className="player-row" key={player.name}>
                    <div className="player-avatar" style={{ background: player.background }}>
                      {player.initials}
                    </div>
                    <div className="player-info">
                      <div className="player-name">{player.name}</div>
                      <div className="player-server">{player.server}</div>
                    </div>
                    <span className="player-time">{player.time}</span>
                  </div>
                ))}
              </div>
            </div>

            <div className="card">
              <div className="card-header">
                <div className="card-title">{dashboard.bandwidth.title}</div>
              </div>
              <div className="card-body" style={{ paddingBottom: '8px' }}>
                <div className="chart-container-sm">
                  <BandwidthChart theme={theme} chartData={dashboard.bandwidth} />
                </div>
              </div>
            </div>

            <div className="card">
              <div className="card-header">
                <div className="card-title">{dashboard.networkHealth.title}</div>
              </div>
              <div className="card-body-sm">
                <div style={{ display: 'flex', flexDirection: 'column', gap: '10px', marginTop: '8px' }}>
                  {dashboard.networkHealth.regions.map((region) => (
                    <div key={region.name}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12.5px', marginBottom: '5px' }}>
                        <span style={{ color: 'var(--text-secondary)' }}>{region.name}</span>
                        <span style={{ fontFamily: "'DM Mono',monospace", color: region.color, fontWeight: 600 }}>{region.value}</span>
                      </div>
                      <div className="progress-track">
                        <div className="progress-fill" style={{ width: region.width, background: region.color }}></div>
                      </div>
                    </div>
                  ))}
                  <div
                    style={{
                      borderTop: '1px solid var(--border)',
                      paddingTop: '12px',
                      display: 'grid',
                      gridTemplateColumns: '1fr 1fr',
                      gap: '10px',
                      marginTop: '4px',
                    }}
                  >
                    {dashboard.networkHealth.stats.map((item) => (
                      <div style={{ textAlign: 'center' }} key={item.label}>
                        <div
                          style={{
                            fontSize: '18px',
                            fontWeight: 700,
                            fontFamily: "'DM Mono',monospace",
                            color: item.color,
                          }}
                        >
                          {item.value}
                        </div>
                        <div style={{ fontSize: '11px', color: 'var(--text-muted)' }}>{item.label}</div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </main>
      </div>
    </>
  )
}

export default App
