const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

async function testMinimalReact() {
    console.log('🧪 Testing Minimal React Case for Error #31');
    console.log('=' .repeat(50));
    
    const browser = await chromium.launch({ headless: false, devtools: true });
    const page = await browser.newPage();
    
    // Track all console messages
    const consoleMessages = [];
    const errors = [];
    const warnings = [];
    
    page.on('console', msg => {
        const text = msg.text();
        consoleMessages.push({
            type: msg.type(),
            text: text,
            timestamp: new Date().toISOString()
        });
        
        console.log(`[${msg.type().toUpperCase()}] ${text}`);
        
        if (msg.type() === 'error') {
            errors.push(text);
        } else if (msg.type() === 'warning') {
            warnings.push(text);
        }
    });
    
    // Track page errors
    page.on('pageerror', error => {
        console.log(`❌ PAGE ERROR: ${error.message}`);
        console.log(`Stack: ${error.stack}`);
        errors.push(`PAGE ERROR: ${error.message}`);
    });
    
    try {
        // Load the minimal React test file
        const htmlPath = path.resolve('/home/nathan/Projects/valknut/debug_minimal_react.html');
        const fileUrl = `file://${htmlPath}`;
        
        console.log(`📂 Loading: ${fileUrl}`);
        await page.goto(fileUrl, { waitUntil: 'networkidle' });
        
        // Wait for React to load and render
        console.log('⏳ Waiting for component to mount...');
        await page.waitForTimeout(3000);
        
        // Check if component rendered successfully
        const rootElement = await page.$('#root');
        const rootContent = await rootElement?.innerHTML();
        
        console.log('\n📊 Analysis Results:');
        console.log('=' .repeat(30));
        
        // Check for React Error #31 specifically
        const reactError31 = errors.find(error => 
            error.includes('31') || 
            error.toLowerCase().includes('invalid') ||
            error.toLowerCase().includes('element')
        );
        
        if (reactError31) {
            console.log('🔴 React Error #31 FOUND:');
            console.log(`   ${reactError31}`);
        } else {
            console.log('✅ No React Error #31 detected');
        }
        
        // Check rendering success
        if (rootContent && rootContent.trim().length > 0) {
            console.log('✅ Component rendered successfully');
            console.log(`   Root content length: ${rootContent.length} chars`);
        } else {
            console.log('❌ Component did not render or root is empty');
        }
        
        // Summary of all errors
        console.log(`\n📋 Error Summary:`);
        console.log(`   Total errors: ${errors.length}`);
        console.log(`   Total warnings: ${warnings.length}`);
        
        if (errors.length > 0) {
            console.log('\n🔴 All Errors:');
            errors.forEach((error, index) => {
                console.log(`   ${index + 1}. ${error}`);
            });
        }
        
        if (warnings.length > 0) {
            console.log('\n🟡 All Warnings:');
            warnings.forEach((warning, index) => {
                console.log(`   ${index + 1}. ${warning}`);
            });
        }
        
        // Check for specific React development warnings
        const reactWarnings = warnings.filter(w => 
            w.includes('React') || w.includes('Warning:')
        );
        
        if (reactWarnings.length > 0) {
            console.log('\n⚠️ React-specific Warnings:');
            reactWarnings.forEach((warning, index) => {
                console.log(`   ${index + 1}. ${warning}`);
            });
        }
        
        // Take a screenshot for visual verification
        await page.screenshot({ 
            path: '/home/nathan/Projects/valknut/minimal_react_test_result.png',
            fullPage: true 
        });
        console.log('📸 Screenshot saved: minimal_react_test_result.png');
        
        // Final assessment
        console.log('\n🎯 Assessment:');
        if (errors.length === 0 && rootContent && rootContent.trim().length > 0) {
            console.log('✅ PASS: Minimal React case works without errors');
            console.log('   This suggests the issue is likely in react-arborist integration');
        } else {
            console.log('❌ FAIL: Minimal React case has issues');
            console.log('   This suggests the problem is in our React element creation logic');
        }
        
    } catch (error) {
        console.log(`❌ Test failed: ${error.message}`);
        console.log(`Stack: ${error.stack}`);
    } finally {
        await browser.close();
    }
}

// Run the test
testMinimalReact().catch(console.error);