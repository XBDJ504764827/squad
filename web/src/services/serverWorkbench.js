export const DEFAULT_WORKBENCH_SECTION = 'overview'

export const SERVER_WORKBENCH_SECTIONS = [
  { id: 'overview', label: '概要', icon: 'grid', description: '查看服务器总览、关键指标和近期状态。' },
  { id: 'control', label: '控制面板', icon: 'sliders', description: '预览运维动作、节点控制和发布操作。' },
  { id: 'realtime-logs', label: '实时日志', icon: 'terminal', description: '查看游戏服务器控制台日志的实时流式面板。' },
  { id: 'chat', label: '聊天记录', icon: 'messages', description: '查看游戏内聊天频道、过滤和会话概览。' },
  { id: 'flight', label: '飞天记录', icon: 'radar', description: '查看飞天告警、风险等级和处理状态。' },
  { id: 'knockdown', label: '击倒记录', icon: 'crosshair', description: '查看击倒事件、战斗热点和异常趋势。' },
  { id: 'match', label: '比赛记录', icon: 'trophy', description: '查看对局回合、结算表现和赛程摘要。' },
  { id: 'config-files', label: '配置文件', icon: 'file-code', description: '预览配置目录、版本和变更差异。' },
  { id: 'config-panel', label: '配置面板', icon: 'settings-sliders', description: '用表单方式管理常用配置项和策略。' },
  { id: 'operations', label: '操作记录', icon: 'history', description: '查看后台操作审计、执行结果和责任人。' },
  { id: 'players', label: '玩家信息', icon: 'players-single', description: '查看活跃玩家、会话状态和风险标记。' },
  { id: 'permissions', label: '权限设置', icon: 'key-round', description: '查看角色、权限范围和审计策略。' },
]

const SECTION_IDS = new Set(SERVER_WORKBENCH_SECTIONS.map((section) => section.id))

export function normalizeWorkbenchSection(sectionId) {
  return SECTION_IDS.has(sectionId) ? sectionId : DEFAULT_WORKBENCH_SECTION
}

const REALTIME_LOG_TEMPLATES = [
  { level: 'INFO', source: 'server', message: '开始同步世界状态快照' },
  { level: 'SYSTEM', source: 'scheduler', message: '定时任务 heartbeat tick=60ms 正常' },
  { level: 'CHAT', source: 'chat', message: '[世界] RiverFox: 南侧据点已清场' },
  { level: 'WARN', source: 'network', message: '检测到部分玩家延迟抖动，正在持续观测' },
  { level: 'ERROR', source: 'rcon', message: '控制台命令队列出现一次重试，已自动恢复' },
]

function createRealtimeLogEntry(template, serverName, offset = 0) {
  return {
    id: `${serverName}-${template.level}-${offset}`,
    time: `19:${String(18 + (offset % 40)).padStart(2, '0')}:${String(10 + (offset % 50)).padStart(2, '0')}`,
    level: template.level,
    source: template.source,
    server: serverName,
    message: template.message,
  }
}

export function createInitialRealtimeLogs(serverName) {
  return REALTIME_LOG_TEMPLATES.map((template, index) => createRealtimeLogEntry(template, serverName, index))
}

export function appendRealtimeLogEntry(entries, serverName) {
  const nextIndex = entries.length
  const template = REALTIME_LOG_TEMPLATES[nextIndex % REALTIME_LOG_TEMPLATES.length]

  return [
    ...entries,
    createRealtimeLogEntry(template, serverName, nextIndex),
  ]
}

export function filterRealtimeLogEntries(entries, { level = 'ALL', searchTerm = '' }) {
  const normalizedSearchTerm = searchTerm.trim().toLowerCase()

  return entries.filter((entry) => {
    const levelMatches = level === 'ALL' || entry.level === level
    const searchMatches =
      normalizedSearchTerm === '' ||
      `${entry.time} ${entry.level} ${entry.server} ${entry.source} ${entry.message}`
        .toLowerCase()
        .includes(normalizedSearchTerm)

    return levelMatches && searchMatches
  })
}

export function normalizeAgentStreamEvent(eventType, rawData) {
  const payload = JSON.parse(rawData)

  if (eventType === 'agent.logChunk') {
    return {
      type: 'logChunk',
      payload,
    }
  }

  if (eventType === 'agent.fileChanged') {
    return {
      type: 'fileChanged',
      payload: {
        logicalPath: payload.logicalPath ?? payload.logical_path ?? '',
      },
    }
  }

  return null
}

