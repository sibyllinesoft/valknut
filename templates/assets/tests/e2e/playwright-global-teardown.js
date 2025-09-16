/**
 * Playwright Global Teardown
 * Cleanup and reporting after all tests complete
 */

import fs from 'fs';
import path from 'path';

async function globalTeardown(config) {
  console.log('🧹 Cleaning up Playwright test artifacts...');
  
  try {
    // Optional: Archive test results for CI
    if (process.env.CI) {
      archiveTestResults();
    }
    
    // Optional: Clean up temporary files (keep for debugging in dev)
    if (process.env.CI) {
      cleanupTempFiles();
    } else {
      console.log('💾 Test files preserved for debugging in ./test-results/');
    }
    
    console.log('✅ Cleanup completed');
    
  } catch (error) {
    console.warn('⚠️  Cleanup encountered issues:', error.message);
    // Don't fail on cleanup issues
  }
}

function archiveTestResults() {
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  const archiveName = `test-results-${timestamp}`;
  
  // In a real CI environment, you might upload to S3, etc.
  console.log(`📦 Test results would be archived as: ${archiveName}`);
}

function cleanupTempFiles() {
  const tempFiles = [
    './test-results/normal-results.html',
    './test-results/empty-candidates.html',
    './test-results/large-dataset.html',
    './test-results/mixed-results.html'
  ];
  
  tempFiles.forEach(file => {
    if (fs.existsSync(file)) {
      fs.unlinkSync(file);
    }
  });
  
  console.log('🗑️  Temporary test files cleaned up');
}

export default globalTeardown;