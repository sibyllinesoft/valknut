/**
 * React Error #31 Debugging Test Suite
 * 
 * This comprehensive test captures browser console errors, specifically targeting
 * React Error #31 which occurs when objects are passed as React children.
 * 
 * React Error #31: "Objects are not valid as a React child (found: object with keys {}).
 * If you meant to render a collection of children, use an array instead."
 */

const { test, expect } = require('@playwright/test');
const path = require('path');

test.describe('React Error #31 Debugging', () => {
  let consoleErrors = [];
  let consoleWarnings = [];
  let consoleMessages = [];

  test.beforeEach(async ({ page }) => {
    // Reset error collectors
    consoleErrors = [];
    consoleWarnings = [];
    consoleMessages = [];

    // Listen for all console events
    page.on('console', msg => {
      const message = {
        type: msg.type(),
        text: msg.text(),
        location: msg.location(),
        timestamp: new Date().toISOString()
      };
      
      consoleMessages.push(message);
      
      if (msg.type() === 'error') {
        consoleErrors.push(message);
        console.log(`🚨 BROWSER ERROR: ${msg.text()}`);
        console.log(`📍 Location:`, msg.location());
      } else if (msg.type() === 'warning') {
        consoleWarnings.push(message);
        console.log(`⚠️ BROWSER WARNING: ${msg.text()}`);
      } else {
        console.log(`📝 BROWSER ${msg.type().toUpperCase()}: ${msg.text()}`);
      }
    });

    // Listen for page errors (JavaScript exceptions)
    page.on('pageerror', error => {
      console.log(`💥 PAGE ERROR: ${error.message}`);
      console.log(`🔍 Stack: ${error.stack}`);
      consoleErrors.push({
        type: 'pageerror',
        text: error.message,
        stack: error.stack,
        timestamp: new Date().toISOString()
      });
    });

    // Listen for request failures  
    page.on('requestfailed', request => {
      console.log(`📡 REQUEST FAILED: ${request.url()} - ${request.failure().errorText}`);
    });
  });

  test('Debug React Error #31 in main report page', async ({ page }) => {
    const reportPath = path.resolve('../../debug-final-test/report_20250914_131615.html');
    
    console.log('🚀 Loading report page:', reportPath);
    
    // Load the page with extended timeout for debugging
    await page.goto(`file://${reportPath}`, { 
      waitUntil: 'networkidle',
      timeout: 10000 
    });
    
    console.log('✅ Page loaded, waiting for React components...');
    
    // Wait for React components to initialize
    await page.waitForTimeout(2000);
    
    // Try to find the tree component or error state
    await page.waitForSelector('body', { timeout: 5000 });
    
    // Check if React tree component mounted
    const hasTreeComponent = await page.evaluate(() => {
      return !!window.CodeAnalysisTree;
    });
    
    console.log('🌳 React tree component available:', hasTreeComponent);
    
    // Take screenshot for visual debugging
    await page.screenshot({ 
      path: 'debug-react-error-main-page.png', 
      fullPage: true 
    });
    
    // Analyze error patterns
    console.log('\n📊 ERROR ANALYSIS:');
    console.log(`Total console errors: ${consoleErrors.length}`);
    console.log(`Total console warnings: ${consoleWarnings.length}`);
    console.log(`Total console messages: ${consoleMessages.length}`);
    
    // Look for React Error #31 specifically
    const reactError31 = consoleErrors.find(error => 
      error.text.includes('Objects are not valid as a React child') ||
      error.text.includes('Error #31') ||
      error.text.includes('use an array instead')
    );
    
    if (reactError31) {
      console.log('\n🎯 FOUND REACT ERROR #31!');
      console.log('Error text:', reactError31.text);
      console.log('Location:', reactError31.location);
      console.log('Timestamp:', reactError31.timestamp);
    }
    
    // Report all unique errors
    const uniqueErrors = [...new Set(consoleErrors.map(e => e.text))];
    uniqueErrors.forEach(error => {
      console.log(`❌ Unique Error: ${error}`);
    });
    
    // Assert we captured some meaningful data
    expect(consoleMessages.length).toBeGreaterThan(0);
  });

  test('Test minimal React component with potential error triggers', async ({ page }) => {
    console.log('🧪 Testing minimal React component setup...');
    
    // Create a minimal test page with the specific pattern that causes Error #31
    const testHTML = `
<!DOCTYPE html>
<html>
<head>
    <title>React Error #31 Test</title>
    <script src="https://unpkg.com/react@18/umd/react.development.js"></script>
    <script src="https://unpkg.com/react-dom@18/umd/react-dom.development.js"></script>
</head>
<body>
    <div id="root"></div>
    
    <script>
      console.log('🚀 Starting React Error #31 test...');
      
      // Test pattern that commonly causes Error #31
      const TestComponent = () => {
        const items = [
          { name: 'test1', data: { nested: true } },
          { name: 'test2', data: { nested: false } }
        ];
        
        // POTENTIAL ERROR PATTERN: Boolean expressions as children
        const children = [];
        
        items.forEach((item, index) => {
          children.push(React.createElement('div', { key: index }, item.name));
          
          // This pattern can cause Error #31:
          item.data && children.push(React.createElement('span', { key: 'data-' + index }, 'has data'));
          
          // Another problematic pattern:
          children.push(item.data.nested && React.createElement('span', { key: 'nested-' + index }, 'nested'));
        });
        
        return React.createElement('div', {}, children);
      };
      
      // Mount the component
      const root = ReactDOM.createRoot(document.getElementById('root'));
      root.render(React.createElement(TestComponent));
      
      console.log('✅ Component rendered');
    </script>
</body>
</html>`;

    // Set page content directly
    await page.setContent(testHTML);
    
    // Wait for React to process
    await page.waitForTimeout(1000);
    
    // Check if component rendered
    const hasContent = await page.locator('#root').textContent();
    console.log('📄 Rendered content:', hasContent);
    
    // Take screenshot
    await page.screenshot({ 
      path: 'debug-react-minimal-test.png' 
    });
    
    // Analyze errors from this specific test
    console.log('\n📊 MINIMAL TEST ANALYSIS:');
    console.log(`Errors: ${consoleErrors.length}`);
    console.log(`Warnings: ${consoleWarnings.length}`);
    
    if (consoleErrors.length > 0) {
      consoleErrors.forEach((error, index) => {
        console.log(`\n❌ Error ${index + 1}:`);
        console.log(`  Text: ${error.text}`);
        console.log(`  Type: ${error.type}`);
        if (error.location) {
          console.log(`  File: ${error.location.url}`);
          console.log(`  Line: ${error.location.lineNumber}:${error.location.columnNumber}`);
        }
      });
    }
  });

  test('Analyze tree.js source patterns for Error #31 causes', async ({ page }) => {
    console.log('🔍 Analyzing tree.js source patterns...');
    
    // Load a page that includes our tree.js component
    const testHTML = `
<!DOCTYPE html>
<html>
<head>
    <title>Tree.js Analysis</title>
    <script src="https://unpkg.com/react@18/umd/react.development.js"></script>
    <script src="https://unpkg.com/react-dom@18/umd/react-dom.development.js"></script>
    <script type="application/json" id="tree-data">
    {
      "refactoringCandidatesByFile": [
        {
          "filePath": "src/test.js",
          "highestPriority": "high",
          "entityCount": 2,
          "avgScore": 0.75,
          "totalIssues": 3,
          "entities": [
            {
              "name": "testFunction",
              "priority": "high",
              "score": 0.8,
              "lineRange": [1, 10],
              "issues": ["complexity"],
              "suggestions": ["refactor"]
            }
          ]
        }
      ],
      "directoryHealthTree": {
        "directories": {
          "src": {
            "health_score": 0.85,
            "file_count": 5,
            "entity_count": 10,
            "refactoring_needed": true,
            "critical_issues": 1,
            "high_priority_issues": 2,
            "avg_refactoring_score": 0.75
          }
        }
      }
    }
    </script>
</head>
<body>
    <div id="root"></div>
    
    <script>
      // Copy the exact patterns from tree.js that might cause Error #31
      const TreeNode = ({ node, style, dragHandle, tree }) => {
        const { data } = node;
        const isFolder = data.type === 'folder';
        const isFile = data.type === 'file';
        
        const children = [
          React.createElement('span', { key: 'label' }, data.name)
        ];
        
        // SUSPECTED ERROR PATTERN from tree.js lines 44-54:
        // This conditional push pattern may cause Error #31
        if (isFolder && data.healthScore) {
          children.push(React.createElement('div', {
            key: 'health',
            className: 'tree-badge tree-badge-low'
          }, 'Health: ' + (data.healthScore * 100).toFixed(0) + '%'));
        }
        
        // SUSPECTED ERROR PATTERN from lines 57-63:
        if (data.priority || data.highestPriority) {
          children.push(React.createElement('div', {
            key: 'priority',
            className: 'tree-badge'
          }, data.priority || data.highestPriority));
        }
        
        // SUSPECTED ERROR PATTERN from lines 66-72:
        if (data.entityCount || data.fileCount) {
          children.push(React.createElement('div', {
            key: 'count',
            className: 'tree-badge tree-badge-low'
          }));
        }
        
        return React.createElement('div', {
          style: { ...style, display: 'flex', alignItems: 'center' }
        }, children);
      };
      
      // Test with sample data
      const testNode = {
        data: {
          name: 'test.js',
          type: 'file',
          priority: 'high',
          entityCount: 5,
          healthScore: 0.85
        }
      };
      
      console.log('🧪 Testing TreeNode with sample data...');
      
      const root = ReactDOM.createRoot(document.getElementById('root'));
      root.render(React.createElement(TreeNode, { 
        node: testNode,
        style: {},
        dragHandle: React.createRef(),
        tree: { toggle: () => {} }
      }));
      
      console.log('✅ TreeNode test rendered');
    </script>
</body>
</html>`;

    await page.setContent(testHTML);
    await page.waitForTimeout(1500);
    
    // Take screenshot
    await page.screenshot({ 
      path: 'debug-tree-source-analysis.png' 
    });
    
    console.log('\n📊 TREE.JS PATTERN ANALYSIS:');
    console.log(`Errors: ${consoleErrors.length}`);
    console.log(`Warnings: ${consoleWarnings.length}`);
    
    // Look for specific error patterns related to our code
    const suspiciousErrors = consoleErrors.filter(error => 
      error.text.includes('React child') ||
      error.text.includes('array instead') ||
      error.text.includes('object with keys')
    );
    
    if (suspiciousErrors.length > 0) {
      console.log('\n🎯 FOUND SUSPICIOUS ERRORS IN TREE.JS PATTERNS:');
      suspiciousErrors.forEach((error, index) => {
        console.log(`\n❌ Suspicious Error ${index + 1}:`);
        console.log(`  Text: ${error.text}`);
        console.log(`  Location: ${error.location?.url} at ${error.location?.lineNumber}:${error.location?.columnNumber}`);
      });
    }
    
    expect(page.locator('#root')).toBeTruthy();
  });

  test.afterEach(async ({ page }, testInfo) => {
    // Generate detailed error report after each test
    console.log(`\n📋 TEST COMPLETION REPORT: ${testInfo.title}`);
    console.log('═'.repeat(60));
    
    if (consoleErrors.length > 0) {
      console.log(`\n❌ ERRORS FOUND (${consoleErrors.length}):`);
      consoleErrors.forEach((error, index) => {
        console.log(`\n${index + 1}. ${error.type.toUpperCase()}: ${error.text}`);
        if (error.location) {
          console.log(`   📍 ${error.location.url}:${error.location.lineNumber}:${error.location.columnNumber}`);
        }
        if (error.stack) {
          console.log(`   🔍 Stack: ${error.stack.split('\n')[0]}`);
        }
      });
      
      // Check for React Error #31 specifically
      const reactError31s = consoleErrors.filter(error => 
        error.text.includes('Objects are not valid as a React child') ||
        error.text.includes('use an array instead') ||
        error.text.includes('object with keys')
      );
      
      if (reactError31s.length > 0) {
        console.log(`\n🎯 REACT ERROR #31 DETECTED (${reactError31s.length} instances):`);
        reactError31s.forEach((error, index) => {
          console.log(`\n${index + 1}. ${error.text}`);
          console.log(`   ⏰ ${error.timestamp}`);
          if (error.location) {
            console.log(`   📍 ${error.location.url}:${error.location.lineNumber}:${error.location.columnNumber}`);
          }
        });
        
        console.log('\n🔧 RECOMMENDED FIXES:');
        console.log('1. Check all children.push() calls in tree.js');
        console.log('2. Ensure boolean expressions use {condition && element} pattern');
        console.log('3. Verify no objects are accidentally passed as children');
        console.log('4. Use React.Fragment or arrays for multiple children');
      }
    } else {
      console.log('✅ No console errors detected');
    }
    
    if (consoleWarnings.length > 0) {
      console.log(`\n⚠️ WARNINGS (${consoleWarnings.length}):`);
      consoleWarnings.slice(0, 5).forEach((warning, index) => {
        console.log(`${index + 1}. ${warning.text}`);
      });
    }
    
    console.log('\n📊 SUMMARY:');
    console.log(`Total Messages: ${consoleMessages.length}`);
    console.log(`Errors: ${consoleErrors.length}`);
    console.log(`Warnings: ${consoleWarnings.length}`);
    console.log('═'.repeat(60));
  });
});