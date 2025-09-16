#!/usr/bin/env node

/**
 * Direct test of React fix without Playwright complexity
 * This runs the tests directly in a browser-like environment
 */

const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

async function runReactFixTest() {
  console.log('🧪 Running direct React fix validation...');
  
  let browser;
  let success = true;
  const results = [];
  
  try {
    browser = await chromium.launch();
    const page = await browser.newPage();
    
    // Track console messages
    const consoleMessages = [];
    page.on('console', (msg) => {
      consoleMessages.push({
        type: msg.type(),
        text: msg.text()
      });
    });
    
    // Create test HTML
    const testHtml = `
<!DOCTYPE html>
<html>
<head>
    <title>React Fix Validation</title>
    <style>
        :root { --text: #333; --muted: #666; }
        body { font-family: system-ui; margin: 20px; }
        .test-result { margin: 10px 0; padding: 10px; border-radius: 4px; }
        .success { background: #d4f6d4; color: #2d5a2d; }
        .error { background: #f6d4d4; color: #5a2d2d; }
    </style>
</head>
<body>
    <h1>React Fix Validation Test</h1>
    <div id="test-container"></div>
    <div id="results"></div>
    
    <script src="../../../dist/react-tree-bundle.js"></script>
    <script>
        const results = document.getElementById('results');
        
        function addResult(message, success) {
            const div = document.createElement('div');
            div.className = 'test-result ' + (success ? 'success' : 'error');
            div.textContent = (success ? '✅ ' : '❌ ') + message;
            results.appendChild(div);
            console.log((success ? '✅' : '❌'), message);
        }
        
        // Wait for the bundle to load and expose globals
        function waitForGlobals() {
            if (typeof React !== 'undefined' && typeof ReactDOM !== 'undefined' && typeof ReactTreeBundle !== 'undefined') {
                runTests();
            } else {
                setTimeout(waitForGlobals, 100);
            }
        }
        
        function runTests() {
        try {
            // Test 1: Empty state rendering (our main fix)
            const emptyData = {
                refactoringCandidatesByFile: [],
                directoryHealthTree: null
            };
            
            const container = document.getElementById('test-container');
            const root = ReactDOM.createRoot(container);
            root.render(React.createElement(ReactTreeBundle, { data: emptyData }));
            
            addResult('React component created without errors', true);
            
            // Wait a bit and check for rendered content
            setTimeout(() => {
                const title = container.querySelector('h3');
                const desc = container.querySelector('p');
                
                if (title && title.textContent.includes('No Refactoring Candidates Found')) {
                    addResult('Empty state title rendered correctly', true);
                } else {
                    addResult('Empty state title not found', false);
                }
                
                if (desc && desc.textContent.includes('Your code is in excellent shape!')) {
                    addResult('Empty state description rendered correctly', true);
                } else {
                    addResult('Empty state description not found', false);
                }
                
                // Test 2: Null data handling
                try {
                    root.render(React.createElement(ReactTreeBundle, { data: null }));
                    addResult('Null data handled without errors', true);
                } catch (error) {
                    addResult('Null data caused error: ' + error.message, false);
                }
                
                // Test 3: Undefined data handling
                try {
                    root.render(React.createElement(ReactTreeBundle, { data: undefined }));
                    addResult('Undefined data handled without errors', true);
                } catch (error) {
                    addResult('Undefined data caused error: ' + error.message, false);
                }
                
                console.log('All tests completed');
            }, 1000);
            
        } catch (error) {
            addResult('React rendering failed: ' + error.message, false);
            console.error('React error:', error);
        }
    </script>
</body>
</html>`;

    // Use data URL
    const dataUrl = `data:text/html;charset=utf-8,${encodeURIComponent(testHtml)}`;
    await page.goto(dataUrl);
    
    // Wait for tests to complete
    await page.waitForTimeout(3000);
    
    // Check results
    const testResults = await page.evaluate(() => {
      const resultElements = document.querySelectorAll('.test-result');
      return Array.from(resultElements).map(el => ({
        text: el.textContent,
        success: el.classList.contains('success')
      }));
    });
    
    console.log('\n📊 Test Results:');
    testResults.forEach(result => {
      console.log(result.text);
      if (!result.success) success = false;
    });
    
    // Check for React errors in console
    const reactErrors = consoleMessages.filter(msg => 
      msg.type === 'error' && (
        msg.text.includes('Objects are not valid as a React child') ||
        msg.text.includes('React error #31') ||
        msg.text.includes('Uncaught Error')
      )
    );
    
    if (reactErrors.length > 0) {
      console.log('\n❌ React errors detected:');
      reactErrors.forEach(error => console.log(`  ${error.text}`));
      success = false;
    } else {
      console.log('\n✅ No React errors detected');
    }
    
    // Show all console messages for debugging
    console.log('\n📝 All console messages:');
    consoleMessages.forEach(msg => {
      console.log(`  [${msg.type.toUpperCase()}] ${msg.text}`);
    });
    
  } catch (error) {
    console.error('❌ Test execution failed:', error);
    success = false;
  } finally {
    if (browser) {
      await browser.close();
    }
  }
  
  console.log('\n🏁 Final Result:', success ? '✅ ALL TESTS PASSED' : '❌ SOME TESTS FAILED');
  return success;
}

// Run the test
if (require.main === module) {
  runReactFixTest().then(success => {
    process.exit(success ? 0 : 1);
  }).catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
  });
}

module.exports = { runReactFixTest };