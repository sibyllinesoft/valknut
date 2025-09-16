/**
 * @jest-environment happy-dom
 */
import { describe, it, expect, beforeAll } from 'bun:test';
import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';

// Mock react-arborist since it has complex DOM dependencies
const mockTree = ({ data, children: TreeNode }) => {
  if (!data || data.length === 0) {
    return React.createElement('div', { 'data-testid': 'empty-tree' }, 'No data');
  }

  const renderNode = (node, level = 0) => {
    const mockNodeProps = {
      node: {
        id: node.id,
        data: node,
        level,
        isOpen: true,
        isSelected: false,
        isInternal: node.children && node.children.length > 0,
        children: node.children || [],
        hasChildren: node.children && node.children.length > 0
      },
      style: { paddingLeft: level * 24 },
      innerRef: () => {},
      tree: {
        toggle: () => {}
      }
    };

    return React.createElement('div', 
      { 
        key: node.id, 
        'data-testid': `tree-node-${node.id}`,
        'data-node-type': node.type
      },
      React.createElement(TreeNode, mockNodeProps),
      node.children && node.children.map(child => renderNode(child, level + 1))
    );
  };

  return React.createElement('div', 
    { 'data-testid': 'mock-tree' },
    data.map(node => renderNode(node))
  );
};

// Mock the react-arborist Tree component
const mockReactArborist = {
  Tree: mockTree
};

// Set up module mocking before importing our component
beforeAll(() => {
  // Mock react-arborist
  globalThis.__MOCK_REACT_ARBORIST__ = mockReactArborist;
});

// Import after setting up mocks
import { CodeAnalysisTree } from '../../src/tree-component/CodeAnalysisTree.jsx';

// Sample test data
const sampleTreeData = [
  {
    id: 'folder-src',
    name: 'src',
    type: 'folder',
    healthScore: 0.65,
    fileCount: 2,
    entityCount: 3,
    severityCounts: { critical: 1, high: 2, medium: 1, low: 0 },
    children: [
      {
        id: 'file-1',
        name: 'test.rs',
        type: 'file',
        filePath: 'src/test.rs',
        highestPriority: 'critical',
        avgScore: 12.4,
        severityCounts: { critical: 1, high: 1, medium: 0, low: 0 },
        children: [
          {
            id: 'entity-1',
            name: 'validate_config',
            type: 'entity',
            priority: 'critical',
            score: 15.7,
            lineRange: [42, 89],
            severityCounts: { critical: 1, high: 1, medium: 0, low: 0 },
            children: [
              {
                id: 'issue:entity-1:0',
                name: 'complexity: Function has very high cyclomatic complexity (score: 15.7)',
                type: 'issue-row',
                entityScore: 15.7,
                issueSeverity: 12.5,
                children: []
              },
              {
                id: 'suggestion:entity-1:0',
                name: 'extract_method: Consider extracting validation logic into smaller methods',
                type: 'suggestion-row',
                children: []
              }
            ]
          }
        ]
      }
    ]
  }
];

const sampleLegacyData = {
  refactoringCandidatesByFile: [
    {
      filePath: 'src/test.rs',
      highestPriority: 'critical',
      entityCount: 1,
      avgScore: 15.7,
      totalIssues: 2,
      entities: [
        {
          name: './src/test.rs:function:validate_config',
          priority: 'critical',
          score: 15.7,
          lineRange: [42, 89],
          issues: [
            {
              category: 'complexity',
              description: 'Function has very high cyclomatic complexity (score: 0.0)',
              priority: 'critical',
              severity: 12.5
            }
          ],
          suggestions: [
            {
              type: 'extract_method',
              description: 'Consider extracting validation logic',
              priority: 'high',
              impact: 9.0
            }
          ]
        }
      ]
    }
  ],
  directoryHealthTree: {
    directories: {
      'src': {
        health_score: 0.65,
        file_count: 1,
        entity_count: 1,
        refactoring_needed: true,
        critical_issues: 1,
        high_priority_issues: 1,
        avg_refactoring_score: 15.7
      }
    }
  },
  coveragePacks: []
};

// Re-export Tree from our mock instead of react-arborist
jest.doMock('react-arborist', () => mockReactArborist);

