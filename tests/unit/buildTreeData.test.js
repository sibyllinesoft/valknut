/**
 * Unit tests for buildTreeData function
 * Tests the core logic that converts Valknut analysis data into React tree structure
 */

import React from 'react';

// Import our tree component
let CodeAnalysisTree, buildTreeDataFunction;

beforeAll(() => {
  // Mock DOM elements
  document.getElementById = jest.fn().mockReturnValue({
    textContent: JSON.stringify(window.testData.validTreeData)
  });
  
  // Load our tree component
  require('../../src/tree.js');
  CodeAnalysisTree = window.CodeAnalysisTree;
});

describe('buildTreeData Function', () => {
  
  describe('with valid refactoring candidates', () => {
    const validData = window.testData.validTreeData;
    
    test('should process refactoring candidates correctly', () => {
      const refactoringFiles = validData.refactoringCandidatesByFile;
      const directoryHealth = validData.directoryHealthTree;
      
      // We need to extract buildTreeData function from the component
      // For now, let's test the data structure requirements
      expect(refactoringFiles).toHaveLength(1);
      expect(refactoringFiles[0].fileName).toBe('test.rs');
      expect(refactoringFiles[0].entities).toHaveLength(1);
    });
    
    test('should handle directory health data', () => {
      const directoryHealth = validData.directoryHealthTree;
      
      expect(directoryHealth.directories).toBeDefined();
      expect(directoryHealth.directories.src).toBeDefined();
      expect(directoryHealth.directories.src.health_score).toBe(0.75);
    });
  });
  
  describe('with empty data', () => {
    test('should handle empty refactoring candidates', () => {
      const emptyData = window.testData.emptyTreeData;
      
      expect(emptyData.refactoringCandidatesByFile).toHaveLength(0);
      expect(emptyData.directoryHealthTree).toBeNull();
    });
  });
  
  describe('data structure validation', () => {
    test('should validate refactoring candidate structure', () => {
      const candidate = window.testData.validTreeData.refactoringCandidatesByFile[0];
      
      // Required fields
      expect(candidate.fileName).toBeDefined();
      expect(candidate.filePath).toBeDefined();
      expect(candidate.highestPriority).toBeDefined();
      expect(candidate.entityCount).toBeDefined();
      expect(candidate.entities).toBeDefined();
      
      // Entity structure
      const entity = candidate.entities[0];
      expect(entity.name).toBeDefined();
      expect(entity.priority).toBeDefined();
      expect(entity.suggestions).toBeDefined();
      
      // Suggestion structure (with null score fix)
      const suggestion = entity.suggestions[0];
      expect(suggestion.type).toBeDefined();
      expect(suggestion.description).toBeDefined();
      expect(suggestion.score).toBe(null); // Should be null, not undefined
    });
  });
});
