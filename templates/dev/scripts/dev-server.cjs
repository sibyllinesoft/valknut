const fs = require('fs');
const path = require('path');
const { spawnSync, spawn } = require('child_process');
const net = require('net');
const chokidar = require('chokidar');

const reportsDir = path.join(__dirname, '../../../.valknut');
const devDataDir = path.join(__dirname, '../data');
const analysisJsonPath = path.join(devDataDir, 'analysis.json');
const projectRoot = path.join(__dirname, '..');

let bundleProcess = null;

function buildBundleOnce() {
  const bunExec = process.env.BUN_BIN || 'bun';
  const args = [
    'build',
    'src/tree-component/index.js',
    '--outfile=../assets/react-tree-bundle.js',
    '--format=iife',
    '--global-name=ReactTreeBundle',
    '--target=browser',
    '--sourcemap',
  ];

  console.info('[dev-server] Building bundle once:', `${bunExec} ${args.join(' ')}`);
  const result = spawnSync(bunExec, args, {
    cwd: projectRoot,
    stdio: 'inherit',
  });

  if ((result.status ?? 0) !== 0) {
    throw new Error(`Initial bundle build failed with status ${result.status}`);
  }
}

function startBundleWatch() {
  if (bundleProcess) {
    return;
  }

  buildBundleOnce();

  const bunExec = process.env.BUN_BIN || 'bun';
  const args = [
    'build',
    'src/tree-component/index.js',
    '--outfile=../assets/react-tree-bundle.js',
    '--format=iife',
    '--global-name=ReactTreeBundle',
    '--target=browser',
    '--sourcemap',
    '--watch',
  ];

  console.info('[dev-server] Starting bundle watcher:', `${bunExec} ${args.join(' ')}`);
  bundleProcess = spawn(bunExec, args, {
    cwd: projectRoot,
    stdio: 'inherit',
  });

  bundleProcess.on('exit', (code, signal) => {
    if (signal) {
      console.warn(`[dev-server] Bundle watcher exited with signal ${signal}`);
    } else if ((code ?? 0) !== 0) {
      console.error(`[dev-server] Bundle watcher exited with code ${code}`);
    } else {
      console.info('[dev-server] Bundle watcher stopped.');
    }
    bundleProcess = null;
  });
}

async function runExtractTree() {
  const extractScript = path.join(__dirname, 'extract-data.cjs');
  const result = spawnSync('node', [extractScript], { stdio: 'inherit' });
  if (result.status !== 0) {
    throw new Error(`extract-data.cjs exited with ${result.status}`);
  }
}

function copyLatestAnalysisJson() {
  if (!fs.existsSync(reportsDir)) {
    return null;
  }

  ensureDir(devDataDir);

  const candidates = fs
    .readdirSync(reportsDir)
    .filter((name) => name.toLowerCase().endsWith('.json'))
    .map((name) => path.join(reportsDir, name))
    .sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs);

  if (candidates.length === 0) {
    return null;
  }

  const latest = candidates[0];
  try {
    fs.copyFileSync(latest, analysisJsonPath);
    console.log(
      `📄 Copied latest analysis JSON (${path.basename(latest)}) → ${analysisJsonPath}`
    );
    return analysisJsonPath;
  } catch (error) {
    console.warn('[dev-server] Unable to copy analysis JSON:', error.message);
    return null;
  }
}

function ensureDir(dir) {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
}

let renderInFlight = false;
let rerunScheduled = false;

async function refreshDataAndRender() {
  if (renderInFlight) {
    rerunScheduled = true;
    return;
  }

  renderInFlight = true;
  try {
    await runExtractTree();
    const copiedPath = copyLatestAnalysisJson();
    if (!copiedPath && !fs.existsSync(analysisJsonPath)) {
      console.warn('[dev-server] analysis.json not found; templates will use stub data until JSON output is provided.');
    }
    runRender();
  } catch (error) {
    console.error('[dev-server] Refresh error:', error.message || error);
  } finally {
    renderInFlight = false;
    if (rerunScheduled) {
      rerunScheduled = false;
      refreshDataAndRender();
    }
  }
}

