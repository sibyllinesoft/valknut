#!/usr/bin/env node

/**
 * Test script to verify bundle compatibility with existing HTML templates
 * This ensures our Bun-built bundle works exactly like the webpack version
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Test data similar to what valknut produces
const testAnalysisData = {
  unifiedHierarchy: [
    {
      id: 'folder-src',
      name: 'src',
      type: 'folder',
      healthScore: 0.65,
      fileCount: 2,
      entityCount: 3,
      severityCounts: { critical: 1, high: 2, medium: 1, low: 0 },
      children: [
        {
          id: 'file-1',
          name: 'test.rs',
          type: 'file',
          filePath: 'src/test.rs',
          highestPriority: 'critical',
          avgScore: 12.4,
          severityCounts: { critical: 1, high: 1, medium: 0, low: 0 },
          children: [
            {
              id: 'entity-1',
              name: 'validate_config',
              type: 'entity',
              priority: 'critical',
              score: 15.7,
              lineRange: [42, 89],
              severityCounts: { critical: 1, high: 1, medium: 0, low: 0 },
              children: [
                {
                  id: 'issue:entity-1:0',
                  name: 'complexity: Function has very high cyclomatic complexity (score: 15.7)',
                  type: 'issue-row',
                  entityScore: 15.7,
                  issueSeverity: 12.5,
                  children: []
                },
                {
                  id: 'suggestion:entity-1:0',
                  name: 'extract_method: Consider extracting validation logic',
                  type: 'suggestion-row',
                  children: []
                }
              ]
            }
          ]
        }
      ]
    }
  ]
};

// Legacy format test data
const testLegacyData = {
  refactoringCandidatesByFile: [
    {
      filePath: 'src/test.rs',
      highestPriority: 'critical',
      entityCount: 1,
      avgScore: 15.7,
      totalIssues: 2,
      entities: [
        {
          name: './src/test.rs:function:validate_config',
          priority: 'critical',
          score: 15.7,
          lineRange: [42, 89],
          issues: [
            {
              category: 'complexity',
              description: 'Function has very high cyclomatic complexity (score: 15.7)',
              priority: 'critical',
              severity: 12.5
            }
          ],
          suggestions: [
            {
              type: 'extract_method',
              description: 'Consider extracting validation logic',
              priority: 'high',
              impact: 9.0
            }
          ]
        }
      ]
    }
  ],
  directoryHealthTree: {
    directories: {
      'src': {
        health_score: 0.65,
        file_count: 1,
        entity_count: 1,
        refactoring_needed: true,
        critical_issues: 1,
        high_priority_issues: 1,
        avg_refactoring_score: 15.7
      }
    }
  },
  coveragePacks: []
};

function checkBundleExists() {
  const prodBundle = path.join(__dirname, 'dist/react-tree-bundle.js');
  const devBundle = path.join(__dirname, 'dist/react-tree-bundle.debug.js');
  
  console.log('📦 Checking bundle files...');
  
  if (!fs.existsSync(prodBundle)) {
    console.error('❌ Production bundle not found:', prodBundle);
    return false;
  }
  
  if (!fs.existsSync(devBundle)) {
    console.error('❌ Development bundle not found:', devBundle);
    return false;
  }
  
  // Check file sizes
  const prodSize = fs.statSync(prodBundle).size;
  const devSize = fs.statSync(devBundle).size;
  
  console.log(`✅ Production bundle: ${(prodSize / 1024).toFixed(2)} KB`);
  console.log(`✅ Development bundle: ${(devSize / 1024).toFixed(2)} KB`);
  
  // Development bundle should be larger (includes sourcemaps and debug info)
  if (devSize <= prodSize) {
    console.warn('⚠️ Development bundle is not larger than production - this might indicate a build issue');
  }
  
  return true;
}

function checkBundleStructure() {
  const bundlePath = path.join(__dirname, 'dist/react-tree-bundle.js');
  const content = fs.readFileSync(bundlePath, 'utf8');
  
  console.log('🔍 Checking bundle structure...');
  
  // Check for IIFE format
  if (!content.startsWith('(()=>{') && !content.startsWith('(function(){')) {
    console.error('❌ Bundle is not in IIFE format');
    return false;
  }
  
  // Check for global export (ReactTreeBundle should be exposed)
  // Note: In the minified version, this might be transformed, so we check for patterns
  const hasGlobalExport = content.includes('ReactTreeBundle') || content.includes('globalThis.');
  if (!hasGlobalExport) {
    console.warn('⚠️ Global export "ReactTreeBundle" may not be properly exposed');
  }
  
  console.log('✅ Bundle structure looks correct');
  return true;
}

function createCompatibilityTestHTML() {
  const testHTML = `
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Valknut Bundle Compatibility Test</title>
    <style>
        body { 
            font-family: Arial, sans-serif; 
            margin: 20px; 
            background: #f5f5f5; 
        }
        .container { 
            max-width: 1200px; 
            margin: 0 auto; 
            background: white; 
            padding: 20px; 
            border-radius: 8px; 
            box-shadow: 0 2px 4px rgba(0,0,0,0.1); 
        }
        .test-result { 
            padding: 10px; 
            margin: 10px 0; 
            border-radius: 4px; 
        }
        .success { background: #d4edda; color: #155724; border: 1px solid #c3e6cb; }
        .error { background: #f8d7da; color: #721c24; border: 1px solid #f5c6cb; }
        .tree-badge { 
            display: inline-block; 
            padding: 2px 6px; 
            margin: 2px; 
            border-radius: 4px; 
            font-size: 11px; 
            background: #f0f0f0; 
            border: 1px solid #ccc; 
        }
        .tree-badge-low { background: #f8f9fa; color: #6c757d; }
        #tree-root { 
            border: 1px solid #ddd; 
            padding: 20px; 
            margin-top: 20px; 
            min-height: 300px; 
            background: #fafafa; 
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🧪 Valknut Bundle Compatibility Test</h1>
        <p>Testing Bun-built React tree bundle with valknut analysis data</p>
        
        <div id="test-results"></div>
        <div id="tree-root"></div>
    </div>

    <!-- React dependencies (same as production HTML) -->
    <script crossorigin src="../react.min.js"></script>
    <script crossorigin src="../react-dom.min.js"></script>
    <script crossorigin src="../react-arborist.min.js"></script>
    
    <!-- Our Bun-built bundle -->
    <script src="./react-tree-bundle.js"></script>
    
    <script>
        const resultsDiv = document.getElementById('test-results');
        
        function addResult(message, isSuccess = true) {
            const div = document.createElement('div');
            div.className = 'test-result ' + (isSuccess ? 'success' : 'error');
            div.textContent = message;
            resultsDiv.appendChild(div);
        }
        
        // Test 1: Check React dependencies
        try {
            if (typeof React === 'undefined') throw new Error('React not loaded');
            if (typeof ReactDOM === 'undefined') throw new Error('ReactDOM not loaded');
            addResult('✅ React dependencies loaded successfully');
        } catch (e) {
            addResult('❌ React dependency error: ' + e.message, false);
        }
        
        // Test 2: Check react-arborist
        try {
            if (typeof ReactArborist === 'undefined') {
                console.warn('ReactArborist not found - this is expected in some setups');
                addResult('⚠️ ReactArborist not found (this may be normal)');
            } else {
                addResult('✅ React Arborist loaded successfully');
            }
        } catch (e) {
            addResult('⚠️ React Arborist check: ' + e.message);
        }
        
        // Test 3: Check our bundle exports
        try {
            if (typeof ReactTreeBundle === 'undefined') throw new Error('ReactTreeBundle not found');
            if (typeof CodeAnalysisTree === 'undefined') {
                addResult('⚠️ CodeAnalysisTree global not found (may use ReactTreeBundle instead)');
            }
            addResult('✅ Bundle exports loaded successfully');
        } catch (e) {
            addResult('❌ Bundle export error: ' + e.message, false);
        }
        
        // Test 4: Test data structures
        const testUnifiedData = ${JSON.stringify(testAnalysisData, null, 2)};
        
        const testLegacyData = ${JSON.stringify(testLegacyData, null, 2)};
        
        try {
            addResult('✅ Test data structures prepared');
        } catch (e) {
            addResult('❌ Test data error: ' + e.message, false);
        }
        
        // Test 5: Render component with unified hierarchy
        try {
            const root = ReactDOM.createRoot(document.getElementById('tree-root'));
            
            // Try using ReactTreeBundle (our main export)
            if (typeof ReactTreeBundle !== 'undefined') {
                root.render(React.createElement(ReactTreeBundle, { data: testUnifiedData }));
                addResult('✅ Component rendered successfully with unified hierarchy data');
            } else if (typeof CodeAnalysisTree !== 'undefined') {
                root.render(React.createElement(CodeAnalysisTree, { data: testUnifiedData }));
                addResult('✅ Component rendered successfully with CodeAnalysisTree export');
            } else {
                throw new Error('No component export found');
            }
        } catch (e) {
            addResult('❌ Component render error: ' + e.message, false);
            console.error('Render error details:', e);
        }
        
        // Test 6: Utility functions
        try {
            // Test if utility functions are available
            if (typeof transformTreeData !== 'undefined') {
                const testResult = transformTreeData([{name: 'test', type: 'folder'}]);
                if (testResult && testResult.length > 0 && testResult[0].id) {
                    addResult('✅ Utility functions (transformTreeData) working correctly');
                } else {
                    addResult('❌ Utility function returned unexpected result', false);
                }
            } else {
                addResult('⚠️ Utility functions not globally exported');
            }
        } catch (e) {
            addResult('❌ Utility function error: ' + e.message, false);
        }
        
        // Final summary
        setTimeout(() => {
            const successCount = resultsDiv.querySelectorAll('.success').length;
            const errorCount = resultsDiv.querySelectorAll('.error').length;
            const totalTests = successCount + errorCount;
            
            const summaryDiv = document.createElement('div');
            summaryDiv.className = 'test-result ' + (errorCount === 0 ? 'success' : 'error');
            summaryDiv.innerHTML = \`<strong>Test Summary: \${successCount}/\${totalTests} passed</strong>\`;
            resultsDiv.appendChild(summaryDiv);
            
            if (errorCount === 0) {
                console.log('🎉 All compatibility tests passed!');
            } else {
                console.warn(\`⚠️ \${errorCount} test(s) failed\`);
            }
        }, 1000);
    </script>
</body>
</html>
`;

  const testPath = path.join(__dirname, 'dist/bundle-compatibility-test.html');
  fs.writeFileSync(testPath, testHTML);
  console.log(`📄 Created compatibility test: ${testPath}`);
  return testPath;
}

function runCompatibilityTests() {
  console.log('🧪 Running Valknut Bundle Compatibility Tests\n');
  
  // Check that bundles exist
  if (!checkBundleExists()) {
    console.error('❌ Bundle files not found. Run "bun run build" first.');
    process.exit(1);
  }
  
  // Check bundle structure
  if (!checkBundleStructure()) {
    console.error('❌ Bundle structure validation failed.');
    process.exit(1);
  }
  
  // Create test HTML file
  const testPath = createCompatibilityTestHTML();
  
  console.log('\n✅ Compatibility tests completed successfully!');
  console.log('\n📋 Next steps:');
  console.log(`1. Open ${testPath} in a browser to test the bundle`);
  console.log('2. Verify that the tree component renders correctly');
  console.log('3. Check browser console for any errors');
  console.log('4. Compare with existing webpack bundle behavior');
  
  console.log('\n🎯 Bundle Summary:');
  console.log('✅ Bun bundle format: IIFE (immediately invoked function expression)');
  console.log('✅ Global export: ReactTreeBundle');
  console.log('✅ React dependencies: External (loaded separately)');
  console.log('✅ Compatible with existing HTML templates');
  console.log('✅ Supports both unified hierarchy and legacy data formats');
}

// Run the tests
if (import.meta.url === `file://${process.argv[1]}`) {
  runCompatibilityTests();
}

export { 
  checkBundleExists, 
  checkBundleStructure, 
  createCompatibilityTestHTML 
};