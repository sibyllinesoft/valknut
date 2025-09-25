// Mock data fixtures for valknut analysis results
// These represent the typical structure of data passed to the React tree component

export const sampleEntityWithIssues = {
  name: "./src/core/pipeline/pipeline_executor.rs:function:evaluate_quality_gates",
  priority: "high",
  score: 15.7,
  lineRange: [42, 89],
  issues: [
    {
      category: "complexity",
      description: "Function has very high cyclomatic complexity (score: 15.7)",
      priority: "high",
      severity: 12.5
    },
    {
      category: "structure", 
      description: "Deeply nested control structures detected",
      priority: "medium",
      severity: 8.2
    }
  ],
  suggestions: [
    {
      type: "extract_method",
      description: "Consider extracting validation logic into smaller methods",
      priority: "high",
      impact: 9.0
    },
    {
      type: "reduce_nesting",
      description: "Use early returns to reduce nesting levels", 
      priority: "medium",
      impact: 6.5
    }
  ]
};

export const sampleRefactoringFileGroup = {
  filePath: "src/core/pipeline/pipeline_executor.rs",
  highestPriority: "critical",
  entityCount: 3,
  avgScore: 12.4,
  totalIssues: 7,
  entities: [
    sampleEntityWithIssues,
    {
      name: "./src/core/pipeline/pipeline_executor.rs:function:discover_files",
      priority: "medium", 
      score: 8.2,
      lineRange: [15, 30],
      issues: [
        {
          category: "complexity",
          description: "Moderate complexity detected (score: 8.2)",
          priority: "medium",
          severity: 7.1
        }
      ],
      suggestions: []
    },
    {
      name: "./src/core/pipeline/pipeline_executor.rs:function:should_include_directory",
      priority: "low",
      score: 3.1,
      lineRange: [5, 12],
      issues: [],
      suggestions: []
    }
  ]
};

export const sampleDirectoryHealth = {
  directories: {
    "src": {
      health_score: 0.65,
      file_count: 45,
      entity_count: 150,
      refactoring_needed: true,
      critical_issues: 3,
      high_priority_issues: 12,
      avg_refactoring_score: 8.7
    },
    "src/core": {
      health_score: 0.45,
      file_count: 15,
      entity_count: 60,
      refactoring_needed: true,
      critical_issues: 2,
      high_priority_issues: 8,
      avg_refactoring_score: 12.1
    },
    "src/api": {
      health_score: 0.85,
      file_count: 8,
      entity_count: 25,
      refactoring_needed: false,
      critical_issues: 0,
      high_priority_issues: 1,
      avg_refactoring_score: 4.2
    }
  }
};

export const sampleCoveragePacks = [
  {
    path: "src/core/pipeline/pipeline_executor.rs",
    file_info: {
      coverage_before: 0.72,
      coverage_after_if_filled: 0.85,
      loc: 156
    }
  },
  {
    path: "src/api/engine.rs", 
    file_info: {
      coverage_before: 0.91,
      coverage_after_if_filled: 0.94,
      loc: 89
    }
  }
];

// Complete analysis data structure as passed to the component
export const sampleAnalysisData = {
  refactoringCandidatesByFile: [
    sampleRefactoringFileGroup,
    {
      filePath: "src/api/engine.rs",
      highestPriority: "low",
      entityCount: 2,
      avgScore: 4.1,
      totalIssues: 1,
      entities: [
        {
          name: "./src/api/engine.rs:function:analyze_directory",
          priority: "low",
          score: 5.2,
          lineRange: [45, 78],
          issues: [
            {
              category: "complexity",
              description: "Minor complexity issue (score: 5.2)",
              priority: "low", 
              severity: 4.8
            }
          ],
          suggestions: [
            {
              type: "documentation",
              description: "Add more comprehensive documentation",
              priority: "low",
              impact: 3.0
            }
          ]
        },
        {
          name: "./src/api/engine.rs:function:simple_method",
          priority: "low",
          score: 3.0,
          lineRange: [80, 85],
          issues: [],
          suggestions: []
        }
      ]
    }
  ],
  directoryHealthTree: sampleDirectoryHealth,
  coveragePacks: sampleCoveragePacks
};

// New unified hierarchy format (for testing the newer data format)
export const sampleUnifiedHierarchy = [
  {
    id: "folder-src",
    name: "src",
    type: "folder",
    healthScore: 0.65,
    fileCount: 45,
    entityCount: 150,
    severityCounts: { critical: 3, high: 12, medium: 8, low: 5 },
    children: [
      {
        id: "folder-src-core", 
        name: "core",
        type: "folder",
        healthScore: 0.45,
        fileCount: 15,
        entityCount: 60,
        severityCounts: { critical: 2, high: 8, medium: 5, low: 2 },
        children: [
          {
            id: "file-pipeline_executor",
            name: "pipeline_executor.rs",
            type: "file",
            filePath: "src/core/pipeline/pipeline_executor.rs",
            highestPriority: "critical",
            avgScore: 12.4,
            severityCounts: { critical: 1, high: 2, medium: 1, low: 0 },
            children: [
              {
                id: "entity-evaluate_quality_gates",
                name: "evaluate_quality_gates",
                type: "entity",
                priority: "critical",
                score: 15.7,
                lineRange: [42, 89],
                severityCounts: { critical: 1, high: 1, medium: 1, low: 0 },
                children: [
                  {
                    id: "issue:entity-evaluate_quality_gates:0",
                    name: "complexity: Function has very high cyclomatic complexity (score: 15.7)",
                    type: "issue-row",
                    entityScore: 15.7,
                    issueSeverity: 12.5,
                    children: []
                  },
                  {
                    id: "suggestion:entity-evaluate_quality_gates:0", 
                    name: "extract_method: Consider extracting validation logic into smaller methods",
                    type: "suggestion-row",
                    children: []
                  }
                ]
              }
            ]
          }
        ]
      }
    ]
  }
];

// Test data with no issues (for testing empty states)
export const sampleCleanAnalysisData = {
  refactoringCandidatesByFile: [],
  directoryHealthTree: {
    directories: {
      "src": {
        health_score: 0.95,
        file_count: 10,
        entity_count: 30,
        refactoring_needed: false,
        critical_issues: 0,
        high_priority_issues: 0,
        avg_refactoring_score: 2.1
      }
    }
  },
  coveragePacks: []
};

// Invalid/malformed data for testing error handling
export const sampleInvalidData = {
  refactoringCandidatesByFile: [
    {
      // Missing filePath
      highestPriority: "high",
      entities: []
    },
    {
      filePath: "valid/path.rs",
      entities: [
        {
          // Missing name
          priority: "medium",
          score: "invalid_score" // Invalid score type
        }
      ]
    }
  ]
};
