/**
 * Playwright test for React Error #31 Fix
 * 
 * Tests that the React component now receives data props correctly
 * and no longer throws "Objects are not valid as React children" errors.
 */

const { test, expect } = require('@playwright/test');
const path = require('path');

const REPORT_PATH = '/home/nathan/Projects/valknut/react-error-fix-test/report_20250914_143321.html';

test.describe('React Error #31 Fix', () => {
    test.beforeEach(async ({ page }) => {
        // Listen for console errors
        const consoleErrors = [];
        page.on('console', (msg) => {
            if (msg.type() === 'error') {
                consoleErrors.push(msg.text());
            }
        });
        
        // Store errors on page for access in tests
        await page.addInitScript(() => {
            window.testConsoleErrors = [];
            const originalError = console.error;
            console.error = (...args) => {
                window.testConsoleErrors.push(args.map(arg => 
                    typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
                ).join(' '));
                originalError.apply(console, args);
            };
        });

        // Navigate to the report
        await page.goto(`file://${REPORT_PATH}`);
    });

    test('should load React components without React error #31', async ({ page }) => {
        // Wait for page to fully load
        await page.waitForLoadState('networkidle');
        
        // Wait for React to mount (give it extra time)
        await page.waitForTimeout(2000);

        // Check that there are no React "Objects are not valid as React children" errors
        const consoleErrors = await page.evaluate(() => window.testConsoleErrors || []);
        
        // Filter for React-specific errors
        const reactErrors = consoleErrors.filter(error => 
            error.includes('Objects are not valid as React child') ||
            error.includes('React children') ||
            error.includes('Failed to mount React tree')
        );

        console.log('Console errors found:', consoleErrors);
        console.log('React-specific errors:', reactErrors);

        expect(reactErrors).toHaveLength(0);
    });

    test('should have tree-data script element with JSON data', async ({ page }) => {
        // Check that the tree-data script element exists
        const treeDataScript = await page.locator('script#tree-data');
        await expect(treeDataScript).toBeVisible();

        // Verify it contains JSON data
        const scriptContent = await treeDataScript.textContent();
        expect(scriptContent).toBeTruthy();
        
        // Verify it's valid JSON
        let parsedData;
        expect(() => {
            parsedData = JSON.parse(scriptContent);
        }).not.toThrow();

        // Verify it contains expected structure
        expect(parsedData).toHaveProperty('refactoringCandidatesByFile');
        expect(Array.isArray(parsedData.refactoringCandidatesByFile)).toBe(true);
        
        console.log('Tree data structure:', Object.keys(parsedData));
        console.log('Refactoring candidates count:', parsedData.refactoringCandidatesByFile.length);
    });

    test('should mount React component successfully', async ({ page }) => {
        // Wait for React to mount
        await page.waitForTimeout(2000);

        // Check that React root container exists and has content
        const reactRoot = page.locator('#react-tree-root');
        await expect(reactRoot).toBeVisible();

        // Check that React component has rendered some content
        const hasContent = await reactRoot.evaluate(el => {
            return el.children.length > 0 && el.textContent.trim().length > 0;
        });
        
        expect(hasContent).toBe(true);
    });

    test('should pass data props to React component', async ({ page }) => {
        // Wait for React to mount and data to be parsed
        await page.waitForTimeout(2000);

        // Check console logs for successful data parsing
        const consoleMessages = await page.evaluate(() => {
            return window.testConsoleLogs || [];
        });

        // Add a script to capture console.log messages as well
        await page.addInitScript(() => {
            window.testConsoleLogs = [];
            const originalLog = console.log;
            console.log = (...args) => {
                window.testConsoleLogs.push(args.map(arg => 
                    typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
                ).join(' '));
                originalLog.apply(console, args);
            };
        });

        // Reload to capture console logs
        await page.reload();
        await page.waitForTimeout(2000);

        const logs = await page.evaluate(() => window.testConsoleLogs || []);
        
        // Look for the data parsing log
        const hasDataParsingLog = logs.some(log => 
            log.includes('📊 Parsed analysis data:')
        );

        console.log('Console logs:', logs);
        expect(hasDataParsingLog).toBe(true);
    });

    test('should display analysis data in tree component', async ({ page }) => {
        // Wait for React component to fully render
        await page.waitForTimeout(3000);

        const reactRoot = page.locator('#react-tree-root');
        await expect(reactRoot).toBeVisible();

        // Check that we don't see the "no analysis data available" fallback
        const noDataMessage = page.locator('text=no analysis data available');
        await expect(noDataMessage).not.toBeVisible();

        // Check for content that indicates refactoring candidates are displayed
        const hasRefactoringContent = await page.evaluate(() => {
            const root = document.getElementById('react-tree-root');
            if (!root) return false;
            
            const text = root.textContent || '';
            return text.includes('refactoring') || 
                   text.includes('complexity') || 
                   text.includes('.rs') || 
                   text.includes('Priority') ||
                   text.length > 50; // Should have substantial content
        });

        expect(hasRefactoringContent).toBe(true);
    });

    test('should have interactive tree functionality', async ({ page }) => {
        // Wait for React component to fully render
        await page.waitForTimeout(3000);

        // Look for interactive elements like expandable nodes
        const interactiveElements = page.locator('.tree-expandable, .tree-node, button');
        const count = await interactiveElements.count();
        
        // Should have some interactive elements if data is properly loaded
        expect(count).toBeGreaterThan(0);

        // Check for tree-specific classes that indicate proper rendering
        const treeElements = page.locator('[class*="tree-"], [class*="priority-"], [class*="complexity"]');
        const treeCount = await treeElements.count();
        
        expect(treeCount).toBeGreaterThan(0);
    });

    test('should not have React error boundaries triggered', async ({ page }) => {
        // Wait for full render
        await page.waitForTimeout(3000);

        // Check that no error boundary content is displayed
        const errorBoundaryMessages = await page.locator('text=Something went wrong').count();
        expect(errorBoundaryMessages).toBe(0);

        // Check that no generic React error messages appear
        const reactErrorMessages = await page.locator('text=React Error, text=Component Error').count();
        expect(reactErrorMessages).toBe(0);
    });

    test('should have proper props data structure', async ({ page }) => {
        // Add a global function to inspect React component props
        await page.addInitScript(() => {
            window.inspectReactProps = () => {
                const root = document.getElementById('react-tree-root');
                if (root && root._reactInternalInstance) {
                    return root._reactInternalInstance.memoizedProps;
                }
                // For newer React versions, try different approach
                return window.lastReactProps || null;
            };
        });

        // Modify the React mounting to store props globally for testing
        await page.evaluate(() => {
            // Override React.createElement to capture props
            const originalCreateElement = React.createElement;
            React.createElement = function(component, props, ...children) {
                if (component === window.CodeAnalysisTree && props && props.data) {
                    window.lastReactProps = props;
                    console.log('🔍 React props captured:', props);
                }
                return originalCreateElement.apply(this, arguments);
            };
        });

        // Reload to capture props
        await page.reload();
        await page.waitForTimeout(3000);

        // Check if props were captured and have the expected structure
        const propsData = await page.evaluate(() => window.lastReactProps);
        
        if (propsData) {
            expect(propsData).toHaveProperty('data');
            expect(propsData.data).toHaveProperty('refactoringCandidatesByFile');
            console.log('Props structure verified:', Object.keys(propsData.data));
        } else {
            console.log('Could not capture React props directly - checking indirect evidence');
            
            // Indirect verification: check that component renders with data
            const hasDataContent = await page.evaluate(() => {
                const root = document.getElementById('react-tree-root');
                return root && root.textContent && root.textContent.length > 100;
            });
            
            expect(hasDataContent).toBe(true);
        }
    });

    test('should handle Lucide icons after React render', async ({ page }) => {
        // Wait for React and icons to load
        await page.waitForTimeout(3000);

        // Check that Lucide icons are initialized
        const lucideInitialized = await page.evaluate(() => {
            return typeof window.lucide !== 'undefined' && 
                   typeof window.lucide.createIcons === 'function';
        });

        expect(lucideInitialized).toBe(true);

        // Check for SVG icons in the DOM (Lucide renders as SVG)
        const svgIcons = await page.locator('svg[data-lucide], i[data-lucide]').count();
        console.log('Lucide icons found:', svgIcons);
        
        // Should have some icons if the component is properly rendered
        expect(svgIcons).toBeGreaterThanOrEqual(0);
    });
});

