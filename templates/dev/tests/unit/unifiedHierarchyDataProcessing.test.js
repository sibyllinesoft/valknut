/**
 * Unit tests for unified hierarchy data processing logic
 * Testing the bug: loads nodes but shows "No Refactoring Candidates Found"
 */

import { describe, test, expect } from 'bun:test';

// Test data that reproduces the issue
const mockUnifiedHierarchyData = {
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
              "name": "pipeline_executor.rs",
              "id": "file-pipeline_executor",
              "path": "src/core/pipeline/pipeline_executor.rs",
              "priority": "Critical",
              "avg_score": 12.4,
              "avgScore": 12.4,
              "entity_count": 8,
              "entityCount": 8,
              "children": [
                {
                  "type": "entity", 
                  "name": "evaluate_quality_gates",
                  "id": "entity-evaluate-gates",
                  "entity_id": "src/core/pipeline/pipeline_executor.rs:function:evaluate_quality_gates",
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
                      "type": "suggestion",
                      "id": "suggestion-extract-1",
                      "name": "extract_method: High cognitive complexity (score: 15.7). Reduce mental overhead by extracting complex logic into well-named helper methods",
                      "priority": "High",
                      "refactoring_type": "extract_method",
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

describe('Unified Hierarchy Data Processing', () => {
  test('should correctly identify unified hierarchy format', () => {
    // This tests the data format detection logic
    const data = mockUnifiedHierarchyData;
    
    expect(data.unifiedHierarchy).toBeDefined();
    expect(Array.isArray(data.unifiedHierarchy)).toBe(true);
    expect(data.unifiedHierarchy.length).toBeGreaterThan(0);
  });
  
  test('should extract hierarchy data correctly', () => {
    // This tests the hierarchyData extraction logic
    const data = mockUnifiedHierarchyData;
    const hierarchyData = data.unifiedHierarchy || [];
    
    expect(hierarchyData.length).toBeGreaterThan(0);
    expect(hierarchyData[0]).toHaveProperty('type', 'folder');
    expect(hierarchyData[0]).toHaveProperty('name', 'src');
  });
  
  test('should have valid node structure for tree rendering', () => {
    const hierarchyData = mockUnifiedHierarchyData.unifiedHierarchy;
    
    // Check root node structure
    const rootNode = hierarchyData[0];
    expect(rootNode).toHaveProperty('type');
    expect(rootNode).toHaveProperty('name');
    expect(rootNode).toHaveProperty('id');
    expect(rootNode).toHaveProperty('children');
    
    // Check nested structure
    expect(Array.isArray(rootNode.children)).toBe(true);
    expect(rootNode.children.length).toBeGreaterThan(0);
    
    // Check that we have deep nesting with entities, issues, and suggestions
    const coreFolder = rootNode.children[0];
    expect(coreFolder.type).toBe('folder');
    expect(coreFolder.children.length).toBeGreaterThan(0);
    
    const file = coreFolder.children[0];
    expect(file.type).toBe('file');
    expect(file.children.length).toBeGreaterThan(0);
    
    const entity = file.children[0];
    expect(entity.type).toBe('entity');
    expect(entity.children.length).toBeGreaterThan(0);
    
    // Check that we have both issues and suggestions
    const hasIssues = entity.children.some(child => child.type === 'issue');
    const hasSuggestions = entity.children.some(child => child.type === 'suggestion');
    expect(hasIssues).toBe(true);
    expect(hasSuggestions).toBe(true);
  });
  
  test('should count total nodes correctly', () => {
    // This is the key test - counting nodes to see why it says 3186 nodes loaded
    function countNodes(nodes) {
      let count = 0;
      for (const node of nodes) {
        count += 1; // Count this node
        if (node.children && Array.isArray(node.children)) {
          count += countNodes(node.children); // Recursively count children
        }
      }
      return count;
    }
    
    const totalNodes = countNodes(mockUnifiedHierarchyData.unifiedHierarchy);
    
    // This test data has: 1 root folder + 1 core folder + 1 file + 1 entity + 2 issues/suggestions = 6 nodes
    expect(totalNodes).toBe(6);
    
    // The actual issue: if data loads 3186 nodes but shows empty, 
    // either the data is being filtered out or the tree rendering logic has a bug
  });
  
  test('should validate that tree data would not be empty', () => {
    // This tests the core bug: hierarchyData exists but treeData.length === 0
    const hierarchyData = mockUnifiedHierarchyData.unifiedHierarchy;
    
    // Simulate what happens in the React component
    const treeData = hierarchyData; // This is what setTreeData(hierarchyData) should set
    
    // This should NOT be empty
    expect(treeData.length).toBeGreaterThan(0);
    expect(Array.isArray(treeData)).toBe(true);
    
    // If this test passes but the component still shows empty, 
    // the bug is in the React state management or rendering logic
  });
  
  test('should validate node ID uniqueness', () => {
    // Check that all nodes have unique IDs (required for the virtual tree renderer)
    function collectIds(nodes) {
      const ids = [];
      for (const node of nodes) {
        if (node.id) {
          ids.push(node.id);
        }
        if (node.children && Array.isArray(node.children)) {
          ids.push(...collectIds(node.children));
        }
      }
      return ids;
    }
    
    const allIds = collectIds(mockUnifiedHierarchyData.unifiedHierarchy);
    const uniqueIds = new Set(allIds);
    
    expect(allIds.length).toBe(uniqueIds.size); // No duplicate IDs
    expect(allIds.length).toBeGreaterThan(0); // Has IDs
  });
  
  test('should validate tree data format requirements', () => {
    // The virtual tree expects a consistent shape
    const hierarchyData = mockUnifiedHierarchyData.unifiedHierarchy;
    
    function validateNodeForArborist(node) {
      // Virtual tree requires nodes to have id property
      expect(node).toHaveProperty('id');
      expect(typeof node.id).toBe('string');
      
      // Should have type and name for rendering
      expect(node).toHaveProperty('type');
      expect(node).toHaveProperty('name');
      
      // Children should be array if present
      if (node.children) {
        expect(Array.isArray(node.children)).toBe(true);
        node.children.forEach(child => validateNodeForArborist(child));
      }
    }
    
    hierarchyData.forEach(node => validateNodeForArborist(node));
  });
  
  test('should detect the specific bug scenario', () => {
    // This test specifically reproduces the bug scenario:
    // "Loaded 3186 nodes successfully" but shows "No Refactoring Candidates Found"
    
    const data = mockUnifiedHierarchyData;
    
    // Step 1: Data loading (this works)
    const hierarchyData = data.unifiedHierarchy || [];
    const nodeCount = countNodes(hierarchyData);
    expect(nodeCount).toBeGreaterThan(0); // "Loaded X nodes successfully"
    
    // Step 2: Data processing (this should work)
    const hasUnifiedHierarchy = Boolean(data.unifiedHierarchy);
    expect(hasUnifiedHierarchy).toBe(true); // Should use unifiedHierarchy format
    
    // Step 3: Tree data assignment (this might be the issue)
    const treeData = hasUnifiedHierarchy ? hierarchyData : [];
    expect(treeData.length).toBeGreaterThan(0); // Should NOT be empty
    
    // Step 4: Empty state check (this is where the bug manifests)
    const shouldShowEmpty = treeData.length === 0;
    expect(shouldShowEmpty).toBe(false); // Should NOT show "No Refactoring Candidates Found"
    
    function countNodes(nodes) {
      let count = 0;
      for (const node of nodes) {
        count += 1;
        if (node.children && Array.isArray(node.children)) {
          count += countNodes(node.children);
        }
      }
      return count;
    }
  });
});
