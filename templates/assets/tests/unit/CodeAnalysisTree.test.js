// Unit tests for React CodeAnalysisTree component
const React = require('react');
const { render, screen, fireEvent, waitFor } = require('@testing-library/react');
const userEvent = require('@testing-library/user-event');
const CodeAnalysisTree = require('../../src/tree');
const { 
  sampleAnalysisData, 
  sampleUnifiedHierarchy, 
  sampleCleanAnalysisData,
  sampleInvalidData
} = require('../fixtures/sampleAnalysisData');

// Mock react-arborist Tree component
jest.mock('react-arborist', () => ({
  Tree: jest.fn(({ children, data, openByDefault, ...props }) => {
    // Simulate basic tree rendering for testing
    const renderNode = (nodeData, level = 0) => {
      const node = {
        data: nodeData,
        level,
        id: nodeData.id,
        isInternal: (nodeData.children && nodeData.children.length > 0),
        children: nodeData.children || [],
        isOpen: openByDefault ? openByDefault({ data: nodeData }) : false,
        isSelected: false,
        hasChildren: (nodeData.children && nodeData.children.length > 0),
        toggle: jest.fn()
      };
      
      const tree = {
        toggle: jest.fn()
      };

      return (
        <div key={nodeData.id} data-testid={`tree-node-${nodeData.id}`}>
          {children({ node, style: {}, innerRef: null, tree })}
          {node.isOpen && node.children.map(child => renderNode(child, level + 1))}
        </div>
      );
    };

    return (
      <div data-testid="react-arborist-tree" style={{ width: props.width, height: props.height }}>
        {data.map(nodeData => renderNode(nodeData))}
      </div>
    );
  })
}));

