// Builds the React UI as a plain web app: the Tauri app's front end, and the browser preview (npm run dev:web).
import { resolve } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  root: resolve(__dirname, 'src/renderer'),
  base: './',
  plugins: [react()],
  resolve: {
    alias: { '@shared': resolve(__dirname, 'src/shared'), '@': resolve(__dirname, 'src/renderer/src') }
  },
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { outDir: resolve(__dirname, 'out/web'), emptyOutDir: true, target: 'es2022' }
})
