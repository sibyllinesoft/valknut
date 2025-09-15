// Debug script to examine the React component in our bundle and find React error #31 cause
const { test, expect } = require('@playwright/test');

test('Debug React error #31 - examine what objects are being passed as children', async ({ page }) => {
    const reportPath = '/home/nathan/Projects/valknut/final-react-fix/report_20250914_155810.html';
    
    // Enable detailed console logging
    const consoleLogs = [];
    page.on('console', msg => {
        consoleLogs.push({
            type: msg.type(),
            text: msg.text(),
            timestamp: new Date().toISOString()
        });
    });
    
    // Load the report
    await page.goto(`file://${reportPath}`);
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(3000);
    
    // Debug: try to understand what objects are being passed as children
    const debugInfo = await page.evaluate(() => {
        // Look for the React error and try to understand what's happening
        const reactTreeRoot = document.getElementById('react-tree-root');
        const treeDataScript = document.getElementById('tree-data');
        
        let analysisData = null;
        if (treeDataScript) {
            try {
                analysisData = JSON.parse(treeDataScript.textContent);
            } catch (e) {
                console.log('Failed to parse tree data:', e);
            }
        }
        
        // Check if React is available
        const hasReact = typeof window.React !== 'undefined';
        const hasReactDOM = typeof window.ReactDOM !== 'undefined';
        const hasCodeAnalysisTree = typeof window.CodeAnalysisTree !== 'undefined';
        
        // Try to understand the data structure being passed to React
        let dataStructure = null;
        if (analysisData) {
            dataStructure = {
                refactoringCandidatesType: typeof analysisData.refactoringCandidatesByFile,
                refactoringCandidatesLength: analysisData.refactoringCandidatesByFile ? analysisData.refactoringCandidatesByFile.length : 'N/A',
                directoryHealthTreeType: typeof analysisData.directoryHealthTree,
                sampleCandidate: analysisData.refactoringCandidatesByFile?.[0] || null
            };
        }
        
        return {
            reactAvailable: hasReact,
            reactDomAvailable: hasReactDOM,
            codeAnalysisTreeAvailable: hasCodeAnalysisTree,
            analysisDataParsed: analysisData !== null,
            dataStructure,
            treeDataScriptExists: treeDataScript !== null,
            reactTreeRootExists: reactTreeRoot !== null
        };
    });
    
    console.log('🔍 Debug Information:');
    console.log(JSON.stringify(debugInfo, null, 2));
    
    // Look for React errors specifically mentioning objects as children
    const reactChildrenErrors = consoleLogs.filter(log => 
        log.type === 'error' && 
        log.text.includes('Objects are not valid as a React child')
    );
    
    console.log('🚨 React Children Errors Found:');
    reactChildrenErrors.forEach(error => {
        console.log(`  - ${error.text}`);
    });
    
    // Look for React error #31
    const reactError31 = consoleLogs.filter(log => 
        log.type === 'error' && 
        log.text.includes('react error #31') || log.text.includes('reactjs.org/docs/error-decoder.html?invariant=31')
    );
    
    console.log('🚨 React Error #31 Found:');
    reactError31.forEach(error => {
        console.log(`  - ${error.text}`);
    });
    
    // Extract the URL parameters from React error #31 to understand what object is being passed
    if (reactError31.length > 0) {
        const errorText = reactError31[0].text;
        const urlMatch = errorText.match(/args\[]=([^&\s]+)/);
        if (urlMatch) {
            console.log('🔍 Error Arguments:', decodeURIComponent(urlMatch[1]));
        }
    }
    
    console.log('\n📋 All Console Messages:');
    consoleLogs.forEach(log => {
        console.log(`[${log.timestamp}] [${log.type}] ${log.text}`);
    });
});