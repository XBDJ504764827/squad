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
  async getServerAgentAuth(serverUuid) {
    return request(`/servers/${serverUuid}/agent-auth`)
  },
  async rotateServerAgentKey(serverUuid) {
    return request(`/servers/${serverUuid}/agent-auth-key`, {
      method: 'POST',
    })
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
  async getServerFileTree(serverUuid, logicalPath) {
    return request(`/servers/${serverUuid}/files/tree?path=${encodeURIComponent(logicalPath)}`)
  },
  async getServerFileContent(serverUuid, logicalPath) {
    return request(`/servers/${serverUuid}/files/content?path=${encodeURIComponent(logicalPath)}`)
  },
  async updateServerFileContent(serverUuid, payload) {
    return request(`/servers/${serverUuid}/files/content`, {
      method: 'PUT',
      body: JSON.stringify(payload),
    })
  },
  async getServerParseRules(serverUuid) {
    return request(`/servers/${serverUuid}/parse-rules`)
  },
  async getServerFeatureFlags(serverUuid) {
    return request(`/servers/${serverUuid}/feature-flags`)
  },
  async updateServerFeatureFlag(serverUuid, featureKey, payload) {
    return request(`/servers/${serverUuid}/feature-flags/${featureKey}`, {
      method: 'PUT',
      body: JSON.stringify(payload),
    })
  },
  async updateServerParseRules(serverUuid, payload) {
    return request(`/servers/${serverUuid}/parse-rules`, {
      method: 'PUT',
      body: JSON.stringify(payload),
    })
  },
  async getServerParsedEvents(serverUuid, { eventType, limit, before } = {}) {
    const query = new URLSearchParams()
    if (eventType) {
      query.set('eventType', eventType)
    }
    if (limit != null) {
      query.set('limit', String(limit))
    }
    if (before != null) {
      query.set('before', String(before))
    }

    const suffix = query.size > 0 ? `?${query.toString()}` : ''
    return request(`/servers/${serverUuid}/parsed-events${suffix}`)
  },
  openServerEvents(serverUuid, { EventSourceCtor = globalThis.EventSource } = {}) {
    return new EventSourceCtor(`${API_BASE_URL}/servers/${serverUuid}/events`)
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