function runRender() {
  const renderScript = path.join(__dirname, 'render-report.cjs');
  const result = spawnSync('node', [renderScript], { stdio: 'inherit' });
  if (result.status !== 0) {
    console.error('[dev-server] render-report.cjs failed');
  }
}

function isPortAvailable(port, host = '0.0.0.0') {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();

    server.on('error', (err) => {
      if (err.code === 'EADDRINUSE' || err.code === 'EACCES') {
        resolve(false);
      } else {
        reject(err);
      }
    });

    server.listen({ port, host }, () => {
      const { port: boundPort } = server.address();
      server.close(() => resolve(boundPort === port));
    });
  });
}

async function findAvailablePort(preferred) {
  const start = Number(preferred) || 8080;
  const maxAttempts = 100;

  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    const candidate = start + attempt;
    // eslint-disable-next-line no-await-in-loop
    const available = await isPortAvailable(candidate);
    if (available) {
      return candidate;
    }
  }

  throw new Error(`Unable to find a free port starting at ${start}`);
}

async function launchDevServer() {
  startBundleWatch();
  try {
    await refreshDataAndRender();
  } catch (error) {
    console.error('[dev-server] Initial data refresh failed:', error.message || error);
  }

  setupWatchers();

  const preferredPort = process.env.PORT || process.env.DEV_SERVER_PORT || 8083;
  const port = await findAvailablePort(preferredPort);

  if (String(port) !== String(preferredPort)) {
    console.info(`ℹ️  Port ${preferredPort} in use. Using ${port} instead.`);
  }

  const webpackCli = require.resolve('webpack-cli/bin/cli.js');
  const child = spawn(
    process.execPath,
    [
      webpackCli,
      'serve',
      '--mode=development',
      '--port',
      String(port),
    ],
    {
      stdio: 'inherit',
      env: {
        ...process.env,
        PORT: String(port),
        DEV_SERVER_PORT: String(port)
      }
    }
  );

  child.on('exit', (code, signal) => {
    if (signal) {
      shutdownBundleWatch();
      process.kill(process.pid, signal);
    } else {
      shutdownBundleWatch();
      process.exit(code ?? 0);
    }
  });
}

launchDevServer().catch((err) => {
  console.error('Error starting dev server:', err.message || err);
  process.exit(1);
});

function debounce(fn, wait = 150) {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), wait);
  };
}

function setupWatchers() {
  const rerender = debounce(() => runRender());
  const partialWatcher = chokidar.watch(
    [path.join(__dirname, '../../partials/**/*.hbs'), path.join(__dirname, '../../report.hbs')],
    { ignoreInitial: true }
  );

  partialWatcher.on('all', (event, filePath) => {
    console.log(`[dev-server] Template change detected (${event} ${filePath}); re-rendering.`);
    rerender();
  });

  const analysisWatcher = chokidar.watch(path.join(devDataDir, '*.json'), {
    ignoreInitial: true,
  });
  analysisWatcher.on('all', (event, filePath) => {
    console.log(`[dev-server] Analysis JSON updated (${event} ${filePath}); re-rendering.`);
    rerender();
  });

  const reportsWatcher = chokidar.watch(
    [path.join(reportsDir, '**/*.html'), path.join(reportsDir, '**/*.json')],
    { ignoreInitial: true }
  );

  const refresh = debounce(() => {
    console.log('[dev-server] Source reports changed; refreshing tree data and analysis.');
    refreshDataAndRender();
  }, 250);

  reportsWatcher.on('all', refresh);

  const bundleWatcher = chokidar.watch(
    path.join(__dirname, '../..', 'assets', 'react-tree-bundle.js'),
    { ignoreInitial: true }
  );

  bundleWatcher.on('all', (event, filePath) => {
    console.log(`[dev-server] Bundle updated (${event} ${filePath}); re-rendering.`);
    rerender();
  });
}

function shutdownBundleWatch() {
  if (bundleProcess && !bundleProcess.killed) {
    bundleProcess.kill();
    bundleProcess = null;
  }
}

['SIGINT', 'SIGTERM', 'exit'].forEach((event) => {
  process.on(event, () => {
    shutdownBundleWatch();
    if (event !== 'exit') {
      process.exit();
    }
  });
});
