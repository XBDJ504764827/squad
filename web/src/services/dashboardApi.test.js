import test from 'node:test'
import assert from 'node:assert/strict'

import { extractErrorMessage, normalizeApiBaseUrl } from './dashboardApi.js'

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
