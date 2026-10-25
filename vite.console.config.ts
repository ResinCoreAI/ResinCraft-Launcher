// Builds the web console (console.html): the launcher's own server pages, with ResinCraft Relay as the engine. The
// relay hosts out/console at console.<relay domain> behind its sign-in (see the relay's remote-control prompt).
import { resolve } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  root: resolve(__dirname, 'src/renderer'),
  base: '/',
  plugins: [react()],
  resolve: {
    alias: { '@shared': resolve(__dirname, 'src/shared'), '@': resolve(__dirname, 'src/renderer/src') }
  },
  define: { 'import.meta.env.VITE_RESIN_CONSOLE': JSON.stringify('1') },
  clearScreen: false,
  server: { port: 1430, strictPort: true, open: '/console.html' },
  build: {
    outDir: resolve(__dirname, 'out/console'),
    emptyOutDir: true,
    target: 'es2022',
    rollupOptions: { input: resolve(__dirname, 'src/renderer/console.html') }
  }
})
