const fs = require('fs');
const path = require('path');
const cheerio = require('cheerio');

// Paths relative to scripts/
const reportsDir = path.join(__dirname, '../../../.valknut');
const preferredReport = path.join(reportsDir, 'team_report.html');
const publicDataPath = path.join(__dirname, '../public/data.json');
const localDataDir = path.join(__dirname, '../data');
const localTreeDataPath = path.join(localDataDir, 'tree-data.json');

try {
  if (!fs.existsSync(reportsDir)) {
    console.error(`Error: ${reportsDir} not found. Run 'cargo run --bin valknut analyze . --format html' first.`);
    process.exit(1);
  }

  let htmlPath = preferredReport;

  if (!fs.existsSync(htmlPath)) {
    const htmlCandidates = fs
      .readdirSync(reportsDir)
      .filter((name) => name.toLowerCase().endsWith('.html'))
      .map((name) => path.join(reportsDir, name));

    if (htmlCandidates.length === 0) {
      console.error(
        `Error: No HTML reports found under ${reportsDir}. Run 'cargo run --bin valknut analyze . --format html' first.`
      );
      process.exit(1);
    }

    htmlCandidates.sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs);
    htmlPath = htmlCandidates[0];
  }

  const html = fs.readFileSync(htmlPath, 'utf8');
  const $ = cheerio.load(html);
  const dataScript = $('#tree-data').text().trim();

  if (!dataScript) {
    console.error('Error: No #tree-data script found in HTML.');
    process.exit(1);
  }

  const data = JSON.parse(dataScript);

  // Ensure output directories exist
  const publicDir = path.dirname(publicDataPath);
  if (!fs.existsSync(publicDir)) {
    fs.mkdirSync(publicDir, { recursive: true });
  }

  if (!fs.existsSync(localDataDir)) {
    fs.mkdirSync(localDataDir, { recursive: true });
  }

  fs.writeFileSync(publicDataPath, JSON.stringify(data, null, 2));
  fs.writeFileSync(localTreeDataPath, JSON.stringify(data, null, 2));

  console.log(`✅ Extracted analysis data from ${path.basename(htmlPath)} to ${publicDataPath}`);
  console.log(`📦 Local tree data snapshot saved to ${localTreeDataPath}`);
  console.log(`Data keys: ${Object.keys(data).join(', ')}`);
} catch (error) {
  console.error('Error extracting data:', error.message);
  process.exit(1);
}
