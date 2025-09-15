#!/usr/bin/env node

const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

async function testReactFixes() {
    console.log('🧪 Starting React Error #31 Fix Validation Tests');
    console.log('=' .repeat(60));
    
    const browser = await chromium.launch({ 
        headless: false,
        args: ['--disable-dev-shm-usage', '--disable-gpu']
    });
    const context = await browser.newContext();
    
    // Set up console logging and error tracking
    const consoleMessages = [];
    const errors = [];
    
    const page = await context.newPage();
    
    page.on('console', (msg) => {
        const text = msg.text();
        consoleMessages.push({
            type: msg.type(),
            text: text,
            timestamp: new Date().toISOString()
        });
        console.log(`[CONSOLE ${msg.type().toUpperCase()}]: ${text}`);
    });
    
    page.on('pageerror', (error) => {
        errors.push({
            message: error.message,
            stack: error.stack,
            timestamp: new Date().toISOString()
        });
        console.error(`[PAGE ERROR]: ${error.message}`);
    });

    const testResults = {
        fixedReport: {
            path: '',
            loaded: false,
            reactErrors: [],
            treeComponentLoaded: false,
            dataDisplayed: false,
            noAnalysisMessage: false,
            refactoringCandidates: 0
        },
        brokenReport: {
            path: '',
            loaded: false,
            reactErrors: [],
            treeComponentLoaded: false,
            dataDisplayed: false,
            noAnalysisMessage: false,
            refactoringCandidates: 0
        }
    };

    try {
        console.log('\n📊 Testing FINAL WORKING Report');
        console.log('-'.repeat(40));
        
        const fixedReportPath = path.resolve(__dirname, 'final-demo-WORKING/report_20250914_211112.html');
        testResults.fixedReport.path = fixedReportPath;
        
        console.log(`Loading: ${fixedReportPath}`);
        
        if (!fs.existsSync(fixedReportPath)) {
            console.error(`❌ Fixed report not found at: ${fixedReportPath}`);
            return;
        }
        
        // Clear previous messages
        consoleMessages.length = 0;
        errors.length = 0;
        
        await page.goto(`file://${fixedReportPath}`, { waitUntil: 'networkidle' });
        testResults.fixedReport.loaded = true;
        
        console.log('✅ Fixed report loaded successfully');
        
        // Wait for React to render
        await page.waitForTimeout(3000);
        
        // Check for React error #31 specifically
        const reactError31Found = consoleMessages.some(msg => 
            msg.text.includes('Warning: Each child in a list should have a unique "key" prop') ||
            msg.text.includes('Warning: Failed to validate a prop type') ||
            msg.text.includes('Cannot read properties of null') ||
            msg.text.includes('Cannot read properties of undefined')
        );
        
        testResults.fixedReport.reactErrors = consoleMessages.filter(msg => 
            msg.type === 'error' || msg.text.includes('Warning:')
        );
        
        // Check if React tree component loaded
        const treeContainer = await page.locator('#react-tree-container').count();
        testResults.fixedReport.treeComponentLoaded = treeContainer > 0;
        
        // Check for "no analysis data available" message
        const noDataMessage = await page.locator('text=no analysis data available').count();
        testResults.fixedReport.noAnalysisMessage = noDataMessage > 0;
        
        // Check for refactoring candidates
        const candidates = await page.locator('[data-testid*="candidate"], .refactoring-candidate, .tree-node').count();
        testResults.fixedReport.refactoringCandidates = candidates;
        
        // Check if tree actually rendered with nodes
        await page.waitForTimeout(2000);
        const treeNodes = await page.locator('.tree-node, [class*="node"], [data-node]').count();
        
        console.log(`📋 Fixed Report Results:`);
        console.log(`   React Tree Container: ${testResults.fixedReport.treeComponentLoaded ? '✅' : '❌'}`);
        console.log(`   Tree Nodes Rendered: ${treeNodes} nodes`);
        console.log(`   No Analysis Message: ${testResults.fixedReport.noAnalysisMessage ? '❌' : '✅'}`);
        console.log(`   React Errors Found: ${testResults.fixedReport.reactErrors.length}`);
        console.log(`   Refactoring Candidates: ${testResults.fixedReport.refactoringCandidates}`);
        
        if (reactError31Found) {
            console.log('❌ React Error #31 still present!');
        } else {
            console.log('✅ React Error #31 resolved!');
        }

        // Test broken report for comparison
        console.log('\n💔 Testing BROKEN Report (for comparison)');
        console.log('-'.repeat(40));
        
        const brokenReportPath = path.resolve(__dirname, 'debug-final-test/report_20250914_131615.html');
        testResults.brokenReport.path = brokenReportPath;
        
        if (fs.existsSync(brokenReportPath)) {
            // Clear messages for broken report test
            consoleMessages.length = 0;
            errors.length = 0;
            
            await page.goto(`file://${brokenReportPath}`, { waitUntil: 'networkidle' });
            testResults.brokenReport.loaded = true;
            
            await page.waitForTimeout(3000);
            
            const brokenReactError31 = consoleMessages.some(msg => 
                msg.text.includes('Warning: Each child in a list should have a unique "key" prop') ||
                msg.text.includes('Cannot read properties of null') ||
                msg.text.includes('Cannot read properties of undefined')
            );
            
            testResults.brokenReport.reactErrors = consoleMessages.filter(msg => 
                msg.type === 'error' || msg.text.includes('Warning:')
            );
            
            const brokenTreeContainer = await page.locator('#react-tree-container').count();
            testResults.brokenReport.treeComponentLoaded = brokenTreeContainer > 0;
            
            const brokenNoDataMessage = await page.locator('text=no analysis data available').count();
            testResults.brokenReport.noAnalysisMessage = brokenNoDataMessage > 0;
            
            const brokenCandidates = await page.locator('[data-testid*="candidate"], .refactoring-candidate, .tree-node').count();
            testResults.brokenReport.refactoringCandidates = brokenCandidates;
            
            console.log(`📋 Broken Report Results:`);
            console.log(`   React Tree Container: ${testResults.brokenReport.treeComponentLoaded ? '✅' : '❌'}`);
            console.log(`   No Analysis Message: ${testResults.brokenReport.noAnalysisMessage ? '❌' : '✅'}`);
            console.log(`   React Errors Found: ${testResults.brokenReport.reactErrors.length}`);
            console.log(`   Refactoring Candidates: ${testResults.brokenReport.refactoringCandidates}`);
            
            if (brokenReactError31) {
                console.log('❌ React Error #31 confirmed in broken report');
            }
        } else {
            console.log(`⚠️  Broken report not found at: ${brokenReportPath}`);
        }

        // Generate comprehensive comparison
        console.log('\n📊 COMPARISON RESULTS');
        console.log('=' .repeat(60));
        
        const fixedBetter = testResults.fixedReport.reactErrors.length < testResults.brokenReport.reactErrors.length;
        const treeWorking = testResults.fixedReport.treeComponentLoaded && !testResults.fixedReport.noAnalysisMessage;
        
        console.log(`React Error #31 Fix: ${!reactError31Found ? '✅ SUCCESS' : '❌ FAILED'}`);
        console.log(`Tree Component Working: ${treeWorking ? '✅ SUCCESS' : '❌ FAILED'}`);
        console.log(`Error Reduction: ${testResults.brokenReport.reactErrors.length} → ${testResults.fixedReport.reactErrors.length} errors`);
        console.log(`Data Display: ${testResults.fixedReport.refactoringCandidates} refactoring candidates shown`);
        
        if (fixedBetter && treeWorking && !reactError31Found) {
            console.log('\n🎉 ALL FIXES SUCCESSFUL! React Error #31 has been resolved.');
        } else {
            console.log('\n⚠️  Some issues may still exist. Further investigation needed.');
        }
        
        // Save detailed test results
        const resultsJson = {
            timestamp: new Date().toISOString(),
            testResults,
            consoleMessages: consoleMessages.slice(-50), // Last 50 messages
            errors: errors,
            summary: {
                reactError31Fixed: !reactError31Found,
                treeComponentWorking: treeWorking,
                errorReduction: testResults.brokenReport.reactErrors.length - testResults.fixedReport.reactErrors.length,
                overallSuccess: fixedBetter && treeWorking && !reactError31Found
            }
        };
        
        fs.writeFileSync(
            path.resolve(__dirname, 'react_fix_test_results.json'),
            JSON.stringify(resultsJson, null, 2)
        );
        
        console.log('\n📝 Test results saved to: react_fix_test_results.json');

    } catch (error) {
        console.error('Test execution failed:', error);
    } finally {
        await browser.close();
    }
}

// Run the tests
testReactFixes().catch(console.error);