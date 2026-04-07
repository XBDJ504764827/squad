import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig, loadEnv } from 'vite'
import react from '@vitejs/plugin-react'

const currentDir = path.dirname(fileURLToPath(import.meta.url))
const backendEnvDir = path.resolve(currentDir, '../backend')

function createBackendProxyTarget(mode) {
  const webEnv = loadEnv(mode, currentDir, '')
  const backendEnv = loadEnv(mode, backendEnvDir, '')
  const backendPort = backendEnv.PORT ?? '3000'

  return webEnv.BACKEND_PROXY_TARGET ?? `http://127.0.0.1:${backendPort}`
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => ({
  plugins: [react()],
  server: {
    proxy: {
      '/api': {
        target: createBackendProxyTarget(mode),
        changeOrigin: true,
      },
    },
  },
}))