function inferRealtimeLogLevel(rawLine) {
  const normalizedLine = rawLine.toUpperCase()

  if (normalizedLine.includes('ERROR')) {
    return 'ERROR'
  }
  if (normalizedLine.includes('WARN')) {
    return 'WARN'
  }
  if (normalizedLine.includes('CHAT')) {
    return 'CHAT'
  }
  if (normalizedLine.includes('SYSTEM')) {
    return 'SYSTEM'
  }

  return 'INFO'
}

function formatObservedAt(observedAt) {
  const value = Number(observedAt)
  if (!Number.isFinite(value)) {
    return '--:--:--'
  }

  return new Date(value).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
}

export function appendAgentLogChunk(entries, chunk, serverName) {
  const nextEntries = chunk.entries.map((entry, index) => ({
    id: `${entry.cursor ?? 'cursor'}-${entry.lineNumber ?? entry.line_number ?? index}`,
    time: formatObservedAt(entry.observedAt ?? entry.observed_at),
    level: inferRealtimeLogLevel(entry.rawLine ?? entry.raw_line ?? ''),
    source: entry.source ?? 'server',
    server: serverName,
    message: entry.rawLine ?? entry.raw_line ?? '',
  }))

  return [...entries, ...nextEntries]
}

function formatFileSize(size) {
  if (size == null) {
    return '--'
  }
  if (size < 1024) {
    return `${size} B`
  }

  return `${Math.round((size / 1024) * 10) / 10} KB`
}

export function buildConfigFileItems(entries) {
  return entries.map((entry) => {
    const logicalPath = entry.logicalPath ?? entry.logical_path ?? ''
    const segments = logicalPath.split('/').filter(Boolean)

    return {
      name: segments.at(-1) ?? logicalPath,
      path: logicalPath,
      isDir: Boolean(entry.isDir ?? entry.is_dir),
      sizeLabel: formatFileSize(entry.size ?? null),
    }
  })
}

