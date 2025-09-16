/**
 * Unit tests for the exact malformed data issue
 * Reproducing the bug: valknut generates malformed root nodes missing type/name
 */

import { describe, test, expect } from 'bun:test';

// This is the ACTUAL malformed data structure that valknut produces
const malformedUnifiedHierarchyData = {
  "unifiedHierarchy": [
    {
      // MISSING: type and name fields (the root cause of the bug)
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
          "path": "src/core",
          "health_score": 0.45,
          "healthScore": 0.45,
          "entity_count": 150,
          "entityCount": 150,
          "children": [
            {
              "type": "file",
              "name": "pipeline_config.rs",
              "path": "src/core/pipeline_config.rs",
              "priority": "Critical",
              "avg_score": 12.4,
              "avgScore": 12.4,
              "children": [
                {
                  "type": "entity",
                  "name": "validate_configuration",
                  "entity_id": "src/core/pipeline_config.rs:function:validate_configuration",
                  "priority": "Critical",
                  "score": 15.7,
                  "children": [
                    {
                      "type": "issue",
                      "category": "complexity",
                      "name": "complexity: This entity has very high complexity",
                      "priority": "Critical",
                      "score": 15.7,
                      "children": []
                    },
                    {
                      "type": "suggestion",
                      "name": "extract_method: Reduce complexity",
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

describe('Malformed Data Fix', () => {
  // Copy the transformNode function from the template to test it
  function transformNode(node, parentPath = '') {
    if (!node || typeof node !== 'object') {
      console.warn('⚠️ Invalid node:', node);
      return null;
    }
    
    // Handle malformed root nodes that are missing type/name (backend issue workaround)
    let nodeType = node.type;
    let nodeName = node.name;
    
    if (!nodeType && !nodeName && node.children && Array.isArray(node.children)) {
      // This appears to be a malformed root directory node - infer from structure
      nodeType = 'folder'; // React Arborist expects 'folder', not 'directory'
      
      // Try to infer name from first child's path
      if (node.children.length > 0 && node.children[0].path) {
        const childPath = node.children[0].path;
        const pathParts = childPath.split('/');
        nodeName = pathParts.length > 1 ? pathParts[0] : 'root';
      } else {
        nodeName = parentPath || 'Project Root';
      }
      console.warn('⚠️ Inferring missing type/name for malformed node:', { 
        inferred: { type: nodeType, name: nodeName }, 
        original: node 
      });
    }
    
    // Generate unique ID based on path and type
    const currentPath = parentPath ? `${parentPath}/${nodeName}` : nodeName;
    const id = `${nodeType || 'unknown'}_${(currentPath || 'unknown').replace(/[^a-zA-Z0-9]/g, '_')}_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    
    const transformed = {
      id,
      name: nodeName || 'Unknown',
      type: nodeType || 'unknown',
      isLeaf: !node.children || node.children.length === 0,
      data: {
        // Preserve all original properties
        ...node,
        // Add computed properties
        path: currentPath,
        hasIssues: (node.issue_count || 0) > 0,
        hasSuggestions: (node.suggestion_count || 0) > 0,
        // Format score for display
        displayScore: node.score ? parseFloat(node.score).toFixed(1) : null,
        displayHealth: node.health_score ? parseFloat(node.health_score).toFixed(1) : null
      }
    };
    
    // Recursively transform children
    if (node.children && Array.isArray(node.children) && node.children.length > 0) {
      transformed.children = node.children
        .map(child => transformNode(child, currentPath))
        .filter(child => child !== null);
      
      // Update isLeaf based on actual children after filtering
      transformed.isLeaf = transformed.children.length === 0;
    }
    
    return transformed;
  }
  
  function validateTreeData(data) {
    const errors = [];
    
    if (!Array.isArray(data)) {
      errors.push('Data must be an array');
      return { isValid: false, errors, nodeCount: 0 };
    }
    
    function validateNode(node, path = '') {
      if (!node) {
        errors.push(`Null node at path: ${path}`);
        return;
      }
      
      if (!node.id) {
        errors.push(`Missing id at path: ${path}`);
      }
      
      if (!node.name) {
        errors.push(`Missing name at path: ${path}`);
      }
      
      if (typeof node.isLeaf !== 'boolean') {
        errors.push(`Missing or invalid isLeaf at path: ${path}`);
      }
      
      if (node.children) {
        if (!Array.isArray(node.children)) {
          errors.push(`Children must be array at path: ${path}`);
        } else {
          node.children.forEach((child, index) => {
            validateNode(child, `${path}[${index}]`);
          });
        }
      }
    }
    
    data.forEach((node, index) => {
      validateNode(node, `root[${index}]`);
    });
    
    function countNodes(data) {
      let count = 0;
      function traverse(nodes) {
        if (!Array.isArray(nodes)) return;
        count += nodes.length;
        nodes.forEach(node => {
          if (node.children) {
            traverse(node.children);
          }
        });
      }
      traverse(data);
      return count;
    }
    
    return {
      isValid: errors.length === 0,
      errors,
      nodeCount: countNodes(data)
    };
  }
  
  function transformTreeData(rawData) {
    let unifiedHierarchy;
    
    if (rawData && rawData.unifiedHierarchy) {
      unifiedHierarchy = rawData.unifiedHierarchy;
    } else if (Array.isArray(rawData)) {
      unifiedHierarchy = rawData;
    } else {
      return [];
    }
    
    if (!Array.isArray(unifiedHierarchy)) {
      return [];
    }
    
    // Transform the data to add required properties
    return unifiedHierarchy.map(item => transformNode(item)).filter(item => item !== null);
  }
  
  test('should detect malformed root nodes', () => {
    const data = malformedUnifiedHierarchyData;
    const hierarchyData = data.unifiedHierarchy;
    
    // The root node should be missing type and name
    const rootNode = hierarchyData[0];
    expect(rootNode.type).toBeUndefined();
    expect(rootNode.name).toBeUndefined();
    expect(rootNode.children).toBeDefined();
    expect(Array.isArray(rootNode.children)).toBe(true);
  });
  
  test('should repair malformed root nodes during transformation', () => {
    const transformedData = transformTreeData(malformedUnifiedHierarchyData);
    
    // Should not be empty after transformation
    expect(transformedData.length).toBeGreaterThan(0);
    
    // The transformed root node should have type and name
    const rootNode = transformedData[0];
    expect(rootNode.type).toBeDefined();
    expect(rootNode.name).toBeDefined();
    expect(rootNode.id).toBeDefined();
    
    // Should infer the root name from the first child path
    expect(rootNode.name).toBe('src'); // From "src/core" path in first child
    expect(rootNode.type).toBe('folder');
  });
  
  test('should pass validation after transformation', () => {
    const transformedData = transformTreeData(malformedUnifiedHierarchyData);
    const validation = validateTreeData(transformedData);
    
    expect(validation.isValid).toBe(true);
    expect(validation.errors).toHaveLength(0);
    expect(validation.nodeCount).toBeGreaterThan(0);
  });
  
  test('should preserve all nested data after transformation', () => {
    const transformedData = transformTreeData(malformedUnifiedHierarchyData);
    const rootNode = transformedData[0];
    
    // Should have children
    expect(rootNode.children).toBeDefined();
    expect(rootNode.children.length).toBeGreaterThan(0);
    
    // Check deep nesting is preserved
    const coreFolder = rootNode.children[0];
    expect(coreFolder.type).toBe('folder');
    expect(coreFolder.name).toBe('core');
    
    const file = coreFolder.children[0];
    expect(file.type).toBe('file');
    expect(file.name).toBe('pipeline_config.rs');
    
    const entity = file.children[0];
    expect(entity.type).toBe('entity');
    expect(entity.name).toBe('validate_configuration');
    
    // Should have issues and suggestions
    expect(entity.children.length).toBeGreaterThan(0);
    const hasIssues = entity.children.some(child => child.type === 'issue');
    const hasSuggestions = entity.children.some(child => child.type === 'suggestion');
    expect(hasIssues).toBe(true);
    expect(hasSuggestions).toBe(true);
  });
  
  test('should reproduce the exact bug scenario', () => {
    // This test reproduces the exact bug: counts many nodes but returns empty array
    
    const originalData = malformedUnifiedHierarchyData.unifiedHierarchy;
    
    // Count nodes in original data (this is what validation counts)
    function countOriginalNodes(nodes) {
      let count = 0;
      function traverse(nodes) {
        if (!Array.isArray(nodes)) return;
        count += nodes.length;
        nodes.forEach(node => {
          if (node.children) {
            traverse(node.children);
          }
        });
      }
      traverse(nodes);
      return count;
    }
    
    const originalNodeCount = countOriginalNodes(originalData);
    expect(originalNodeCount).toBeGreaterThan(0); // Should count nodes successfully
    
    // Transform data (this is where nodes get filtered out in the bug)
    const transformedData = transformTreeData(malformedUnifiedHierarchyData);
    
    // The bug would be: transformedData.length === 0 despite originalNodeCount > 0
    // With the fix, this should NOT be empty
    expect(transformedData.length).toBeGreaterThan(0);
    
    const validation = validateTreeData(transformedData);
    expect(validation.isValid).toBe(true);
    expect(validation.nodeCount).toBeGreaterThan(0);
  });
  
  test('should handle edge case with no child paths', () => {
    const malformedWithoutPaths = {
      "unifiedHierarchy": [
        {
          // Missing type and name, AND children have no paths
          "health_score": 0.5,
          "children": [
            {
              "type": "file",
              "name": "test.rs"
              // Missing path field
            }
          ]
        }
      ]
    };
    
    const transformedData = transformTreeData(malformedWithoutPaths);
    expect(transformedData.length).toBeGreaterThan(0);
    
    const rootNode = transformedData[0];
    expect(rootNode.name).toBe('Project Root'); // Fallback name
    expect(rootNode.type).toBe('folder');
  });
});