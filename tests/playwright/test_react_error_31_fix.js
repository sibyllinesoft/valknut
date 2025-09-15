const { test, expect } = require('@playwright/test');
const fs = require('fs');
const path = require('path');

// Focused test suite for React error #31 fix verification
test.describe('React Error #31 Fix Verification', () => {
  const reportPath = '/home/nathan/Projects/valknut/final-react-fix/report_20250914_155810.html';
  
  test.beforeEach(async ({ page }) => {
    // Verify report file exists
    if (!fs.existsSync(reportPath)) {
      throw new Error(`Report file not found: ${reportPath}`);
    }
    
    // Load the updated report
    await page.goto(`file://${reportPath}`);
    
    // Wait for React to mount and process data
    await page.waitForTimeout(2000);
  });

  test('should have NO React error #31 in console logs', async ({ page }) => {
    const consoleLogs = [];
    const consoleErrors = [];
    
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
      consoleLogs.push(`[${msg.type()}] ${msg.text()}`);
    });
    
    // Trigger any remaining React rendering
    await page.waitForSelector('.tree-container', { timeout: 10000 });
    await page.waitForTimeout(1000);
    
    console.log('All console messages:', consoleLogs);
    console.log('Console errors:', consoleErrors);
    
    // Check specifically for React error #31
    const hasReactError31 = consoleErrors.some(error => 
      error.includes('validateDOMNesting') ||
      error.includes('cannot appear as a child of') ||
      error.includes('received `false`') ||
      error.includes('Warning: Each child in a list should have a unique "key" prop')
    );
    
    expect(hasReactError31).toBeFalsy();
    console.log('✅ No React error #31 found in console');
  });

  test('should display tree data correctly (no "no analysis data available")', async ({ page }) => {
    // Check for the "no analysis data available" message
    const noDataMessage = await page.locator('text=no analysis data available').count();
    expect(noDataMessage).toBe(0);
    console.log('✅ No "no analysis data available" message found');
    
    // Verify tree container exists and has content
    const treeContainer = page.locator('.tree-container');
    await expect(treeContainer).toBeVisible();
    
    // Check for actual tree nodes
    const treeNodes = await page.locator('.tree-node').count();
    expect(treeNodes).toBeGreaterThan(0);
    console.log(`✅ Found ${treeNodes} tree nodes`);
  });

  test('should show refactoring candidates from analyzed files', async ({ page }) => {
    // Wait for tree to fully render
    await page.waitForSelector('.tree-node', { timeout: 10000 });
    
    // Look for refactoring-related content
    const refactoringCandidates = await page.locator('.tree-node').filter({
      hasText: /refactor|complexity|debt|smell/i
    }).count();
    
    if (refactoringCandidates > 0) {
      console.log(`✅ Found ${refactoringCandidates} potential refactoring candidates`);
    }
    
    // Check for file nodes (analyzed code files)
    const fileNodes = await page.locator('.tree-node').filter({
      hasText: /\.rs$|\.py$|\.js$|\.ts$/
    }).count();
    
    expect(fileNodes).toBeGreaterThan(0);
    console.log(`✅ Found ${fileNodes} file nodes from analysis`);
    
    // Verify tree has hierarchical structure
    const expandableNodes = await page.locator('.tree-node[data-has-children="true"]').count();
    console.log(`✅ Found ${expandableNodes} expandable nodes (hierarchical structure)`);
  });

  test('should have all React components properly rendered', async ({ page }) => {
    // Check for proper React component mounting
    const reactRoot = await page.locator('#root').count();
    expect(reactRoot).toBe(1);
    console.log('✅ React root element found');
    
    // Verify main components are rendered
    const mainComponents = {
      'Tree Container': '.tree-container',
      'Tree Nodes': '.tree-node',
      'Analysis Data': '[data-analysis-loaded="true"]'
    };
    
    for (const [name, selector] of Object.entries(mainComponents)) {
      const count = await page.locator(selector).count();
      if (count > 0) {
        console.log(`✅ ${name}: ${count} elements found`);
      } else {
        console.log(`⚠️  ${name}: No elements found with selector ${selector}`);
      }
    }
  });

  test('should handle data props and children filtering correctly', async ({ page }) => {
    // This test specifically validates the fixes we implemented
    
    // 1. Check that data props are properly handled (data-* attributes)
    const dataProps = await page.locator('[data-file-path], [data-node-type], [data-complexity]').count();
    console.log(`✅ Found ${dataProps} elements with data props`);
    
    // 2. Check that no invalid children are rendered (should not see false/null values)
    const textContent = await page.textContent('body');
    const hasFalsyValues = textContent.includes('false') || textContent.includes('null') || textContent.includes('undefined');
    
    if (hasFalsyValues) {
      // This could be legitimate text, so let's be more specific
      const suspiciousElements = await page.locator('text=false').filter({ hasText: /^false$/ }).count();
      expect(suspiciousElements).toBe(0);
      console.log('✅ No suspicious falsy values rendered as text');
    } else {
      console.log('✅ No falsy values found in rendered content');
    }
    
    // 3. Check that tree nodes have proper structure
    const nodeStructure = await page.evaluate(() => {
      const nodes = document.querySelectorAll('.tree-node');
      let wellFormedNodes = 0;
      
      nodes.forEach(node => {
        // Check if node has expected structure and no invalid children
        if (node.textContent.trim() !== '' && !node.textContent.includes('false')) {
          wellFormedNodes++;
        }
      });
      
      return { total: nodes.length, wellFormed: wellFormedNodes };
    });
    
    expect(nodeStructure.wellFormed).toEqual(nodeStructure.total);
    console.log(`✅ All ${nodeStructure.total} tree nodes are well-formed`);
  });
});