#!/usr/bin/env node

/**
 * Debug React Loading - Simple test to understand why React isn't loading
 */

const { chromium } = require('playwright');
const path = require('path');

async function debugReactLoading() {
    console.log('🔍 Starting React loading debug session...');
    
    const browser = await chromium.launch({ 
        headless: false,  // Show browser for debugging
        slowMo: 1000      // Slow down actions
    });
    
    const page = await browser.newPage();
    
    // Set up detailed console logging
    page.on('console', msg => {
        const type = msg.type();
        const text = msg.text();
        console.log(`[${type.toUpperCase()}] ${text}`);
    });
    
    // Set up error handling
    page.on('pageerror', error => {
        console.log(`[PAGE ERROR] ${error.message}`);
        console.log(error.stack);
    });
    
    try {
        // Navigate to the test HTML
        const testHtmlPath = path.resolve('/tmp/react-bundle-test/test.html');
        console.log(`📂 Opening: file://${testHtmlPath}`);
        
        await page.goto(`file://${testHtmlPath}`);
        
        // Wait for page load
        await page.waitForLoadState('networkidle');
        console.log('✅ Page loaded');
        
        // Check what scripts are present
        const scripts = await page.$$eval('script', scripts => 
            scripts.map(s => ({ 
                src: s.src, 
                type: s.type, 
                hasContent: !!s.textContent,
                contentLength: s.textContent ? s.textContent.length : 0
            }))
        );
        
        console.log('📜 Scripts found:', JSON.stringify(scripts, null, 2));
        
        // Check if React bundle file exists and is accessible
        const bundleExists = await page.evaluate(async () => {
            try {
                const response = await fetch('./react-tree-bundle.debug.js');
                return {
                    status: response.status,
                    statusText: response.statusText,
                    ok: response.ok
                };
            } catch (error) {
                return { error: error.message };
            }
        });
        
        console.log('📦 Bundle fetch test:', bundleExists);
        
        // Check React availability
        const reactCheck = await page.evaluate(() => {
            return {
                React: typeof window.React,
                ReactDOM: typeof window.ReactDOM,
                windowKeys: Object.keys(window).filter(k => k.toLowerCase().includes('react'))
            };
        });
        
        console.log('⚛️  React check:', reactCheck);
        
        // Check if tree root exists
        const treeRoot = await page.$('#react-tree-root');
        console.log('🌳 Tree root exists:', !!treeRoot);
        
        if (treeRoot) {
            const rootContent = await page.textContent('#react-tree-root');
            console.log('🌳 Tree root content:', rootContent);
        }
        
        // Wait a bit to see what happens
        console.log('⏱️  Waiting 5 seconds for React to load...');
        await page.waitForTimeout(5000);
        
        // Final React check
        const finalReactCheck = await page.evaluate(() => {
            return {
                React: typeof window.React,
                ReactDOM: typeof window.ReactDOM,
                hasTreeNodes: document.querySelectorAll('.tree-node').length,
                treeRootContent: document.getElementById('react-tree-root')?.innerHTML || 'not found'
            };
        });
        
        console.log('🔍 Final state:', finalReactCheck);
        
        // Take screenshot
        await page.screenshot({ path: '/tmp/react-debug-screenshot.png', fullPage: true });
        console.log('📸 Screenshot saved to /tmp/react-debug-screenshot.png');
        
    } catch (error) {
        console.error('❌ Debug failed:', error);
    } finally {
        await browser.close();
    }
}

if (require.main === module) {
    debugReactLoading().catch(console.error);
}

module.exports = debugReactLoading;