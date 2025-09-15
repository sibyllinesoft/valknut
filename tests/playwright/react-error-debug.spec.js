/**
 * Playwright tests for React Tree Component - Error #31 Debugging
 * These tests run in real browsers to catch the actual React rendering issues
 */

import { test, expect } from '@playwright/test';
import path from 'path';

const reportPath = path.resolve('../../debug-final-test/report_20250914_131615.html');

test.describe('React Tree Component Browser Tests', () => {
  
  test('should detect React error #31 in console', async ({ page }) => {
    const consoleErrors = [];
    const reactErrors = [];
    
    // Capture console errors
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
        if (msg.text().includes('Error #31')) {
          reactErrors.push(msg.text());
        }
      }
    });
    
    // Capture JavaScript errors
    page.on('pageerror', error => {
      consoleErrors.push(error.message);
      if (error.message.includes('Error #31')) {
        reactErrors.push(error.message);
      }
    });
    
    // Navigate to our generated report
    await page.goto(`file://${reportPath}`);
    
    // Wait for React to attempt mounting
    await page.waitForTimeout(2000);
    
    // Log all console outputs for debugging
    console.log('All console errors:', consoleErrors);
    console.log('React #31 errors:', reactErrors);
    
    // The test should capture the React error #31
    expect(reactErrors.length).toBeGreaterThan(0);
  });
  
  test('should show debug logs from our component', async ({ page }) => {
    const debugLogs = [];
    
    page.on('console', msg => {
      const text = msg.text();
      if (text.includes('🔍') || text.includes('📄') || text.includes('✅') || text.includes('🏗️')) {
        debugLogs.push(text);
      }
    });
    
    await page.goto(`file://${reportPath}`);
    await page.waitForTimeout(2000);
    
    console.log('Debug logs captured:', debugLogs);
    
    // Should see our loading logs
    const loadingLogs = debugLogs.filter(log => log.includes('Loading tree data'));
    expect(loadingLogs.length).toBeGreaterThan(0);
  });
  
  test('should verify tree data is present in DOM', async ({ page }) => {
    await page.goto(`file://${reportPath}`);
    
    // Check if tree-data script element exists
    const treeDataElement = await page.locator('#tree-data').first();
    expect(await treeDataElement.count()).toBe(1);
    
    // Get the JSON content
    const jsonContent = await treeDataElement.textContent();
    expect(jsonContent.length).toBeGreaterThan(1000); // Should be substantial JSON
    
    // Verify JSON is parseable
    expect(() => JSON.parse(jsonContent)).not.toThrow();
    
    const data = JSON.parse(jsonContent);
    expect(data.refactoringCandidatesByFile).toBeDefined();
    expect(data.refactoringCandidatesByFile.length).toBeGreaterThan(0);
  });
  
  test('should check React dependencies are loaded', async ({ page }) => {
    await page.goto(`file://${reportPath}`);
    
    // Wait for scripts to load
    await page.waitForTimeout(1000);
    
    // Check if React is available
    const reactLoaded = await page.evaluate(() => typeof window.React !== 'undefined');
    expect(reactLoaded).toBe(true);
    
    // Check if ReactDOM is available  
    const reactDOMLoaded = await page.evaluate(() => typeof window.ReactDOM !== 'undefined');
    expect(reactDOMLoaded).toBe(true);
    
    // Check if our component is available
    const componentLoaded = await page.evaluate(() => typeof window.CodeAnalysisTree !== 'undefined');
    expect(componentLoaded).toBe(true);
  });
  
  test('should identify where React error occurs in component lifecycle', async ({ page }) => {
    const lifecycleLogs = [];
    
    page.on('console', msg => {
      const text = msg.text();
      // Capture component lifecycle logs
      if (text.includes('buildTreeData') || 
          text.includes('mounting') || 
          text.includes('createElement') ||
          text.includes('render')) {
        lifecycleLogs.push(text);
      }
    });
    
    await page.goto(`file://${reportPath}`);
    await page.waitForTimeout(3000);
    
    console.log('Component lifecycle logs:', lifecycleLogs);
    
    // Should see buildTreeData being called
    const buildTreeLogs = lifecycleLogs.filter(log => log.includes('buildTreeData'));
    expect(buildTreeLogs.length).toBeGreaterThan(0);
  });
  
  test('should test minimal React component isolation', async ({ page }) => {
    // Create a minimal test page
    const minimalHTML = `
<!DOCTYPE html>
<html>
<head>
    <title>Minimal React Test</title>
</head>
<body>
    <div id="react-test-root"></div>
    
    <script id="tree-data" type="application/json">
    {"refactoringCandidatesByFile": [{"fileName": "test.rs", "entities": []}], "directoryHealthTree": {"directories": {}}}
    </script>
    
    <script src="react.min.js"></script>
    <script src="react-dom.min.js"></script>
    <script src="react-tree-bundle.min.js"></script>
    
    <script>
        window.addEventListener('DOMContentLoaded', () => {
            console.log('🧪 Minimal test starting...');
            try {
                const { createRoot } = ReactDOM;
                const root = createRoot(document.getElementById('react-test-root'));
                root.render(React.createElement(window.CodeAnalysisTree));
                console.log('✅ Minimal test succeeded');
            } catch (error) {
                console.error('❌ Minimal test failed:', error);
            }
        });
    </script>
</body>
</html>`;
    
    // Set page content directly
    await page.setContent(minimalHTML);
    
    // Wait for test to run
    await page.waitForTimeout(2000);
    
    // Check for success/failure messages
    const testLogs = await page.evaluate(() => {
      return window.testResults || [];
    });
    
    console.log('Minimal test results:', testLogs);
  });
});
