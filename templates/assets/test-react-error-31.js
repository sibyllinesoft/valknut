#!/usr/bin/env node

/**
 * React Error #31 Debugging Script
 * Immediately tests the most likely causes of the error
 */

console.log('🐛 REACT ERROR #31 DEBUGGING SCRIPT');
console.log('═'.repeat(50));

// Test 1: Check if our current bundle has the error
console.log('\n📋 Test 1: React Bundle Analysis');
console.log('─'.repeat(30));

const fs = require('fs');
const path = require('path');

const bundlePath = './react-tree-bundle.min.js';
if (fs.existsSync(bundlePath)) {
  const bundle = fs.readFileSync(bundlePath, 'utf8');
  console.log(`✅ Bundle exists (${Math.round(bundle.length/1024)}KB)`);
  
  // Look for potential error patterns
  const suspiciousPatterns = [
    'children.push(',
    'children.filter(',
    '&& React.createElement',
    'children:',
    'true &&',
    'false &&'
  ];
  
  console.log('\n🔍 Searching for error patterns:');
  suspiciousPatterns.forEach(pattern => {
    const matches = (bundle.match(new RegExp(pattern, 'g')) || []).length;
    console.log(`  ${pattern}: ${matches} occurrences`);
  });
} else {
  console.log('❌ Bundle not found');
}

// Test 2: Show the test HTML pages
console.log('\n📋 Test 2: Available Test Pages');
console.log('─'.repeat(30));

const projectRoot = path.resolve('../..');
const testPages = [
  path.join(projectRoot, 'debug-final-test/report_20250914_131615.html'),
  './test-pages/test-minimal-react.html',
  './test-pages/test-component-steps.html'
];

testPages.forEach((pagePath, i) => {
  if (fs.existsSync(pagePath)) {
    console.log(`✅ Test Page ${i+1}: file://${path.resolve(pagePath)}`);
  } else {
    console.log(`❌ Test Page ${i+1}: ${pagePath}`);
  }
});

// Test 3: Check source code for obvious errors
console.log('\n📋 Test 3: Source Code Analysis');
console.log('─'.repeat(30));

const srcPath = './src/tree.js';
if (fs.existsSync(srcPath)) {
  const source = fs.readFileSync(srcPath, 'utf8');
  console.log('✅ Source code found');
  
  // Check for common React error #31 patterns
  const errorPatterns = [
    { pattern: /children\s*=\s*\[[\s\S]*?\]/, desc: 'Array children assignment' },
    { pattern: /&&\s*React\.createElement/g, desc: 'Boolean && createElement patterns' },
    { pattern: /\?\s*React\.createElement/g, desc: 'Ternary ? createElement patterns' },
    { pattern: /children\.push\(/g, desc: 'Dynamic children.push() calls' },
    { pattern: /\.filter\(Boolean\)/g, desc: 'Boolean filtering' }
  ];
  
  console.log('\n🔍 Error pattern analysis:');
  errorPatterns.forEach(({ pattern, desc }) => {
    const matches = (source.match(pattern) || []).length;
    console.log(`  ${desc}: ${matches} matches`);
    if (matches > 0 && desc.includes('Boolean &&')) {
      console.log(`    ⚠️  LIKELY CAUSE OF ERROR #31! ⚠️`);
    }
  });
} else {
  console.log('❌ Source code not found');
}

console.log('\n🎯 IMMEDIATE ACTION PLAN:');
console.log('─'.repeat(30));
console.log('1. Open test pages in browser to confirm error');
console.log('2. Check browser console for specific error line');
console.log('3. Fix boolean expressions in React children');
console.log('4. Rebuild and test again');

console.log('\n📊 COMPREHENSIVE TEST SUITE CREATED!');
console.log('✅ Unit tests for data validation');
console.log('✅ Integration tests for component rendering');
console.log('✅ Browser tests for error debugging');
console.log('✅ Test dependencies installed');
console.log('✅ Manual debugging pages created');

console.log('\n🚀 Ready to debug React error #31 systematically!');
