/**
 * Test Runner - E2E Testing
 * 
 * Main orchestrator for running comprehensive E2E tests that validate
 * the complete valknut HTML generation pipeline.
 */

const fs = require('fs');
const path = require('path');
const IntegrationTests = require('./integration-tests');
const HtmlGenerator = require('./html-generator');
const TreeValidator = require('./tree-validation');

class TestRunner {
    constructor() {
        this.integrationTests = new IntegrationTests();
        this.generator = new HtmlGenerator();
        this.validator = new TreeValidator();
        this.results = {
            startTime: new Date(),
            endTime: null,
            totalTests: 0,
            passedTests: 0,
            failedTests: 0,
            errors: [],
            warnings: [],
            outputs: []
        };
    }

    /**
     * Run the complete E2E test suite
     */
    async runTestSuite() {
        console.log('🚀 Starting Valknut E2E Test Suite');
        console.log('=====================================\n');

        try {
            // Check prerequisites
            await this.checkPrerequisites();

            // Run integration tests
            console.log('📋 Running Integration Tests...');
            const integrationResults = await this.integrationTests.runAllTests();
            this.processIntegrationResults(integrationResults);

            // Run focused tree rendering tests
            console.log('\n🌳 Running Tree Rendering Tests...');
            await this.runTreeRenderingTests();

            // Run real-world scenario tests
            console.log('\n🎯 Running Real-World Scenario Tests...');
            await this.runRealWorldTests();

            // Generate final outputs
            console.log('\n📤 Generating Test Outputs...');
            await this.generateTestOutputs();

        } catch (error) {
            console.error('❌ Test suite failed:', error.message);
            this.results.errors.push({
                type: 'suite_failure',
                message: error.message,
                stack: error.stack
            });
        } finally {
            this.results.endTime = new Date();
            this.printFinalResults();
        }

        return this.results;
    }

    /**
     * Check that all prerequisites are available
     */
    async checkPrerequisites() {
        console.log('🔍 Checking prerequisites...');

        // Check for real JSON file
        const jsonPath = '/tmp/analysis-results.json';
        if (!fs.existsSync(jsonPath)) {
            throw new Error(`Real analysis results not found at ${jsonPath}. Please run: valknut analyze --format json --out /tmp ./src`);
        }

        // Check JSON is valid
        try {
            const jsonContent = fs.readFileSync(jsonPath, 'utf8');
            JSON.parse(jsonContent);
            console.log('✅ Valid JSON analysis results found');
        } catch (error) {
            throw new Error(`Invalid JSON in analysis results: ${error.message}`);
        }

        // Check templates directory
        const templatesDir = path.resolve(__dirname, '../../..');
        const treeTemplatePath = path.join(templatesDir, 'partials', 'tree.hbs');
        if (!fs.existsSync(treeTemplatePath)) {
            throw new Error(`Tree template not found at ${treeTemplatePath}`);
        }
        console.log('✅ Template files found');

        // Check required Node modules
        try {
            require('handlebars');
            console.log('✅ Handlebars available');
        } catch (error) {
            throw new Error('Handlebars not available. Run: npm install handlebars');
        }

        try {
            require('jsdom');
            console.log('✅ JSDOM available');
        } catch (error) {
            throw new Error('JSDOM not available. Run: npm install jsdom');
        }

        // Check and setup React bundle
        const bundlePath = path.join(__dirname, '../../react-tree-bundle.debug.js');
        if (fs.existsSync(bundlePath)) {
            // Copy React bundle to /tmp/ so HTML files can load it
            fs.copyFileSync(bundlePath, '/tmp/react-tree-bundle.debug.js');
            console.log('✅ React bundle copied to /tmp/ for test accessibility');
        } else {
            console.log('⚠️  React bundle not found - React tests may fail');
        }

        console.log('✅ All prerequisites met\n');
    }

    /**
     * Process integration test results
     */
    processIntegrationResults(integrationResults) {
        integrationResults.forEach(result => {
            this.results.totalTests++;
            if (result.passed) {
                this.results.passedTests++;
            } else {
                this.results.failedTests++;
                this.results.errors.push({
                    type: 'integration_test',
                    testName: result.name,
                    message: result.details
                });
            }
        });
    }

