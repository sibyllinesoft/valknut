const fs = require('fs');
const path = require('path');
const cheerio = require('cheerio');

// Paths relative to scripts/
const htmlPath = path.join(__dirname, '../../../.valknut/team_report.html');
const dataPath = path.join(__dirname, '../public/data.json');

try {
  if (!fs.existsSync(htmlPath)) {
    console.error(`Error: ${htmlPath} not found. Run 'cargo run --bin valknut analyze . --format html' first.`);
    process.exit(1);
  }

  const html = fs.readFileSync(htmlPath, 'utf8');
  const $ = cheerio.load(html);
  const dataScript = $('#tree-data').text().trim();

  if (!dataScript) {
    console.error('Error: No #tree-data script found in HTML.');
    process.exit(1);
  }

  const data = JSON.parse(dataScript);

  // Ensure public dir exists
  const publicDir = path.dirname(dataPath);
  if (!fs.existsSync(publicDir)) {
    fs.mkdirSync(publicDir, { recursive: true });
  }

  fs.writeFileSync(dataPath, JSON.stringify(data, null, 2));
  console.log(`✅ Extracted analysis data to ${dataPath}`);
  console.log(`Data keys: ${Object.keys(data).join(', ')}`);
} catch (error) {
  console.error('Error extracting data:', error.message);
  process.exit(1);
}