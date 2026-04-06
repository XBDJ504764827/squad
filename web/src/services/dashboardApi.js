import { createDefaultDashboardData } from '../data/defaultDashboard'

const clone = (value) => {
  if (typeof structuredClone === 'function') {
    return structuredClone(value)
  }
  return JSON.parse(JSON.stringify(value))
}

let dataAdapter = {
  async fetchDashboard() {
    return clone(createDefaultDashboardData())
  },
}

let actionHandlers = {
  async onRefresh() {},
  async onAddServer() {},
  async onExportPlayers() {},
  async onViewAllActivity() {},
  async onServerConsole() {},
  async onServerRestart() {},
  async onServerStart() {},
}

export const dashboardApi = {
  setDataAdapter(nextAdapter = {}) {
    dataAdapter = { ...dataAdapter, ...nextAdapter }
  },
  setActionHandlers(nextHandlers = {}) {
    actionHandlers = { ...actionHandlers, ...nextHandlers }
  },
  async getDashboardData() {
    const payload = await dataAdapter.fetchDashboard()
    return this.normalizeDashboardData(payload)
  },
  normalizeDashboardData(payload) {
    return payload
  },
  get actions() {
    return actionHandlers
  },
}