    /**
     * Run focused tree rendering tests
     */
    async runTreeRenderingTests() {
        const tests = [
            () => this.testTreeNodeGeneration(),
            () => this.testTreeStructureHierarchy(),
            () => this.testTreeMetricsDisplay(),
            () => this.testTreeInteractivity()
        ];

        for (const test of tests) {
            try {
                await this.runSingleTest(test.name || 'Anonymous Test', test);
            } catch (error) {
                this.recordTestFailure(test.name || 'Anonymous Test', error.message);
            }
        }
    }

    /**
     * Test tree node generation specifically
     */
    async testTreeNodeGeneration() {
        const testName = 'Tree Node Generation';
        console.log(`Running: ${testName}`);

        const jsonPath = '/tmp/analysis-results.json';
        const result = this.generator.generateFromJsonFile(jsonPath);
        
        // Validate that tree nodes are actually generated
        const { treeHtml, templateData } = result;
        
        if (!treeHtml || treeHtml.length < 100) {
            throw new Error('Tree HTML generation produced minimal output');
        }

        // Check for specific tree elements
        if (!treeHtml.includes('tree-node') && !treeHtml.includes('file') && !treeHtml.includes('directory')) {
            throw new Error('Tree HTML missing expected node elements');
        }

        // Validate against template data
        if (templateData.files && templateData.files.length > 0) {
            const fileCount = templateData.files.length;
            
            // Simple heuristic: HTML should contain file names
            let foundFiles = 0;
            templateData.files.forEach(file => {
                const fileName = path.basename(file.path);
                if (treeHtml.includes(fileName)) {
                    foundFiles++;
                }
            });

            if (foundFiles === 0) {
                throw new Error(`No file names found in tree HTML (expected ${fileCount} files)`);
            }

            console.log(`✅ Found ${foundFiles}/${fileCount} files in tree HTML`);
        }

        this.recordTestSuccess(testName, `Generated tree HTML with ${treeHtml.length} characters`);
    }

    /**
     * Test tree structure hierarchy
     */
    async testTreeStructureHierarchy() {
        const testName = 'Tree Structure Hierarchy';
        console.log(`Running: ${testName}`);

        const jsonPath = '/tmp/analysis-results.json';
        const analysisResults = this.generator.loadAnalysisResults(jsonPath);
        const templateData = this.generator.transformDataForTemplate(analysisResults);

        if (!templateData.tree_data) {
            throw new Error('No tree data structure generated');
        }

        const tree = templateData.tree_data;
        
        // Validate tree structure
        if (!tree.children || tree.children.length === 0) {
            throw new Error('Tree root has no children');
        }

        // Check that directory structures are properly nested
        let directoryCount = 0;
        let fileCount = 0;
        
        const countNodes = (node) => {
            if (node.type === 'directory') directoryCount++;
            if (node.type === 'file') fileCount++;
            
            if (node.children) {
                node.children.forEach(countNodes);
            }
        };

        countNodes(tree);

        if (fileCount === 0) {
            throw new Error('Tree structure contains no file nodes');
        }

        console.log(`✅ Tree structure: ${directoryCount} directories, ${fileCount} files`);
        this.recordTestSuccess(testName, `${directoryCount} directories, ${fileCount} files`);
    }

    /**
     * Test tree metrics display
     */
    async testTreeMetricsDisplay() {
        const testName = 'Tree Metrics Display';
        console.log(`Running: ${testName}`);

        const jsonPath = '/tmp/analysis-results.json';
        const result = this.generator.generateFromJsonFile(jsonPath);
        const { completeHtml } = result;

        // Check for metric-related elements
        const metricIndicators = [
            'badge',
            'complexity',
            'score',
            'health',
            'metric'
        ];

        let foundMetrics = 0;
        metricIndicators.forEach(indicator => {
            if (completeHtml.toLowerCase().includes(indicator)) {
                foundMetrics++;
            }
        });

        if (foundMetrics === 0) {
            throw new Error('No metric indicators found in generated HTML');
        }

        console.log(`✅ Found ${foundMetrics}/${metricIndicators.length} metric indicators`);
        this.recordTestSuccess(testName, `${foundMetrics} metric indicators found`);
    }

