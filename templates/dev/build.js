#!/usr/bin/env bun

/**
 * Bun build script to replace webpack for React tree component bundling
 * Produces both minified and debug versions compatible with existing HTML templates
 */

import { spawnSync } from 'bun';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs';
import { resolve, dirname } from 'path';

const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m'
};

function log(message, color = colors.reset) {
  console.log(`${color}${message}${colors.reset}`);
}

function createDistDirectory() {
  const distDir = resolve('./dist');
  if (!existsSync(distDir)) {
    mkdirSync(distDir, { recursive: true });
    log('📁 Created dist directory', colors.blue);
  }
}

async function buildBundle(mode = 'production') {
  const isProduction = mode === 'production';
  const outputFile = isProduction ? 'react-tree-bundle.js' : 'react-tree-bundle.debug.js';
  
  log(`🔨 Building ${mode} bundle...`, colors.yellow);
  
  const buildResult = spawnSync({
    cmd: [
      'bun', 'build',
      'templates/dev/src/tree-component/index.js',
      '--outdir', 'dist',
      '--outfile', `${outputFile}`,
      '--format', 'esm',
      '--target', 'browser',
      ...(isProduction ? ['--minify'] : ['--sourcemap'])
    ],
    stdout: 'pipe',
    stderr: 'pipe'
  });

  if (buildResult.exitCode !== 0) {
    log(`❌ Build failed for ${mode}:`, colors.red);
    log(buildResult.stderr.toString(), colors.red);
    return false;
  }

  log(`✅ ${mode} bundle created: dist/${outputFile}`, colors.green);
  
  // Add file size info
  try {
    const stats = Bun.file(`dist/${outputFile}`);
    const size = await stats.size();
    const sizeKB = (size / 1024).toFixed(2);
    log(`📦 Bundle size: ${sizeKB} KB`, colors.cyan);
  } catch (e) {
    // Size check failed, not critical
  }
  
  return true;
}

function addGlobalWrappers(mode = 'production') {
  const isProduction = mode === 'production';
  const outputFile = isProduction ? 'react-tree-bundle.js' : 'react-tree-bundle.debug.js';
  const filePath = `dist/${outputFile}`;
  
  try {
    let content = readFileSync(filePath, 'utf8');
    
    // Add global wrapper for browser compatibility
    const wrapper = `
(function(global) {
  'use strict';
  
  // Ensure React and ReactDOM are available
  if (typeof global.React === 'undefined') {
    throw new Error('React is required but not found on window. Please include React before this bundle.');
  }
  if (typeof global.ReactDOM === 'undefined') {
    throw new Error('ReactDOM is required but not found on window. Please include ReactDOM before this bundle.');
  }
  // Provide modules for the bundle
  const modules = {
    'react': global.React,
    'react-dom': global.ReactDOM
  };
  
  // Original bundle content
  ${content}
  
})(typeof window !== 'undefined' ? window : this);
`;
    
    // Escape closing script tags so HTML inlining doesn't break the bundle
    const escapedWrapper = wrapper.replace(/<\/script>/g, '<\\/script>');

    writeFileSync(filePath, escapedWrapper);
    log(`🔧 Added global wrappers to ${outputFile}`, colors.magenta);
    
  } catch (error) {
    log(`❌ Failed to add wrappers to ${outputFile}: ${error.message}`, colors.red);
    return false;
  }
  
  return true;
}

