import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [svelte()],
  // Tauri expects a fixed port and prints its own logs.
  clearScreen: false,
  server: { port: 5173, strictPort: true },
})
