#!/usr/bin/env node

/**
 * Simple E2E test using Playwright without the test framework
 * This avoids the test.describe() context issues
 */

const { chromium } = require('playwright');
const path = require('path');

async function runE2ETests() {
  console.log('🚀 Running simple E2E tests...');
  
  let browser;
  let allTestsPassed = true;
  
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
    
    // Test 1: Empty state HTML file
    console.log('\n📝 Test 1: Empty state rendering');
    const emptyStateFile = path.resolve('./test-results/empty-candidates.html');
    await page.goto(`file://${emptyStateFile}`);
    await page.waitForTimeout(3000);
    
    // Debug: check what's actually on the page
    const bodyText = await page.textContent('body');
    console.log('📄 Page content snippet:', bodyText.substring(0, 200) + '...');
    
    // Check for empty state message using element selectors
    const h3Elements = await page.locator('h3').all();
    const pElements = await page.locator('p').all();
    
    console.log(`📊 Found ${h3Elements.length} h3 elements, ${pElements.length} p elements`);
    
    // Check h3 elements for empty state title
    let titleFound = false;
    for (const h3 of h3Elements) {
      const text = await h3.textContent();
      if (text && text.includes('No Refactoring Candidates Found')) {
        console.log('✅ Empty state title found');
        titleFound = true;
        break;
      }
    }
    if (!titleFound) {
      console.log('❌ Empty state title not found');
      allTestsPassed = false;
    }
    
    // Check p elements for empty state description
    let descFound = false;
    for (const p of pElements) {
      const text = await p.textContent();
      if (text && text.includes('Your code is in excellent shape!')) {
        console.log('✅ Empty state description found');
        descFound = true;
        break;
      }
    }
    if (!descFound) {
      console.log('❌ Empty state description not found');
      allTestsPassed = false;
    }
    
    // Test 2: Normal results HTML file
    console.log('\n📝 Test 2: Normal results rendering');
    const normalResultsFile = path.resolve('./test-results/normal-results.html');
    await page.goto(`file://${normalResultsFile}`);
    
    // Wait longer for React to render
    console.log('⏳ Waiting for React component to render...');
    await page.waitForTimeout(5000);
    
    // Check for tree structure using correct selectors
    const reactTreeRoot = await page.locator('#react-tree-root').count();
    const valknutTreeContainer = await page.locator('.valknut-tree-container').count();
    const treeHeaderRows = await page.locator('.tree-header-row').count(); // Custom tree rows
    const arboristElements = await page.locator('[data-arborist-instance]').count(); // React Arborist
    const totalTreeElements = reactTreeRoot + valknutTreeContainer + treeHeaderRows + arboristElements;
    
    console.log(`🌳 Tree element counts: root=${reactTreeRoot}, valknut=${valknutTreeContainer}, rows=${treeHeaderRows}, arborist=${arboristElements}`);
    
    if (totalTreeElements > 0) {
      console.log('✅ Tree structure found in normal results');
    } else {
      console.log('❌ Tree structure not found in normal results');
      
      // Check if react-tree-root exists at all
      const reactRoot = await page.locator('#react-tree-root').first();
      if (await reactRoot.isVisible()) {
        const rootContent = await reactRoot.innerHTML();
        console.log(`📄 React root content (first 500 chars): ${rootContent.substring(0, 500)}...`);
        
        // Check for any React-rendered elements
        const reactElements = await page.locator('#react-tree-root *').count();
        console.log(`🔍 Elements inside react root: ${reactElements}`);
        
        // Check for the valknut tree container specifically
        const valknutContainer = await page.locator('#react-tree-root .valknut-tree-container').count();
        console.log(`🌲 Valknut tree containers in root: ${valknutContainer}`);
      } else {
        console.log('📄 React tree root not found or not visible');
      }
      allTestsPassed = false;
    }
    
    // Test 3: Check for React errors
    console.log('\n📝 Test 3: Checking for React errors');
    const reactErrors = consoleMessages.filter(msg => 
      msg.type === 'error' && (
        msg.text.includes('Objects are not valid as a React child') ||
        msg.text.includes('React error #31') ||
        msg.text.includes('Uncaught Error')
      )
    );
    
    if (reactErrors.length === 0) {
      console.log('✅ No React errors detected');
    } else {
      console.log('❌ React errors detected:');
      reactErrors.forEach(error => console.log(`  ${error.text}`));
      allTestsPassed = false;
    }
    
    // Test 4: Check for Lucide icons
    console.log('\n📝 Test 4: Checking for Lucide icons');
    const lucideIcons = await page.locator('[data-lucide]').count();
    if (lucideIcons > 0) {
      console.log(`✅ Found ${lucideIcons} Lucide icons`);
    } else {
      console.log('⚠️ No Lucide icons found (may be normal)');
    }
    
    // Test 5: Responsive check
    console.log('\n📝 Test 5: Responsive design check');
    await page.setViewportSize({ width: 768, height: 1024 });
    await page.waitForTimeout(1000);
    
    const responsiveTree = await page.locator('[role="tree"], .tree-container, #tree-container').isVisible();
    if (responsiveTree) {
      console.log('✅ Tree remains visible on tablet viewport');
    } else {
      console.log('❌ Tree not visible on tablet viewport');
      allTestsPassed = false;
    }
    
    // Show all console messages for debugging
    console.log('\n📝 Console messages summary:');
    const errorCount = consoleMessages.filter(msg => msg.type === 'error').length;
    const warningCount = consoleMessages.filter(msg => msg.type === 'warning').length;
    const logCount = consoleMessages.filter(msg => msg.type === 'log').length;
    console.log(`  Errors: ${errorCount}, Warnings: ${warningCount}, Logs: ${logCount}, Total: ${consoleMessages.length}`);
    
    if (errorCount > 0) {
      console.log('\n❌ Console errors:');
      consoleMessages.filter(msg => msg.type === 'error').forEach(msg => {
        console.log(`  ${msg.text}`);
      });
    }
    
    // Show React-related log messages
    const reactLogs = consoleMessages.filter(msg => 
      msg.text.includes('React') || 
      msg.text.includes('render') || 
      msg.text.includes('tree') ||
      msg.text.includes('✅') ||
      msg.text.includes('❌')
    );
    if (reactLogs.length > 0) {
      console.log('\n📋 React-related console messages:');
      reactLogs.forEach(msg => {
        console.log(`  [${msg.type.toUpperCase()}] ${msg.text}`);
      });
    }
    
  } catch (error) {
    console.error('❌ Test execution failed:', error);
    allTestsPassed = false;
  } finally {
    if (browser) {
      await browser.close();
    }
  }
  
  console.log('\n🏁 Final Result:', allTestsPassed ? '✅ ALL TESTS PASSED' : '❌ SOME TESTS FAILED');
  return allTestsPassed;
}

// Run the tests
if (require.main === module) {
  runE2ETests().then(success => {
    process.exit(success ? 0 : 1);
  }).catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
  });
}

module.exports = { runE2ETests };