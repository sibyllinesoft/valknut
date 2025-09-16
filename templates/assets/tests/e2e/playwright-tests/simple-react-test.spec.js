import { test, expect } from '@playwright/test';

test.describe('Simple React Fix Test', () => {
  test('should verify React bundle loads without errors', async ({ page }) => {
    // Track console errors
    const consoleErrors = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    // Create a simple test HTML file
    const testHtml = `
<!DOCTYPE html>
<html>
<head>
    <title>React Fix Test</title>
    <style>
        :root {
            --text: #333;
            --muted: #666;
        }
        body { font-family: system-ui; margin: 20px; }
    </style>
</head>
<body>
    <div id="test-container"></div>
    <script src="../../../dist/react-tree-bundle.js"></script>
    <script>
        try {
            // Test empty state rendering (the fix we made)
            const emptyData = {
                refactoringCandidatesByFile: [],
                directoryHealthTree: null
            };
            
            const container = document.getElementById('test-container');
            const root = ReactDOM.createRoot(container);
            root.render(React.createElement(ReactTreeBundle, { data: emptyData }));
            
            console.log('✅ React component rendered successfully');
        } catch (error) {
            console.error('❌ React rendering failed:', error);
        }
    </script>
</body>
</html>`;

    // Use data URL to avoid file system complexity
    const dataUrl = `data:text/html;charset=utf-8,${encodeURIComponent(testHtml)}`;
    await page.goto(dataUrl);

    // Wait for React to render
    await page.waitForTimeout(3000);

    // Check for the empty state message
    const titleElement = page.locator('h3:has-text("No Refactoring Candidates Found")');
    const descElement = page.locator('p:has-text("Your code is in excellent shape!")');

    // These should be visible
    await expect(titleElement).toBeVisible({ timeout: 5000 });
    await expect(descElement).toBeVisible({ timeout: 5000 });

    // Most importantly: no React errors
    const reactErrors = consoleErrors.filter(error => 
      error.includes('Objects are not valid as a React child') ||
      error.includes('React error #31') ||
      error.includes('Uncaught Error')
    );

    expect(reactErrors).toHaveLength(0);

    console.log(`Test completed. Console errors detected: ${consoleErrors.length}`);
    if (consoleErrors.length > 0) {
      console.log('All console messages:', consoleErrors);
    }
  });

  test('should handle invalid data gracefully', async ({ page }) => {
    const consoleErrors = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    const testHtml = `
<!DOCTYPE html>
<html>
<head><title>React Invalid Data Test</title></head>
<body>
    <div id="test-container"></div>
    <script src="../../../dist/react-tree-bundle.js"></script>
    <script>
        try {
            // Test with null data
            const root = ReactDOM.createRoot(document.getElementById('test-container'));
            root.render(React.createElement(ReactTreeBundle, { data: null }));
        } catch (error) {
            console.error('React error:', error);
        }
    </script>
</body>
</html>`;

    const dataUrl = `data:text/html;charset=utf-8,${encodeURIComponent(testHtml)}`;
    await page.goto(dataUrl);
    await page.waitForTimeout(2000);

    // Should still show empty state message
    await expect(page.locator('text=No Refactoring Candidates Found')).toBeVisible();

    // No React errors
    const reactErrors = consoleErrors.filter(error => 
      error.includes('Objects are not valid as a React child')
    );
    expect(reactErrors).toHaveLength(0);
  });
});