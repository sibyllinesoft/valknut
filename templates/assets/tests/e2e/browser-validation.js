/**
 * Browser Validation - Real Browser Testing
 * 
 * This module uses Playwright to open generated HTML in a real browser
 * and verify that React components actually load and render correctly.
 */

class BrowserValidator {
    constructor() {
        this.playwright = null;
        this.browser = null;
        this.page = null;
    }

    /**
     * Initialize browser automation (try to load Playwright)
     */
    async initialize() {
        try {
            // Try to load Playwright dynamically
            const { chromium } = await import('playwright');
            this.playwright = chromium;
            return true;
        } catch (error) {
            console.log('⚠️  Playwright not available - skipping browser validation');
            return false;
        }
    }

    /**
     * Launch browser and create page
     */
    async launchBrowser() {
        if (!this.playwright) {
            throw new Error('Playwright not initialized');
        }

        this.browser = await this.playwright.launch({ 
            headless: true,
            args: ['--no-sandbox', '--disable-dev-shm-usage']
        });
        this.page = await this.browser.newPage();
        
        // Set up console logging
        this.page.on('console', msg => {
            if (msg.type() === 'error') {
                console.log(`Browser Error: ${msg.text()}`);
            }
        });
        
        // Set up error handling
        this.page.on('pageerror', error => {
            console.log(`Page Error: ${error.message}`);
        });
    }

    /**
     * Validate React tree rendering in browser
     */
    async validateReactTreeRendering(htmlFilePath) {
        if (!this.page) {
            throw new Error('Browser not launched');
        }

        const fs = require('fs');
        const path = require('path');

        // Check if HTML file exists
        if (!fs.existsSync(htmlFilePath)) {
            throw new Error(`HTML file not found: ${htmlFilePath}`);
        }

        // Navigate to the HTML file
        await this.page.goto(`file://${path.resolve(htmlFilePath)}`);

        // Wait for the page to load
        await this.page.waitForLoadState('networkidle');

        // Check if React and ReactDOM are available
        const reactCheck = await this.page.evaluate(() => {
            return {
                reactLoaded: typeof window.React !== 'undefined',
                reactDOMLoaded: typeof window.ReactDOM !== 'undefined',
                scripts: Array.from(document.querySelectorAll('script')).map(s => ({
                    src: s.src,
                    type: s.type,
                    hasContent: !!s.textContent
                })),
                errors: window.lastError || null
            };
        });

        if (!reactCheck.reactLoaded || !reactCheck.reactDOMLoaded) {
            console.log('React check details:', JSON.stringify(reactCheck, null, 2));
            throw new Error(`React not loaded: React=${reactCheck.reactLoaded}, ReactDOM=${reactCheck.reactDOMLoaded}`);
        }

        // Wait for React tree container
        try {
            await this.page.waitForSelector('#react-tree-root', { timeout: 5000 });
        } catch (error) {
            throw new Error('React tree root not found');
        }

        // Wait for React components to render using a more intelligent approach
        // Instead of waiting for a specific selector, wait for the tree to be built
        try {
            await this.page.waitForFunction(() => {
                // Check if React has rendered any tree nodes
                const treeNodes = document.querySelectorAll('.tree-node');
                if (treeNodes.length > 0) {
                    return 'tree-rendered';
                }
                
                // Check if React has successfully processed data but shows "no results"
                const container = document.getElementById('react-tree-root');
                if (container) {
                    const text = container.textContent;
                    if (text.includes('Loaded') && text.includes('nodes successfully')) {
                        return 'data-processed';
                    }
                    if (text.includes('React components not available')) {
                        return 'error';
                    }
                }
                
                // Still loading/processing
                return false;
            }, { timeout: 45000, polling: 1000 });
        } catch (error) {
            // Get final state for debugging
            const finalState = await this.page.evaluate(() => {
                const container = document.getElementById('react-tree-root');
                const treeNodes = document.querySelectorAll('.tree-node');
                return {
                    containerText: container ? container.textContent.slice(0, 200) : 'not found',
                    treeNodeCount: treeNodes.length,
                    hasReact: typeof window.React !== 'undefined',
                    hasReactDOM: typeof window.ReactDOM !== 'undefined',
                    windowKeys: Object.keys(window).filter(k => k.toLowerCase().includes('react')).slice(0, 5)
                };
            });
            
            console.log('Final state when React test failed:', JSON.stringify(finalState, null, 2));
            
            if (finalState.containerText.includes('React components not available')) {
                throw new Error('React components failed to load: ' + finalState.containerText);
            } else {
                throw new Error(`React tree did not render. Tree nodes: ${finalState.treeNodeCount}, React available: ${finalState.hasReact}`);
            }
        }

        // Check the final state - either tree nodes rendered or data processed successfully
        const finalResult = await this.page.evaluate(() => {
            const treeNodes = document.querySelectorAll('.tree-node');
            const fileNodes = document.querySelectorAll('.tree-node--file');
            const folderNodes = document.querySelectorAll('.tree-node--folder');
            const container = document.getElementById('react-tree-root');
            const containerText = container ? container.textContent : '';
            
            return {
                treeNodeCount: treeNodes.length,
                fileNodeCount: fileNodes.length,
                folderNodeCount: folderNodes.length,
                containerText: containerText,
                hasTreeNodes: treeNodes.length > 0,
                dataProcessed: containerText.includes('Loaded') && containerText.includes('nodes successfully')
            };
        });

        if (!finalResult.hasTreeNodes && !finalResult.dataProcessed) {
            throw new Error(`Neither tree nodes rendered nor data processed successfully. Container: ${finalResult.containerText.slice(0, 100)}`);
        }

        return {
            reactLoaded: true,
            treeNodeCount: finalResult.treeNodeCount,
            fileNodeCount: finalResult.fileNodeCount,
            folderNodeCount: finalResult.folderNodeCount,
            dataProcessed: finalResult.dataProcessed,
            success: true
        };
    }

    /**
     * Take screenshot for debugging
     */
    async takeScreenshot(outputPath) {
        if (this.page) {
            await this.page.screenshot({ path: outputPath, fullPage: true });
        }
    }

    /**
     * Close browser
     */
    async cleanup() {
        if (this.browser) {
            await this.browser.close();
        }
    }
}

module.exports = BrowserValidator;