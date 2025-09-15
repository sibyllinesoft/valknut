// Simple debug to check what data is causing React error #31
const fs = require('fs');

// Read the generated HTML report
const reportPath = '/home/nathan/Projects/valknut/final-working-demo/report_20250914_165323.html';
const html = fs.readFileSync(reportPath, 'utf8');

// Extract the JSON data from the script tag
const match = html.match(/<script id="tree-data" type="application\/json">\s*(.*?)\s*<\/script>/s);
if (!match) {
    console.log('❌ No tree-data script found');
    process.exit(1);
}

try {
    const jsonData = JSON.parse(match[1]);
    console.log('✅ JSON parsed successfully');
    
    // Check the structure for objects that might be rendered as children
    console.log('\n🔍 Analyzing data structure for React error #31 causes...\n');
    
    const refactoringCandidates = jsonData.refactoringCandidatesByFile || [];
    console.log(`Found ${refactoringCandidates.length} refactoring candidates`);
    
    refactoringCandidates.forEach((file, fileIndex) => {
        console.log(`\n📁 File ${fileIndex}: ${file.fileName}`);
        console.log(`  - fileName: ${typeof file.fileName} = "${file.fileName}"`);
        console.log(`  - filePath: ${typeof file.filePath} = "${file.filePath}"`);
        console.log(`  - highestPriority: ${typeof file.highestPriority} = "${file.highestPriority}"`);
        
        if (file.entities) {
            file.entities.forEach((entity, entityIndex) => {
                console.log(`\n  🔧 Entity ${entityIndex}: ${entity.name}`);
                console.log(`    - name: ${typeof entity.name} = "${entity.name}"`);
                console.log(`    - priority: ${typeof entity.priority} = "${entity.priority}"`);
                console.log(`    - score: ${typeof entity.score} = ${entity.score}`);
                
                // Check for objects in issues/suggestions that might be rendered
                if (entity.issues && Array.isArray(entity.issues)) {
                    entity.issues.forEach((issue, issueIndex) => {
                        console.log(`    📋 Issue ${issueIndex}:`);
                        console.log(`      - category: ${typeof issue.category} = "${issue.category}"`);
                        console.log(`      - severity: ${typeof issue.severity} = ${issue.severity}`);
                        console.log(`      - description: ${typeof issue.description} = "${issue.description.substring(0, 50)}..."`);
                        
                        if (issue.contributingFactors && Array.isArray(issue.contributingFactors)) {
                            issue.contributingFactors.forEach((factor, factorIndex) => {
                                console.log(`      🔍 Factor ${factorIndex}:`);
                                console.log(`        - name: ${typeof factor.name} = "${factor.name}"`);
                                console.log(`        - value: ${typeof factor.value} = ${factor.value}`);
                                console.log(`        - impact: ${typeof factor.impact} = ${factor.impact}`);
                                
                                // CHECK FOR OBJECTS!
                                if (typeof factor.name === 'object') {
                                    console.error(`        ❌ FOUND OBJECT AS NAME: ${JSON.stringify(factor.name)}`);
                                }
                                if (typeof factor.value === 'object') {
                                    console.error(`        ❌ FOUND OBJECT AS VALUE: ${JSON.stringify(factor.value)}`);
                                }
                                if (typeof factor.impact === 'object') {
                                    console.error(`        ❌ FOUND OBJECT AS IMPACT: ${JSON.stringify(factor.impact)}`);
                                }
                            });
                        }
                    });
                }
                
                if (entity.suggestions && Array.isArray(entity.suggestions)) {
                    entity.suggestions.forEach((suggestion, sugIndex) => {
                        console.log(`    💡 Suggestion ${sugIndex}:`);
                        console.log(`      - type: ${typeof suggestion.type} = "${suggestion.type}"`);
                        console.log(`      - description: ${typeof suggestion.description} = "${suggestion.description.substring(0, 50)}..."`);
                        console.log(`      - score: ${typeof suggestion.score} = ${suggestion.score}`);
                        
                        // CHECK FOR OBJECTS!
                        if (typeof suggestion.type === 'object') {
                            console.error(`      ❌ FOUND OBJECT AS TYPE: ${JSON.stringify(suggestion.type)}`);
                        }
                        if (typeof suggestion.description === 'object') {
                            console.error(`      ❌ FOUND OBJECT AS DESCRIPTION: ${JSON.stringify(suggestion.description)}`);
                        }
                        if (typeof suggestion.score === 'object') {
                            console.error(`      ❌ FOUND OBJECT AS SCORE: ${JSON.stringify(suggestion.score)}`);
                        }
                    });
                }
                
                // Check other entity properties
                if (typeof entity.name === 'object') {
                    console.error(`    ❌ ENTITY NAME IS OBJECT: ${JSON.stringify(entity.name)}`);
                }
                if (typeof entity.priority === 'object') {
                    console.error(`    ❌ ENTITY PRIORITY IS OBJECT: ${JSON.stringify(entity.priority)}`);
                }
            });
        }
    });
    
} catch (error) {
    console.error('❌ Failed to parse JSON:', error.message);
}