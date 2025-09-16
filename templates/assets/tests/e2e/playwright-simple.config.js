// @ts-check
import { defineConfig, devices } from '@playwright/test';

/**
 * Simplified Playwright configuration for React fix validation
 */
export default defineConfig({
  testDir: './playwright-tests',
  testMatch: 'simple-react-test.spec.js',
  timeout: 30 * 1000,
  expect: {
    timeout: 10000
  },
  fullyParallel: false, // Run sequentially for simpler debugging
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: [
    ['list'],
    ['html', { outputDir: './test-results/playwright-report', open: 'never' }]
  ],
  use: {
    actionTimeout: 0,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    }
  ],
});