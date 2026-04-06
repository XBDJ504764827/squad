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
    throw new Error(`API request failed: ${response.status}`)
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
  actions: {
    async onAddServer() {},
    async onExportPlayers() {},
    async onViewAllActivity() {},
    async onServerConsole() {},
    async onServerRestart() {},
    async onServerStart() {},
  },
}
