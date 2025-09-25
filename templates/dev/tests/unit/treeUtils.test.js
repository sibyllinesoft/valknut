import { describe, it, expect } from 'bun:test';
import { 
  transformTreeData, 
  validateTreeData, 
  getSeverityLevel, 
  countSeverityLevels, 
  generateNodeId, 
  filterBySeverity 
} from '../../src/tree-component/treeUtils.js';

describe('transformTreeData', () => {
  it('should transform simple data with missing IDs', () => {
    const input = [
      { name: 'folder1', type: 'folder' },
      { name: 'file1.js', type: 'file' }
    ];
    
    const result = transformTreeData(input);
    
    expect(result).toHaveLength(2);
    expect(result[0].id).toBe('folder1');
    expect(result[1].id).toBe('file1_js');
  });

  it('should preserve existing IDs', () => {
    const input = [
      { id: 'custom-id', name: 'folder1', type: 'folder' }
    ];
    
    const result = transformTreeData(input);
    
    expect(result[0].id).toBe('custom-id');
  });

  it('should use entity_id as fallback', () => {
    const input = [
      { entity_id: 'entity-123', name: 'function1', type: 'entity' }
    ];
    
    const result = transformTreeData(input);
    
    expect(result[0].id).toBe('entity-123');
  });

  it('should handle children recursively', () => {
    const input = [
      {
        name: 'src',
        type: 'folder',
        children: [
          { name: 'file1.js', type: 'file' },
          {
            name: 'subfolder',
            type: 'folder',
            children: [
              { name: 'file2.js', type: 'file' }
            ]
          }
        ]
      }
    ];
    
    const result = transformTreeData(input);
    
    expect(result[0].id).toBe('src');
    expect(result[0].children[0].id).toBe('src_file1_js');
    expect(result[0].children[1].id).toBe('src_subfolder');
    expect(result[0].children[1].children[0].id).toBe('src_subfolder_file2_js');
  });

  it('should generate index-based IDs when name is missing', () => {
    const input = [
      { type: 'entity' },
      { type: 'entity' }
    ];
    
    const result = transformTreeData(input);
    
    expect(result[0].id).toBe('node_0');
    expect(result[1].id).toBe('node_1');
  });

  it('should filter out invalid nodes', () => {
    const input = [
      { name: 'valid', type: 'folder' },
      null,
      undefined,
      'invalid string',
      { name: 'also valid', type: 'file' }
    ];
    
    const result = transformTreeData(input);
    
    expect(result).toHaveLength(2);
    expect(result[0].name).toBe('valid');
    expect(result[1].name).toBe('also valid');
  });

  it('should handle empty and invalid input', () => {
    expect(transformTreeData([])).toEqual([]);
    expect(transformTreeData(null)).toEqual([]);
    expect(transformTreeData(undefined)).toEqual([]);
    expect(transformTreeData('invalid')).toEqual([]);
  });
});