export function createServerWorkbenchContent(server) {
  const serverName = server?.name ?? '未命名服务器'
  const serverUuid = server?.serverUuid ?? '--'
  const serverIp = server?.ip ?? '--'
  const serverPort = server?.rconPort ?? '--'
  const serverStatus = server?.statusLabel ?? '● 在线'

  return {
    overview: {
      metrics: [
        { label: '服务状态', value: serverStatus, meta: '近 24 小时稳定' },
        { label: '连接端点', value: `${serverIp}:${serverPort}`, meta: '已纳入统一代理' },
        { label: '配置版本', value: 'v2026.04.07', meta: '最近 2 小时更新' },
        { label: '告警等级', value: '低风险', meta: '暂无阻断项' },
      ],
      highlights: [
        { title: '最近发布', body: '控制面板和配置页骨架已就绪，后续可直接接入真实运维动作。 ' },
        { title: '关注点', body: '当前页面以服务器 UUID 作为主键，可继续扩展控制台、实时状态和查询类接口。 ' },
        { title: '推荐下一步', body: '优先补实时状态、聊天记录和玩家详情接口，这三个模块复用率最高。 ' },
      ],
      activity: [
        { title: '后台接入完成', detail: `${serverName} 已进入新的服务器工作台布局`, time: '刚刚' },
        { title: '详情页升级', detail: '二级导航和模块内容区已准备好接入真实逻辑', time: '2 分钟前' },
        { title: '数据结构稳定', detail: `当前主键为 ${serverUuid}`, time: '5 分钟前' },
      ],
    },
    control: {
      actionGroups: [
        {
          title: '生命周期',
          items: ['启动服务器', '停止服务器', '重启服务器', '进入维护模式'],
        },
        {
          title: '发布与回滚',
          items: ['同步配置', '执行热更新', '回滚上个版本', '清理临时文件'],
        },
        {
          title: '系统工具',
          items: ['查看控制台', '下载日志包', '刷新节点状态', '重新验证 RCON'],
        },
      ],
      safety: [
        '敏感动作需要二次确认',
        '高风险操作需要管理员授权',
        '执行结果将写入操作记录',
      ],
    },
    'realtime-logs': {
      connectionLabel: '已连接日志频道',
      streamName: `${serverName}-console`,
      levelOptions: ['ALL', 'INFO', 'SYSTEM', 'CHAT', 'WARN', 'ERROR'],
      commandPlaceholder: '后续接入控制台命令输入',
    },
    chat: {
      filters: ['全部频道', '世界频道', '队伍频道', '系统广播'],
      rows: [
        { time: '19:41', player: 'RiverFox', channel: '世界频道', content: '北部据点集合，5 分钟后开团。' },
        { time: '19:38', player: 'ZeroDust', channel: '队伍频道', content: '补给箱坐标已经标记，注意狙点。' },
        { time: '19:35', player: 'System', channel: '系统广播', content: '例行维护将在今日 23:30 开始。' },
      ],
    },
    flight: {
      incidents: [
        { level: '高', area: '矿山上空', player: 'NightScout', detail: '垂直位移异常，连续 3 次跨地形跳跃', status: '待复核' },
        { level: '中', area: '空投航线', player: 'BlueMint', detail: '短时间滞空移动，疑似技能或外挂脚本', status: '观察中' },
        { level: '低', area: '东海岸', player: 'StoneArc', detail: '单次高度异常，可能由载具物理碰撞导致', status: '已忽略' },
      ],
    },
    knockdown: {
      summary: [
        { label: '今日击倒', value: '182' },
        { label: '高热区域', value: '发电站' },
        { label: '平均交火时长', value: '43 秒' },
      ],
      rows: [
        { time: '19:43', attacker: 'Apex', defender: 'ColdRain', weapon: 'M39', distance: '86m' },
        { time: '19:40', attacker: 'Harbor', defender: 'Mint', weapon: 'AK', distance: '24m' },
        { time: '19:32', attacker: 'Nova', defender: 'Rift', weapon: 'L96', distance: '192m' },
      ],
    },
    match: {
      cards: [
        { title: '最近一局', value: '海港争夺战', sub: '时长 28 分钟 · 18 支队伍' },
        { title: '平均结算时长', value: '31 分钟', sub: '较上周缩短 7%' },
        { title: '观战峰值', value: '214', sub: '来自淘汰赛阶段' },
      ],
      timeline: [
        { title: '资格赛', detail: '今日 20:00 开始，已配置预热广播与候场规则' },
        { title: '淘汰赛', detail: '明日 19:30 进行，建议提前锁定配置版本' },
        { title: '总决赛', detail: '本周六 21:00，需额外启用裁判权限模板' },
      ],
    },
    'config-files': {
      files: [
        { name: 'server.cfg', path: '/configs/server.cfg', status: '已同步', size: '12 KB' },
        { name: 'anti-cheat.json', path: '/configs/security/anti-cheat.json', status: '待审核', size: '8 KB' },
        { name: 'rotation.yaml', path: '/configs/match/rotation.yaml', status: '最新', size: '4 KB' },
      ],
      preview: [
        'server_name = "GamePanel Arena"',
        'tick_rate = 60',
        'max_players = 200',
        'rcon_enabled = true',
      ],
    },
    'config-panel': {
      groups: [
        {
          title: '基础配置',
          fields: ['服务器名称', '最大人数', '地图轮换', '广播频率'],
        },
        {
          title: '安全策略',
          fields: ['反作弊阈值', '黑名单策略', '异常动作告警', '管理员审批'],
        },
        {
          title: '赛事开关',
          fields: ['观战模式', '裁判权限', '淘汰播报', '战绩回写'],
        },
      ],
    },
    operations: {
      rows: [
        { time: '19:42', operator: '超级管理员', action: '更新服务器配置', result: '成功', target: serverName },
        { time: '18:57', operator: '部署机器人', action: '执行详情页结构迁移', result: '成功', target: serverUuid },
        { time: '17:31', operator: '审计服务', action: '刷新权限模板', result: '已记录', target: '权限设置' },
      ],
    },
    players: {
      cards: [
        { label: '当前在线', value: '58', sub: '高峰期占用 29%' },
        { label: '风险玩家', value: '3', sub: '待人工复核' },
        { label: '新进玩家', value: '14', sub: '近 24 小时' },
      ],
      rows: [
        { name: 'RiverFox', level: 'Lv.48', status: '在线', note: '世界频道活跃' },
        { name: 'BlueMint', level: 'Lv.32', status: '待观察', note: '飞天记录中出现 1 次告警' },
        { name: 'ColdRain', level: 'Lv.66', status: '离线', note: '最近 7 天击倒数靠前' },
      ],
    },
    permissions: {
      roles: [
        { title: '服主', description: '拥有配置发布、危险操作和赛事控制权限。', scope: '全局管理' },
        { title: '值班管理员', description: '可查看日志、处理告警、编辑基础配置。', scope: '运营维护' },
        { title: '裁判', description: '可查看比赛记录、控制赛事开关、管理观战。', scope: '赛事运营' },
      ],
      matrix: [
        { permission: '查看服务器详情', owner: true, admin: true, referee: true },
        { permission: '修改配置文件', owner: true, admin: true, referee: false },
        { permission: '执行危险操作', owner: true, admin: false, referee: false },
        { permission: '查看比赛记录', owner: true, admin: true, referee: true },
      ],
    },
  }
}
