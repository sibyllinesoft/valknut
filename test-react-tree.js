#!/usr/bin/env node

const fs = require('fs');
const { JSDOM } = require('jsdom');

// Read the HTML file
const htmlPath = '/home/nathan/Projects/valknut/final-demo-fixed/report_20250914_210337.html';
console.log('Loading HTML file:', htmlPath);

const htmlContent = fs.readFileSync(htmlPath, 'utf8');

// Create JSDOM instance with React support
const dom = new JSDOM(htmlContent, {
    url: "http://localhost/",
    pretendToBeVisual: true,
    resources: "usable",
    runScripts: "dangerously"
});

const window = dom.window;
const document = window.document;

// Make globals available
global.window = window;
global.document = document;
global.navigator = window.navigator;

// Track console messages and errors
let consoleMessages = [];
let errors = [];

// Override console methods to capture output
const originalConsole = {
    log: console.log,
    error: console.error,
    warn: console.warn,
    info: console.info
};

window.console.log = (...args) => {
    consoleMessages.push({ type: 'log', message: args.join(' ') });
    originalConsole.log(...args);
};

window.console.error = (...args) => {
    errors.push({ type: 'error', message: args.join(' ') });
    originalConsole.error('ERROR:', ...args);
};

window.console.warn = (...args) => {
    consoleMessages.push({ type: 'warn', message: args.join(' ') });
    originalConsole.warn('WARN:', ...args);
};

// Handle uncaught errors
window.addEventListener('error', (event) => {
    errors.push({
        type: 'uncaught',
        message: event.error?.message || event.message,
        stack: event.error?.stack
    });
});

// Handle React errors
window.addEventListener('unhandledrejection', (event) => {
    errors.push({
        type: 'promise_rejection',
        message: event.reason?.message || event.reason
    });
});

console.log('Testing React tree component functionality...\n');

// Wait for scripts to load and execute
setTimeout(() => {
    console.log('=== DOM ANALYSIS ===');
    
    // Check if React is loaded
    const reactScripts = document.querySelectorAll('script[src*="react"]');
    console.log(`React scripts found: ${reactScripts.length}`);
    
    // Check if our React components are present
    const reactRoot = document.getElementById('react-root');
    console.log('React root element:', reactRoot ? 'Found' : 'NOT FOUND');
    
    if (reactRoot) {
        console.log('React root innerHTML length:', reactRoot.innerHTML.length);
    }
    
    // Look for tree-related elements
    const treeElements = document.querySelectorAll('.tree-node, .directory-tree, [data-tree]');
    console.log(`Tree elements found: ${treeElements.length}`);
    
    // Check for CodeAnalysisTree component
    const analysisTree = document.querySelector('[data-component="CodeAnalysisTree"], .code-analysis-tree');
    console.log('CodeAnalysisTree component:', analysisTree ? 'Found' : 'NOT FOUND');
    
    // Check for data availability
    const scriptTags = document.querySelectorAll('script');
    let hasTreeData = false;
    let dataSize = 0;
    
    scriptTags.forEach(script => {
        if (script.textContent.includes('window.analysisData') || script.textContent.includes('treeData')) {
            hasTreeData = true;
            dataSize = script.textContent.length;
        }
    });
    
    console.log('Tree data available:', hasTreeData ? `Yes (${dataSize} chars)` : 'NO');
    
    // Check for specific React components in the DOM
    const reactComponents = document.querySelectorAll('[data-reactroot], [data-react-class]');
    console.log(`React components rendered: ${reactComponents.length}`);
    
    console.log('\n=== ERROR ANALYSIS ===');
    if (errors.length === 0) {
        console.log('✅ No errors detected!');
    } else {
        console.log(`❌ ${errors.length} error(s) found:`);
        errors.forEach((error, index) => {
            console.log(`  ${index + 1}. [${error.type}] ${error.message}`);
            if (error.stack) {
                console.log(`     Stack: ${error.stack.split('\n')[0]}`);
            }
        });
    }
    
    console.log('\n=== CONSOLE MESSAGES ===');
    if (consoleMessages.length === 0) {
        console.log('No console messages');
    } else {
        consoleMessages.slice(0, 10).forEach((msg, index) => {
            console.log(`  [${msg.type}] ${msg.message}`);
        });
        if (consoleMessages.length > 10) {
            console.log(`  ... and ${consoleMessages.length - 10} more messages`);
        }
    }
    
    console.log('\n=== FUNCTIONALITY TEST ===');
    
    // Test tree expansion functionality
    const expandableNodes = document.querySelectorAll('.tree-expandable, [data-expandable="true"]');
    console.log(`Expandable tree nodes: ${expandableNodes.length}`);
    
    // Test if click handlers are attached
    let clickHandlersFound = 0;
    expandableNodes.forEach(node => {
        if (node.onclick || node.addEventListener) {
            clickHandlersFound++;
        }
    });
    console.log(`Nodes with click handlers: ${clickHandlersFound}`);
    
    // Check for tree data structure
    if (window.analysisData || window.treeData) {
        const data = window.analysisData || window.treeData;
        console.log('✅ Tree data loaded successfully');
        console.log(`   Data type: ${typeof data}`);
        if (typeof data === 'object') {
            console.log(`   Keys: ${Object.keys(data).join(', ')}`);
        }
    } else {
        console.log('❌ No tree data found in window object');
    }
    
    // Final assessment
    console.log('\n=== FINAL ASSESSMENT ===');
    const hasErrors = errors.length > 0;
    const hasReactRoot = !!reactRoot;
    const hasData = hasTreeData;
    const hasComponents = treeElements.length > 0;
    
    if (!hasErrors && hasReactRoot && hasData && hasComponents) {
        console.log('🎉 SUCCESS: React tree component appears to be working correctly!');
        console.log('   ✅ No React errors detected');
        console.log('   ✅ React root element found');  
        console.log('   ✅ Tree data is available');
        console.log('   ✅ Tree components are rendered');
    } else {
        console.log('❌ ISSUES DETECTED:');
        if (hasErrors) console.log('   - React errors found');
        if (!hasReactRoot) console.log('   - React root missing');
        if (!hasData) console.log('   - Tree data missing');
        if (!hasComponents) console.log('   - Tree components not rendered');
    }
    
    process.exit(hasErrors ? 1 : 0);
    
}, 5000); // Wait 5 seconds for scripts to load