function copyCompatibilityFiles() {
  // Copy package.json for reference
  if (existsSync('bun-package.json')) {
    try {
      const packageContent = readFileSync('bun-package.json', 'utf8');
      writeFileSync('dist/package.json', packageContent);
      log('📋 Copied package.json to dist/', colors.blue);
    } catch (error) {
      log(`⚠️ Failed to copy package.json: ${error.message}`, colors.yellow);
    }
  }
  
  // Create a simple index.html for testing the bundle
  const testHtml = `
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Valknut Tree Component Test</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .container { max-width: 1200px; margin: 0 auto; }
        .tree-badge { 
            display: inline-block; 
            padding: 2px 6px; 
            margin: 2px; 
            border-radius: 4px; 
            font-size: 11px; 
            background: #f0f0f0; 
            border: 1px solid #ccc; 
        }
        .tree-badge-low { background: #f8f9fa; color: #6c757d; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Valknut Tree Component Test</h1>
        <div id="tree-root"></div>
    </div>

    <!-- Include React dependencies -->
    <script crossorigin src="../react.min.js"></script>
    <script crossorigin src="../react-dom.min.js"></script>
    
    <!-- Include our bundle -->
    <script src="./react-tree-bundle.js"></script>
    
    <script>
        // Test data
        const testData = {
            unifiedHierarchy: [
                {
                    id: 'folder-src',
                    name: 'src',
                    type: 'folder',
                    healthScore: 0.65,
                    fileCount: 2,
                    children: [
                        {
                            id: 'file-1',
                            name: 'test.rs',
                            type: 'file',
                            avgScore: 12.4,
                            children: [
                                {
                                    id: 'entity-1',
                                    name: 'test_function',
                                    type: 'entity',
                                    score: 15.7,
                                    children: []
                                }
                            ]
                        }
                    ]
                }
            ]
        };
        
        // Render the component
        const root = ReactDOM.createRoot(document.getElementById('tree-root'));
        root.render(React.createElement(ReactTreeBundle, { data: testData }));
    </script>
</body>
</html>
`;
  
  try {
    writeFileSync('dist/test.html', testHtml);
    log('🌐 Created test.html for bundle testing', colors.blue);
  } catch (error) {
    log(`⚠️ Failed to create test.html: ${error.message}`, colors.yellow);
  }
}

function validateBundle(mode = 'production') {
  const isProduction = mode === 'production';
  const outputFile = isProduction ? 'react-tree-bundle.js' : 'react-tree-bundle.debug.js';
  const filePath = `dist/${outputFile}`;
  
  try {
    const content = readFileSync(filePath, 'utf8');
    
    // Check for required exports
    const hasReactTreeBundle = content.includes('ReactTreeBundle');
    const hasCodeAnalysisTree = content.includes('CodeAnalysisTree') || content.includes('window.CodeAnalysisTree');
    
    if (!hasReactTreeBundle) {
      log(`❌ Bundle validation failed: ReactTreeBundle not found in ${outputFile}`, colors.red);
      return false;
    }
    
    log(`✅ Bundle validation passed for ${outputFile}`, colors.green);
    return true;
    
  } catch (error) {
    log(`❌ Bundle validation failed for ${outputFile}: ${error.message}`, colors.red);
    return false;
  }
}

async function main() {
  const args = process.argv.slice(2);
  const mode = args.includes('--dev') ? 'development' : 'production';
  const watch = args.includes('--watch');
  
  log('🚀 Valknut Tree Component Build Script', colors.bright);
  log(`Mode: ${mode}`, colors.cyan);
  
  createDistDirectory();
  
  if (mode === 'production') {
    // Build both production and debug versions
    log('📦 Building both production and debug bundles...', colors.yellow);
    
    const prodSuccess = await buildBundle('production');
    if (prodSuccess) {
      addGlobalWrappers('production');
      validateBundle('production');
    }
    
    const devSuccess = await buildBundle('development');
    if (devSuccess) {
      addGlobalWrappers('development');
      validateBundle('development');
    }
    
    if (prodSuccess && devSuccess) {
      copyCompatibilityFiles();
      log('🎉 Build completed successfully!', colors.green);
      log('📁 Output files:', colors.cyan);
      log('  - dist/react-tree-bundle.js (production)', colors.cyan);
      log('  - dist/react-tree-bundle.debug.js (development)', colors.cyan);
      log('  - dist/test.html (for testing)', colors.cyan);
    } else {
      log('❌ Build failed!', colors.red);
      process.exit(1);
    }
  } else {
    // Development mode - build debug version only
    const success = await buildBundle('development');
    if (success) {
      addGlobalWrappers('development');
      validateBundle('development');
      copyCompatibilityFiles();
      log('🎉 Development build completed!', colors.green);
    } else {
      log('❌ Development build failed!', colors.red);
      process.exit(1);
    }
  }
  
  if (watch) {
    log('👀 Watching for changes...', colors.yellow);
    // Note: Bun build --watch would be used in package.json scripts
    log('Use "bun run dev" for automatic rebuilding', colors.cyan);
  }
}

// Run the build
main().catch(error => {
  log(`❌ Build script error: ${error.message}`, colors.red);
  process.exit(1);
});