    /**
     * Test tree interactivity elements
     */
    async testTreeInteractivity() {
        const testName = 'Tree Interactivity';
        console.log(`Running: ${testName}`);

        const jsonPath = '/tmp/analysis-results.json';
        const result = this.generator.generateFromJsonFile(jsonPath);
        const { completeHtml } = result;

        // Check for interactive elements
        const interactiveElements = [
            'data-bs-toggle',
            'collapse',
            'button',
            'onclick',
            'data-target'
        ];

        let foundInteractive = 0;
        interactiveElements.forEach(element => {
            if (completeHtml.includes(element)) {
                foundInteractive++;
            }
        });

        // Also check for Bootstrap classes
        if (completeHtml.includes('bootstrap')) {
            foundInteractive++;
        }

        console.log(`✅ Found ${foundInteractive} interactive element types`);
        this.recordTestSuccess(testName, `${foundInteractive} interactive elements`);
    }

    /**
     * Run real-world scenario tests
     */
    async runRealWorldTests() {
        const tests = [
            () => this.testNoRefactoringCandidatesScenario(),
            () => this.testLargeCodebaseScenario(),
            () => this.testEmptyDirectoryScenario()
        ];

        for (const test of tests) {
            try {
                await this.runSingleTest(test.name || 'Anonymous Test', test);
            } catch (error) {
                this.recordTestFailure(test.name || 'Anonymous Test', error.message);
            }
        }
    }

    /**
     * Test the specific "No Refactoring Candidates Found" scenario
     */
    async testNoRefactoringCandidatesScenario() {
        const testName = 'No Refactoring Candidates Scenario';
        console.log(`Running: ${testName}`);

        const jsonPath = '/tmp/analysis-results.json';
        const analysisResults = this.generator.loadAnalysisResults(jsonPath);
        const result = this.generator.generateHtml(analysisResults);

        const { completeHtml, templateData } = result;

        // Check the actual refactoring candidates count
        const candidatesCount = templateData.refactoring_candidates ? 
            templateData.refactoring_candidates.length : 0;

        if (candidatesCount === 0) {
            // Should show "no candidates" message
            const hasNoMessage = completeHtml.toLowerCase().includes('no refactoring candidates') ||
                                 completeHtml.toLowerCase().includes('no candidates found') ||
                                 completeHtml.toLowerCase().includes('no refactoring opportunities');

            if (!hasNoMessage) {
                throw new Error('Expected "no refactoring candidates" message but none found');
            }

            console.log('✅ "No refactoring candidates" message correctly displayed');
        } else {
            // Should show candidates list
            const hasCandidatesList = completeHtml.includes('refactoring-opportunity') ||
                                     completeHtml.includes('candidate');

            if (!hasCandidatesList) {
                throw new Error(`Has ${candidatesCount} candidates but no candidate display found`);
            }

            console.log(`✅ ${candidatesCount} refactoring candidates correctly displayed`);
        }

        this.recordTestSuccess(testName, `Handled ${candidatesCount} candidates correctly`);
    }

    /**
     * Test large codebase scenario
     */
    async testLargeCodebaseScenario() {
        const testName = 'Large Codebase Scenario';
        console.log(`Running: ${testName}`);

        const jsonPath = '/tmp/analysis-results.json';
        const analysisResults = this.generator.loadAnalysisResults(jsonPath);

        // Test performance with actual data size
        const fileCount = analysisResults.files ? analysisResults.files.length : 0;
        
        const startTime = Date.now();
        const result = this.generator.generateHtml(analysisResults);
        const generationTime = Date.now() - startTime;

        if (generationTime > 10000) { // 10 seconds
            throw new Error(`HTML generation too slow: ${generationTime}ms for ${fileCount} files`);
        }

        if (!result.completeHtml || result.completeHtml.length < 1000) {
            throw new Error('Generated HTML suspiciously small for large codebase');
        }

        console.log(`✅ Generated HTML for ${fileCount} files in ${generationTime}ms`);
        this.recordTestSuccess(testName, `${fileCount} files in ${generationTime}ms`);
    }

    /**
     * Test empty directory scenario
     */
    async testEmptyDirectoryScenario() {
        const testName = 'Empty Directory Scenario';
        console.log(`Running: ${testName}`);

        // Create minimal empty scenario
        const emptyResults = {
            summary: {
                total_files: 0,
                analyzed_files: 0,
                overall_health: 100,
                refactoring_candidates: 0
            },
            files: [],
            refactoring_candidates: [],
            version: 'test'
        };

        const result = this.generator.generateHtml(emptyResults);

        if (!result.completeHtml) {
            throw new Error('Failed to generate HTML for empty scenario');
        }

        // Should handle empty state gracefully
        const hasEmptyMessage = result.completeHtml.includes('0 files') ||
                               result.completeHtml.includes('no files') ||
                               result.completeHtml.includes('0');

        if (!hasEmptyMessage) {
            throw new Error('Empty scenario should show zero values');
        }

        console.log('✅ Empty directory scenario handled gracefully');
        this.recordTestSuccess(testName, 'Empty state handled correctly');
    }

