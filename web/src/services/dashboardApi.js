export function normalizeApiBaseUrl(value) {
  const normalizedValue = value?.trim()

  if (!normalizedValue) {
    return '/api'
  }

  return normalizedValue.replace(/\/+$/, '') || '/api'
}

export async function extractErrorMessage(response) {
  let message = `API request failed: ${response.status}`

  try {
    const errorPayload = await response.json()
    if (typeof errorPayload?.message === 'string' && errorPayload.message.trim() !== '') {
      message = errorPayload.message
    }
  } catch {
    // Ignore JSON parsing failures for non-JSON error responses.
  }

  return message
}

const API_BASE_URL = normalizeApiBaseUrl(import.meta.env?.VITE_API_BASE_URL)

async function request(path, options = {}) {
  const response = await fetch(`${API_BASE_URL}${path}`, {
    headers: {
      'Content-Type': 'application/json',
      ...(options.headers ?? {}),
    },
    ...options,
  })

  if (!response.ok) {
    throw new Error(await extractErrorMessage(response))
  }

  if (response.status === 204) {
    return null
  }

  return response.json()
}

function buildAgentPath(agentId, path) {
  return `/agents/${encodeURIComponent(agentId)}${path}`
}

export const dashboardApi = {
  async getDashboardData() {
    return request('/dashboard')
  },
  async getHealth() {
    return request('/health')
  },
  async getServer(serverUuid) {
    return request(`/servers/${serverUuid}`)
  },
  async addServer(payload) {
    return request('/servers', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
  },
  async updateServer(serverUuid, payload) {
    return request(`/servers/${serverUuid}`, {
      method: 'PUT',
      body: JSON.stringify(payload),
    })
  },
  async deleteServer(serverUuid) {
    return request(`/servers/${serverUuid}`, {
      method: 'DELETE',
    })
  },
  async getAgentFileTree(agentId, logicalPath) {
    return request(`${buildAgentPath(agentId, `/files/tree?path=${encodeURIComponent(logicalPath)}`)}`)
  },
  async getAgentFileContent(agentId, logicalPath) {
    return request(`${buildAgentPath(agentId, `/files/content?path=${encodeURIComponent(logicalPath)}`)}`)
  },
  async updateAgentFileContent(agentId, payload) {
    return request(buildAgentPath(agentId, '/files/content'), {
      method: 'PUT',
      body: JSON.stringify(payload),
    })
  },
  openAgentEvents(agentId, { EventSourceCtor = globalThis.EventSource } = {}) {
    return new EventSourceCtor(`${API_BASE_URL}${buildAgentPath(agentId, '/events')}`)
  },
  actions: {
    async onAddServer() {},
    async onExportPlayers() {},
    async onViewAllActivity() {},
    async onServerConsole() {},
    async onServerRestart() {},
    async onServerStart() {},
  },
}
