// Test runner script for comprehensive test execution
// This script provides utilities for running and analyzing test results

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

/**
 * Test execution helper
 */
class TestRunner {
  constructor() {
    this.testResults = {};
    this.coverageResults = {};
  }

  /**
   * Run all test suites
   */
  async runAllTests() {
    console.log('🧪 Running comprehensive test suite for React Tree Component...\n');

    try {
      // Run unit tests
      console.log('📋 Running unit tests...');
      await this.runTestSuite('unit', 'tests/unit');

      // Run integration tests
      console.log('🔗 Running integration tests...');
      await this.runTestSuite('integration', 'tests/integration');

      // Generate coverage report
      console.log('📊 Generating coverage report...');
      await this.generateCoverageReport();

      // Display summary
      this.displaySummary();

    } catch (error) {
      console.error('❌ Test execution failed:', error.message);
      process.exit(1);
    }
  }

  /**
   * Run specific test suite
   */
  async runTestSuite(suiteName, testPath) {
    try {
      const command = `npx jest ${testPath} --verbose --coverage=false`;
      const output = execSync(command, { 
        encoding: 'utf8', 
        cwd: process.cwd(),
        stdio: ['pipe', 'pipe', 'pipe']
      });

      this.testResults[suiteName] = {
        success: true,
        output,
        testCount: this.extractTestCount(output),
        passCount: this.extractPassCount(output),
        failCount: this.extractFailCount(output)
      };

      console.log(`✅ ${suiteName} tests completed successfully`);
      console.log(`   Tests: ${this.testResults[suiteName].testCount}`);
      console.log(`   Passed: ${this.testResults[suiteName].passCount}`);
      console.log(`   Failed: ${this.testResults[suiteName].failCount}\n`);

    } catch (error) {
      this.testResults[suiteName] = {
        success: false,
        error: error.message,
        output: error.stdout || error.stderr || ''
      };

      console.log(`❌ ${suiteName} tests failed`);
      console.log(`   Error: ${error.message}\n`);
    }
  }

  /**
   * Generate comprehensive coverage report
   */
  async generateCoverageReport() {
    try {
      const command = 'npx jest --coverage --coverageReporters=text --coverageReporters=json-summary';
      const output = execSync(command, { 
        encoding: 'utf8', 
        cwd: process.cwd() 
      });

      // Parse coverage summary
      const coveragePath = path.join(process.cwd(), 'coverage/coverage-summary.json');
      if (fs.existsSync(coveragePath)) {
        const coverageData = JSON.parse(fs.readFileSync(coveragePath, 'utf8'));
        this.coverageResults = coverageData.total;
      }

      console.log('✅ Coverage report generated\n');

    } catch (error) {
      console.log('⚠️  Coverage generation failed:', error.message);
    }
  }

  /**
   * Display comprehensive test summary
   */
  displaySummary() {
    console.log('📋 TEST EXECUTION SUMMARY');
    console.log('=' .repeat(50));

    // Test results summary
    Object.entries(this.testResults).forEach(([suite, results]) => {
      if (results.success) {
        console.log(`✅ ${suite.toUpperCase()} TESTS: ${results.passCount}/${results.testCount} passed`);
      } else {
        console.log(`❌ ${suite.toUpperCase()} TESTS: Failed to execute`);
      }
    });

    console.log('');

    // Coverage summary
    if (this.coverageResults.lines) {
      console.log('📊 COVERAGE SUMMARY');
      console.log('-'.repeat(30));
      console.log(`Lines:      ${this.coverageResults.lines.pct}%`);
      console.log(`Functions:  ${this.coverageResults.functions.pct}%`);
      console.log(`Branches:   ${this.coverageResults.branches.pct}%`);
      console.log(`Statements: ${this.coverageResults.statements.pct}%`);
    }

    console.log('');

    // Overall status
    const allTestsPassed = Object.values(this.testResults).every(r => r.success);
    const coverageMeetsThreshold = this.coverageResults.lines?.pct >= 80;

    if (allTestsPassed && coverageMeetsThreshold) {
      console.log('🎉 ALL TESTS PASSED WITH GOOD COVERAGE!');
    } else if (allTestsPassed) {
      console.log('✅ All tests passed, but coverage could be improved');
    } else {
      console.log('❌ Some tests failed - see details above');
    }
  }

  /**
   * Extract test metrics from Jest output
   */
  extractTestCount(output) {
    const match = output.match(/Tests:\s+(\d+) total/);
    return match ? parseInt(match[1]) : 0;
  }

  extractPassCount(output) {
    const match = output.match(/(\d+) passed/);
    return match ? parseInt(match[1]) : 0;
  }

  extractFailCount(output) {
    const match = output.match/(\d+) failed/);
    return match ? parseInt(match[1]) : 0;
  }

  /**
   * Run specific test file
   */
  async runSpecificTest(testFile) {
    console.log(`🧪 Running specific test: ${testFile}`);
    
    try {
      const command = `npx jest ${testFile} --verbose`;
      const output = execSync(command, { 
        encoding: 'utf8', 
        cwd: process.cwd() 
      });

      console.log('✅ Test completed successfully');
      console.log(output);

    } catch (error) {
      console.log('❌ Test failed');
      console.log(error.stdout || error.message);
    }
  }

  /**
   * Watch mode for development
   */
  async runWatchMode() {
    console.log('👀 Starting test watch mode...');
    console.log('Press Ctrl+C to exit\n');

    try {
      execSync('npx jest --watch --verbose', { 
        stdio: 'inherit',
        cwd: process.cwd()
      });
    } catch (error) {
      console.log('Watch mode ended');
    }
  }
}

// CLI interface
if (require.main === module) {
  const args = process.argv.slice(2);
  const runner = new TestRunner();

  if (args.includes('--watch')) {
    runner.runWatchMode();
  } else if (args.includes('--file')) {
    const fileIndex = args.indexOf('--file') + 1;
    const testFile = args[fileIndex];
    if (testFile) {
      runner.runSpecificTest(testFile);
    } else {
      console.log('❌ Please specify a test file with --file <filename>');
    }
  } else {
    runner.runAllTests();
  }
}

module.exports = TestRunner;