#!/usr/bin/env node

const fs = require('fs');
const { JSDOM } = require('jsdom');
const path = require('path');

// Read the HTML file
const htmlPath = '/home/nathan/Projects/valknut/final-demo-fixed/report_20250914_210337.html';
const reportDir = path.dirname(htmlPath);

console.log('Loading HTML file:', htmlPath);
console.log('Report directory:', reportDir);

let htmlContent = fs.readFileSync(htmlPath, 'utf8');

// Check what CSS and JS files exist in the report directory
const reportFiles = fs.readdirSync(reportDir);
console.log('Files in report directory:', reportFiles);

// Update paths to be absolute file:// URLs
const basePath = `file://${reportDir}/`;
htmlContent = htmlContent.replace(/href="\.\/([^"]+)"/g, `href="${basePath}$1"`);
htmlContent = htmlContent.replace(/src="\.\/([^"]+)"/g, `src="${basePath}$1"`);

// Create JSDOM instance
const dom = new JSDOM(htmlContent, {
    url: basePath,
    pretendToBeVisual: true,
    resources: "usable",
    runScripts: "dangerously"
});

const window = dom.window;
const document = window.document;

// Track console messages and errors
let consoleMessages = [];
let errors = [];

// Override console methods
window.console.log = (...args) => {
    const msg = args.join(' ');
    consoleMessages.push({ type: 'log', message: msg });
    console.log('JS LOG:', msg);
};

window.console.error = (...args) => {
    const msg = args.join(' ');
    errors.push({ type: 'error', message: msg });
    console.error('JS ERROR:', msg);
};

window.console.warn = (...args) => {
    const msg = args.join(' ');
    consoleMessages.push({ type: 'warn', message: msg });
    console.warn('JS WARN:', msg);
};

// Handle errors
window.addEventListener('error', (event) => {
    errors.push({
        type: 'uncaught',
        message: event.error?.message || event.message,
        stack: event.error?.stack
    });
    console.error('UNCAUGHT ERROR:', event.error?.message || event.message);
});

console.log('Testing React tree component functionality...\n');

// Check for tree data in the HTML content itself
const treeDataMatch = htmlContent.match(/window\.analysisData\s*=\s*({.+?});/s);
const hasAnalysisData = !!treeDataMatch;

console.log('=== STATIC ANALYSIS ===');
console.log('Analysis data in HTML:', hasAnalysisData ? 'Found' : 'NOT FOUND');

if (hasAnalysisData) {
    try {
        const dataStr = treeDataMatch[1];
        console.log('Analysis data size:', dataStr.length, 'characters');
        
        // Try to parse the data
        const analysisData = JSON.parse(dataStr);
        console.log('Data structure:');
        console.log('  - Type:', typeof analysisData);
        console.log('  - Keys:', Object.keys(analysisData));
        
        if (analysisData.entities) {
            console.log('  - Entities count:', analysisData.entities.length);
        }
        if (analysisData.tree_structure) {
            console.log('  - Tree structure found');
        }
    } catch (e) {
        console.error('Failed to parse analysis data:', e.message);
    }
}

// Check for React component structure in HTML
const hasReactTreeDiv = htmlContent.includes('id="react-tree-container"');
const hasCodeAnalysisTree = htmlContent.includes('CodeAnalysisTree');
const hasReactScripts = htmlContent.includes('react.min.js');

console.log('React tree container div:', hasReactTreeDiv ? 'Found' : 'NOT FOUND');
console.log('CodeAnalysisTree references:', hasCodeAnalysisTree ? 'Found' : 'NOT FOUND'); 
console.log('React scripts referenced:', hasReactScripts ? 'Found' : 'NOT FOUND');

// Check what actually exists in the DOM after parsing
setTimeout(() => {
    console.log('\n=== DOM ANALYSIS (after load) ===');
    
    // Check for key elements
    const treeContainer = document.getElementById('react-tree-container');
    const reactRoot = document.getElementById('react-root');
    
    console.log('react-tree-container element:', treeContainer ? 'Found' : 'NOT FOUND');
    console.log('react-root element:', reactRoot ? 'Found' : 'NOT FOUND');
    
    if (treeContainer) {
        console.log('Tree container innerHTML length:', treeContainer.innerHTML.length);
        if (treeContainer.innerHTML.length > 0) {
            console.log('Tree container content preview:', treeContainer.innerHTML.substring(0, 200) + '...');
        }
    }
    
    // Look for tree-related classes and elements
    const treeNodes = document.querySelectorAll('.tree-node, .directory-tree, [data-tree], .tree-item');
    const expandableNodes = document.querySelectorAll('[data-expandable], .expandable, .tree-expandable');
    
    console.log('Tree-related elements found:', treeNodes.length);
    console.log('Expandable nodes found:', expandableNodes.length);
    
    // Check if window object has our data
    if (window.analysisData) {
        console.log('✅ window.analysisData is available');
        console.log('   Type:', typeof window.analysisData);
        console.log('   Keys:', Object.keys(window.analysisData));
    } else {
        console.log('❌ window.analysisData not found');
    }
    
    // Check for React
    console.log('React available:', typeof window.React !== 'undefined' ? 'Yes' : 'No');
    console.log('ReactDOM available:', typeof window.ReactDOM !== 'undefined' ? 'Yes' : 'No');
    
    console.log('\n=== ERROR SUMMARY ===');
    if (errors.length === 0) {
        console.log('✅ No JavaScript errors detected');
    } else {
        console.log(`❌ ${errors.length} error(s):`);
        errors.forEach((error, i) => {
            console.log(`  ${i+1}. [${error.type}] ${error.message}`);
        });
    }
    
    console.log('\n=== FINAL VERDICT ===');
    const criteria = {
        hasData: hasAnalysisData,
        hasContainer: hasReactTreeDiv,
        hasScripts: hasReactScripts,
        noErrors: errors.length === 0,
        dataInWindow: !!window.analysisData
    };
    
    console.log('Assessment criteria:');
    Object.entries(criteria).forEach(([key, value]) => {
        console.log(`  ${value ? '✅' : '❌'} ${key}:`, value);
    });
    
    const allGood = Object.values(criteria).every(v => v);
    
    if (allGood) {
        console.log('\n🎉 SUCCESS: React tree component setup appears correct!');
        console.log('The previous React error #31 seems to be resolved.');
    } else {
        console.log('\n⚠️  MIXED RESULTS: Some components are working, others may need attention.');
        
        if (!criteria.hasData) console.log('   - Missing analysis data structure');
        if (!criteria.hasContainer) console.log('   - Missing React container element'); 
        if (!criteria.hasScripts) console.log('   - Missing React script references');
        if (!criteria.noErrors) console.log('   - JavaScript errors detected');
        if (!criteria.dataInWindow) console.log('   - Data not loaded into window object');
    }
    
    process.exit(0);
    
}, 3000);