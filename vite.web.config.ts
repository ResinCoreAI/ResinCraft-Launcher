// Builds the React UI as a plain web app: the Tauri app's front end, and the browser preview (npm run dev:web).
import { resolve } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  root: resolve(__dirname, 'src/renderer'),
  base: './',
  plugins: [react()],
  // The day this copy was built, for a Beta's version name ("B2.0.1 Build2026.11.23.1", versionName.ts).
  // This PC's own date (not UTC), so a build made just after midnight in Thailand says that day.
  define: { __BUILD_DATE__: JSON.stringify(new Date().toLocaleDateString('sv').replace(/-/g, '.')) },
  resolve: {
    alias: { '@shared': resolve(__dirname, 'src/shared'), '@': resolve(__dirname, 'src/renderer/src') }
  },
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { outDir: resolve(__dirname, 'out/web'), emptyOutDir: true, target: 'es2022' }
})
