import '@testing-library/jest-dom';

// Mock React Arborist since it has complex dependencies
jest.mock('react-arborist', () => ({
  Tree: ({ children, data, ...props }) => {
    if (!data || data.length === 0) {
      return React.createElement('div', { 'data-testid': 'empty-tree' }, 'No data');
    }
    return React.createElement('div', { 'data-testid': 'mock-tree', ...props }, 
      data.map((item, index) => 
        React.createElement('div', { key: item.id || index }, 
          children ? children({ node: { data: item }, style: {}, dragHandle: null, tree: { toggle: () => {} } }) : item.name
        )
      )
    );
  }
}));

// Global test data
window.testData = {
  validTreeData: {
    "refactoringCandidatesByFile": [
      {
        "fileName": "test.rs",
        "filePath": "src/test.rs", 
        "highestPriority": "High",
        "entityCount": 2,
        "avgScore": 25.5,
        "totalIssues": 4,
        "entities": [
          {
            "name": "test_function",
            "priority": "High", 
            "score": 25,
            "lineRange": [10, 20],
            "issues": [],
            "suggestions": [{"type": "refactor", "description": "Extract method", "score": null}]
          }
        ]
      }
    ],
    "directoryHealthTree": {
      "directories": {
        "src": {"health_score": 0.75, "file_count": 1, "entity_count": 2}
      }
    }
  },
  emptyTreeData: {
    "refactoringCandidatesByFile": [],
    "directoryHealthTree": null
  }
};
