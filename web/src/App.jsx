import { useEffect, useRef, useState } from 'react'
import Chart from 'chart.js/auto'
import Icon from './components/Icon'
import { createEmptyDashboardData } from './data/defaultDashboard'
import { dashboardApi } from './services/dashboardApi'
import { buildServerManagementPage, buildServerViewAfterDelete } from './services/serverManagementView'
import {
  DEFAULT_WORKBENCH_SECTION,
  SERVER_WORKBENCH_SECTIONS,
  appendAgentLogChunk,
  buildConfigFileItems,
  canUseAgentWorkbench,
  createServerWorkbenchContent,
  describeAgentAuthStatus,
  filterRealtimeLogEntries,
  normalizeAgentStreamEvent,
  normalizeWorkbenchSection,
} from './services/serverWorkbench'

const EMPTY_DASHBOARD = createEmptyDashboardData()

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

function splitServerAddress(address) {
  const separatorIndex = address.lastIndexOf(':')

  if (separatorIndex === -1) {
    return {
      ip: address,
      rconPort: '--',
    }
  }

  return {
    ip: address.slice(0, separatorIndex),
    rconPort: address.slice(separatorIndex + 1),
  }
}

function EmptyState({ message }) {
  return <div className="empty-state">{message}</div>
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

function TableActionButtons({
  actions,
  row,
  onManage,
  onEdit,
  onDelete,
  onConsole,
  onRestart,
  onStart,
}) {
  return (
    <div className="table-action-group">
      {actions.includes('manage') && (
        <button className="btn btn-primary btn-sm" type="button" onClick={() => onManage(row)}>
          管理
        </button>
      )}
      {actions.includes('edit') && (
        <button
          className="btn btn-secondary btn-sm btn-icon"
          title="编辑服务器"
          type="button"
          onClick={() => onEdit(row)}
        >
          <Icon name="pencil" width={12} height={12} />
        </button>
      )}
      {actions.includes('delete') && (
        <button
          className="btn btn-danger btn-sm btn-icon"
          title="删除服务器"
          type="button"
          onClick={() => onDelete(row)}
        >
          <Icon name="trash" width={12} height={12} />
        </button>
      )}
      {actions.includes('console') && (
        <button
          className="btn btn-secondary btn-sm btn-icon"
          title="控制台"
          type="button"
          onClick={() => onConsole(row)}
        >
          <Icon name="terminal" width={12} height={12} />
        </button>
      )}
      {actions.includes('restart') && (
        <button
          className="btn btn-secondary btn-sm btn-icon"
          title="重启"
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

function AddServerModal({
  title = '添加服务器',
  subtitle = '录入服务器信息后，系统会先进行 RCON 验证。',
  submitLabel = '验证并添加',
  form,
  submitting,
  error,
  onChange,
  onClose,
  onSubmit,
}) {
  return (
    <div className="modal-backdrop">
      <div className="modal-card">
        <div className="modal-header">
          <div>
            <div className="modal-title">{title}</div>
            <div className="modal-subtitle">{subtitle}</div>
          </div>
          <button className="icon-btn" type="button" title="关闭" onClick={onClose}>
            <Icon name="plus" width={14} height={14} style={{ transform: 'rotate(45deg)' }} />
          </button>
        </div>
        <form className="modal-form" onSubmit={onSubmit}>
          <label className="form-field">
            <span className="form-label">服务器名称</span>
            <input
              className="form-input"
              name="name"
              value={form.name}
              onChange={onChange}
              placeholder="例如：rust-pvp-main"
            />
          </label>
          <label className="form-field">
            <span className="form-label">服务器 IP</span>
            <input
              className="form-input"
              name="ip"
              value={form.ip}
              onChange={onChange}
              placeholder="例如：127.0.0.1"
            />
          </label>
          <label className="form-field">
            <span className="form-label">RCON 端口</span>
            <input
              className="form-input"
              name="rconPort"
              value={form.rconPort}
              onChange={onChange}
              inputMode="numeric"
              placeholder="例如：25575"
            />
          </label>
          <label className="form-field">
            <span className="form-label">RCON 密码</span>
            <input
              className="form-input"
              name="rconPassword"
              type="password"
              value={form.rconPassword}
              onChange={onChange}
              placeholder="请输入 RCON 密码"
            />
          </label>
          {error ? <div className="form-error">{error}</div> : null}
          <div className="modal-actions">
            <button className="btn btn-secondary" type="button" onClick={onClose} disabled={submitting}>
              取消
            </button>
            <button className="btn btn-primary" type="submit" disabled={submitting}>
              {submitting ? '提交中…' : submitLabel}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

function DeleteServerModal({ server, submitting, error, onClose, onConfirm }) {
  return (
    <div className="modal-backdrop">
      <div className="modal-card modal-card-sm">
        <div className="modal-header">
          <div>
            <div className="modal-title">删除服务器</div>
            <div className="modal-subtitle">该操作会移除当前服务器记录，删除后不可恢复。</div>
          </div>
          <button className="icon-btn" type="button" title="关闭" onClick={onClose}>
            <Icon name="plus" width={14} height={14} style={{ transform: 'rotate(45deg)' }} />
          </button>
        </div>
        <div className="confirm-body">
          <div className="confirm-text">
            确认删除服务器 <strong>{server.name}</strong> 吗？
          </div>
          <div className="confirm-meta">
            <div>地址：{server.ip}</div>
            <div>UUID：{server.uuid}</div>
          </div>
          {error ? <div className="form-error">{error}</div> : null}
          <div className="modal-actions">
            <button className="btn btn-secondary" type="button" onClick={onClose} disabled={submitting}>
              取消
            </button>
            <button className="btn btn-danger" type="button" onClick={onConfirm} disabled={submitting}>
              {submitting ? '删除中…' : '确认删除'}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

const NAVIGATION_VIEW_MAP = {
  仪表盘: 'dashboard',
  服务器管理: 'server-manager',
}

function App() {
  const [dashboard, setDashboard] = useState(EMPTY_DASHBOARD)
  const [theme, setTheme] = useState('light')
  const [collapsed, setCollapsed] = useState(false)
  const [activeView, setActiveView] = useState('dashboard')
  const [selectedServerUuid, setSelectedServerUuid] = useState(null)
  const [activeWorkbenchSection, setActiveWorkbenchSection] = useState(DEFAULT_WORKBENCH_SECTION)
  const [realtimeLogEntries, setRealtimeLogEntries] = useState([])
  const [realtimeLogLevel, setRealtimeLogLevel] = useState('ALL')
  const [realtimeLogSearchTerm, setRealtimeLogSearchTerm] = useState('')
  const [realtimeLogPaused, setRealtimeLogPaused] = useState(false)
  const [realtimeLogAutoScroll, setRealtimeLogAutoScroll] = useState(true)
  const [realtimeLogConnectionLabel, setRealtimeLogConnectionLabel] = useState('等待连接')
  const [currentTime, setCurrentTime] = useState(() => formatCurrentTime())
  const [globalSearch, setGlobalSearch] = useState('')
  const [tableSearch, setTableSearch] = useState('')
  const [gameFilter, setGameFilter] = useState(EMPTY_DASHBOARD.table.gameOptions[0])
  const [statusFilter, setStatusFilter] = useState(EMPTY_DASHBOARD.table.statusOptions[0])
  const [activeTab, setActiveTab] = useState(EMPTY_DASHBOARD.playersOverview.activeTab)
  const [isLoading, setIsLoading] = useState(true)
  const [isServerFormModalOpen, setIsServerFormModalOpen] = useState(false)
  const [serverFormMode, setServerFormMode] = useState('add')
  const [editingServerUuid, setEditingServerUuid] = useState(null)
  const [serverForm, setServerForm] = useState({
    name: '',
    ip: '',
    rconPort: '',
    rconPassword: '',
  })
  const [serverFormError, setServerFormError] = useState('')
  const [serverFormSubmitting, setServerFormSubmitting] = useState(false)
  const [deleteTargetServer, setDeleteTargetServer] = useState(null)
  const [deleteServerSubmitting, setDeleteServerSubmitting] = useState(false)
  const [deleteServerError, setDeleteServerError] = useState('')
  const [managedServerDetail, setManagedServerDetail] = useState(null)
  const [serverAgentAuth, setServerAgentAuth] = useState(null)
  const [isServerAgentAuthLoading, setIsServerAgentAuthLoading] = useState(false)
  const [serverAgentAuthError, setServerAgentAuthError] = useState('')
  const [agentKeySubmitting, setAgentKeySubmitting] = useState(false)
  const [generatedAgentKey, setGeneratedAgentKey] = useState('')
  const [isServerDetailLoading, setIsServerDetailLoading] = useState(false)
  const [serverDetailError, setServerDetailError] = useState('')
  const [flashMessage, setFlashMessage] = useState(null)
  const [dashboardLoadError, setDashboardLoadError] = useState('')
  const [connectionStatus, setConnectionStatus] = useState({
    tone: 'loading',
    label: '连接中',
  })
  const [configFileItems, setConfigFileItems] = useState([])
  const [selectedConfigFilePath, setSelectedConfigFilePath] = useState('')
  const [configFileContent, setConfigFileContent] = useState('')
  const [configFileDraft, setConfigFileDraft] = useState('')
  const [configFileVersion, setConfigFileVersion] = useState('')
  const [configFileLoading, setConfigFileLoading] = useState(false)
  const [configFileSaving, setConfigFileSaving] = useState(false)
  const [configFileError, setConfigFileError] = useState('')
  const [configFileRefreshToken, setConfigFileRefreshToken] = useState(0)
  const realtimeLogViewportRef = useRef(null)
  const selectedConfigFilePathRef = useRef('')
  const configFileDirtyRef = useRef(false)
  const normalizedWorkbenchSection = normalizeWorkbenchSection(activeWorkbenchSection)
  const activeWorkbenchSectionConfig = SERVER_WORKBENCH_SECTIONS.find(
    (section) => section.id === normalizedWorkbenchSection,
  ) ?? SERVER_WORKBENCH_SECTIONS[0]
  const serverManagementPage = buildServerManagementPage({
    rows: dashboard.table.rows,
    activeView,
    selectedServerUuid,
  })
  const isConfigFileDirty = configFileDraft !== configFileContent

  const applyDashboardPayload = (payload) => {
    setDashboard(payload)
    setActiveTab(payload.playersOverview.activeTab || payload.playersOverview.tabs[0] || '')
    setGameFilter(payload.table.gameOptions[0] || '')
    setStatusFilter(payload.table.statusOptions[0] || '')
    setDashboardLoadError('')
    setConnectionStatus({
      tone: 'connected',
      label: payload.header.liveLabel || '后端已连接',
    })
  }

  const applyEmptyDashboardState = (message) => {
    setDashboard(createEmptyDashboardData())
    setActiveTab(EMPTY_DASHBOARD.playersOverview.activeTab)
    setGameFilter(EMPTY_DASHBOARD.table.gameOptions[0])
    setStatusFilter(EMPTY_DASHBOARD.table.statusOptions[0])
    setDashboardLoadError(message)
    setConnectionStatus({
      tone: 'error',
      label: '后端未连接',
    })
  }

  const resetServerForm = () => {
    setServerForm({
      name: '',
      ip: '',
      rconPort: '',
      rconPassword: '',
    })
    setServerFormError('')
    setEditingServerUuid(null)
  }

  const loadServerDetail = async (serverUuid) => {
    setIsServerDetailLoading(true)
    setServerDetailError('')

    try {
      const detail = await dashboardApi.getServer(serverUuid)
      setManagedServerDetail(detail)
      return detail
    } catch (error) {
      const message = error instanceof Error ? error.message : '读取服务器详情失败，请稍后重试。'
      setManagedServerDetail(null)
      setServerDetailError(message)
      throw new Error(message)
    } finally {
      setIsServerDetailLoading(false)
    }
  }

  const loadServerAgentAuth = async (serverUuid) => {
    setIsServerAgentAuthLoading(true)
    setServerAgentAuthError('')

    try {
      const auth = await dashboardApi.getServerAgentAuth(serverUuid)
      setServerAgentAuth(auth)
      return auth
    } catch (error) {
      const message = error instanceof Error ? error.message : '读取 Agent 鉴权状态失败，请稍后重试。'
      setServerAgentAuth(null)
      setServerAgentAuthError(message)
      throw new Error(message)
    } finally {
      setIsServerAgentAuthLoading(false)
    }
  }

  useEffect(() => {
    let cancelled = false

    async function loadInitialDashboardData() {
      setIsLoading(true)
      setConnectionStatus({
        tone: 'loading',
        label: '连接中',
      })
      setDashboardLoadError('')

      try {
        const payload = await dashboardApi.getDashboardData()
        if (cancelled) {
          return
        }

        applyDashboardPayload(payload)
      } catch (error) {
        console.error('加载仪表盘数据失败', error)
        if (cancelled) {
          return
        }

        applyEmptyDashboardState(
          error instanceof Error
            ? `后端联调失败：${error.message}`
            : '后端联调失败，请确认后端和数据库已启动。',
        )
      } finally {
        if (!cancelled) {
          setIsLoading(false)
        }
      }
    }

    loadInitialDashboardData()

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
    setRealtimeLogEntries([])
    setRealtimeLogLevel('ALL')
    setRealtimeLogSearchTerm('')
    setRealtimeLogPaused(false)
    setRealtimeLogAutoScroll(true)
    setRealtimeLogConnectionLabel('等待连接')
    setConfigFileItems([])
    setSelectedConfigFilePath('')
    setConfigFileContent('')
    setConfigFileDraft('')
    setConfigFileVersion('')
    setConfigFileError('')
    setConfigFileRefreshToken(0)
  }, [managedServerDetail?.agentId, selectedServerUuid])

  useEffect(() => {
    setServerAgentAuth(null)
    setServerAgentAuthError('')
    setGeneratedAgentKey('')
  }, [selectedServerUuid])

  const canAccessAgentWorkbench = canUseAgentWorkbench({
    hasKey: serverAgentAuth?.hasKey,
    agentOnline: managedServerDetail?.agentOnline,
    agentId: managedServerDetail?.agentId,
  })

  useEffect(() => {
    selectedConfigFilePathRef.current = selectedConfigFilePath
    configFileDirtyRef.current = isConfigFileDirty
  }, [selectedConfigFilePath, isConfigFileDirty])

  const filteredRealtimeLogEntries = filterRealtimeLogEntries(realtimeLogEntries, {
    level: realtimeLogLevel,
    searchTerm: realtimeLogSearchTerm,
  })

  useEffect(() => {
    if (activeView !== 'server-detail' || !canAccessAgentWorkbench) {
      return undefined
    }

    const rootPath = managedServerDetail.workspaceRoots?.[0]?.logicalPath
    if (!rootPath) {
      setConfigFileItems([])
      setSelectedConfigFilePath('')
      return undefined
    }

    let cancelled = false

    async function loadConfigTree() {
      try {
        const payload = await dashboardApi.getAgentFileTree(managedServerDetail.agentId, rootPath)
        if (cancelled) {
          return
        }

        const nextItems = buildConfigFileItems(payload.entries ?? [])
        setConfigFileItems(nextItems)
        setConfigFileError('')
        setSelectedConfigFilePath((currentPath) => {
          if (currentPath && nextItems.some((item) => item.path === currentPath && !item.isDir)) {
            return currentPath
          }

          return nextItems.find((item) => !item.isDir)?.path ?? ''
        })
      } catch (error) {
        if (cancelled) {
          return
        }

        setConfigFileItems([])
        setSelectedConfigFilePath('')
        setConfigFileError(error instanceof Error ? error.message : '读取配置目录失败')
      }
    }

    loadConfigTree()

    return () => {
      cancelled = true
    }
  }, [activeView, canAccessAgentWorkbench, managedServerDetail?.agentId, managedServerDetail?.workspaceRoots, configFileRefreshToken])

  useEffect(() => {
    if (activeView !== 'server-detail' || !canAccessAgentWorkbench || !selectedConfigFilePath) {
      return undefined
    }

    const selectedItem = configFileItems.find((item) => item.path === selectedConfigFilePath)
    if (selectedItem?.isDir) {
      return undefined
    }

    let cancelled = false

    async function loadConfigFile() {
      setConfigFileLoading(true)

      try {
        const payload = await dashboardApi.getAgentFileContent(
          managedServerDetail.agentId,
          selectedConfigFilePath,
        )
        if (cancelled) {
          return
        }

        setConfigFileContent(payload.content ?? '')
        setConfigFileDraft(payload.content ?? '')
        setConfigFileVersion(payload.version ?? '')
        setConfigFileError('')
      } catch (error) {
        if (cancelled) {
          return
        }

        setConfigFileContent('')
        setConfigFileDraft('')
        setConfigFileVersion('')
        setConfigFileError(error instanceof Error ? error.message : '读取文件内容失败')
      } finally {
        if (!cancelled) {
          setConfigFileLoading(false)
        }
      }
    }

    loadConfigFile()

    return () => {
      cancelled = true
    }
  }, [activeView, canAccessAgentWorkbench, managedServerDetail?.agentId, selectedConfigFilePath, configFileItems, configFileRefreshToken])

  useEffect(() => {
    if (activeView !== 'server-detail' || !canAccessAgentWorkbench) {
      return undefined
    }

    let eventSource

    try {
      eventSource = dashboardApi.openAgentEvents(managedServerDetail.agentId)
      setRealtimeLogConnectionLabel('连接中')
    } catch {
      setRealtimeLogConnectionLabel('当前环境不支持 SSE')
      return undefined
    }

    const handleLogChunk = (event) => {
      const normalizedEvent = normalizeAgentStreamEvent(event.type, event.data)
      if (!normalizedEvent || normalizedEvent.type !== 'logChunk') {
        return
      }

      setRealtimeLogConnectionLabel('已连接日志频道')
      if (realtimeLogPaused) {
        return
      }

      setRealtimeLogEntries((currentEntries) =>
        appendAgentLogChunk(currentEntries, normalizedEvent.payload, managedServerDetail.name ?? 'GamePanel'))
    }

    const handleFileChanged = (event) => {
      const normalizedEvent = normalizeAgentStreamEvent(event.type, event.data)
      if (!normalizedEvent || normalizedEvent.type !== 'fileChanged') {
        return
      }

      setConfigFileRefreshToken((currentValue) => currentValue + 1)
      if (normalizedEvent.payload.logicalPath === selectedConfigFilePathRef.current) {
        if (configFileDirtyRef.current) {
          setConfigFileError(`文件已在远端变更：${normalizedEvent.payload.logicalPath}`)
          return
        }

        setConfigFileRefreshToken((currentValue) => currentValue + 1)
      }
    }

    eventSource.addEventListener('agent.logChunk', handleLogChunk)
    eventSource.addEventListener('agent.fileChanged', handleFileChanged)
    eventSource.onerror = () => {
      setRealtimeLogConnectionLabel('连接中断')
    }

    return () => {
      eventSource.close()
    }
  }, [activeView, canAccessAgentWorkbench, managedServerDetail?.agentId, managedServerDetail?.name, realtimeLogPaused])

  useEffect(() => {
    if (!realtimeLogAutoScroll || normalizedWorkbenchSection !== 'realtime-logs') {
      return
    }

    const viewport = realtimeLogViewportRef.current
    if (!viewport) {
      return
    }

    viewport.scrollTop = viewport.scrollHeight
  }, [filteredRealtimeLogEntries, realtimeLogAutoScroll, normalizedWorkbenchSection])

  const headerLiveLabel = connectionStatus.label
  const currentPageTitle = activeView === 'server-detail'
    ? managedServerDetail?.name ?? serverManagementPage.title ?? '服务器详情'
    : activeView === 'server-manager'
      ? dashboard.table.title
      : dashboard.page.title
  const currentPageSubtitle = activeView === 'server-detail'
    ? `服务器工作台 · ${activeWorkbenchSectionConfig.label}`
    : activeView === 'server-manager'
      ? dashboard.table.subtitle
      : dashboard.page.subtitle

  const handleSidebarItemClick = (itemLabel) => {
    const nextView = NAVIGATION_VIEW_MAP[itemLabel]
    if (!nextView) {
      return
    }

    if (nextView !== 'server-detail') {
      setSelectedServerUuid(null)
      setManagedServerDetail(null)
      setServerAgentAuth(null)
      setServerAgentAuthError('')
      setGeneratedAgentKey('')
      setServerDetailError('')
      setActiveWorkbenchSection(DEFAULT_WORKBENCH_SECTION)
    }

    setActiveView(nextView)
  }

  const handleRefresh = async () => {
    setIsLoading(true)
    setConnectionStatus({
      tone: 'loading',
      label: '刷新中',
    })
    setDashboardLoadError('')

    try {
      const payload = await dashboardApi.getDashboardData()
      applyDashboardPayload(payload)
      if (selectedServerUuid) {
        try {
          const detail = await dashboardApi.getServer(selectedServerUuid)
          const auth = await dashboardApi.getServerAgentAuth(selectedServerUuid)
          setManagedServerDetail(detail)
          setServerAgentAuth(auth)
          setServerAgentAuthError('')
          setServerDetailError('')
        } catch (error) {
          const message = error instanceof Error ? error.message : '读取服务器详情失败，请稍后重试。'
          setManagedServerDetail(null)
          setServerAgentAuth(null)
          setServerDetailError(message)
        }
      }
    } catch (error) {
      console.error('刷新仪表盘数据失败', error)
      applyEmptyDashboardState(
        error instanceof Error
          ? `后端联调失败：${error.message}`
          : '后端联调失败，请确认后端和数据库已启动。',
      )
    } finally {
      setIsLoading(false)
    }
  }

  const handleSaveConfigFile = async () => {
    if (!canAccessAgentWorkbench || !managedServerDetail?.agentId || !selectedConfigFilePath) {
      return
    }

    setConfigFileSaving(true)
    setConfigFileError('')

    try {
      const payload = await dashboardApi.updateAgentFileContent(managedServerDetail.agentId, {
        logicalPath: selectedConfigFilePath,
        content: configFileDraft,
        expectedVersion: configFileVersion || null,
      })
      setConfigFileContent(configFileDraft)
      setConfigFileVersion(payload.version ?? '')
      setConfigFileRefreshToken((currentValue) => currentValue + 1)
    } catch (error) {
      setConfigFileError(error instanceof Error ? error.message : '保存配置文件失败')
    } finally {
      setConfigFileSaving(false)
    }
  }

  const handleGenerateAgentKey = async () => {
    if (!selectedServerUuid) {
      return
    }

    setAgentKeySubmitting(true)
    setServerAgentAuthError('')
    setGeneratedAgentKey('')
    setFlashMessage(null)

    try {
      const auth = await dashboardApi.rotateServerAgentKey(selectedServerUuid)
      setServerAgentAuth(auth)
      setGeneratedAgentKey(auth.plainKey ?? '')

      const detail = await loadServerDetail(selectedServerUuid)
      setManagedServerDetail(detail)
      setFlashMessage({
        type: 'success',
        text: auth.hasKey ? 'Agent Key 已生成，请复制到游戏服务器上的 agent 配置。' : 'Agent Key 已更新',
        serverUuid: auth.serverUuid,
      })
    } catch (error) {
      setServerAgentAuthError(error instanceof Error ? error.message : '生成 Agent Key 失败')
    } finally {
      setAgentKeySubmitting(false)
    }
  }

  const handleAddServer = async () => {
    resetServerForm()
    setServerFormMode('add')
    setIsServerFormModalOpen(true)
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

  const handleManageServer = async (server) => {
    setActiveView('server-detail')
    setSelectedServerUuid(server.uuid)
    setActiveWorkbenchSection(DEFAULT_WORKBENCH_SECTION)
    setFlashMessage(null)

    try {
      await Promise.all([
        loadServerDetail(server.uuid),
        loadServerAgentAuth(server.uuid),
      ])
    } catch (error) {
      setFlashMessage({
        type: 'error',
        text: error instanceof Error ? error.message : '读取服务器详情失败，请稍后重试。',
      })
    }
  }

  const handleEditServer = async (server) => {
    setFlashMessage(null)
    setServerFormError('')

    try {
      const detail =
        managedServerDetail?.serverUuid === server.uuid ? managedServerDetail : await loadServerDetail(server.uuid)

      setServerFormMode('edit')
      setEditingServerUuid(server.uuid)
      setServerForm({
        name: detail.name,
        ip: detail.ip,
        rconPort: String(detail.rconPort),
        rconPassword: detail.rconPassword,
      })
      setIsServerFormModalOpen(true)
    } catch (error) {
      setFlashMessage({
        type: 'error',
        text: error instanceof Error ? error.message : '读取服务器详情失败，请稍后重试。',
      })
    }
  }

  const handleDeleteServer = async (server) => {
    setDeleteTargetServer(server)
    setDeleteServerError('')
    setFlashMessage(null)
  }

  const handleBackToServerList = () => {
    setActiveView('server-manager')
    setSelectedServerUuid(null)
    setManagedServerDetail(null)
    setServerAgentAuth(null)
    setServerAgentAuthError('')
    setGeneratedAgentKey('')
    setServerDetailError('')
    setActiveWorkbenchSection(DEFAULT_WORKBENCH_SECTION)
  }

  const handleServerFormChange = (event) => {
    const { name, value } = event.target
    setServerForm((currentValue) => ({
      ...currentValue,
      [name]: value,
    }))
  }

  const handleServerFormSubmit = async (event) => {
    event.preventDefault()
    setServerFormError('')
    setFlashMessage(null)

    if (
      serverForm.name.trim() === '' ||
      serverForm.ip.trim() === '' ||
      serverForm.rconPort.trim() === '' ||
      serverForm.rconPassword.trim() === ''
    ) {
      setServerFormError('请完整填写服务器名称、IP、RCON 端口和 RCON 密码。')
      return
    }

    const parsedPort = Number.parseInt(serverForm.rconPort, 10)
    if (!Number.isInteger(parsedPort) || parsedPort <= 0 || parsedPort > 65535) {
      setServerFormError('请输入有效的 RCON 端口。')
      return
    }

    setServerFormSubmitting(true)

    try {
      const payload = {
        name: serverForm.name.trim(),
        ip: serverForm.ip.trim(),
        rconPort: parsedPort,
        rconPassword: serverForm.rconPassword,
      }
      const response = serverFormMode === 'edit' && editingServerUuid
        ? await dashboardApi.updateServer(editingServerUuid, payload)
        : await dashboardApi.addServer(payload)

      const nextDashboard = await dashboardApi.getDashboardData()
      applyDashboardPayload(nextDashboard)
      if (serverFormMode === 'edit' && editingServerUuid) {
        const detail = await dashboardApi.getServer(editingServerUuid)
        const auth = await dashboardApi.getServerAgentAuth(editingServerUuid)
        setManagedServerDetail(detail)
        setServerAgentAuth(auth)
        setServerAgentAuthError('')
        setServerDetailError('')
      }
      setIsServerFormModalOpen(false)
      resetServerForm()
      setFlashMessage({
        type: 'success',
        text: response.message,
        serverUuid: response.serverUuid,
      })
    } catch (error) {
      setServerFormError(error instanceof Error ? error.message : '添加服务器失败，请稍后重试。')
    } finally {
      setServerFormSubmitting(false)
    }
  }

  const handleDeleteServerConfirm = async () => {
    if (!deleteTargetServer) {
      return
    }

    setDeleteServerSubmitting(true)
    setDeleteServerError('')

    try {
      const response = await dashboardApi.deleteServer(deleteTargetServer.uuid)
      const nextDashboard = await dashboardApi.getDashboardData()
      applyDashboardPayload(nextDashboard)

      const nextViewState = buildServerViewAfterDelete({
        activeView,
        selectedServerUuid,
        deletedServerUuid: deleteTargetServer.uuid,
      })
      setActiveView(nextViewState.activeView)
      setSelectedServerUuid(nextViewState.selectedServerUuid)
      if (nextViewState.selectedServerUuid === null) {
        setManagedServerDetail(null)
        setServerAgentAuth(null)
        setServerAgentAuthError('')
        setGeneratedAgentKey('')
        setServerDetailError('')
        setActiveWorkbenchSection(DEFAULT_WORKBENCH_SECTION)
      }

      setDeleteTargetServer(null)
      setFlashMessage({
        type: 'success',
        text: response.message,
        serverUuid: response.serverUuid,
      })
    } catch (error) {
      setDeleteServerError(error instanceof Error ? error.message : '删除服务器失败，请稍后重试。')
    } finally {
      setDeleteServerSubmitting(false)
    }
  }

  const renderServerTableCard = () => (
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
            {dashboard.table.rows.length > 0 ? (
              dashboard.table.rows.map((row) => (
                <tr key={row.uuid}>
                  <td>
                    <div className="server-name">
                      <span className={`server-dot ${row.dot}`}></span>
                      <div>
                        <div className="server-name-text">{row.name}</div>
                        <div className="server-ip">{row.ip}</div>
                        <div className="server-uuid">UUID: {row.uuid}</div>
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
                      onManage={handleManageServer}
                      onEdit={handleEditServer}
                      onDelete={handleDeleteServer}
                      onConsole={handleServerConsole}
                      onRestart={handleServerRestart}
                      onStart={handleServerStart}
                    />
                  </td>
                </tr>
              ))
            ) : (
              <tr>
                <td colSpan="8">
                  <EmptyState message="暂无服务器数据" />
                </td>
              </tr>
            )}
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
  )

  const renderServerDetailView = () => {
    const fallbackServer = serverManagementPage.server
    const fallbackAddress = fallbackServer ? splitServerAddress(fallbackServer.ip) : null
    const detailServer = managedServerDetail
      ? managedServerDetail
      : fallbackServer
        ? {
            name: fallbackServer.name,
            ip: fallbackAddress?.ip ?? fallbackServer.ip,
            rconPort: fallbackAddress?.rconPort ?? '--',
            rconPassword: '',
            serverUuid: fallbackServer.uuid,
            statusLabel: fallbackServer.status.label,
            agentId: null,
            agentOnline: false,
            workspaceRoots: [],
            primaryLogPath: '',
          }
        : null
    const authAwareServer = detailServer
      ? {
          ...detailServer,
          hasKey: serverAgentAuth?.hasKey ?? false,
        }
      : null
    const authStatusLabel = describeAgentAuthStatus(authAwareServer)
    const workbenchContent = createServerWorkbenchContent(detailServer)
    const sectionContent = workbenchContent[normalizedWorkbenchSection]
    const renderWorkbenchSection = () => {
      switch (normalizedWorkbenchSection) {
        case 'overview':
          return (
            <>
              <div className="workbench-metric-grid">
                {sectionContent.metrics.map((item) => (
                  <div className="workbench-metric-card" key={item.label}>
                    <span className="workbench-metric-label">{item.label}</span>
                    <strong>{item.value}</strong>
                    <span className="workbench-metric-meta">{item.meta}</span>
                  </div>
                ))}
              </div>
              <div className="workbench-content-grid">
                <div className="card">
                  <div className="card-header">
                    <div>
                      <div className="card-title">工作台提示</div>
                      <div className="card-subtitle">为后续接入真实功能预留的运营视角卡片。</div>
                    </div>
                  </div>
                  <div className="workbench-note-list">
                    {sectionContent.highlights.map((item) => (
                      <div className="workbench-note-card" key={item.title}>
                        <strong>{item.title}</strong>
                        <p>{item.body}</p>
                      </div>
                    ))}
                  </div>
                </div>
                <div className="card">
                  <div className="card-header">
                    <div>
                      <div className="card-title">最近动态</div>
                      <div className="card-subtitle">围绕该服务器的后台占位动态流。</div>
                    </div>
                  </div>
                  <div className="workbench-timeline">
                    {sectionContent.activity.map((item) => (
                      <div className="workbench-timeline-item" key={`${item.title}-${item.time}`}>
                        <div className="workbench-timeline-dot"></div>
                        <div>
                          <strong>{item.title}</strong>
                          <p>{item.detail}</p>
                          <span>{item.time}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </>
          )
        case 'control':
          return (
            <div className="workbench-stack">
              <div className="workbench-control-grid">
                {sectionContent.actionGroups.map((group) => (
                  <div className="card" key={group.title}>
                    <div className="card-header">
                      <div>
                        <div className="card-title">{group.title}</div>
                        <div className="card-subtitle">当前阶段仅提供 UI 结构，按钮不执行实际动作。</div>
                      </div>
                    </div>
                    <div className="workbench-action-grid">
                      {group.items.map((item) => (
                        <button className="workbench-action-tile" key={item} type="button">
                          <span>{item}</span>
                          <small>开发中</small>
                        </button>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">安全约束</div>
                    <div className="card-subtitle">用于展示未来控制操作的执行规则。</div>
                  </div>
                </div>
                <div className="workbench-bullet-list">
                  {sectionContent.safety.map((item) => (
                    <div className="workbench-bullet-item" key={item}>
                      <span className="workbench-bullet-dot"></span>
                      {item}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )
        case 'realtime-logs':
          if (!canAccessAgentWorkbench) {
            return (
              <div className="server-detail-empty">
                <EmptyState message={`${authStatusLabel}，当前不可建立日志连接。`} />
              </div>
            )
          }

          return (
            <div className="workbench-stack">
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">日志连接</div>
                    <div className="card-subtitle">当前通过 backend SSE 消费 agent 推送的实时日志。</div>
                  </div>
                </div>
                <div className="workbench-log-status-row">
                  <div className="workbench-log-status-card">
                    <span>连接状态</span>
                    <strong>{realtimeLogConnectionLabel}</strong>
                  </div>
                  <div className="workbench-log-status-card">
                    <span>日志频道</span>
                    <strong>{detailServer?.primaryLogPath || sectionContent.streamName}</strong>
                  </div>
                  <div className="workbench-log-status-card">
                    <span>自动滚动</span>
                    <strong>{realtimeLogAutoScroll ? '已开启' : '已关闭'}</strong>
                  </div>
                </div>
              </div>
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">日志工具栏</div>
                    <div className="card-subtitle">支持关键字筛选、级别过滤、暂停和清空视图。</div>
                  </div>
                </div>
                <div className="workbench-log-toolbar">
                  <div className="table-search workbench-log-search">
                    <Icon name="search" width={13} height={13} />
                    <input
                      type="text"
                      placeholder="搜索时间、来源、消息内容…"
                      value={realtimeLogSearchTerm}
                      onChange={(event) => setRealtimeLogSearchTerm(event.target.value)}
                    />
                  </div>
                  <select
                    className="filter-select"
                    value={realtimeLogLevel}
                    onChange={(event) => setRealtimeLogLevel(event.target.value)}
                  >
                    {sectionContent.levelOptions.map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                  <button className="btn btn-secondary btn-sm" type="button" onClick={() => setRealtimeLogPaused((currentValue) => !currentValue)}>
                    {realtimeLogPaused ? '恢复滚动' : '暂停滚动'}
                  </button>
                  <button className="btn btn-secondary btn-sm" type="button" onClick={() => setRealtimeLogAutoScroll((currentValue) => !currentValue)}>
                    {realtimeLogAutoScroll ? '关闭自动滚动' : '开启自动滚动'}
                  </button>
                  <button className="btn btn-danger btn-sm" type="button" onClick={() => setRealtimeLogEntries([])}>
                    清空视图
                  </button>
                </div>
                <div className="workbench-log-stream" ref={realtimeLogViewportRef}>
                  {filteredRealtimeLogEntries.length > 0 ? (
                    filteredRealtimeLogEntries.map((entry) => (
                      <div className={`workbench-log-line ${entry.level.toLowerCase()}`} key={entry.id}>
                        <span className="workbench-log-time">{entry.time}</span>
                        <span className="workbench-log-level">{entry.level}</span>
                        <span className="workbench-log-source">{entry.source}</span>
                        <span className="workbench-log-message">{entry.message}</span>
                      </div>
                    ))
                  ) : (
                    <div className="workbench-log-empty">当前筛选条件下暂无日志输出。</div>
                  )}
                </div>
                <div className="workbench-log-command">
                  <input type="text" placeholder={sectionContent.commandPlaceholder} disabled />
                  <button className="btn btn-primary btn-sm" type="button" disabled>
                    发送命令
                  </button>
                </div>
              </div>
            </div>
          )
        case 'chat':
          return (
            <div className="workbench-stack">
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">聊天筛选</div>
                    <div className="card-subtitle">后续可接频道、关键词、玩家名筛选。</div>
                  </div>
                </div>
                <div className="workbench-chip-row">
                  {sectionContent.filters.map((filter) => (
                    <button className="workbench-chip active" key={filter} type="button">
                      {filter}
                    </button>
                  ))}
                </div>
              </div>
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">聊天记录</div>
                    <div className="card-subtitle">使用真实表格结构占位，后续直接接接口数据。</div>
                  </div>
                </div>
                <div className="workbench-table-wrap">
                  <table className="workbench-table">
                    <thead>
                      <tr>
                        <th>时间</th>
                        <th>玩家</th>
                        <th>频道</th>
                        <th>内容</th>
                      </tr>
                    </thead>
                    <tbody>
                      {sectionContent.rows.map((row) => (
                        <tr key={`${row.time}-${row.player}`}>
                          <td>{row.time}</td>
                          <td>{row.player}</td>
                          <td>{row.channel}</td>
                          <td>{row.content}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )
        case 'flight':
          return (
            <div className="workbench-card-grid">
              {sectionContent.incidents.map((item) => (
                <div className="card" key={`${item.player}-${item.area}`}>
                  <div className="card-header">
                    <div>
                      <div className="card-title">{item.player}</div>
                      <div className="card-subtitle">{item.area}</div>
                    </div>
                    <span className={`workbench-severity ${item.level === '高' ? 'high' : item.level === '中' ? 'medium' : 'low'}`}>
                      {item.level}风险
                    </span>
                  </div>
                  <div className="workbench-incident-body">
                    <p>{item.detail}</p>
                    <div className="workbench-inline-meta">
                      <span>状态</span>
                      <strong>{item.status}</strong>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )
        case 'knockdown':
          return (
            <div className="workbench-stack">
              <div className="workbench-metric-grid compact">
                {sectionContent.summary.map((item) => (
                  <div className="workbench-metric-card" key={item.label}>
                    <span className="workbench-metric-label">{item.label}</span>
                    <strong>{item.value}</strong>
                  </div>
                ))}
              </div>
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">击倒事件</div>
                    <div className="card-subtitle">预留实时战斗记录和热点分析表格。</div>
                  </div>
                </div>
                <div className="workbench-table-wrap">
                  <table className="workbench-table">
                    <thead>
                      <tr>
                        <th>时间</th>
                        <th>攻击者</th>
                        <th>被击倒者</th>
                        <th>武器</th>
                        <th>距离</th>
                      </tr>
                    </thead>
                    <tbody>
                      {sectionContent.rows.map((row) => (
                        <tr key={`${row.time}-${row.attacker}-${row.defender}`}>
                          <td>{row.time}</td>
                          <td>{row.attacker}</td>
                          <td>{row.defender}</td>
                          <td>{row.weapon}</td>
                          <td>{row.distance}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )
        case 'match':
          return (
            <div className="workbench-stack">
              <div className="workbench-metric-grid">
                {sectionContent.cards.map((item) => (
                  <div className="workbench-metric-card" key={item.title}>
                    <span className="workbench-metric-label">{item.title}</span>
                    <strong>{item.value}</strong>
                    <span className="workbench-metric-meta">{item.sub}</span>
                  </div>
                ))}
              </div>
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">赛程节点</div>
                    <div className="card-subtitle">展示回合、赛段和运营准备项。</div>
                  </div>
                </div>
                <div className="workbench-timeline">
                  {sectionContent.timeline.map((item) => (
                    <div className="workbench-timeline-item" key={item.title}>
                      <div className="workbench-timeline-dot"></div>
                      <div>
                        <strong>{item.title}</strong>
                        <p>{item.detail}</p>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )
        case 'config-files':
          if (!canAccessAgentWorkbench) {
            return (
              <div className="server-detail-empty">
                <EmptyState message={`${authStatusLabel}，当前不可浏览或编辑远端文件。`} />
              </div>
            )
          }

          return (
            <div className="workbench-config-layout">
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">配置目录</div>
                    <div className="card-subtitle">
                      {detailServer?.workspaceRoots?.[0]?.logicalPath
                        ? `当前根目录：${detailServer.workspaceRoots[0].logicalPath}`
                        : '当前服务器尚未上报可浏览的工作目录。'}
                    </div>
                  </div>
                </div>
                <div className="workbench-file-list">
                  {configFileItems.length > 0 ? configFileItems.map((file) => (
                    <button
                      className={`workbench-file-item${selectedConfigFilePath === file.path ? ' active' : ''}`}
                      key={file.path}
                      type="button"
                      onClick={() => {
                        if (!file.isDir) {
                          setSelectedConfigFilePath(file.path)
                        }
                      }}
                    >
                      <div>
                        <strong>{file.name}</strong>
                        <span>{file.path}</span>
                      </div>
                      <div className="workbench-file-meta">
                        <span>{file.isDir ? '目录' : '文件'}</span>
                        <span>{file.sizeLabel}</span>
                      </div>
                    </button>
                  )) : (
                    <div className="workbench-log-empty">当前目录下暂无可展示文件。</div>
                  )}
                </div>
              </div>
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">文件预览</div>
                    <div className="card-subtitle">
                      {selectedConfigFilePath || '选择左侧文件后可查看并保存内容。'}
                    </div>
                  </div>
                </div>
                {configFileError ? <div className="form-error workbench-inline-error">{configFileError}</div> : null}
                <textarea
                  className="workbench-code-preview workbench-code-editor"
                  value={configFileDraft}
                  onChange={(event) => setConfigFileDraft(event.target.value)}
                  placeholder={configFileLoading ? '正在加载文件内容…' : '当前没有选中文件'}
                  disabled={configFileLoading || !selectedConfigFilePath}
                />
                <div className="workbench-log-command">
                  <input
                    type="text"
                    value={configFileVersion ? `版本：${configFileVersion}${isConfigFileDirty ? ' · 已修改' : ''}` : '未加载版本'}
                    disabled
                  />
                  <button
                    className="btn btn-primary btn-sm"
                    type="button"
                    onClick={handleSaveConfigFile}
                    disabled={!selectedConfigFilePath || configFileLoading || configFileSaving || !isConfigFileDirty}
                  >
                    {configFileSaving ? '保存中…' : '保存文件'}
                  </button>
                </div>
              </div>
            </div>
          )
        case 'config-panel':
          return (
            <div className="workbench-card-grid">
              {sectionContent.groups.map((group) => (
                <div className="card" key={group.title}>
                  <div className="card-header">
                    <div>
                      <div className="card-title">{group.title}</div>
                      <div className="card-subtitle">表单和开关布局已预置，后续接真实配置即可。</div>
                    </div>
                  </div>
                  <div className="workbench-settings-list">
                    {group.fields.map((field) => (
                      <div className="workbench-setting-row" key={field}>
                        <div>
                          <strong>{field}</strong>
                          <span>占位配置项</span>
                        </div>
                        <button className="workbench-toggle" type="button">
                          <span></span>
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )
        case 'operations':
          return (
            <div className="card">
              <div className="card-header">
                <div>
                  <div className="card-title">操作审计</div>
                  <div className="card-subtitle">记录管理员、自动化服务和系统动作的占位视图。</div>
                </div>
              </div>
              <div className="workbench-table-wrap">
                <table className="workbench-table">
                  <thead>
                    <tr>
                      <th>时间</th>
                      <th>操作者</th>
                      <th>动作</th>
                      <th>结果</th>
                      <th>目标</th>
                    </tr>
                  </thead>
                  <tbody>
                    {sectionContent.rows.map((row) => (
                      <tr key={`${row.time}-${row.action}`}>
                        <td>{row.time}</td>
                        <td>{row.operator}</td>
                        <td>{row.action}</td>
                        <td>{row.result}</td>
                        <td>{row.target}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )
        case 'players':
          return (
            <div className="workbench-stack">
              <div className="workbench-metric-grid compact">
                {sectionContent.cards.map((item) => (
                  <div className="workbench-metric-card" key={item.label}>
                    <span className="workbench-metric-label">{item.label}</span>
                    <strong>{item.value}</strong>
                    <span className="workbench-metric-meta">{item.sub}</span>
                  </div>
                ))}
              </div>
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">玩家列表</div>
                    <div className="card-subtitle">用于承接在线状态、标签、黑名单和行为轨迹。</div>
                  </div>
                </div>
                <div className="workbench-player-list">
                  {sectionContent.rows.map((row) => (
                    <div className="workbench-player-card" key={row.name}>
                      <div>
                        <strong>{row.name}</strong>
                        <span>{row.level}</span>
                      </div>
                      <div>
                        <strong>{row.status}</strong>
                        <span>{row.note}</span>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )
        case 'permissions':
          return (
            <div className="workbench-stack">
              <div className="workbench-card-grid">
                {sectionContent.roles.map((role) => (
                  <div className="card" key={role.title}>
                    <div className="card-header">
                      <div>
                        <div className="card-title">{role.title}</div>
                        <div className="card-subtitle">{role.scope}</div>
                      </div>
                    </div>
                    <div className="workbench-note-card single">
                      <p>{role.description}</p>
                    </div>
                  </div>
                ))}
              </div>
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">权限矩阵</div>
                    <div className="card-subtitle">未来可在此接角色继承、批量授权和审批流。</div>
                  </div>
                </div>
                <div className="workbench-table-wrap">
                  <table className="workbench-table">
                    <thead>
                      <tr>
                        <th>权限项</th>
                        <th>服主</th>
                        <th>值班管理员</th>
                        <th>裁判</th>
                      </tr>
                    </thead>
                    <tbody>
                      {sectionContent.matrix.map((row) => (
                        <tr key={row.permission}>
                          <td>{row.permission}</td>
                          <td>{row.owner ? <Icon name="check" width={14} height={14} /> : '—'}</td>
                          <td>{row.admin ? <Icon name="check" width={14} height={14} /> : '—'}</td>
                          <td>{row.referee ? <Icon name="check" width={14} height={14} /> : '—'}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )
        default:
          return null
      }
    }

    return (
      <div className="server-workbench">
        <aside className="card workbench-sidebar">
          <div className="workbench-server-header">
            <div className="workbench-server-kicker">服务器工作台</div>
            <div className="workbench-server-name">{detailServer?.name ?? '服务器详情'}</div>
            <div className="workbench-server-meta">{detailServer?.ip ?? '--'}:{detailServer?.rconPort ?? '--'}</div>
            <div className="workbench-server-meta workbench-server-mono">
              UUID: {detailServer?.serverUuid ?? selectedServerUuid ?? '--'}
            </div>
          </div>
          <div className="workbench-server-summary">
            <div className="workbench-summary-item">
              <span>状态</span>
              <strong>{detailServer?.statusLabel ?? '--'}</strong>
            </div>
            <div className="workbench-summary-item">
              <span>RCON 密码</span>
              <strong>{detailServer?.rconPassword ? '已配置' : '未设置'}</strong>
            </div>
          </div>
          <div className="workbench-nav">
            {SERVER_WORKBENCH_SECTIONS.map((section) => (
              <button
                className={`workbench-nav-item${normalizedWorkbenchSection === section.id ? ' active' : ''}`}
                key={section.id}
                type="button"
                onClick={() => setActiveWorkbenchSection(section.id)}
              >
                <Icon name={section.icon} width={15} height={15} />
                <span>{section.label}</span>
              </button>
            ))}
          </div>
        </aside>
        <div className="workbench-main">
          <div className="card">
            <div className="card-header">
              <div>
                <div className="card-title">{activeWorkbenchSectionConfig.label}</div>
                <div className="card-subtitle">{activeWorkbenchSectionConfig.description}</div>
              </div>
              <div className="server-detail-actions">
                <button className="btn btn-secondary btn-sm" type="button" onClick={handleBackToServerList}>
                  <Icon name="arrow-left" width={13} height={13} />
                  返回列表
                </button>
                {fallbackServer ? (
                  <>
                    <button className="btn btn-secondary btn-sm" type="button" onClick={() => handleEditServer(fallbackServer)}>
                      <Icon name="pencil" width={13} height={13} />
                      编辑服务器
                    </button>
                    <button className="btn btn-danger btn-sm" type="button" onClick={() => handleDeleteServer(fallbackServer)}>
                      <Icon name="trash" width={13} height={13} />
                      删除服务器
                    </button>
                  </>
                ) : null}
              </div>
            </div>
            {isServerDetailLoading ? <div className="server-detail-banner">正在同步服务器详情…</div> : null}
            {serverDetailError ? <div className="server-detail-banner error">{serverDetailError}</div> : null}
            {isServerAgentAuthLoading ? <div className="server-detail-banner">正在同步 Agent 鉴权状态…</div> : null}
            {serverAgentAuthError ? <div className="server-detail-banner error">{serverAgentAuthError}</div> : null}
            {detailServer ? (
              <div className="server-detail-banner">
                <div>Agent 状态：{authStatusLabel}</div>
                <div>
                  当前服务器 UUID：<strong>{detailServer.serverUuid}</strong>
                  {detailServer.agentId ? ` · 在线 Agent：${detailServer.agentId}` : ''}
                  {serverAgentAuth?.keyPreview ? ` · Key 预览：${serverAgentAuth.keyPreview}` : ''}
                </div>
                <div className="server-detail-actions" style={{ marginTop: '12px' }}>
                  <button
                    className="btn btn-primary btn-sm"
                    type="button"
                    onClick={handleGenerateAgentKey}
                    disabled={agentKeySubmitting || !detailServer.serverUuid}
                  >
                    <Icon name="key-round" width={13} height={13} />
                    {serverAgentAuth?.hasKey ? '重置 Agent Key' : '生成 Agent Key'}
                  </button>
                </div>
                {generatedAgentKey ? (
                  <div style={{ marginTop: '12px', fontFamily: "'DM Mono',monospace", wordBreak: 'break-all' }}>
                    新 Key：{generatedAgentKey}
                  </div>
                ) : null}
                <div style={{ marginTop: '8px' }}>
                  将 `server_uuid` 与该 Key 手动填入游戏服务器上的 agent 配置，然后重启 agent 进行联动测试。
                </div>
              </div>
            ) : null}
            {detailServer ? (
              <div className="workbench-section-body">{renderWorkbenchSection()}</div>
            ) : (
              <div className="server-detail-empty">
                <EmptyState message="未找到服务器详情，请返回列表后重试。" />
              </div>
            )}
          </div>
        </div>
      </div>
    )
  }

  const renderDashboardView = () => (
    <>
      <div className="stats-grid">
        {dashboard.stats.map((stat) => (
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
              {dashboard.distribution.labels.length > 0 ? (
                dashboard.distribution.labels.map((label, index) => (
                  <div className="legend-item" key={label}>
                    <div className="legend-dot" style={{ background: dashboard.distribution.colors[index] }}></div>
                    <span className="legend-label">{label}</span>
                    <span className="legend-val">{dashboard.distribution.values[index]}</span>
                  </div>
                ))
              ) : (
                <EmptyState message="暂无游戏分布数据" />
              )}
            </div>
          </div>
          <div style={{ borderTop: '1px solid var(--border)', paddingTop: '16px' }} className="progress-section">
            {dashboard.nodeProgress.length > 0 ? (
              dashboard.nodeProgress.map((item) => (
                <div className="progress-item" key={item.name}>
                  <div className="progress-meta">
                    <span className="progress-name">{item.name}</span>
                    <span className="progress-stat">{item.value}</span>
                  </div>
                  <div className="progress-track">
                    <div className="progress-fill" style={{ width: item.width, background: item.background }}></div>
                  </div>
                </div>
              ))
            ) : (
              <EmptyState message="暂无节点资源数据" />
            )}
          </div>
        </div>
      </div>

      <div className="bottom-grid">
        {renderServerTableCard()}

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
              {dashboard.activities.length > 0 ? (
                dashboard.activities.map((activity, index) => (
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
                ))
              ) : (
                <EmptyState message="暂无动态数据" />
              )}
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
            {dashboard.topPlayers.length > 0 ? (
              dashboard.topPlayers.map((player) => (
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
              ))
            ) : (
              <EmptyState message="暂无在线玩家排行" />
            )}
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
              {dashboard.networkHealth.regions.length > 0 ? (
                dashboard.networkHealth.regions.map((region) => (
                  <div key={region.name}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12.5px', marginBottom: '5px' }}>
                      <span style={{ color: 'var(--text-secondary)' }}>{region.name}</span>
                      <span style={{ fontFamily: "'DM Mono',monospace", color: region.color, fontWeight: 600 }}>{region.value}</span>
                    </div>
                    <div className="progress-track">
                      <div className="progress-fill" style={{ width: region.width, background: region.color }}></div>
                    </div>
                  </div>
                ))
              ) : (
                <EmptyState message="暂无网络健康数据" />
              )}
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
    </>
  )

  const renderServerManagerView = () => (
    <div className="server-manager-layout">
      {serverManagementPage.mode === 'detail' ? renderServerDetailView() : renderServerTableCard()}
    </div>
  )

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
                <button
                  className={`nav-item${NAVIGATION_VIEW_MAP[item.label] === activeView ? ' active' : ''}`}
                  key={item.label}
                  type="button"
                  onClick={() => handleSidebarItemClick(item.label)}
                >
                  <div className="nav-icon">
                    <Icon name={item.icon} width={17} height={17} />
                  </div>
                  <span className="nav-label">{item.label}</span>
                  {item.badge ? <span className="nav-badge">{item.badge}</span> : null}
                  <span className="nav-item-tooltip">{item.tooltip || item.label}</span>
                </button>
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
            <span className={`status-live ${connectionStatus.tone}`} style={{ padding: '0 8px' }}>
              <span className="status-live-dot"></span> {headerLiveLabel}
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
          {dashboardLoadError ? (
            <div className="flash-message error">
              <div>{dashboardLoadError}</div>
            </div>
          ) : null}
          {flashMessage ? (
            <div className={`flash-message ${flashMessage.type}`}>
              <div>{flashMessage.text}</div>
              {flashMessage.serverUuid ? <div className="flash-message-detail">UUID: {flashMessage.serverUuid}</div> : null}
            </div>
          ) : null}
          <div className="page-header">
            <div>
              <div className="page-title">{currentPageTitle}</div>
              <div className="page-subtitle">
                {currentPageSubtitle} · <span id="currentTime">{currentTime}</span>
              </div>
            </div>
            <div className="page-actions">
              <button className="btn btn-secondary btn-sm" type="button" onClick={handleRefresh}>
                <Icon name="refresh" width={13} height={13} />
                {dashboard.page.refreshLabel}
              </button>
              {activeView !== 'server-detail' ? (
                <button className="btn btn-primary btn-sm" type="button" onClick={handleAddServer}>
                  <Icon name="plus" width={13} height={13} />
                  {dashboard.page.addServerLabel}
                </button>
              ) : null}
            </div>
          </div>
          {activeView === 'server-manager' || activeView === 'server-detail' ? renderServerManagerView() : renderDashboardView()}
        </main>
      </div>
      {isServerFormModalOpen ? (
        <AddServerModal
          title={serverFormMode === 'edit' ? '编辑服务器' : '添加服务器'}
          subtitle={
            serverFormMode === 'edit'
              ? '更新服务器信息后，系统会重新进行 RCON 验证。'
              : '录入服务器信息后，系统会先进行 RCON 验证。'
          }
          submitLabel={serverFormMode === 'edit' ? '保存并验证' : '验证并添加'}
          form={serverForm}
          submitting={serverFormSubmitting}
          error={serverFormError}
          onChange={handleServerFormChange}
          onClose={() => {
            if (serverFormSubmitting) {
              return
            }
            setIsServerFormModalOpen(false)
            resetServerForm()
          }}
          onSubmit={handleServerFormSubmit}
        />
      ) : null}
      {deleteTargetServer ? (
        <DeleteServerModal
          server={deleteTargetServer}
          submitting={deleteServerSubmitting}
          error={deleteServerError}
          onClose={() => {
            if (deleteServerSubmitting) {
              return
            }
            setDeleteTargetServer(null)
            setDeleteServerError('')
          }}
          onConfirm={handleDeleteServerConfirm}
        />
      ) : null}
    </>
  )
}

export default App
