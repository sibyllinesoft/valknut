#!/usr/bin/env node

/**
 * Run E2E Tests - Entry Point
 * 
 * Command-line entry point for executing the complete E2E test suite.
 * This script can be run directly or integrated into CI/CD pipelines.
 */

const path = require('path');
const TestRunner = require('./test-runner');

// Handle command line arguments
const args = process.argv.slice(2);
const options = {
    verbose: args.includes('--verbose') || args.includes('-v'),
    quiet: args.includes('--quiet') || args.includes('-q'),
    help: args.includes('--help') || args.includes('-h'),
    generateOnly: args.includes('--generate-only'),
    skipValidation: args.includes('--skip-validation'),
    noOpen: args.includes('--no-open')
};

/**
 * Print usage information
 */
function printUsage() {
    console.log(`
Valknut E2E Test Suite

Usage: node run-e2e.js [options]

Options:
  -h, --help            Show this help message
  -v, --verbose         Enable verbose output
  -q, --quiet           Minimize output (errors only)
  --generate-only       Only generate outputs, skip tests
  --skip-validation     Skip HTML validation tests
  --no-open             Don't auto-open test results in browser

Examples:
  node run-e2e.js                    # Run full test suite
  node run-e2e.js --verbose          # Run with detailed output
  node run-e2e.js --generate-only    # Generate HTML outputs only
  node run-e2e.js --quiet            # Minimal output

Prerequisites:
  1. Real analysis results must exist at /tmp/analysis-results.json
     Run: valknut analyze --format json --out /tmp ./src
  2. Required Node modules: handlebars, jsdom
     Run: npm install handlebars jsdom

Outputs:
  - /tmp/e2e-test-complete.html       # Complete rendered HTML
  - /tmp/e2e-test-tree-fragment.html  # Tree component only
  - /tmp/e2e-test-template-data.json  # Template data structure
  - /tmp/e2e-test-results.json        # Test execution results
`);
}

/**
 * Main execution function
 */
async function main() {
    if (options.help) {
        printUsage();
        process.exit(0);
    }

    if (!options.quiet) {
        console.log('🧪 Valknut E2E Test Suite');
        console.log('🔬 Testing complete HTML generation pipeline\n');
    }

    try {
        const testRunner = new TestRunner();
        
        if (options.generateOnly) {
            if (!options.quiet) {
                console.log('📤 Generation-only mode: Creating outputs without running tests\n');
            }
            
            const HtmlGenerator = require('./html-generator');
            const generator = new HtmlGenerator();
            
            const jsonPath = '/tmp/analysis-results.json';
            const result = generator.generateFromJsonFile(jsonPath);
            
            // Save outputs
            const fs = require('fs');
            fs.writeFileSync('/tmp/e2e-test-complete.html', result.completeHtml, 'utf8');
            fs.writeFileSync('/tmp/e2e-test-tree-fragment.html', result.treeHtml, 'utf8');
            fs.writeFileSync('/tmp/e2e-test-template-data.json', JSON.stringify(result.templateData, null, 2), 'utf8');
            
            if (!options.quiet) {
                console.log('✅ Generated outputs:');
                console.log('   - /tmp/e2e-test-complete.html');
                console.log('   - /tmp/e2e-test-tree-fragment.html');
                console.log('   - /tmp/e2e-test-template-data.json');
            }
            
            process.exit(0);
        }

        // Configure test runner based on options
        if (options.verbose) {
            console.log('🔍 Verbose mode enabled - detailed test output');
        }
        
        if (options.skipValidation) {
            console.log('⚠️  Skipping HTML validation tests');
        }

        // Run the complete test suite
        const results = await testRunner.runTestSuite();
        
        // Determine exit code
        const exitCode = results.failedTests > 0 ? 1 : 0;
        
        if (!options.quiet) {
            if (exitCode === 0) {
                console.log('\n🎉 All tests passed!');
                console.log('✨ The valknut HTML generation pipeline is working correctly.');
                
                // Open the main test output file if tests passed (unless disabled)
                if (results.outputs.length > 0 && !options.noOpen) {
                    const mainOutput = results.outputs.find(output => 
                        output.path.includes('e2e-test-complete.html')
                    );
                    
                    if (mainOutput) {
                        console.log(`\n🌐 Opening test results in browser: ${mainOutput.path}`);
                        
                        // Use platform-appropriate command to open file
                        const { spawn } = require('child_process');
                        let openCommand;
                        
                        if (process.platform === 'darwin') {
                            openCommand = 'open';
                        } else if (process.platform === 'win32') {
                            openCommand = 'start';
                        } else {
                            openCommand = 'xdg-open';
                        }
                        
                        try {
                            spawn(openCommand, [mainOutput.path], { 
                                detached: true, 
                                stdio: 'ignore' 
                            }).unref();
                        } catch (error) {
                            console.log(`⚠️  Could not auto-open file. Please manually open: ${mainOutput.path}`);
                        }
                    }
                }
            } else {
                console.log('\n💥 Some tests failed!');
                console.log('🔧 Check the output above for details on what needs to be fixed.');
            }
            
            if (results.outputs.length > 0) {
                console.log('\n📁 Generated files for manual inspection:');
                results.outputs.forEach(output => {
                    console.log(`   ${output.path} - ${output.description}`);
                });
            }
        }
        
        process.exit(exitCode);
        
    } catch (error) {
        if (!options.quiet) {
            console.error('\n💥 Test suite failed to run:');
            console.error(`❌ ${error.message}`);
            
            if (options.verbose && error.stack) {
                console.error('\nStack trace:');
                console.error(error.stack);
            }
        }
        
        process.exit(2);
    }
}

/**
 * Error handling
 */
process.on('uncaughtException', (error) => {
    console.error('\n💥 Uncaught exception:');
    console.error(`❌ ${error.message}`);
    if (options.verbose) {
        console.error(error.stack);
    }
    process.exit(3);
});

process.on('unhandledRejection', (reason, promise) => {
    console.error('\n💥 Unhandled promise rejection:');
    console.error(`❌ ${reason}`);
    if (options.verbose) {
        console.error('Promise:', promise);
    }
    process.exit(3);
});

// Handle Ctrl+C gracefully
process.on('SIGINT', () => {
    console.log('\n\n🛑 Test suite interrupted by user');
    process.exit(130);
});

// Run the main function
if (require.main === module) {
    main().catch(error => {
        console.error('Fatal error:', error);
        process.exit(4);
    });
}

module.exports = { main, printUsage };