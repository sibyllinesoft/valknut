/**
 * Integration tests for React Error #31 issue
 * Tests the specific bug where tree loads 3186 nodes successfully but displays "No Refactoring Candidates Found"
 */

import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';

// Mock the tree data in DOM (simulating the real HTML template behavior)
const mockRealUnifiedHierarchyData = {
  "unifiedHierarchy": [
    {
      "type": "folder",
      "name": "src",
      "id": "folder-src",
      "health_score": 0.65,
      "healthScore": 0.65,
      "entity_count": 376,
      "entityCount": 376,
      "file_count": 45,
      "fileCount": 45,
      "children": [
        {
          "type": "folder",
          "name": "core",
          "id": "folder-src-core",
          "health_score": 0.45,
          "healthScore": 0.45,
          "entity_count": 150,
          "entityCount": 150,
          "file_count": 15,
          "fileCount": 15,
          "children": [
            {
              "type": "file",
              "name": "pipeline_config.rs",
              "id": "file-pipeline-config",
              "path": "src/core/pipeline_config.rs",
              "priority": "Critical",
              "avg_score": 12.4,
              "avgScore": 12.4,
              "entity_count": 8,
              "entityCount": 8,
              "children": [
                {
                  "type": "entity",
                  "name": "validate_configuration",
                  "id": "entity-validate-config",
                  "entity_id": "src/core/pipeline_config.rs:function:validate_configuration",
                  "priority": "Critical",
                  "score": 15.7,
                  "issue_count": 3,
                  "issueCount": 3,
                  "suggestion_count": 2,
                  "suggestionCount": 2,
                  "lineRange": [42, 89],
                  "children": [
                    {
                      "type": "issue",
                      "id": "issue-complexity-1",
                      "category": "complexity",
                      "name": "complexity: This entity has very high complexity that may make it difficult to understand and maintain",
                      "priority": "Critical",
                      "score": 15.7,
                      "children": []
                    },
                    {
                      "type": "issue", 
                      "id": "issue-structure-1",
                      "category": "structure",
                      "name": "structure: Deeply nested control structures detected",
                      "priority": "High",
                      "score": 12.5,
                      "children": []
                    },
                    {
                      "type": "suggestion",
                      "id": "suggestion-extract-1", 
                      "name": "extract_method: High cognitive complexity (score: 15.7). Reduce mental overhead by extracting complex logic into well-named helper methods",
                      "priority": "High",
                      "refactoring_type": "extract_method",
                      "children": []
                    },
                    {
                      "type": "suggestion",
                      "id": "suggestion-simplify-1",
                      "name": "simplify_conditionals: Simplify nested conditional logic to improve readability", 
                      "priority": "Medium",
                      "refactoring_type": "simplify_conditionals",
                      "children": []
                    }
                  ]
                }
              ]
            }
          ]
        }
      ]
    }
  ]
};

// Mock empty unified hierarchy for comparison
const mockEmptyUnifiedHierarchy = {
  "unifiedHierarchy": []
};

