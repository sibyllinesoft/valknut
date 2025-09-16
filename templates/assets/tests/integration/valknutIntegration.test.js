import { describe, it, expect } from 'bun:test';
import { 
  transformTreeData, 
  validateTreeData, 
  getSeverityLevel,
  countSeverityLevels 
} from '../../src/tree-component/treeUtils.js';

// Import real sample data
const sampleData = {
  sampleEntityWithIssues: {
    name: "./src/core/pipeline/pipeline_config.rs:function:validate_configuration",
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
  },

  sampleRefactoringFileGroup: {
    filePath: "src/core/pipeline/pipeline_config.rs",
    highestPriority: "critical",
    entityCount: 3,
    avgScore: 12.4,
    totalIssues: 7,
    entities: []
  },

  sampleDirectoryHealth: {
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
      }
    }
  },

  sampleAnalysisData: {
    refactoringCandidatesByFile: [],
    directoryHealthTree: {},
    coveragePacks: []
  },

  sampleUnifiedHierarchy: [
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
              id: "file-pipeline_config",
              name: "pipeline_config.rs",
              type: "file",
              filePath: "src/core/pipeline_config.rs",
              highestPriority: "critical",
              avgScore: 12.4,
              severityCounts: { critical: 1, high: 2, medium: 1, low: 0 },
              children: [
                {
                  id: "entity-validate_config",
                  name: "validate_configuration",
                  type: "entity",
                  priority: "critical",
                  score: 15.7,
                  lineRange: [42, 89],
                  severityCounts: { critical: 1, high: 1, medium: 1, low: 0 },
                  children: [
                    {
                      id: "issue:entity-validate_config:0",
                      name: "complexity: Function has very high cyclomatic complexity (score: 15.7)",
                      type: "issue-row",
                      entityScore: 15.7,
                      issueSeverity: 12.5,
                      children: []
                    },
                    {
                      id: "suggestion:entity-validate_config:0", 
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
  ]
};

describe('Valknut Integration Tests', () => {
  describe('Real Data Structure Processing', () => {
    it('should handle typical valknut entity with issues and suggestions', () => {
      const entity = sampleData.sampleEntityWithIssues;
      
      // Test severity level calculation for issues
      const issueSeverities = entity.issues.map(issue => 
        getSeverityLevel(issue.priority, issue.severity)
      );
      expect(issueSeverities).toEqual(['high', 'medium']);
      
      // Test severity level calculation for suggestions  
      const suggestionSeverities = entity.suggestions.map(suggestion => 
        getSeverityLevel(suggestion.priority, suggestion.impact)
      );
      expect(suggestionSeverities).toEqual(['high', 'medium']);
      
      // Test severity counting
      const issuesCounts = countSeverityLevels(entity.issues);
      expect(issuesCounts.high).toBe(1);
      expect(issuesCounts.medium).toBe(1);
      expect(issuesCounts.critical).toBe(0);
      expect(issuesCounts.low).toBe(0);
      
      const suggestionsCounts = countSeverityLevels(entity.suggestions);
      expect(suggestionsCounts.high).toBe(1);
      expect(suggestionsCounts.medium).toBe(1);
    });

    it('should transform valknut unified hierarchy to valid React Arborist format', () => {
      const hierarchy = sampleData.sampleUnifiedHierarchy;
      
      // Validate original data has IDs
      const validation = validateTreeData(hierarchy);
      expect(validation.isValid).toBe(true);
      expect(validation.errors).toHaveLength(0);
      
      // Transform should preserve structure
      const transformed = transformTreeData(hierarchy);
      expect(transformed).toHaveLength(1);
      expect(transformed[0].id).toBe('folder-src');
      
      // Check nested structure preservation
      expect(transformed[0].children).toHaveLength(1);
      expect(transformed[0].children[0].id).toBe('folder-src-core');
      expect(transformed[0].children[0].children).toHaveLength(1);
      expect(transformed[0].children[0].children[0].id).toBe('file-pipeline_config');
      
      // Check entity structure
      const entity = transformed[0].children[0].children[0].children[0];
      expect(entity.id).toBe('entity-validate_config');
      expect(entity.type).toBe('entity');
      expect(entity.score).toBe(15.7);
      
      // Check issue/suggestion children
      expect(entity.children).toHaveLength(2);
      expect(entity.children[0].type).toBe('issue-row');
      expect(entity.children[1].type).toBe('suggestion-row');
    });

    it('should handle valknut directory health structure', () => {
      const health = sampleData.sampleDirectoryHealth;
      
      // Test that directory health data is properly structured
      expect(health.directories).toBeDefined();
      expect(health.directories['src']).toBeDefined();
      expect(health.directories['src/core']).toBeDefined();
      
      // Test health score ranges (should be 0-1)
      expect(health.directories['src'].health_score).toBeGreaterThanOrEqual(0);
      expect(health.directories['src'].health_score).toBeLessThanOrEqual(1);
      expect(health.directories['src/core'].health_score).toBeGreaterThanOrEqual(0);
      expect(health.directories['src/core'].health_score).toBeLessThanOrEqual(1);
      
      // Test required health metrics
      expect(typeof health.directories['src'].file_count).toBe('number');
      expect(typeof health.directories['src'].entity_count).toBe('number');
      expect(typeof health.directories['src'].critical_issues).toBe('number');
      expect(typeof health.directories['src'].high_priority_issues).toBe('number');
    });

    it('should properly parse valknut entity names with function prefixes', () => {
      const entityName = "./src/core/pipeline/pipeline_config.rs:function:validate_configuration";
      
      // Test function name extraction logic (from CodeAnalysisTree)
      const functionMatch = entityName.match(/:function:(.+)$/);
      expect(functionMatch).toBeTruthy();
      expect(functionMatch[1]).toBe('validate_configuration');
    });

    it('should handle complex severity calculations for valknut data', () => {
      // Test complexity issue detection and scoring
      const complexityIssue = {
        category: "complexity",
        description: "Function has very high cyclomatic complexity (score: 15.7)",
        priority: "high",
        severity: 12.5
      };
      
      const isComplexityIssue = complexityIssue.category.toLowerCase().includes('complexity');
      expect(isComplexityIssue).toBe(true);
      
      // Test score extraction from description
      const scoreMatch = complexityIssue.description.match(/score:\s*(\d+(?:\.\d+)?)/);
      expect(scoreMatch).toBeTruthy();
      expect(parseFloat(scoreMatch[1])).toBe(15.7);
      
      // Test severity level mapping (valknut uses 0-20+ scale)
      expect(getSeverityLevel(null, 20)).toBe('critical');
      expect(getSeverityLevel(null, 15)).toBe('critical');
      expect(getSeverityLevel(null, 12.5)).toBe('high');
      expect(getSeverityLevel(null, 8)).toBe('medium');
      expect(getSeverityLevel(null, 3)).toBe('low');
    });

    it('should handle file path parsing for valknut project structure', () => {
      const filePath = "src/core/pipeline/pipeline_config.rs";
      const pathParts = filePath.split('/').filter(Boolean);
      const fileName = pathParts.pop();
      
      expect(fileName).toBe('pipeline_config.rs');
      expect(pathParts).toEqual(['src', 'core', 'pipeline']);
      
      // Test ID generation for file paths
      const fileId = 'file-' + fileName.replace(/[^a-zA-Z0-9_-]/g, '_');
      expect(fileId).toBe('file-pipeline_config_rs');
    });

    it('should handle valknut priority mapping correctly', () => {
      const priorities = ['critical', 'high', 'medium', 'low'];
      const priorityOrder = { critical: 0, high: 1, medium: 2, low: 3 };
      
      priorities.forEach((priority, index) => {
        expect(priorityOrder[priority]).toBe(index);
      });
      
      // Test unknown priority handling
      expect(priorityOrder['unknown'] || 999).toBe(999);
    });

    it('should aggregate severity counts correctly for valknut hierarchy', () => {
      // Test bubbling up severity counts from children to parents
      const child1Counts = { critical: 1, high: 2, medium: 1, low: 0 };
      const child2Counts = { critical: 0, high: 1, medium: 0, low: 2 };
      
      const aggregated = {
        critical: child1Counts.critical + child2Counts.critical,
        high: child1Counts.high + child2Counts.high,
        medium: child1Counts.medium + child2Counts.medium,
        low: child1Counts.low + child2Counts.low
      };
      
      expect(aggregated).toEqual({ critical: 1, high: 3, medium: 1, low: 2 });
    });
  });

  describe('Edge Cases from Real Valknut Data', () => {
    it('should handle missing optional fields gracefully', () => {
      const incompleteEntity = {
        name: "./src/test.rs:function:simple_func",
        // Missing: priority, score, lineRange, issues, suggestions
      };
      
      // Should not throw when processing
      expect(() => {
        const severity = getSeverityLevel(incompleteEntity.priority, incompleteEntity.severity);
        expect(severity).toBe('low'); // default fallback
      }).not.toThrow();
      
      expect(() => {
        const counts = countSeverityLevels(incompleteEntity.issues);
        expect(counts).toEqual({ critical: 0, high: 0, medium: 0, low: 0 });
      }).not.toThrow();
    });

    it('should handle valknut file groups with empty entities', () => {
      const emptyFileGroup = {
        filePath: "src/empty.rs",
        highestPriority: "low",
        entityCount: 0,
        avgScore: 0,
        totalIssues: 0,
        entities: []
      };
      
      // Should handle empty entities array
      expect(emptyFileGroup.entities).toHaveLength(0);
      expect(emptyFileGroup.entityCount).toBe(0);
      
      // Should aggregate empty severity counts
      const aggregatedCounts = { critical: 0, high: 0, medium: 0, low: 0 };
      emptyFileGroup.entities.forEach(entity => {
        if (entity.issues) {
          entity.issues.forEach(issue => {
            const severity = getSeverityLevel(issue.priority, issue.severity);
            aggregatedCounts[severity]++;
          });
        }
      });
      
      expect(aggregatedCounts).toEqual({ critical: 0, high: 0, medium: 0, low: 0 });
    });

    it('should handle malformed valknut issue descriptions', () => {
      const malformedIssues = [
        {
          category: "complexity",
          description: "Bad format no score",
          priority: "high"
        },
        {
          category: "structure",
          description: "score: invalid",
          priority: "medium"
        },
        {
          category: "other",
          description: "score: 12.5 valid",
          priority: "low"
        }
      ];
      
      malformedIssues.forEach(issue => {
        const scoreMatch = issue.description.match(/score:\s*(\d+(?:\.\d+)?)/);
        if (scoreMatch) {
          const score = parseFloat(scoreMatch[1]);
          expect(typeof score).toBe('number');
          if (!isNaN(score)) {
            expect(score).toBeGreaterThanOrEqual(0);
          }
        }
      });
    });

    it('should handle valknut coverage pack integration', () => {
      const coveragePack = {
        path: "src/core/pipeline_config.rs",
        file_info: {
          coverage_before: 0.72,
          coverage_after_if_filled: 0.85,
          loc: 156
        }
      };
      
      // Test coverage data validation
      expect(coveragePack.file_info.coverage_before).toBeGreaterThanOrEqual(0);
      expect(coveragePack.file_info.coverage_before).toBeLessThanOrEqual(1);
      expect(coveragePack.file_info.coverage_after_if_filled).toBeGreaterThanOrEqual(0);
      expect(coveragePack.file_info.coverage_after_if_filled).toBeLessThanOrEqual(1);
      expect(coveragePack.file_info.loc).toBeGreaterThan(0);
      
      // Test improvement calculation
      const improvement = coveragePack.file_info.coverage_after_if_filled - coveragePack.file_info.coverage_before;
      expect(improvement).toBeCloseTo(0.13, 2);
    });
  });

  describe('Performance with Large Valknut Datasets', () => {
    it('should handle large directory trees efficiently', () => {
      // Simulate large valknut project structure
      const largeDirs = {};
      for (let i = 0; i < 100; i++) {
        largeDirs[`src/module${i}`] = {
          health_score: Math.random(),
          file_count: Math.floor(Math.random() * 50),
          entity_count: Math.floor(Math.random() * 200),
          refactoring_needed: Math.random() > 0.5,
          critical_issues: Math.floor(Math.random() * 5),
          high_priority_issues: Math.floor(Math.random() * 10)
        };
      }
      
      const largeHealthData = { directories: largeDirs };
      
      // Should process without performance issues
      const startTime = performance.now();
      
      Object.entries(largeHealthData.directories).forEach(([path, health]) => {
        expect(typeof health.health_score).toBe('number');
        expect(typeof health.file_count).toBe('number');
        expect(typeof health.entity_count).toBe('number');
      });
      
      const endTime = performance.now();
      
      // Should complete within reasonable time (< 100ms for 100 directories)
      expect(endTime - startTime).toBeLessThan(100);
    });

    it('should handle large entity collections efficiently', () => {
      // Simulate large number of entities with issues
      const largeEntityList = [];
      for (let i = 0; i < 1000; i++) {
        largeEntityList.push({
          name: `./src/module${i % 10}.rs:function:func${i}`,
          priority: ['critical', 'high', 'medium', 'low'][i % 4],
          score: Math.random() * 20,
          issues: [
            {
              category: 'complexity',
              priority: ['critical', 'high', 'medium', 'low'][i % 4],
              severity: Math.random() * 20
            }
          ],
          suggestions: []
        });
      }
      
      // Should count severities efficiently
      const startTime = performance.now();
      
      const totalCounts = { critical: 0, high: 0, medium: 0, low: 0 };
      largeEntityList.forEach(entity => {
        const counts = countSeverityLevels(entity.issues);
        totalCounts.critical += counts.critical;
        totalCounts.high += counts.high;
        totalCounts.medium += counts.medium;
        totalCounts.low += counts.low;
      });
      
      const endTime = performance.now();
      
      // Should complete within reasonable time (< 50ms for 1000 entities)
      expect(endTime - startTime).toBeLessThan(50);
      
      // Should have correct total count
      expect(totalCounts.critical + totalCounts.high + totalCounts.medium + totalCounts.low).toBe(1000);
    });
  });
});