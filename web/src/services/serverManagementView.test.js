import test from 'node:test'
import assert from 'node:assert/strict'

import {
  buildServerManagementPage,
  buildServerViewAfterDelete,
} from './serverManagementView.js'

test('buildServerManagementPage creates a detail view for the selected server', () => {
  const rows = [
    { uuid: 'server-a', name: 'Alpha', ip: '127.0.0.1:28016', status: { label: '● 在线' } },
    { uuid: 'server-b', name: 'Bravo', ip: '127.0.0.1:28017', status: { label: '● 在线' } },
  ]

  const page = buildServerManagementPage({
    rows,
    activeView: 'server-manager',
    selectedServerUuid: 'server-b',
  })

  assert.equal(page.mode, 'detail')
  assert.equal(page.server.uuid, 'server-b')
  assert.equal(page.title, 'Bravo')
})

test('buildServerViewAfterDelete returns the list page after deleting the selected server', () => {
  const nextState = buildServerViewAfterDelete({
    activeView: 'server-detail',
    selectedServerUuid: 'server-a',
    deletedServerUuid: 'server-a',
  })

  assert.deepEqual(nextState, {
    activeView: 'server-manager',
    selectedServerUuid: null,
  })
})