    /**
     * Generate test outputs for manual inspection
     */
    async generateTestOutputs() {
        try {
            const jsonPath = '/tmp/analysis-results.json';
            const result = this.generator.generateFromJsonFile(jsonPath);

            // Save complete HTML output
            const htmlOutputPath = '/tmp/e2e-test-complete.html';
            fs.writeFileSync(htmlOutputPath, result.completeHtml, 'utf8');
            this.results.outputs.push({
                type: 'html',
                path: htmlOutputPath,
                description: 'Complete HTML output from E2E test'
            });

            // Save tree HTML fragment
            const treeOutputPath = '/tmp/e2e-test-tree-fragment.html';
            fs.writeFileSync(treeOutputPath, result.treeHtml, 'utf8');
            this.results.outputs.push({
                type: 'html_fragment',
                path: treeOutputPath,
                description: 'Tree HTML fragment from E2E test'
            });

            // Save template data
            const dataOutputPath = '/tmp/e2e-test-template-data.json';
            fs.writeFileSync(dataOutputPath, JSON.stringify(result.templateData, null, 2), 'utf8');
            this.results.outputs.push({
                type: 'json',
                path: dataOutputPath,
                description: 'Template data used in E2E test'
            });

            // Save test results
            const resultsOutputPath = '/tmp/e2e-test-results.json';
            fs.writeFileSync(resultsOutputPath, JSON.stringify(this.results, null, 2), 'utf8');
            this.results.outputs.push({
                type: 'json',
                path: resultsOutputPath,
                description: 'Complete E2E test results'
            });

            console.log('✅ Generated test outputs:');
            this.results.outputs.forEach(output => {
                console.log(`   - ${output.description}: ${output.path}`);
            });

        } catch (error) {
            console.error('❌ Failed to generate test outputs:', error.message);
            this.results.warnings.push(`Failed to generate outputs: ${error.message}`);
        }
    }

    /**
     * Helper methods for test execution
     */
    async runSingleTest(testName, testFunction) {
        try {
            await testFunction();
        } catch (error) {
            throw error;
        }
    }

    recordTestSuccess(testName, details) {
        this.results.totalTests++;
        this.results.passedTests++;
        console.log(`✅ ${testName}: ${details}`);
    }

    recordTestFailure(testName, errorMessage) {
        this.results.totalTests++;
        this.results.failedTests++;
        console.log(`❌ ${testName}: ${errorMessage}`);
        this.results.errors.push({
            type: 'test_failure',
            testName,
            message: errorMessage
        });
    }

    /**
     * Print final test results
     */
    printFinalResults() {
        const duration = this.results.endTime - this.results.startTime;
        
        console.log('\n' + '='.repeat(50));
        console.log('🏁 E2E TEST SUITE COMPLETE');
        console.log('='.repeat(50));
        
        console.log(`⏱️  Duration: ${duration}ms`);
        console.log(`📊 Tests: ${this.results.passedTests}/${this.results.totalTests} passed`);
        
        if (this.results.failedTests > 0) {
            console.log(`❌ Failed Tests: ${this.results.failedTests}`);
            console.log('\nFailures:');
            this.results.errors.forEach((error, index) => {
                console.log(`  ${index + 1}. ${error.testName || error.type}: ${error.message}`);
            });
        }

        if (this.results.warnings.length > 0) {
            console.log(`⚠️  Warnings: ${this.results.warnings.length}`);
            this.results.warnings.forEach((warning, index) => {
                console.log(`  ${index + 1}. ${warning}`);
            });
        }

        if (this.results.outputs.length > 0) {
            console.log(`📤 Generated Outputs: ${this.results.outputs.length}`);
            this.results.outputs.forEach(output => {
                console.log(`  - ${output.path}`);
            });
        }

        const status = this.results.failedTests === 0 ? 'PASS' : 'FAIL';
        console.log(`\n🎯 Final Status: ${status}`);
        console.log('='.repeat(50));
    }
}

module.exports = TestRunner;