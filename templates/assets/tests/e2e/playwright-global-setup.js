/**
 * Playwright Global Setup
 * Generates test HTML files using existing E2E infrastructure
 */

import fs from 'fs';
import path from 'path';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const HtmlGenerator = require('./html-generator');

// Export as ES module for Playwright setup

async function globalSetup(config) {
  console.log('🚀 Setting up Playwright tests - generating HTML files...');
  
  const generator = new HtmlGenerator();
  const testOutputDir = './test-results';
  
  // Ensure test output directory exists
  if (!fs.existsSync(testOutputDir)) {
    fs.mkdirSync(testOutputDir, { recursive: true });
  }
  
  try {
    // Check if we have real analysis results
    const realJsonPath = '/tmp/analysis-results.json';
    let analysisResults = null;
    
    if (fs.existsSync(realJsonPath)) {
      console.log('📊 Using real analysis results from valknut');
      analysisResults = generator.loadAnalysisResults(realJsonPath);
    } else {
      console.log('📝 Generating mock analysis results for testing');
      analysisResults = generateMockAnalysisResults();
    }
    
    // Generate different test scenarios
    await generateTestScenarios(generator, analysisResults, testOutputDir);
    
    console.log('✅ Test HTML files generated successfully');
    
  } catch (error) {
    console.error('❌ Failed to generate test files:', error.message);
    throw error;
  }
}

async function generateTestScenarios(generator, baseResults, outputDir) {
  const scenarios = [
    {
      name: 'normal-results',
      description: 'Normal analysis results with refactoring candidates',
      data: baseResults
    },
    {
      name: 'empty-candidates', 
      description: 'Empty refactoring candidates (our fix target)',
      data: {
        ...baseResults,
        refactoring_candidates: [],
        files: []
      }
    },
    {
      name: 'large-dataset',
      description: 'Large dataset for performance testing',
      data: {
        ...baseResults,
        files: generateLargeFileSet(100) // 100 files
      }
    },
    {
      name: 'mixed-results',
      description: 'Mixed results with some candidates',
      data: {
        ...baseResults,
        refactoring_candidates: baseResults.refactoring_candidates?.slice(0, 3) || []
      }
    }
  ];
  
  for (const scenario of scenarios) {
    console.log(`  📄 Generating ${scenario.name}: ${scenario.description}`);
    
    // Transform data for template
    const templateData = generator.transformDataForTemplate(scenario.data);
    
    // Generate complete HTML
    const result = generator.generateHtml(scenario.data);
    const html = result.completeHtml || result.html || result;
    
    // Write to test file
    const outputPath = path.join(outputDir, `${scenario.name}.html`);
    fs.writeFileSync(outputPath, html);
    
    // Also generate a data file for reference
    const dataPath = path.join(outputDir, `${scenario.name}-data.json`);
    fs.writeFileSync(dataPath, JSON.stringify(templateData, null, 2));
  }
}

function generateMockAnalysisResults() {
  return {
    summary: {
      total_files: 5,
      total_functions: 20,
      avg_complexity: 2.5,
      high_complexity_functions: 3
    },
    files: [
      {
        path: './src/main.rs',
        complexity_score: 3.2,
        functions: [
          {
            name: 'main',
            complexity: 4.5,
            line_start: 10,
            line_end: 45,
            refactoring_suggestions: ['Extract method for error handling']
          },
          {
            name: 'process_files',
            complexity: 2.8,
            line_start: 50,
            line_end: 80,
            refactoring_suggestions: []
          }
        ]
      },
      {
        path: './src/utils.rs',
        complexity_score: 1.8,
        functions: [
          {
            name: 'format_output',
            complexity: 1.5,
            line_start: 5,
            line_end: 20,
            refactoring_suggestions: []
          }
        ]
      }
    ],
    refactoring_candidates: [
      {
        file: './src/main.rs',
        function: 'main',
        priority: 'high',
        issues: ['High cyclomatic complexity'],
        suggestions: ['Break down into smaller functions']
      }
    ]
  };
}

function generateLargeFileSet(count) {
  const files = [];
  for (let i = 0; i < count; i++) {
    files.push({
      path: `./src/module_${i}.rs`,
      complexity_score: Math.random() * 5 + 1,
      functions: [
        {
          name: `function_${i}_1`,
          complexity: Math.random() * 8 + 1,
          line_start: 10,
          line_end: 50,
          refactoring_suggestions: i % 3 === 0 ? ['Extract method'] : []
        }
      ]
    });
  }
  return files;
}

export default globalSetup;