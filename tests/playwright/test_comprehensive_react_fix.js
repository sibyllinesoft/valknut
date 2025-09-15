const { test, expect } = require('@playwright/test');

// Test the comprehensive React error #31 fix
test.describe('Comprehensive React Error #31 Fix', () => {
    const REPORT_PATH = '/home/nathan/Projects/valknut/final-react-fix-comprehensive/report_20250914_155810.html';
    
    test('should have NO React error #31 after comprehensive fix', async ({ page }) => {
        const consoleErrors = [];
        const reactErrors = [];
        
        page.on('console', msg => {
            if (msg.type() === 'error') {
                consoleErrors.push(msg.text());
                
                // Check for React error #31 specifically
                if (msg.text().includes('reactjs.org/docs/error-decoder.html?invariant=31') ||
                    msg.text().includes('Objects are not valid as a React child') ||
                    msg.text().includes('react error #31')) {
                    reactErrors.push(msg.text());
                }
            }
        });
        
        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(3000);
        
        console.log('🔍 Console Errors Found:', consoleErrors.length);
        consoleErrors.forEach(error => console.log(`  - ${error}`));
        
        console.log('🚨 React Error #31 Occurrences:', reactErrors.length);
        reactErrors.forEach(error => console.log(`  - ${error}`));
        
        // CRITICAL ASSERTION: No React error #31 should occur
        expect(reactErrors).toHaveLength(0);
    });
    
    test('should successfully render tree component with data', async ({ page }) => {
        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(2000);
        
        // Check if React component mounted
        const treeContainer = page.locator('.tree-container');
        await expect(treeContainer).toBeVisible();
        
        // Check for tree nodes
        const treeNodes = await page.locator('.tree-node').count();
        console.log(`✅ Found ${treeNodes} tree nodes`);
        expect(treeNodes).toBeGreaterThan(0);
    });
    
    test('should have proper data attributes (no objects)', async ({ page }) => {
        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(2000);
        
        // Check that all data attributes are strings, not objects
        const invalidDataAttrs = await page.evaluate(() => {
            const nodes = document.querySelectorAll('.tree-node');
            const issues = [];
            
            nodes.forEach((node, index) => {
                // Check each data attribute
                for (const attr of node.attributes) {
                    if (attr.name.startsWith('data-')) {
                        const value = attr.value;
                        // Check if value looks like an object (contains { or [)
                        if (value.includes('{') || value.includes('[')) {
                            issues.push({
                                nodeIndex: index,
                                attribute: attr.name,
                                value: value
                            });
                        }
                    }
                }
            });
            
            return issues;
        });
        
        console.log('🔍 Invalid Data Attributes Found:', invalidDataAttrs.length);
        invalidDataAttrs.forEach(issue => {
            console.log(`  - Node ${issue.nodeIndex}: ${issue.attribute} = "${issue.value}"`);
        });
        
        expect(invalidDataAttrs).toHaveLength(0);
    });
    
    test('should handle tree expansion without errors', async ({ page }) => {
        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(2000);
        
        const consoleErrors = [];
        page.on('console', msg => {
            if (msg.type() === 'error') {
                consoleErrors.push(msg.text());
            }
        });
        
        // Try to expand tree nodes
        const expandableNodes = await page.locator('.tree-node[data-has-children="true"]').count();
        console.log(`🌳 Found ${expandableNodes} expandable nodes`);
        
        if (expandableNodes > 0) {
            // Click first expandable node
            await page.locator('.tree-node[data-has-children="true"]').first().click();
            await page.waitForTimeout(500);
        }
        
        // Should have no console errors after interaction
        expect(consoleErrors).toHaveLength(0);
    });
    
    test('should have component successfully loaded message', async ({ page }) => {
        const componentMessages = [];
        page.on('console', msg => {
            if (msg.type() === 'log' && msg.text().includes('CodeAnalysisTree')) {
                componentMessages.push(msg.text());
            }
        });
        
        await page.goto(`file://${REPORT_PATH}`);
        await page.waitForLoadState('networkidle');
        await page.waitForTimeout(2000);
        
        console.log('📋 Component Messages:', componentMessages);
        
        // Should have success message
        const successMessage = componentMessages.find(msg => 
            msg.includes('CodeAnalysisTree component loaded successfully')
        );
        
        expect(successMessage).toBeTruthy();
    });
});