const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? '/api'

async function request(path, options = {}) {
  const response = await fetch(`${API_BASE_URL}${path}`, {
    headers: {
      'Content-Type': 'application/json',
      ...(options.headers ?? {}),
    },
    ...options,
  })

  if (!response.ok) {
    let message = `API request failed: ${response.status}`

    try {
      const errorPayload = await response.json()
      if (typeof errorPayload?.message === 'string') {
        message = errorPayload.message
      }
    } catch {
      // Ignore JSON parsing failures for non-JSON error responses.
    }

    throw new Error(message)
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
  async addServer(payload) {
    return request('/servers', {
      method: 'POST',
      body: JSON.stringify(payload),
    })
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
