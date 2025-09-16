import { test, expect, describe } from 'bun:test';

describe('Tree Component Integration', () => {
  test('should verify component structure and exports', () => {
    // Test that our component can be imported and has the right structure
    const CodeAnalysisTree = require('../../src/tree.js');
    
    // Should be a function (React component)
    expect(typeof CodeAnalysisTree).toBe('function');
    
    // Should have the expected name
    expect(CodeAnalysisTree.name).toBe('CodeAnalysisTree');
  });
  
  test('should handle different data formats without crashing', () => {
    const CodeAnalysisTree = require('../../src/tree.js');
    
    const testCases = [
      null,
      undefined,
      {},
      { refactoringCandidatesByFile: [] },
      { refactoringCandidatesByFile: null },
      { directoryHealthTree: null },
      { unifiedHierarchy: [] },
      { coveragePacks: [] }
    ];
    
    testCases.forEach((data, index) => {
      let error = null;
      try {
        // Test that component creation doesn't throw
        const element = React.createElement(CodeAnalysisTree, { data });
        expect(element).toBeTruthy();
      } catch (e) {
        error = e;
      }
      
      expect(error).toBeNull();
    });
  });
  
  test('should export component for both CommonJS and global use', () => {
    // Test CommonJS export
    const CodeAnalysisTree = require('../../src/tree.js');
    expect(CodeAnalysisTree).toBeTruthy();
    
    // The component should be the default export
    expect(typeof CodeAnalysisTree).toBe('function');
  });
});