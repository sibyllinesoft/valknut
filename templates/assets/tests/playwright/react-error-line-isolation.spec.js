/**
 * React Error #31 - Line Isolation and Source Mapping Test
 * 
 * This test specifically targets finding the exact line in tree.js that causes
 * the React Error #31 by testing individual patterns from the source code.
 */

const { test, expect } = require('@playwright/test');

test.describe('React Error #31 Line Isolation', () => {
  let consoleErrors = [];

  test.beforeEach(async ({ page }) => {
    consoleErrors = [];
    
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push({
          text: msg.text(),
          location: msg.location()
        });
        console.log(`🚨 ERROR: ${msg.text()}`);
      }
    });

    page.on('pageerror', error => {
      consoleErrors.push({
        text: error.message,
        stack: error.stack
      });
      console.log(`💥 PAGE ERROR: ${error.message}`);
    });
  });

  test('Test lines 268-279: Empty tree data conditional render', async ({ page }) => {
    console.log('🎯 Testing the conditional render pattern from tree.js lines 268-279...');

    const testHTML = `
<!DOCTYPE html>
<html>
<head>
    <script src="https://unpkg.com/react@18/umd/react.development.js"></script>
    <script src="https://unpkg.com/react-dom@18/umd/react-dom.development.js"></script>
</head>
<body>
    <div id="root"></div>
    <script>
      console.log('🧪 Testing lines 268-279 pattern...');
      
      // EXACT PATTERN from tree.js lines 268-279
      const TestComponent = () => {
        const treeData = []; // Empty array to trigger the condition
        
        // This is the EXACT pattern from tree.js
        if (treeData.length === 0) {
          console.log('⚠️ Showing "no analysis data" message - treeData.length is 0');
          return React.createElement('div', {
            style: {
              textAlign: 'center',
              padding: '2rem',
              color: 'var(--muted)'
            }
          }, [
            React.createElement('h3', { key: 'title' }, 'No Refactoring Candidates Found'),
            React.createElement('p', { key: 'desc' }, 'Your code is in excellent shape!')
          ]);
        }
        
        return React.createElement('div', {}, 'Should not reach here');
      };
      
      const root = ReactDOM.createRoot(document.getElementById('root'));
      root.render(React.createElement(TestComponent));
      
      console.log('✅ Test component rendered');
    </script>
</body>
</html>`;

    await page.setContent(testHTML);
    await page.waitForTimeout(1000);

    if (consoleErrors.length > 0) {
      console.log('🎯 ERROR FOUND in lines 268-279 pattern!');
      consoleErrors.forEach(error => {
        console.log(`❌ ${error.text}`);
      });
    } else {
      console.log('✅ Lines 268-279 pattern is clean');
    }
  });

  test('Test lines 276-279: Children array with React elements', async ({ page }) => {
    console.log('🎯 Testing the children array pattern from lines 276-279...');

    const testHTML = `
<!DOCTYPE html>
<html>
<head>
    <script src="https://unpkg.com/react@18/umd/react.development.js"></script>
    <script src="https://unpkg.com/react-dom@18/umd/react-dom.development.js"></script>
</head>
<body>
    <div id="root"></div>
    <script>
      console.log('🧪 Testing children array pattern...');
      
      // SUSPECTED PROBLEM: This exact pattern from lines 276-279
      const TestComponent = () => {
        // This array pattern might be problematic
        const children = [
          React.createElement('h3', { key: 'title' }, 'No Refactoring Candidates Found'),
          React.createElement('p', { key: 'desc' }, 'Your code is in excellent shape!')
        ];
        
        return React.createElement('div', {
          style: {
            textAlign: 'center',
            padding: '2rem',
            color: 'var(--muted)'
          }
        }, children); // ← POTENTIAL ISSUE HERE
      };
      
      const root = ReactDOM.createRoot(document.getElementById('root'));
      root.render(React.createElement(TestComponent));
      
      console.log('✅ Children array test rendered');
    </script>
</body>
</html>`;

    await page.setContent(testHTML);
    await page.waitForTimeout(1000);

    if (consoleErrors.length > 0) {
      console.log('🎯 ERROR FOUND in children array pattern!');
      consoleErrors.forEach(error => {
        console.log(`❌ ${error.text}`);
      });
    } else {
      console.log('✅ Children array pattern is clean');
    }
  });

  test('Test TreeNode children.push patterns from lines 28-82', async ({ page }) => {
    console.log('🎯 Testing TreeNode children.push patterns...');

    const testHTML = `
<!DOCTYPE html>
<html>
<head>
    <script src="https://unpkg.com/react@18/umd/react.development.js"></script>
    <script src="https://unpkg.com/react-dom@18/umd/react-dom.development.js"></script>
</head>
<body>
    <div id="root"></div>
    <script>
      console.log('🧪 Testing TreeNode children.push pattern...');
      
      // EXACT PATTERN from TreeNode lines 28-82
      const TreeNodeTest = ({ data }) => {
        const children = [
          // Icon - line 30-34
          React.createElement('i', {
            'data-lucide': 'folder',
            key: 'icon',
            style: { width: '16px', height: '16px', marginRight: '0.5rem' }
          }),
          
          // Label - line 37-40  
          React.createElement('span', {
            key: 'label',
            style: { flex: 1, fontWeight: 'normal' }
          }, data.name)
        ];
        
        // Health score for folders - lines 44-54
        if (data.healthScore) {
          children.push(React.createElement('div', {
            key: 'health',
            className: 'tree-badge tree-badge-low',
            style: { 
              backgroundColor: '#ff000020',
              color: '#ff0000',
              marginLeft: '0.5rem'
            }
          }, 'Health: ' + (data.healthScore * 100).toFixed(0) + '%'));
        }
        
        // Priority badge - lines 57-63
        if (data.priority) {
          children.push(React.createElement('div', {
            key: 'priority',
            className: 'tree-badge',
            style: { marginLeft: '0.5rem' }
          }, data.priority));
        }
        
        // Entity/file count - lines 66-72
        if (data.entityCount) {
          children.push(React.createElement('div', {
            key: 'count',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
          }));
        }
        
        // Average score - lines 75-81
        if (data.avgScore) {
          children.push(React.createElement('div', {
            key: 'score',
            className: 'tree-badge tree-badge-low complexity-score',
            style: { marginLeft: '0.5rem' }
          }));
        }
        
        // POTENTIAL ISSUE: Passing children array directly
        return React.createElement('div', {
          style: {
            display: 'flex',
            alignItems: 'center',
            padding: '0.5rem',
            cursor: 'pointer',
            borderRadius: '4px',
            border: '1px solid transparent',
          }
        }, children); // ← POTENTIAL PROBLEM
      };
      
      const testData = {
        name: 'test.js',
        healthScore: 0.85,
        priority: 'high',
        entityCount: 5,
        avgScore: 7.5
      };
      
      const root = ReactDOM.createRoot(document.getElementById('root'));
      root.render(React.createElement(TreeNodeTest, { data: testData }));
      
      console.log('✅ TreeNode pattern test rendered');
    </script>
</body>
</html>`;

    await page.setContent(testHTML);
    await page.waitForTimeout(1000);

    if (consoleErrors.length > 0) {
      console.log('🎯 ERROR FOUND in TreeNode children.push pattern!');
      consoleErrors.forEach(error => {
        console.log(`❌ ${error.text}`);
      });
    } else {
      console.log('✅ TreeNode children.push pattern is clean');
    }
  });

  test('Test potential boolean expression issue', async ({ page }) => {
    console.log('🎯 Testing potential boolean && expression causing Error #31...');

    const testHTML = `
<!DOCTYPE html>
<html>
<head>
    <script src="https://unpkg.com/react@18/umd/react.development.js"></script>
    <script src="https://unpkg.com/react-dom@18/umd/react-dom.development.js"></script>
</head>
<body>
    <div id="root"></div>
    <script>
      console.log('🧪 Testing boolean expressions...');
      
      // TEST PROBLEMATIC PATTERNS that cause Error #31
      const ProblematicComponent = () => {
        const children = [];
        const data = { priority: 'high', entityCount: 5 };
        
        // PATTERN 1: Boolean result being pushed
        children.push(data.priority && React.createElement('span', { key: 'p1' }, 'Priority'));
        
        // PATTERN 2: Undefined/null being pushed
        children.push(data.nonexistent && React.createElement('span', { key: 'p2' }, 'Missing'));
        
        // PATTERN 3: False values
        children.push(false && React.createElement('span', { key: 'p3' }, 'False'));
        
        // PATTERN 4: Number 0
        children.push(0 && React.createElement('span', { key: 'p4' }, 'Zero'));
        
        console.log('Children array contains:', children.map(c => typeof c));
        
        return React.createElement('div', {}, children);
      };
      
      const root = ReactDOM.createRoot(document.getElementById('root'));
      root.render(React.createElement(ProblematicComponent));
      
      console.log('✅ Boolean expression test rendered');
    </script>
</body>
</html>`;

    await page.setContent(testHTML);
    await page.waitForTimeout(1000);

    if (consoleErrors.length > 0) {
      console.log('🎯 ERROR FOUND in boolean expressions!');
      consoleErrors.forEach(error => {
        console.log(`❌ ${error.text}`);
      });
    } else {
      console.log('✅ Boolean expressions are clean');
    }
  });

  test.afterEach(async ({ page }, testInfo) => {
    await page.screenshot({ 
      path: `debug-isolation-${testInfo.title.replace(/[^a-zA-Z0-9]/g, '-')}.png` 
    });
    
    if (consoleErrors.length > 0) {
      console.log(`\n🎯 ISOLATION TEST: ${testInfo.title}`);
      console.log('💥 ERRORS DETECTED:', consoleErrors.length);
      consoleErrors.forEach((error, i) => {
        console.log(`${i + 1}. ${error.text}`);
        if (error.stack) {
          console.log(`   Stack: ${error.stack.split('\n')[0]}`);
        }
      });
    }
  });
});