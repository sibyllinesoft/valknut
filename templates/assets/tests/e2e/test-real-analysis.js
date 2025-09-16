#!/usr/bin/env node

/**
 * Test Real Valknut Analysis Results
 * This validates our React fixes against an actual valknut analysis report
 */

const { chromium } = require('playwright');
const path = require('path');

async function testRealAnalysis() {
  console.log('🔍 Testing real valknut analysis report...');
  
  let browser;
  let success = true;
  
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
    
    // Load the real analysis report
    const realAnalysisFile = path.resolve('./test-results/real-valknut-analysis.html');
    console.log(`📂 Loading: ${realAnalysisFile}`);
    await page.goto(`file://${realAnalysisFile}`);
    
    // Wait for React to render
    console.log('⏳ Waiting for React to process real analysis data...');
    await page.waitForTimeout(5000);
    
    // Test 1: Check for React component structure
    const reactRoot = await page.locator('#react-tree-root').count();
    const valknutContainer = await page.locator('.valknut-tree-container').count();
    
    console.log(`📊 React elements: root=${reactRoot}, container=${valknutContainer}`);
    
    if (reactRoot > 0 && valknutContainer > 0) {
      console.log('✅ React tree component loaded successfully');
    } else {
      console.log('❌ React tree component failed to load');
      success = false;
    }
    
    // Test 2: Check for actual rendered content 
    const reactRootElement = await page.locator('#react-tree-root').first();
    if (await reactRootElement.isVisible()) {
      const elementCount = await page.locator('#react-tree-root *').count();
      console.log(`🌳 Elements inside React root: ${elementCount}`);
      
      if (elementCount > 3) {
        console.log('✅ React tree has substantial content');
      } else {
        console.log('❌ React tree appears empty or minimal');
        success = false;
      }
    }
    
    // Test 3: Check for React errors specifically
    const reactErrors = consoleMessages.filter(msg => 
      msg.type === 'error' && (
        msg.text.includes('Objects are not valid as a React child') ||
        msg.text.includes('React error #31') ||
        msg.text.includes('Cannot read properties of undefined') ||
        msg.text.includes('TypeError')
      )
    );
    
    if (reactErrors.length === 0) {
      console.log('✅ No React errors detected with real analysis data');
    } else {
      console.log('❌ React errors found with real analysis data:');
      reactErrors.forEach(error => console.log(`  ${error.text}`));
      success = false;
    }
    
    // Test 4: Look for any tree content specifically
    const treeItems = await page.locator('.tree-header-row, [data-arborist-instance] *, .valknut-tree-container *').count();
    console.log(`🌲 Tree content elements found: ${treeItems}`);
    
    if (treeItems > 0) {
      console.log('✅ Tree content is rendering');
    } else {
      console.log('❌ No tree content found');
      success = false;
    }
    
    // Test 5: Check for the specific health score or complexity data
    const bodyText = await page.textContent('body');
    const hasAnalysisData = bodyText.includes('health') || bodyText.includes('complexity') || bodyText.includes('refactoring');
    
    if (hasAnalysisData) {
      console.log('✅ Analysis data is present in the rendered page');
    } else {
      console.log('❌ No analysis data found in rendered content');
      success = false;
    }
    
    // Show React-specific console messages
    const reactLogs = consoleMessages.filter(msg => 
      msg.text.includes('React') || 
      msg.text.includes('✅') || 
      msg.text.includes('❌') ||
      msg.text.includes('tree') ||
      msg.text.includes('Processing')
    );
    
    if (reactLogs.length > 0) {
      console.log('\n📋 React processing messages:');
      reactLogs.slice(0, 10).forEach(msg => {  // Show first 10 to avoid spam
        console.log(`  [${msg.type}] ${msg.text}`);
      });
      if (reactLogs.length > 10) {
        console.log(`  ... and ${reactLogs.length - 10} more messages`);
      }
    }
    
    // Show general stats
    const errorCount = consoleMessages.filter(msg => msg.type === 'error').length;
    const warningCount = consoleMessages.filter(msg => msg.type === 'warning').length;
    console.log(`\n📊 Console summary: ${errorCount} errors, ${warningCount} warnings, ${consoleMessages.length} total messages`);
    
  } catch (error) {
    console.error('❌ Test execution failed:', error);
    success = false;
  } finally {
    if (browser) {
      await browser.close();
    }
  }
  
  console.log('\n🏁 Real Analysis Test Result:', success ? '✅ SUCCESS' : '❌ FAILURE');
  return success;
}

// Run the test
if (require.main === module) {
  testRealAnalysis().then(success => {
    process.exit(success ? 0 : 1);
  }).catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
  });
}

module.exports = { testRealAnalysis };