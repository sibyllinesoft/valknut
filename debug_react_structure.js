// Debug React structure to find object-as-children issues
// This simulates the React component execution without actual React

// Mock React.createElement to detect objects passed as children
const mockReact = {
    createElement: function(type, props, ...children) {
        console.log(`\n🔍 React.createElement called:`)
        console.log(`  Type: ${typeof type === 'function' ? type.name : type}`)
        console.log(`  Props: ${JSON.stringify(props, null, 2)}`)
        
        if (children.length > 0) {
            console.log(`  Children (${children.length}):`)
            children.forEach((child, index) => {
                console.log(`    [${index}]: ${typeof child} - ${child}`)
                
                // Check for React element objects being passed as children
                if (child && typeof child === 'object' && child.$$typeof) {
                    console.error(`❌ REACT ERROR #31: React element passed as child at index ${index}`)
                    console.error(`   Object keys: ${Object.keys(child)}`)
                    return false;
                }
                
                // Check for plain objects being passed as children  
                if (child && typeof child === 'object' && !Array.isArray(child) && !child.$$typeof) {
                    console.error(`❌ REACT ERROR #31: Plain object passed as child at index ${index}`)
                    console.error(`   Object keys: ${Object.keys(child)}`)
                    return false;
                }
            })
        }
        
        return {
            $$typeof: Symbol.for('react.element'),
            type: type,
            props: { ...props, children: children.length === 1 ? children[0] : children },
            key: props?.key || null,
            ref: props?.ref || null
        }
    }
}

// Load the actual tree.js component source
const fs = require('fs');
const treeSource = fs.readFileSync('/home/nathan/Projects/valknut/templates/assets/src/tree.js', 'utf8');

console.log('🧪 Testing React component structure for error #31...\n')

// Replace React with mock and execute
const modifiedSource = treeSource.replace(/React\./g, 'mockReact.');

try {
    // Set up mock environment
    global.React = mockReact;
    global.window = {};
    
    // Mock react-arborist Tree component
    global.Tree = function Tree(props) {
        console.log('\n🌳 Tree component called with props:', Object.keys(props));
        if (props.children && typeof props.children === 'function') {
            console.log('✅ TreeNode passed correctly as children prop');
        }
        return mockReact.createElement('div', { className: 'tree' }, 'Mock Tree');
    };
    
    // Execute the component code
    eval(modifiedSource);
    
    // Test the component with mock data
    const testData = {
        refactoringCandidatesByFile: [{
            fileName: "test.rs",
            filePath: "./test.rs",
            entities: [{
                name: "test_function",
                priority: "High",
                score: 85.5,
                issues: [{
                    category: "complexity",
                    severity: 8.0,
                    description: "High complexity",
                    contributingFactors: []
                }],
                suggestions: [{
                    type: "refactor",
                    description: "Extract method",
                    score: 7.5
                }]
            }]
        }],
        directoryHealthTree: {
            directories: {
                "src": {
                    health_score: 0.75,
                    file_count: 10
                }
            }
        }
    };
    
    console.log('\n🧪 Testing CodeAnalysisTree component...');
    if (global.window.CodeAnalysisTree) {
        const result = global.window.CodeAnalysisTree({ data: testData });
        console.log('✅ Component executed without throwing errors');
    } else {
        console.error('❌ CodeAnalysisTree not found in window');
    }
    
} catch (error) {
    console.error('❌ Error during component execution:', error.message);
    console.error('Stack:', error.stack);
}