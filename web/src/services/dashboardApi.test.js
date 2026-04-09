import test from 'node:test'
import assert from 'node:assert/strict'

import { dashboardApi, extractErrorMessage, normalizeApiBaseUrl } from './dashboardApi.js'

test('normalizeApiBaseUrl defaults to /api when env value is missing', () => {
  assert.equal(normalizeApiBaseUrl(undefined), '/api')
  assert.equal(normalizeApiBaseUrl(''), '/api')
  assert.equal(normalizeApiBaseUrl('   '), '/api')
})

test('normalizeApiBaseUrl trims trailing slashes from custom values', () => {
  assert.equal(normalizeApiBaseUrl('http://127.0.0.1:3000/api/'), 'http://127.0.0.1:3000/api')
  assert.equal(normalizeApiBaseUrl('/api///'), '/api')
})

test('extractErrorMessage prefers backend message payload', async () => {
  const response = new Response(JSON.stringify({ message: '后端连接失败' }), {
    status: 502,
    headers: {
      'Content-Type': 'application/json',
    },
  })

  await assert.doesNotReject(async () => {
    assert.equal(await extractErrorMessage(response), '后端连接失败')
  })
})

test('extractErrorMessage falls back to status code when payload is not json', async () => {
  const response = new Response('gateway error', {
    status: 502,
    headers: {
      'Content-Type': 'text/plain',
    },
  })

  await assert.doesNotReject(async () => {
    assert.equal(await extractErrorMessage(response), 'API request failed: 502')
  })
})

test('server-scoped workbench api methods call real backend endpoints', async () => {
  const calls = []
  const originalFetch = globalThis.fetch

  globalThis.fetch = async (input, options = {}) => {
    calls.push([input, options])

    return new Response(
      JSON.stringify({
        logicalPath: '/game-root/server.cfg',
        version: 'v2',
        entries: [],
        content: 'hostname=test\n',
        rules: [],
        items: [],
      }),
      {
        status: 200,
        headers: {
          'Content-Type': 'application/json',
        },
      },
    )
  }

  try {
    await dashboardApi.getServerFileTree('server-1', '/game-root')
    await dashboardApi.getServerFileContent('server-1', '/game-root/server.cfg')
    await dashboardApi.updateServerFileContent('server-1', {
      logicalPath: '/game-root/server.cfg',
      content: 'hostname=new\n',
      expectedVersion: 'v1',
    })
    await dashboardApi.getServerParseRules('server-1')
    await dashboardApi.updateServerParseRules('server-1', { rules: [] })
    await dashboardApi.getServerParsedEvents('server-1', { eventType: 'chat', limit: 50 })
  } finally {
    globalThis.fetch = originalFetch
  }

  assert.equal(calls[0][0], '/api/servers/server-1/files/tree?path=%2Fgame-root')
  assert.equal(calls[1][0], '/api/servers/server-1/files/content?path=%2Fgame-root%2Fserver.cfg')
  assert.equal(calls[2][0], '/api/servers/server-1/files/content')
  assert.equal(calls[2][1].method, 'PUT')
  assert.match(calls[2][1].body, /"logicalPath":"\/game-root\/server\.cfg"/)
  assert.equal(calls[3][0], '/api/servers/server-1/parse-rules')
  assert.equal(calls[4][0], '/api/servers/server-1/parse-rules')
  assert.equal(calls[4][1].method, 'PUT')
  assert.equal(calls[5][0], '/api/servers/server-1/parsed-events?eventType=chat&limit=50')
})

test('openServerEvents creates an EventSource against the SSE route', () => {
  const events = []

  class MockEventSource {
    constructor(url) {
      this.url = url
      events.push(url)
    }
  }

  const eventSource = dashboardApi.openServerEvents('server-1', {
    EventSourceCtor: MockEventSource,
  })

  assert.equal(events[0], '/api/servers/server-1/events')
  assert.equal(eventSource.url, '/api/servers/server-1/events')
})
