import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      // App.tsx was written for Tauri; on web, stub the event API.
      '@tauri-apps/api/event': path.resolve(__dirname, 'src/lib/tauri-event-shim.ts'),
    },
  },
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://localhost:8787',
    },
  },
  build: {
    outDir: 'dist',
  },
});