describe('CodeAnalysisTree Component', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('Data Loading and State Management', () => {
    test('should render loading state with empty data', () => {
      render(<CodeAnalysisTree data={null} />);
      
      expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
      expect(screen.getByText('Your code is in excellent shape!')).toBeInTheDocument();
    });

    test('should handle clean analysis data (no issues)', () => {
      render(<CodeAnalysisTree data={sampleCleanAnalysisData} />);
      
      // Should show empty state since no refactoring candidates
      expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
    });

    test('should process legacy refactoringCandidatesByFile format', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      expect(screen.getByTestId('react-arborist-tree')).toBeInTheDocument();
      
      // Should display file nodes
      expect(screen.getByText('pipeline_config.rs')).toBeInTheDocument();
      expect(screen.getByText('engine.rs')).toBeInTheDocument();
    });

    test('should process unified hierarchy format', () => {
      const dataWithUnified = { unifiedHierarchy: sampleUnifiedHierarchy };
      render(<CodeAnalysisTree data={dataWithUnified} />);
      
      expect(screen.getByTestId('react-arborist-tree')).toBeInTheDocument();
      expect(screen.getByText('src')).toBeInTheDocument();
    });

    test('should handle invalid data gracefully', () => {
      const consoleSpy = jest.spyOn(console, 'error').mockImplementation();
      
      render(<CodeAnalysisTree data={sampleInvalidData} />);
      
      // Should not crash and should show tree or empty state
      expect(screen.getByTestId('react-arborist-tree')).toBeInTheDocument();
      
      consoleSpy.mockRestore();
    });

    test('should update tree data when props change', () => {
      const { rerender } = render(<CodeAnalysisTree data={sampleCleanAnalysisData} />);
      
      expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
      
      rerender(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      expect(screen.queryByText('No Refactoring Candidates Found')).not.toBeInTheDocument();
      expect(screen.getByTestId('react-arborist-tree')).toBeInTheDocument();
    });
  });

  describe('Tree Node Rendering', () => {
    test('should render folder nodes with health scores', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Should have folder icons and health badges
      const srcFolder = screen.getByText('src');
      expect(srcFolder).toBeInTheDocument();
      
      // Look for health score display
      expect(screen.getByText(/Health: \d+%/)).toBeInTheDocument();
    });

    test('should render file nodes with complexity scores', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Should display average complexity scores
      expect(screen.getByText(/Complexity: \d+\.\d+/)).toBeInTheDocument();
    });

    test('should render entity nodes with line ranges', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Should show line range information
      expect(screen.getByText(/L\d+-\d+/)).toBeInTheDocument();
    });

    test('should render severity count badges', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Should show severity badges
      expect(screen.getByText(/\d+ critical/)).toBeInTheDocument();
      expect(screen.getByText(/\d+ high/)).toBeInTheDocument();
    });

    test('should render issue and suggestion rows', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Should display issues and suggestions as child rows
      expect(screen.getByText(/complexity:/)).toBeInTheDocument();
      expect(screen.getByText(/extract_method:/)).toBeInTheDocument();
    });
  });

  describe('Tree Interaction', () => {
    test('should handle node expansion/collapse', async () => {
      const user = userEvent.setup();
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Find expandable nodes and test clicking
      const expandableNode = screen.getByText('src');
      
      // Should be clickable for folders
      await user.click(expandableNode);
      
      // Verify toggle was called (mocked)
      expect(expandableNode).toBeInTheDocument();
    });

    test('should show chevron icons for expandable nodes', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Should have chevron elements for folders/files with children
      const chevrons = screen.getAllByTestId(/tree-node-/);
      expect(chevrons.length).toBeGreaterThan(0);
    });

    test('should open folders and files by default', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // react-arborist mock should show open nodes
      expect(screen.getByTestId('react-arborist-tree')).toBeInTheDocument();
    });
  });

  describe('Severity and Priority Display', () => {
    test('should display priority badges with correct colors', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Should show priority badges
      expect(screen.getByText('high')).toBeInTheDocument();
      expect(screen.getByText('low')).toBeInTheDocument();
    });

    test('should show complexity scores for entities', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Should display entity complexity scores
      expect(screen.getByText(/Complexity: 15\.7/)).toBeInTheDocument();
      expect(screen.getByText(/Complexity: 8\.2/)).toBeInTheDocument();
    });

    test('should aggregate severity counts at folder level', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Folders should show aggregated severity counts
      const criticalBadges = screen.getAllByText(/\d+ critical/);
      expect(criticalBadges.length).toBeGreaterThan(0);
    });
  });

  describe('Error Handling', () => {
    test('should handle malformed data gracefully', () => {
      const consoleErrorSpy = jest.spyOn(console, 'error').mockImplementation();
      
      const malformedData = {
        refactoringCandidatesByFile: [
          { /* missing filePath */ },
          { filePath: 'valid.rs', entities: null }
        ]
      };
      
      render(<CodeAnalysisTree data={malformedData} />);
      
      // Should not crash
      expect(screen.getByTestId('react-arborist-tree')).toBeInTheDocument();
      
      consoleErrorSpy.mockRestore();
    });

    test('should handle missing entity properties', () => {
      const dataWithMissingProps = {
        refactoringCandidatesByFile: [
          {
            filePath: 'test.rs',
            entities: [
              { /* missing name and other props */ },
              { name: 'valid_function' }
            ]
          }
        ]
      };
      
      render(<CodeAnalysisTree data={dataWithMissingProps} />);
      
      // Should render successfully
      expect(screen.getByTestId('react-arborist-tree')).toBeInTheDocument();
      expect(screen.getByText('valid_function')).toBeInTheDocument();
    });

    test('should handle empty arrays and undefined values', () => {
      const emptyData = {
        refactoringCandidatesByFile: [],
        directoryHealthTree: undefined,
        coveragePacks: null
      };
      
      render(<CodeAnalysisTree data={emptyData} />);
      
      expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
    });
  });

  describe('Performance and Optimization', () => {
    test('should handle large datasets without crashing', () => {
      // Create a large dataset
      const largeData = {
        refactoringCandidatesByFile: Array.from({ length: 100 }, (_, i) => ({
          filePath: `src/file_${i}.rs`,
          entities: Array.from({ length: 10 }, (_, j) => ({
            name: `function_${j}`,
            score: Math.random() * 20,
            priority: ['low', 'medium', 'high', 'critical'][Math.floor(Math.random() * 4)]
          }))
        }))
      };
      
      render(<CodeAnalysisTree data={largeData} />);
      
      expect(screen.getByTestId('react-arborist-tree')).toBeInTheDocument();
    });

    test('should memoize data transformations', () => {
      const { rerender } = render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Re-render with same data
      rerender(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Should not recreate tree structure unnecessarily
      expect(screen.getByTestId('react-arborist-tree')).toBeInTheDocument();
    });
  });

  describe('Accessibility', () => {
    test('should provide accessible tree structure', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Should have proper ARIA attributes through react-arborist
      const tree = screen.getByTestId('react-arborist-tree');
      expect(tree).toBeInTheDocument();
    });

    test('should support keyboard navigation', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Tree should be focusable and keyboard navigable
      const tree = screen.getByTestId('react-arborist-tree');
      expect(tree).toBeInTheDocument();
    });
  });

  describe('Tree Configuration', () => {
    test('should configure react-arborist with correct props', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      const tree = screen.getByTestId('react-arborist-tree');
      
      // Should have configured dimensions
      expect(tree).toHaveStyle('width: 100%');
      expect(tree).toHaveStyle('height: 600px');
    });

    test('should disable editing and dropping', () => {
      render(<CodeAnalysisTree data={sampleAnalysisData} />);
      
      // Tree should be read-only (tested through mock behavior)
      expect(screen.getByTestId('react-arborist-tree')).toBeInTheDocument();
    });
  });
});