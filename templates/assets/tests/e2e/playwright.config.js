// @ts-check
import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for Valknut E2E tests
 * Integrates with existing HTML generation pipeline
 * @see https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  testDir: './playwright-tests',
  timeout: 30 * 1000, // 30 seconds per test
  expect: {
    timeout: 5000 // 5 seconds for assertions
  },
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: [
    ['html', { outputDir: './test-results/playwright-report' }],
    ['junit', { outputFile: './test-results/junit-results.xml' }],
    ['list']
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
    },
    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },
    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'] },
    }
  ],

  // Global setup removed due to bun/module conflicts
  // Use: bun run generate before running playwright tests
});