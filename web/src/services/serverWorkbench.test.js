import test from 'node:test'
import assert from 'node:assert/strict'

import {
  SERVER_WORKBENCH_SECTIONS,
  appendAgentLogChunk,
  appendRealtimeLogEntry,
  buildConfigFileItems,
  canUseAgentWorkbench,
  createInitialRealtimeLogs,
  describeAgentAuthStatus,
  filterRealtimeLogEntries,
  normalizeAgentStreamEvent,
  normalizeWorkbenchSection,
} from './serverWorkbench.js'

test('SERVER_WORKBENCH_SECTIONS contains all expected management modules', () => {
  assert.deepEqual(
    SERVER_WORKBENCH_SECTIONS.map((section) => section.id),
    [
      'overview',
      'control',
      'realtime-logs',
      'chat',
      'flight',
      'knockdown',
      'match',
      'config-files',
      'config-panel',
      'operations',
      'players',
      'permissions',
    ],
  )
})

test('normalizeWorkbenchSection falls back to overview when section id is unknown', () => {
  assert.equal(normalizeWorkbenchSection('unknown-section'), 'overview')
  assert.equal(normalizeWorkbenchSection('players'), 'players')
})

test('appendRealtimeLogEntry appends a new log with a stable shape', () => {
  const initialLogs = createInitialRealtimeLogs('Alpha')
  const nextLogs = appendRealtimeLogEntry(initialLogs, 'Alpha')

  assert.equal(nextLogs.length, initialLogs.length + 1)
  assert.equal(typeof nextLogs.at(-1).message, 'string')
  assert.equal(typeof nextLogs.at(-1).level, 'string')
  assert.equal(nextLogs.at(-1).server, 'Alpha')
})

test('filterRealtimeLogEntries filters by level and search term', () => {
  const logs = [
    { id: '1', level: 'INFO', server: 'Alpha', source: 'system', message: 'boot complete' },
    { id: '2', level: 'WARN', server: 'Alpha', source: 'chat', message: 'high ping detected' },
  ]

  assert.equal(filterRealtimeLogEntries(logs, { level: 'ALL', searchTerm: '' }).length, 2)
  assert.equal(filterRealtimeLogEntries(logs, { level: 'WARN', searchTerm: '' }).length, 1)
  assert.equal(filterRealtimeLogEntries(logs, { level: 'ALL', searchTerm: 'boot' }).length, 1)
})

test('normalizeAgentStreamEvent parses log chunk and file change payloads', () => {
  const logEvent = normalizeAgentStreamEvent('agent.logChunk', JSON.stringify({
    entries: [
      {
        agent_id: 'agent-1',
        source: 'server',
        cursor: '1',
        line_number: 1,
        raw_line: '[WARN] high ping',
        observed_at: '1710000000000',
      },
    ],
  }))
  const fileEvent = normalizeAgentStreamEvent('agent.fileChanged', JSON.stringify({
    logical_path: '/game-root/server.cfg',
  }))

  assert.equal(logEvent.type, 'logChunk')
  assert.equal(logEvent.payload.entries[0].raw_line, '[WARN] high ping')
  assert.equal(fileEvent.type, 'fileChanged')
  assert.equal(fileEvent.payload.logicalPath, '/game-root/server.cfg')
})

test('appendAgentLogChunk converts backend log envelopes into workbench log rows', () => {
  const nextLogs = appendAgentLogChunk([], {
    entries: [
      {
        agent_id: 'agent-1',
        source: 'server',
        cursor: '2',
        line_number: 2,
        raw_line: '[ERROR] command failed',
        observed_at: '1710000000000',
      },
    ],
  }, 'Alpha')

  assert.equal(nextLogs.length, 1)
  assert.equal(nextLogs[0].level, 'ERROR')
  assert.equal(nextLogs[0].server, 'Alpha')
  assert.equal(nextLogs[0].message, '[ERROR] command failed')
})

test('buildConfigFileItems keeps files and folders in a stable workbench shape', () => {
  const items = buildConfigFileItems([
    { logicalPath: '/game-root', isDir: true, size: null },
    { logicalPath: '/game-root/server.cfg', isDir: false, size: 128 },
  ])

  assert.equal(items.length, 2)
  assert.equal(items[0].name, 'game-root')
  assert.equal(items[1].name, 'server.cfg')
  assert.equal(items[1].sizeLabel, '128 B')
})

test('canUseAgentWorkbench only allows access when key exists and agent is online', () => {
  assert.equal(canUseAgentWorkbench({ hasKey: false, agentOnline: false, agentId: null }), false)
  assert.equal(canUseAgentWorkbench({ hasKey: true, agentOnline: false, agentId: 'agent-1' }), false)
  assert.equal(canUseAgentWorkbench({ hasKey: true, agentOnline: true, agentId: '' }), false)
  assert.equal(canUseAgentWorkbench({ hasKey: true, agentOnline: true, agentId: 'agent-1' }), true)
})

test('describeAgentAuthStatus returns user-facing auth state labels', () => {
  assert.equal(describeAgentAuthStatus({ hasKey: false, agentOnline: false }), '未生成 Agent Key')
  assert.equal(describeAgentAuthStatus({ hasKey: true, agentOnline: false }), '已生成 Key，等待 Agent 连接')
  assert.equal(describeAgentAuthStatus({ hasKey: true, agentOnline: true }), 'Agent 在线，可进行测试')
})
