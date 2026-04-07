import test from 'node:test'
import assert from 'node:assert/strict'

import {
  SERVER_WORKBENCH_SECTIONS,
  appendRealtimeLogEntry,
  createInitialRealtimeLogs,
  filterRealtimeLogEntries,
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
