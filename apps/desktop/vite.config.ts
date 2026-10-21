import react from '@vitejs/plugin-react'
import {defineConfig} from 'vite'

export default defineConfig({
  plugins: [react()],
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) return
          if (id.includes('@primer/')) return 'vendor-primer'
          if (id.includes('lucide-react')) return 'vendor-icons'
          if (id.includes('@tauri-apps/') || id.includes('@auditaur/')) return 'vendor-tauri'
          return 'vendor'
        },
      },
    },
  },
  clearScreen: false,
  server: {
    strictPort: true,
    host: '127.0.0.1',
    port: 17420,
  },
  envPrefix: ['VITE_', 'TAURI_'],
})