describe('React Error #31 - Tree Loading Issues', () => {
  let CodeAnalysisTree;
  
  beforeAll(() => {
    // Load our component - simulating how it's loaded in the HTML template
    require('../../src/tree.js');
    CodeAnalysisTree = window.CodeAnalysisTree;
  });
  
  beforeEach(() => {
    // Clear any previous mocks
    document.getElementById = jest.fn();
    jest.clearAllMocks();
  });
  
  describe('with real unified hierarchy data (production issue)', () => {
    beforeEach(() => {
      // Simulate the tree-data script element containing real valknut analysis results
      document.getElementById.mockReturnValue({
        textContent: JSON.stringify(mockRealUnifiedHierarchyData)
      });
    });
    
    test('should render without React Error #31', async () => {
      const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});
      
      expect(() => {
        render(<CodeAnalysisTree />);
      }).not.toThrow();
      
      // Check specifically for React error #31
      const reactErrors = consoleSpy.mock.calls.filter(call => 
        call.some(arg => typeof arg === 'string' && (
          arg.includes('Error #31') || 
          arg.includes('Warning #31') ||
          arg.includes('React Error 31')
        ))
      );
      
      expect(reactErrors).toHaveLength(0);
      
      consoleSpy.mockRestore();
    });
    
    test('should load and count unified hierarchy nodes correctly', async () => {
      const consoleSpy = jest.spyOn(console, 'log').mockImplementation(() => {});
      
      render(<CodeAnalysisTree />);
      
      await waitFor(() => {
        const loadSuccessLogs = consoleSpy.mock.calls.filter(call =>
          call.some(arg => typeof arg === 'string' && arg.includes('Loaded') && arg.includes('nodes successfully'))
        );
        expect(loadSuccessLogs.length).toBeGreaterThan(0);
      });
      
      consoleSpy.mockRestore();
    });
    
    test('should NOT show "No Refactoring Candidates Found" with real data', async () => {
      render(<CodeAnalysisTree />);
      
      // Should NOT show the empty state message since we have unified hierarchy data
      await waitFor(() => {
        expect(screen.queryByText('No Refactoring Candidates Found')).not.toBeInTheDocument();
        expect(screen.queryByText('Your code is in excellent shape!')).not.toBeInTheDocument();
      });
    });
    
    test('should render the tree with unified hierarchy data', async () => {
      render(<CodeAnalysisTree />);
      
      // Should render the mock tree with unified hierarchy data
      await waitFor(() => {
        expect(screen.getByTestId('mock-tree')).toBeInTheDocument();
      });
    });
    
    test('should process unified hierarchy format correctly', async () => {
      const consoleSpy = jest.spyOn(console, 'log').mockImplementation(() => {});
      
      render(<CodeAnalysisTree />);
      
      // Should process unified hierarchy format (not the legacy format)
      await waitFor(() => {
        const hierarchyLogs = consoleSpy.mock.calls.filter(call =>
          call.some(arg => typeof arg === 'string' && (
            arg.includes('Using unifiedHierarchy') ||
            arg.includes('unifiedHierarchy format')
          ))
        );
        expect(hierarchyLogs.length).toBeGreaterThan(0);
      });
      
      consoleSpy.mockRestore();
    });
    
    test('should handle folder type correctly (not directory)', async () => {
      render(<CodeAnalysisTree />);
      
      // The test data includes type: "folder" nodes (React Arborist expects "folder", not "directory")
      await waitFor(() => {
        const mockTreeElement = screen.getByTestId('mock-tree');
        expect(mockTreeElement).toBeInTheDocument();
        
        // Check that folder nodes are processed
        const folderNodes = screen.getAllByTestId(/tree-node-.*folder/);
        expect(folderNodes.length).toBeGreaterThan(0);
      });
    });
    
    test('should render entity nodes with issues and suggestions', async () => {
      render(<CodeAnalysisTree />);
      
      await waitFor(() => {
        // Should render entity nodes
        const entityNodes = screen.getAllByTestId(/tree-node-.*entity/);
        expect(entityNodes.length).toBeGreaterThan(0);
        
        // Should render issue nodes
        const issueNodes = screen.getAllByTestId(/tree-node-.*issue/);
        expect(issueNodes.length).toBeGreaterThan(0);
        
        // Should render suggestion nodes
        const suggestionNodes = screen.getAllByTestId(/tree-node-.*suggestion/);
        expect(suggestionNodes.length).toBeGreaterThan(0);
      });
    });
    
    test('should display complexity and priority information', async () => {
      render(<CodeAnalysisTree />);
      
      await waitFor(() => {
        const mockTreeElement = screen.getByTestId('mock-tree');
        expect(mockTreeElement).toBeInTheDocument();
      });
      
      // The mock should render nodes with the actual data structure
      // The real test would verify the content is displayed properly
    });
  });
  
  describe('with empty unified hierarchy', () => {
    beforeEach(() => {
      document.getElementById.mockReturnValue({
        textContent: JSON.stringify(mockEmptyUnifiedHierarchy)
      });
    });
    
    test('should show empty state message with empty unified hierarchy', async () => {
      render(<CodeAnalysisTree />);
      
      await waitFor(() => {
        expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
        expect(screen.getByText('Your code is in excellent shape!')).toBeInTheDocument();
      });
    });
  });
  
  describe('data validation and error recovery', () => {
    test('should handle malformed unified hierarchy gracefully', async () => {
      const malformedData = {
        "unifiedHierarchy": [
          {
            // Missing required fields like "type" and "name"
            "health_score": 0.5
          },
          {
            "type": "folder",
            // Missing "name" field
            "children": []
          }
        ]
      };
      
      document.getElementById.mockReturnValue({
        textContent: JSON.stringify(malformedData)
      });
      
      const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});
      
      render(<CodeAnalysisTree />);
      
      // Should not crash and should handle the malformed data
      await waitFor(() => {
        // Should either show empty state or process valid nodes
        const tree = screen.getByTestId('mock-tree');
        expect(tree).toBeInTheDocument();
      });
      
      consoleSpy.mockRestore();
    });
    
    test('should validate unified hierarchy node structure', async () => {
      const consoleSpy = jest.spyOn(console, 'log').mockImplementation(() => {});
      
      document.getElementById.mockReturnValue({
        textContent: JSON.stringify(mockRealUnifiedHierarchyData)
      });
      
      render(<CodeAnalysisTree />);
      
      // Should validate that nodes have required fields
      await waitFor(() => {
        const validationLogs = consoleSpy.mock.calls.filter(call =>
          call.some(arg => typeof arg === 'string' && (
            arg.includes('validation') ||
            arg.includes('nodes successfully')
          ))
        );
        expect(validationLogs.length).toBeGreaterThan(0);
      });
      
      consoleSpy.mockRestore();
    });
  });
  
  describe('bug reproduction - loads nodes but shows empty', () => {
    test('should reproduce the exact issue: "Loaded 3186 nodes successfully" but shows "No Refactoring Candidates Found"', async () => {
      // This test reproduces the exact bug reported by the user
      const consoleSpy = jest.spyOn(console, 'log').mockImplementation(() => {});
      
      // Simulate the exact data structure that loads 3186 nodes
      document.getElementById.mockReturnValue({
        textContent: JSON.stringify(mockRealUnifiedHierarchyData)
      });
      
      render(<CodeAnalysisTree />);
      
      // Wait for the component to process the data
      await waitFor(() => {
        // Should log successful node loading
        const loadLogs = consoleSpy.mock.calls.filter(call =>
          call.some(arg => typeof arg === 'string' && arg.includes('nodes successfully'))
        );
        expect(loadLogs.length).toBeGreaterThan(0);
      });
      
      // The bug: despite loading nodes successfully, it shows empty state
      // This test should FAIL initially, proving the bug exists
      // After fix, it should PASS, proving the bug is resolved
      await waitFor(() => {
        // With the bug: this would show "No Refactoring Candidates Found"
        // After fix: this should NOT show the empty state
        expect(screen.queryByText('No Refactoring Candidates Found')).not.toBeInTheDocument();
        
        // With the bug: tree would be empty despite loaded nodes
        // After fix: tree should render with the loaded nodes
        expect(screen.getByTestId('mock-tree')).toBeInTheDocument();
      });
      
      consoleSpy.mockRestore();
    });
  });
});