/**
 * Focused test for React Error #31 Fix - Core functionality only
 * 
 * Tests the most important aspects:
 * 1. No React errors in console
 * 2. Data props are passed correctly 
 * 3. Component mounts without errors
 */

const { test, expect } = require('@playwright/test');

const REPORT_PATH = '/home/nathan/Projects/valknut/final-react-fix/report_20250914_155810.html';

test.describe('React Error #31 Fix - Core Verification', () => {
    test('should fix React error #31 - no "Objects are not valid as React children" errors', async ({ page }) => {
        // Capture all console messages
        const consoleMessages = [];
        page.on('console', (msg) => {
            consoleMessages.push({
                type: msg.type(),
                text: msg.text()
            });
        });

        // Navigate to the report
        await page.goto(`file://${REPORT_PATH}`);
        
        // Wait for page to fully load and React to initialize
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(3000);

        // Check that there are NO React "Objects are not valid as React children" errors
        const reactErrors = consoleMessages.filter(msg => 
            msg.type === 'error' && (
                msg.text.includes('Objects are not valid as React child') ||
                msg.text.includes('React children') ||
                msg.text.includes('Error: Objects are not valid') ||
                msg.text.includes('Uncaught Error')
            )
        );

        console.log('📋 All console messages:');
        consoleMessages.forEach(msg => {
            console.log(`  ${msg.type}: ${msg.text}`);
        });

        console.log('\n🔍 React-specific errors found:');
        console.log(reactErrors);

        // The critical test - should have zero React errors
        expect(reactErrors).toHaveLength(0);
    });

    test('should successfully parse and pass JSON data to React component', async ({ page }) => {
        // Add script to capture data parsing
        await page.addInitScript(() => {
            window.testResults = {
                dataParsingSuccess: false,
                reactMountSuccess: false,
                propsReceived: false
            };

            // Override console.log to capture specific messages
            const originalLog = console.log;
            console.log = function(...args) {
                const message = args.join(' ');
                if (message.includes('📊 Parsed analysis data:')) {
                    window.testResults.dataParsingSuccess = true;
                }
                return originalLog.apply(console, args);
            };

            // Override React.createElement to verify props
            const originalCreateElement = React?.createElement;
            if (originalCreateElement) {
                React.createElement = function(component, props, ...children) {
                    if (component === window.CodeAnalysisTree && props && props.data) {
                        window.testResults.reactMountSuccess = true;
                        window.testResults.propsReceived = true;
                        window.testResults.propsData = props.data;
                    }
                    return originalCreateElement.apply(this, arguments);
                };
            }
        });

        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(3000);

        // Check test results
        const results = await page.evaluate(() => window.testResults);
        
        console.log('🧪 Test Results:', results);

        // Verify data parsing succeeded
        expect(results.dataParsingSuccess).toBe(true);
        
        // Verify React component was mounted with props
        expect(results.reactMountSuccess).toBe(true);
        expect(results.propsReceived).toBe(true);
    });

    test('should have valid JSON data structure embedded in HTML', async ({ page }) => {
        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForLoadState('networkidle');

        // Extract and validate the embedded JSON data
        const jsonData = await page.evaluate(() => {
            const script = document.getElementById('tree-data');
            if (!script) return null;
            
            try {
                return JSON.parse(script.textContent);
            } catch (e) {
                return { error: e.message };
            }
        });

        // Verify data structure
        expect(jsonData).toBeTruthy();
        expect(jsonData.error).toBeUndefined();
        expect(jsonData).toHaveProperty('refactoringCandidatesByFile');
        expect(Array.isArray(jsonData.refactoringCandidatesByFile)).toBe(true);
        
        console.log('📊 JSON Data Summary:');
        console.log(`  Files: ${jsonData.refactoringCandidatesByFile.length}`);
        
        if (jsonData.refactoringCandidatesByFile.length > 0) {
            const sampleFile = jsonData.refactoringCandidatesByFile[0];
            console.log(`  Sample file: ${sampleFile.fileName}`);
            console.log(`  Sample candidates: ${sampleFile.candidates?.length || 0}`);
        }
    });

    test('should mount React component without throwing exceptions', async ({ page }) => {
        // Capture JavaScript errors
        const jsErrors = [];
        page.on('pageerror', (error) => {
            jsErrors.push(error.message);
        });

        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(3000);

        // Check for JavaScript errors during React mounting
        const reactMountingErrors = jsErrors.filter(error =>
            error.includes('React') || 
            error.includes('createElement') ||
            error.includes('render') ||
            error.includes('mount')
        );

        console.log('🚨 JavaScript Errors:', jsErrors);
        console.log('🔍 React Mounting Errors:', reactMountingErrors);

        // Should have no React mounting errors
        expect(reactMountingErrors).toHaveLength(0);

        // React root should exist
        const reactRoot = page.locator('#react-tree-root');
        await expect(reactRoot).toBeVisible();
    });

    test('should have fallback error handling in place', async ({ page }) => {
        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForLoadState('networkidle');

        // Check that error handling logic exists in the HTML
        const hasErrorHandling = await page.evaluate(() => {
            const html = document.documentElement.innerHTML;
            return html.includes('catch (error)') && 
                   html.includes('Failed to mount React tree') &&
                   html.includes('render(React.createElement(window.CodeAnalysisTree))'); // fallback
        });

        expect(hasErrorHandling).toBe(true);
        console.log('✅ Error handling and fallback rendering logic found');
    });
});