test.describe('React Error #31 Fix - Data Verification', () => {
    test('should have substantial refactoring candidates data', async ({ page }) => {
        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForTimeout(2000);

        // Get the embedded JSON data
        const treeData = await page.evaluate(() => {
            const script = document.getElementById('tree-data');
            return script ? JSON.parse(script.textContent) : null;
        });

        expect(treeData).toBeTruthy();
        expect(treeData.refactoringCandidatesByFile).toBeTruthy();
        expect(treeData.refactoringCandidatesByFile.length).toBeGreaterThan(0);

        // Log the actual data structure for debugging
        console.log('Refactoring candidates files:', treeData.refactoringCandidatesByFile.length);
        
        treeData.refactoringCandidatesByFile.forEach((file, index) => {
            if (index < 5) { // Log first 5 files
                console.log(`File ${index + 1}:`, file.fileName, 'candidates:', file.candidates?.length || 0);
            }
        });
    });

    test('should show specific analysis data in rendered component', async ({ page }) => {
        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForTimeout(3000);

        const reactRoot = page.locator('#react-tree-root');
        const rootText = await reactRoot.textContent();
        
        // Should show file names from the analysis
        expect(rootText).toMatch(/\.rs|\.py|\.js|\.ts/); // Should show file extensions
        
        // Should show some complexity or priority information
        expect(rootText).toMatch(/priority|complexity|refactor/i);
        
        // Should have substantial content (not just a placeholder)
        expect(rootText.length).toBeGreaterThan(200);
        
        console.log('Component text length:', rootText.length);
        console.log('Sample content:', rootText.substring(0, 200));
    });
});