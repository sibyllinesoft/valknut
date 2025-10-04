import { expect, test } from 'bun:test';
import path from 'path';
import fs from 'fs';
import { chromium } from 'playwright';

const projectRoot = path.resolve(__dirname, '..');
const reportPath = path.resolve(projectRoot, '../examples/live-report-sample.html');

if (!fs.existsSync(reportPath)) {
  throw new Error(`Sample report not found at ${reportPath}`);
}

let playwrightUnavailable = false;

class PlaywrightUnavailableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'PlaywrightUnavailableError';
  }
}

async function openSampleReport() {
  if (playwrightUnavailable) {
    throw new PlaywrightUnavailableError('Playwright browser sandbox not available');
  }

  const browser = await chromium
    .launch({
      headless: true,
      args: [
        '--no-sandbox',
        '--disable-setuid-sandbox',
        '--disable-dev-shm-usage',
        '--disable-web-security',
        '--disable-gpu',
      ],
    })
    .catch((error) => {
      const message = String(error instanceof Error ? error.message : error);
      const missingBinary = message.includes("Executable doesn't exist");
      const sandboxBlocked = message.includes('sandbox_host_linux') || message.includes('Operation not permitted');
      if (sandboxBlocked || missingBinary) {
        playwrightUnavailable = true;
        throw new PlaywrightUnavailableError(message);
      }
      throw error;
    });

  const page = await browser.newPage();
  await page.goto(`file://${reportPath}`);
  return { browser, page };
}

test('playwright: renders analysis summary in sample report', async () => {
  if (playwrightUnavailable) {
    test.skip('Playwright sandbox is unavailable in this environment');
    return;
  }

  let browser;
  let page;
  try {
    ({ browser, page } = await openSampleReport());
  } catch (error) {
    if (error instanceof PlaywrightUnavailableError) {
      test.skip('Playwright sandbox is unavailable in this environment');
      return;
    }
    throw error;
  }

  try {
    await expect(await page.locator('.hero-title', { hasText: 'Valknut Analysis Report' }).isVisible()).toBe(true);
    await expect(await page.locator('.demo-metric-card h3', { hasText: 'Files Selected' }).isVisible()).toBe(true);
  } finally {
    await browser.close();
  }
});

test('playwright: exposes tree container and file metadata', async () => {
  if (playwrightUnavailable) {
    test.skip('Playwright sandbox is unavailable in this environment');
    return;
  }

  let browser;
  let page;
  try {
    ({ browser, page } = await openSampleReport());
  } catch (error) {
    if (error instanceof PlaywrightUnavailableError) {
      test.skip('Playwright sandbox is unavailable in this environment');
      return;
    }
    throw error;
  }

  try {
    await expect(await page.locator('#file-tree-container').isVisible()).toBe(true);
    const metaText = await page.locator('.analysis-subheader').first().innerText();
    expect(metaText).toContain('Score');
  } finally {
    await browser.close();
  }
});