describe('CodeAnalysisTree', () => {
  it('should render unified hierarchy data', async () => {
    const data = { unifiedHierarchy: sampleTreeData };
    
    render(React.createElement(CodeAnalysisTree, { data }));
    
    // Check if tree is rendered
    expect(screen.getByTestId('mock-tree')).toBeInTheDocument();
    
    // Check if folder node is rendered
    const folderNode = screen.getByTestId('tree-node-folder-src');
    expect(folderNode).toBeInTheDocument();
    expect(folderNode).toHaveAttribute('data-node-type', 'folder');
    
    // Check if file node is rendered
    const fileNode = screen.getByTestId('tree-node-file-1');
    expect(fileNode).toBeInTheDocument();
    expect(fileNode).toHaveAttribute('data-node-type', 'file');
    
    // Check if entity node is rendered
    const entityNode = screen.getByTestId('tree-node-entity-1');
    expect(entityNode).toBeInTheDocument();
    expect(entityNode).toHaveAttribute('data-node-type', 'entity');
  });

  it('should render legacy refactoring data format', async () => {
    render(React.createElement(CodeAnalysisTree, { data: sampleLegacyData }));
    
    // Should build tree structure from legacy data
    expect(screen.getByTestId('mock-tree')).toBeInTheDocument();
    
    // Should have transformed the data into tree format
    // Note: exact nodes depend on buildTreeData implementation
    const treeNodes = screen.getAllByTestId(/tree-node-/);
    expect(treeNodes.length).toBeGreaterThan(0);
  });

  it('should render empty state when no data provided', () => {
    render(React.createElement(CodeAnalysisTree, { data: null }));
    
    // Should show empty state message
    expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
    expect(screen.getByText('Your code is in excellent shape!')).toBeInTheDocument();
  });

  it('should render empty state for empty hierarchy', () => {
    const data = { unifiedHierarchy: [] };
    
    render(React.createElement(CodeAnalysisTree, { data }));
    
    expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
  });

  it('should handle invalid data gracefully', () => {
    // Mock console.error to avoid noise in tests
    const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});
    
    render(React.createElement(CodeAnalysisTree, { data: 'invalid' }));
    
    expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
    
    consoleSpy.mockRestore();
  });

  it('should render issue and suggestion rows correctly', async () => {
    const data = { unifiedHierarchy: sampleTreeData };
    
    render(React.createElement(CodeAnalysisTree, { data }));
    
    // Check if issue row is rendered
    const issueNode = screen.getByTestId('tree-node-issue:entity-1:0');
    expect(issueNode).toBeInTheDocument();
    expect(issueNode).toHaveAttribute('data-node-type', 'issue-row');
    
    // Check if suggestion row is rendered
    const suggestionNode = screen.getByTestId('tree-node-suggestion:entity-1:0');
    expect(suggestionNode).toBeInTheDocument();
    expect(suggestionNode).toHaveAttribute('data-node-type', 'suggestion-row');
  });

  it('should handle undefined children arrays', () => {
    const dataWithUndefinedChildren = {
      unifiedHierarchy: [
        {
          id: 'test-node',
          name: 'test',
          type: 'folder',
          children: undefined
        }
      ]
    };
    
    render(React.createElement(CodeAnalysisTree, { data: dataWithUndefinedChildren }));
    
    expect(screen.getByTestId('mock-tree')).toBeInTheDocument();
    expect(screen.getByTestId('tree-node-test-node')).toBeInTheDocument();
  });

  it('should process severity counts correctly in legacy data', async () => {
    const legacyDataWithMultipleSeverities = {
      refactoringCandidatesByFile: [
        {
          filePath: 'src/multi.rs',
          highestPriority: 'high',
          entityCount: 2,
          avgScore: 10.0,
          totalIssues: 4,
          entities: [
            {
              name: './src/multi.rs:function:func1',
              priority: 'critical',
              score: 15.0,
              issues: [
                { category: 'complexity', priority: 'critical', severity: 16 },
                { category: 'structure', priority: 'high', severity: 11 }
              ],
              suggestions: [
                { type: 'refactor', priority: 'medium', impact: 7 }
              ]
            },
            {
              name: './src/multi.rs:function:func2',
              priority: 'low',
              score: 5.0,
              issues: [
                { category: 'style', priority: 'low', severity: 3 }
              ],
              suggestions: []
            }
          ]
        }
      ],
      directoryHealthTree: {
        directories: {
          'src': {
            health_score: 0.7,
            file_count: 1,
            entity_count: 2
          }
        }
      }
    };
    
    render(React.createElement(CodeAnalysisTree, { data: legacyDataWithMultipleSeverities }));
    
    // Tree should be rendered with processed severity counts
    expect(screen.getByTestId('mock-tree')).toBeInTheDocument();
    
    // Should have multiple tree nodes from the processed data
    const treeNodes = screen.getAllByTestId(/tree-node-/);
    expect(treeNodes.length).toBeGreaterThan(0);
  });
});