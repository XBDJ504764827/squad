export function buildServerManagementPage({ rows, activeView, selectedServerUuid }) {
  const selectedServer = rows.find((row) => row.uuid === selectedServerUuid) ?? null

  if ((activeView === 'server-detail' || selectedServerUuid) && selectedServer) {
    return {
      mode: 'detail',
      title: selectedServer.name,
      server: selectedServer,
    }
  }

  return {
    mode: 'list',
    title: '服务器管理',
    server: null,
  }
}

export function buildServerViewAfterDelete({
  activeView,
  selectedServerUuid,
  deletedServerUuid,
}) {
  if (selectedServerUuid === deletedServerUuid) {
    return {
      activeView: 'server-manager',
      selectedServerUuid: null,
    }
  }

  return {
    activeView,
    selectedServerUuid,
  }
}