describe('validateTreeData', () => {
  it('should validate correct tree data', () => {
    const data = [
      {
        id: 'folder1',
        name: 'src',
        children: [
          { id: 'file1', name: 'file1.js' }
        ]
      }
    ];
    
    const result = validateTreeData(data);
    
    expect(result.isValid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it('should detect missing IDs', () => {
    const data = [
      {
        name: 'src',
        children: [
          { id: 'file1', name: 'file1.js' }
        ]
      }
    ];
    
    const result = validateTreeData(data);
    
    expect(result.isValid).toBe(false);
    expect(result.errors).toContain("Missing 'id' property at data[0]");
  });

  it('should detect invalid nodes', () => {
    const data = [
      null,
      { id: 'valid' }
    ];
    
    const result = validateTreeData(data);
    
    expect(result.isValid).toBe(false);
    expect(result.errors).toContain("Invalid node at data[0]: object");
  });

  it('should validate children recursively', () => {
    const data = [
      {
        id: 'folder1',
        children: [
          { name: 'missing-id' }
        ]
      }
    ];
    
    const result = validateTreeData(data);
    
    expect(result.isValid).toBe(false);
    expect(result.errors).toContain("Missing 'id' property at data[0].children[0]");
  });

  it('should reject non-array input', () => {
    const result = validateTreeData('invalid');
    
    expect(result.isValid).toBe(false);
    expect(result.errors).toContain('Tree data must be an array');
  });
});

describe('getSeverityLevel', () => {
  it('should parse string priorities correctly', () => {
    expect(getSeverityLevel('critical')).toBe('critical');
    expect(getSeverityLevel('Critical Issue')).toBe('critical');
    expect(getSeverityLevel('high')).toBe('high');
    expect(getSeverityLevel('High Priority')).toBe('high');
    expect(getSeverityLevel('medium')).toBe('medium');
    expect(getSeverityLevel('moderate')).toBe('medium');
    expect(getSeverityLevel('low')).toBe('low');
    expect(getSeverityLevel('Low Priority')).toBe('low');
  });

  it('should parse numeric severity correctly', () => {
    expect(getSeverityLevel(null, 20)).toBe('critical');
    expect(getSeverityLevel(null, 15)).toBe('critical');
    expect(getSeverityLevel(null, 12)).toBe('high');
    expect(getSeverityLevel(null, 10)).toBe('high');
    expect(getSeverityLevel(null, 7)).toBe('medium');
    expect(getSeverityLevel(null, 5)).toBe('medium');
    expect(getSeverityLevel(null, 3)).toBe('low');
    expect(getSeverityLevel(null, 0)).toBe('low');
  });

  it('should prioritize string priority over numeric severity', () => {
    expect(getSeverityLevel('critical', 5)).toBe('critical');
    expect(getSeverityLevel('low', 15)).toBe('low');
  });

  it('should handle edge cases', () => {
    expect(getSeverityLevel()).toBe('low');
    expect(getSeverityLevel(null, null)).toBe('low');
    expect(getSeverityLevel(undefined, undefined)).toBe('low');
    expect(getSeverityLevel('unknown', 'invalid')).toBe('low');
  });
});

describe('countSeverityLevels', () => {
  it('should count severity levels correctly', () => {
    const items = [
      { priority: 'critical', severity: 15 },
      { priority: 'high', severity: 12 },
      { priority: 'high', severity: 11 },
      { priority: 'medium', impact: 7 },
      { priority: 'low', impact: 3 }
    ];
    
    const result = countSeverityLevels(items);
    
    expect(result.critical).toBe(1);
    expect(result.high).toBe(2);
    expect(result.medium).toBe(1);
    expect(result.low).toBe(1);
  });

  it('should handle empty and invalid input', () => {
    expect(countSeverityLevels([])).toEqual({ critical: 0, high: 0, medium: 0, low: 0 });
    expect(countSeverityLevels(null)).toEqual({ critical: 0, high: 0, medium: 0, low: 0 });
    expect(countSeverityLevels(undefined)).toEqual({ critical: 0, high: 0, medium: 0, low: 0 });
  });

  it('should use impact as fallback for severity', () => {
    const items = [
      { priority: 'medium', impact: 8 }
    ];
    
    const result = countSeverityLevels(items);
    
    expect(result.medium).toBe(1);
  });
});

describe('generateNodeId', () => {
  it('should preserve existing ID', () => {
    const node = { id: 'existing-id', name: 'test' };
    expect(generateNodeId(node)).toBe('existing-id');
  });

  it('should use entity_id as fallback', () => {
    const node = { entity_id: 'entity-123', name: 'test' };
    expect(generateNodeId(node)).toBe('entity-123');
  });

  it('should generate ID from name', () => {
    const node = { name: 'My Function Name' };
    expect(generateNodeId(node)).toBe('My_Function_Name');
  });

  it('should include parent ID in generated ID', () => {
    const node = { name: 'child' };
    expect(generateNodeId(node, 'parent', 0)).toBe('parent_child');
  });

  it('should use index-based ID as last resort', () => {
    const node = { type: 'entity' };
    expect(generateNodeId(node, 'parent', 5)).toBe('parent_node_5');
    expect(generateNodeId(node, '', 3)).toBe('node_3');
  });

  it('should sanitize special characters', () => {
    const node = { name: 'function::with@special#chars!' };
    expect(generateNodeId(node)).toBe('function__with_special_chars_');
  });
});

describe('filterBySeverity', () => {
  const sampleData = [
    {
      id: 'folder1',
      name: 'src',
      type: 'folder',
      severityCounts: { critical: 1, high: 2, medium: 0, low: 0 },
      children: [
        {
          id: 'file1',
          name: 'file1.js',
          type: 'file',
          priority: 'high',
          children: []
        },
        {
          id: 'file2',
          name: 'file2.js',
          type: 'file',
          priority: 'low',
          children: []
        }
      ]
    },
    {
      id: 'folder2',
      name: 'tests',
      type: 'folder',
      severityCounts: { critical: 0, high: 0, medium: 1, low: 2 },
      children: []
    }
  ];

  it('should filter by single severity level', () => {
    const result = filterBySeverity(sampleData, ['high']);
    
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('folder1');
    expect(result[0].children).toHaveLength(1);
    expect(result[0].children[0].id).toBe('file1');
  });

  it('should filter by multiple severity levels', () => {
    const result = filterBySeverity(sampleData, ['critical', 'high']);
    
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('folder1');
  });

  it('should include parents with matching children', () => {
    const result = filterBySeverity(sampleData, ['medium']);
    
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('folder2');
  });

  it('should handle empty severity list', () => {
    const result = filterBySeverity(sampleData, []);
    
    expect(result).toEqual(sampleData);
  });

  it('should handle invalid input', () => {
    expect(filterBySeverity(null, ['high'])).toBe(null);
    expect(filterBySeverity(sampleData, null)).toEqual(sampleData);
    expect(filterBySeverity([], ['high'])).toEqual([]);
  });

  it('should filter out nodes with no matching severity', () => {
    const result = filterBySeverity(sampleData, ['critical']);
    
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('folder1');
    expect(result[0].children).toHaveLength(0); // high priority file filtered out
  });
});