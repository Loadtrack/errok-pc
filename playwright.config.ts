import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 30_000,
  fullyParallel: false,
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:7100',
    viewport: { width: 1440, height: 900 },
    launchOptions: { channel: 'msedge' },
  },
  webServer: {
    command: 'npm run dev -- --port 7100',
    url: 'http://127.0.0.1:7100',
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
