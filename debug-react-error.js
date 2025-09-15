#!/usr/bin/env node

const fs = require('fs');

console.log('🔍 Debugging React Error #31 - Objects are not valid as a React child\n');

// Extract the tree data from the HTML file
const htmlPath = '/home/nathan/Projects/valknut/final-demo-fixed/report_20250914_210337.html';
const htmlContent = fs.readFileSync(htmlPath, 'utf8');

// Extract just the JSON data
const treeDataMatch = htmlContent.match(/<script id="tree-data" type="application\/json">\s*([\s\S]*?)\s*<\/script>/);

if (treeDataMatch) {
    const jsonStr = treeDataMatch[1].trim();
    console.log('✅ Found tree data JSON');
    console.log('📏 JSON size:', jsonStr.length, 'characters');
    
    try {
        const data = JSON.parse(jsonStr);
        console.log('✅ JSON parsing successful');
        console.log('📊 Data structure:');
        console.log('   - Type:', typeof data);
        console.log('   - Keys:', Object.keys(data));
        
        // Analyze the structure for potential React rendering issues
        function analyzeForReactIssues(obj, path = 'root') {
            if (obj === null || obj === undefined) {
                return [`${path}: null/undefined value`];
            }
            
            if (typeof obj === 'object' && !Array.isArray(obj)) {
                // Check for React element-like objects that shouldn't be rendered as children
                if (obj.hasOwnProperty('$$typeof') || obj.hasOwnProperty('type') || obj.hasOwnProperty('props')) {
                    return [`${path}: Looks like a React element object - this causes error #31`];
                }
                
                // Check for objects with problematic structures
                let issues = [];
                for (const [key, value] of Object.entries(obj)) {
                    if (typeof value === 'object' && value !== null) {
                        issues.push(...analyzeForReactIssues(value, `${path}.${key}`));
                    }
                }
                return issues;
            }
            
            if (Array.isArray(obj)) {
                let issues = [];
                obj.forEach((item, index) => {
                    if (typeof item === 'object' && item !== null) {
                        issues.push(...analyzeForReactIssues(item, `${path}[${index}]`));
                    }
                });
                return issues;
            }
            
            return [];
        }
        
        console.log('\n🔍 Analyzing data structure for React rendering issues...');
        const issues = analyzeForReactIssues(data);
        
        if (issues.length === 0) {
            console.log('✅ No obvious React rendering issues found in data structure');
            
            // Let's check the specific fields that might be problematic
            console.log('\n📋 Detailed field analysis:');
            if (data.refactoringCandidatesByFile && Array.isArray(data.refactoringCandidatesByFile)) {
                console.log('   - refactoringCandidatesByFile: Array with', data.refactoringCandidatesByFile.length, 'items');
                
                const firstItem = data.refactoringCandidatesByFile[0];
                if (firstItem) {
                    console.log('   - First item keys:', Object.keys(firstItem));
                    console.log('   - First item entities:', firstItem.entities ? firstItem.entities.length : 'none');
                    
                    if (firstItem.entities && firstItem.entities[0]) {
                        const firstEntity = firstItem.entities[0];
                        console.log('   - First entity keys:', Object.keys(firstEntity));
                        console.log('   - First entity values:');
                        Object.entries(firstEntity).forEach(([key, value]) => {
                            console.log(`     ${key}: ${typeof value} = ${JSON.stringify(value).substring(0, 50)}...`);
                        });
                    }
                }
            }
            
            console.log('\n💡 The error is likely in how the React component handles this data');
            console.log('   Common causes of React error #31:');
            console.log('   1. Trying to render an object as a child instead of a string/number');
            console.log('   2. Missing key prop when rendering arrays');
            console.log('   3. Returning undefined/null from a component without proper handling');
            
        } else {
            console.log('❌ Found potential React rendering issues:');
            issues.forEach(issue => console.log('   -', issue));
        }
        
        // Check the React component source for clues
        console.log('\n🔍 Examining React bundle for debugging info...');
        const bundlePath = '/home/nathan/Projects/valknut/final-demo-fixed/react-tree-bundle.min.js';
        if (fs.existsSync(bundlePath)) {
            const bundleContent = fs.readFileSync(bundlePath, 'utf8');
            
            // Look for debug information or function names
            if (bundleContent.includes('CodeAnalysisTree')) {
                console.log('✅ CodeAnalysisTree component found in bundle');
            }
            
            // The bundle is minified, so let's suggest next steps
            console.log('\n🛠️  Recommended debugging steps:');
            console.log('   1. Check the CodeAnalysisTree component source code');
            console.log('   2. Add console.log statements in the component to trace data flow');
            console.log('   3. Verify all rendered values are primitives (string, number) or valid React elements');
            console.log('   4. Check array.map() operations have proper key props');
            console.log('   5. Ensure no objects are accidentally rendered as children');
            
        } else {
            console.log('❌ React bundle not found at:', bundlePath);
        }
        
    } catch (e) {
        console.error('❌ Failed to parse JSON:', e.message);
        console.log('📝 First 500 chars of JSON:');
        console.log(jsonStr.substring(0, 500));
    }
} else {
    console.log('❌ Could not find tree data in HTML file');
}

console.log('\n🎯 CONCLUSION:');
console.log('The React error #31 "Objects are not valid as a React child" is still occurring.');
console.log('This suggests the issue is in the React component code itself, not the data structure.');
console.log('The component is likely trying to render an object directly instead of its string representation.');