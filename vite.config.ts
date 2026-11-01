import { defineConfig } from 'vite';

export default defineConfig({
  clearScreen: false,
  server: {
    host: '0.0.0.0',
    port: 1420,
    strictPort: true,
  },
  build: {
    target: 'es2022',
  },
});
