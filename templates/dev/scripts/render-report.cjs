const fs = require('fs');
const path = require('path');
const handlebars = require('handlebars');

const ROOT_DIR = path.resolve(__dirname, '..');
const TEMPLATE_ROOT = path.resolve(__dirname, '../../');
const PROJECT_ROOT = path.resolve(TEMPLATE_ROOT, '..');
const PARTIALS_DIR = path.join(TEMPLATE_ROOT, 'partials');
const ASSETS_DIR = path.join(TEMPLATE_ROOT, 'assets');
const OUTPUT_DIR = path.join(ROOT_DIR, 'public');
const DATA_DIR = path.join(ROOT_DIR, 'data');
const ANALYSIS_JSON = path.join(DATA_DIR, 'analysis.json');
const OUTPUT_HTML = path.join(OUTPUT_DIR, 'report-dev.html');
const OUTPUT_DATA_JSON = path.join(OUTPUT_DIR, 'data.json');
const WEB_ASSETS_DIR = path.join(ASSETS_DIR, 'webpage_files');
const OUTPUT_WEB_ASSETS_DIR = path.join(OUTPUT_DIR, 'webpage_files');

const PRIORITY_ORDER = {
  critical: 0,
  high: 1,
  medium: 2,
  low: 3,
};

function roundTo(value, digits = 1) {
  const num = Number(value);
  if (!Number.isFinite(num)) {
    return 0;
  }
  const factor = 10 ** digits;
  return Math.round(num * factor) / factor;
}

function ensureDir(dir) {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
}

function copyFileIfDifferent(source, destination) {
  if (!fs.existsSync(source)) {
    return;
  }

  const destDir = path.dirname(destination);
  ensureDir(destDir);

  try {
    const sourceStat = fs.statSync(source);
    if (fs.existsSync(destination)) {
      const destStat = fs.statSync(destination);
      if (
        destStat.size === sourceStat.size &&
        destStat.mtimeMs >= sourceStat.mtimeMs
      ) {
        return;
      }
    }
  } catch (error) {
    // Ignore stat errors; we'll attempt to copy below
  }

  fs.copyFileSync(source, destination);
}

function copyStaticAssets() {
  if (fs.existsSync(WEB_ASSETS_DIR)) {
    ensureDir(OUTPUT_WEB_ASSETS_DIR);
    const entries = fs.readdirSync(WEB_ASSETS_DIR, { withFileTypes: true });
    entries.forEach((entry) => {
      if (!entry.isFile()) {
        return;
      }
      const source = path.join(WEB_ASSETS_DIR, entry.name);
      const destination = path.join(OUTPUT_WEB_ASSETS_DIR, entry.name);
      copyFileIfDifferent(source, destination);
    });
  } else {
    console.warn('[render-report] webpage assets directory missing:', WEB_ASSETS_DIR);
  }
}

function readJson(filePath) {
  try {
    const raw = fs.readFileSync(filePath, 'utf8');
    return JSON.parse(raw);
  } catch (error) {
    return null;
  }
}

function normalizePath(value) {
  if (typeof value !== 'string' || value.length === 0) {
    return value;
  }

  let candidate = value;

  if (path.isAbsolute(candidate)) {
    const rel = path.relative(PROJECT_ROOT, candidate);
    if (!rel.startsWith('..')) {
      candidate = rel;
    }
  }

  if (candidate.startsWith('./')) {
    candidate = candidate.slice(2);
  }

  return candidate.replace(/\\/g, '/');
}

function normalizeEntityId(entityId = '') {
  if (typeof entityId !== 'string' || entityId.length === 0) {
    return entityId;
  }

  const parts = entityId.split(':');
  if (parts.length === 0) {
    return entityId;
  }

  parts[0] = normalizePath(parts[0]);
  return parts.join(':');
}

function simplifyEntityName(name = '') {
  if (typeof name !== 'string') {
    return name;
  }
  const parts = name.split(':');
  return parts[parts.length - 1] || name;
}

function cleanRefactoringCandidates(candidates = []) {
  return candidates.map((candidate) => ({
    ...candidate,
    entity_id: normalizeEntityId(candidate.entity_id || ''),
    name: simplifyEntityName(candidate.name || candidate.entity_id || ''),
    file_path: normalizePath(candidate.file_path || ''),
    filePath: normalizePath(candidate.file_path || ''),
    line_range: candidate.line_range ?? candidate.lineRange ?? null,
    lineRange: candidate.line_range ?? candidate.lineRange ?? null,
    score: roundTo(candidate.score ?? 0),
  }));
}

function cleanGroups(groups = []) {
  return groups.map((group) => {
    const normalizedFilePath = normalizePath(group.file_path || '');
    const entities = cleanRefactoringCandidates(group.entities || []);
    const highestPriority = group.highest_priority ?? group.highestPriority ?? 'Low';
    const rawAvgScore = group.avg_score ?? group.avgScore ?? 0;
    const avgScore = roundTo(rawAvgScore);
    const totalIssues = group.total_issues ?? group.totalIssues ?? 0;
    const entityCount = group.entity_count ?? group.entityCount ?? entities.length;

    return {
      ...group,
      file_path: normalizedFilePath,
      filePath: normalizedFilePath,
      file_name: group.file_name || path.basename(normalizedFilePath),
      fileName: group.file_name || path.basename(normalizedFilePath),
      highest_priority: highestPriority,
      highestPriority,
      avg_score: avgScore,
      avgScore,
      total_issues: totalIssues,
      totalIssues,
      entity_count: entityCount,
      entityCount,
      entities,
    };
  });
}

function cleanDirectoryTree(tree) {
  if (!tree) return null;
  const clone = JSON.parse(JSON.stringify(tree));

  if (clone.root) {
    clone.root.path = normalizePath(clone.root.path || '');
    if (Array.isArray(clone.root.children)) {
      clone.root.children = clone.root.children.map((child) => normalizePath(child || ''));
    }
    if (clone.root.parent) {
      clone.root.parent = normalizePath(clone.root.parent);
    }
  }

  if (clone.directories) {
    const newDirs = {};
    Object.entries(clone.directories).forEach(([key, value]) => {
      const cleanedKey = normalizePath(key);
      const cleanedValue = {
        ...value,
        path: normalizePath(value.path || ''),
        parent: value.parent ? normalizePath(value.parent) : value.parent,
        children: Array.isArray(value.children)
          ? value.children.map((child) => normalizePath(child || ''))
          : value.children,
      };
      newDirs[cleanedKey] = cleanedValue;
    });
    clone.directories = newDirs;
  }

  if (clone.tree_statistics && Array.isArray(clone.tree_statistics.hotspot_directories)) {
    clone.tree_statistics.hotspot_directories = clone.tree_statistics.hotspot_directories.map((hotspot) => ({
      ...hotspot,
      path: normalizePath((hotspot && hotspot.path) || ''),
    }));
  }

  return clone;
}

function cleanUnifiedHierarchy(nodes) {
  if (!Array.isArray(nodes)) {
    return [];
  }

  return nodes.map((node) => {
    const cleaned = { ...node };

    if (cleaned.path) {
      cleaned.path = normalizePath(cleaned.path);
    }

    if (cleaned.file_path) {
      cleaned.file_path = normalizePath(cleaned.file_path);
    }

    if (typeof cleaned.id === 'string') {
      const normalizedProjectRoot = PROJECT_ROOT.replace(/\\/g, '/');
      cleaned.id = cleaned.id.replace(normalizedProjectRoot, '').replace(/\\/g, '/');
    }

    if (Array.isArray(cleaned.children)) {
      cleaned.children = cleanUnifiedHierarchy(cleaned.children);
    }

    return cleaned;
  });
}

function summarizeIssueCategories(directories = {}) {
  const totals = new Map();

  Object.values(directories).forEach((dir) => {
    const categories = dir?.issue_categories || {};
    Object.values(categories).forEach((category) => {
      const key = category?.category || 'uncategorized';
      const affected = category?.affected_entities ?? 0;
      const healthImpact = category?.health_impact ?? 0;
      const avgSeverity = category?.avg_severity ?? 0;
      const maxSeverity = category?.max_severity ?? 0;

      if (!totals.has(key)) {
        totals.set(key, {
          category: key,
          affectedEntities: 0,
          totalSeverity: 0,
          healthImpact: 0,
          maxSeverity: 0,
          directories: 0,
        });
      }

      const entry = totals.get(key);
      entry.affectedEntities += affected;
      entry.totalSeverity += avgSeverity * (affected || 1);
      entry.healthImpact += healthImpact;
      entry.maxSeverity = Math.max(entry.maxSeverity, maxSeverity);
      entry.directories += 1;
    });
  });

  return Array.from(totals.values())
    .map(({ totalSeverity, affectedEntities, ...rest }) => ({
      ...rest,
      avgSeverity:
        affectedEntities > 0 ? totalSeverity / affectedEntities : 0,
    }))
    .sort((a, b) => b.healthImpact - a.healthImpact);
}

function buildGraphInsights(tree) {
  if (!tree) {
    return {
      hasInsights: false,
      hasHotspots: false,
      hasCategorySummary: false,
      hotspots: [],
      category_summary: [],
    };
  }

  const directories = tree.directories || {};
  const stats = tree.tree_statistics || {};
  const hotspotEntries = Array.isArray(stats.hotspot_directories)
    ? stats.hotspot_directories
    : [];

  const resolveDirectory = (pathValue) => {
    if (!pathValue) {
      return null;
    }
    const normalized = normalizePath(pathValue);
    return (
      directories[normalized] ||
      directories[pathValue] ||
      Object.values(directories).find((dir) => dir?.path === normalized)
    );
  };

  const hotspots = hotspotEntries.map((hotspot) => {
    const directoryInfo = resolveDirectory(hotspot.path);
    const categories = directoryInfo?.issue_categories || {};
    const categoryList = Object.values(categories)
      .map((category) => ({
        category: category?.category || 'uncategorized',
        affectedEntities: category?.affected_entities ?? 0,
        avgSeverity: category?.avg_severity ?? 0,
        maxSeverity: category?.max_severity ?? 0,
        healthImpact: category?.health_impact ?? 0,
      }))
      .sort((a, b) => b.healthImpact - a.healthImpact);

    return {
      rank: hotspot?.rank ?? null,
      path: normalizePath(hotspot?.path || directoryInfo?.path || '.'),
      recommendation: hotspot?.recommendation || '',
      healthScore: directoryInfo?.health_score ?? hotspot?.health_score ?? null,
      primaryIssueCategory:
        hotspot?.primary_issue_category || categoryList[0]?.category || null,
      categories: categoryList,
    };
  });

  const categorySummary = summarizeIssueCategories(directories);

  return {
    hasInsights: hotspots.length > 0 || categorySummary.length > 0,
    hasHotspots: hotspots.length > 0,
    hasCategorySummary: categorySummary.length > 0,
    hotspots,
    category_summary: categorySummary,
  };
}

function normalizeCloneAnalysis(cloneAnalysis) {
  if (!cloneAnalysis) {
    return {
      hasData: false,
      notes: [],
    };
  }

  const avgSimilarity =
    typeof cloneAnalysis.avg_similarity === 'number'
      ? cloneAnalysis.avg_similarity
      : null;
  const maxSimilarity =
    typeof cloneAnalysis.max_similarity === 'number'
      ? cloneAnalysis.max_similarity
      : null;
  const candidatesAfter = cloneAnalysis.candidates_after_denoising ?? null;
  const candidatesBefore = cloneAnalysis.candidates_before_denoising ?? null;

  return {
    hasData: true,
    denoisingEnabled: Boolean(cloneAnalysis.denoising_enabled),
    candidatesAfter,
    candidatesBefore,
    avgSimilarity,
    maxSimilarity,
    qualityScore:
      typeof cloneAnalysis.quality_score === 'number'
        ? cloneAnalysis.quality_score
        : avgSimilarity,
    notes: Array.isArray(cloneAnalysis.notes) ? cloneAnalysis.notes : [],
  };
}

function getPriorityRank(node) {
  const priority = node?.priority ?? node?.highest_priority ?? node?.highestPriority ?? '';
  const normalized = String(priority || '').trim().toLowerCase();
  return PRIORITY_ORDER[normalized] ?? 999;
}

function sortHierarchy(nodes) {
  if (!Array.isArray(nodes) || nodes.length === 0) {
    return [];
  }

  const sorted = [...nodes].sort((a, b) => {
    if (a?.type === 'folder' && b?.type !== 'folder') return -1;
    if (b?.type === 'folder' && a?.type !== 'folder') return 1;

    if (a?.type === 'folder' && b?.type === 'folder') {
      const aHealth = typeof a?.health_score === 'number' ? a.health_score : 1;
      const bHealth = typeof b?.health_score === 'number' ? b.health_score : 1;
      if (aHealth !== bHealth) return aHealth - bHealth;
    }

    const aRank = getPriorityRank(a);
    const bRank = getPriorityRank(b);
    if (aRank !== bRank) return aRank - bRank;

    const aScore = typeof a?.score === 'number' ? a.score : -Infinity;
    const bScore = typeof b?.score === 'number' ? b.score : -Infinity;
    if (aScore !== bScore) return bScore - aScore;

    const aName = String(a?.name || '').toLowerCase();
    const bName = String(b?.name || '').toLowerCase();
    return aName.localeCompare(bName);
  });

  return sorted.map((node) => ({
    ...node,
    children: sortHierarchy(node?.children || []),
  }));
}

function groupFilesByDirectory(groups) {
  const map = new Map();

  groups.forEach((group) => {
    const filePath = normalizePath(group.file_path || '');
    const dirPath = normalizePath(path.dirname(filePath));
    if (!map.has(dirPath)) {
      map.set(dirPath, []);
    }
    map.get(dirPath).push(group);
  });

  return map;
}

function severityBucket(value) {
  const score = Number(value);
  if (!Number.isFinite(score)) {
    return 'low';
  }

  if (score >= 80) {
    return 'critical';
  }
  if (score >= 60) {
    return 'high';
  }
  if (score >= 40) {
    return 'medium';
  }
  return 'low';
}

function priorityBucket(priority) {
  if (!priority) {
    return 'low';
  }
  const normalized = String(priority).toLowerCase();
  if (normalized.includes('critical')) return 'critical';
  if (normalized.includes('high')) return 'high';
  if (normalized.includes('medium')) return 'medium';
  return 'low';
}

function buildEntityNode(entity, codeDictionary, severityCounts) {
  const entityScore = roundTo(entity.score ?? 0);
  const issueDetails = (entity.issues || []).map((issue, index) => {
    const meta = codeDictionary?.issues?.[issue.code] || {};
    const severity = roundTo(issue.severity ?? 0);
    const bucket = severityBucket(severity);
    severityCounts[bucket] += 1;

    return {
      ...issue,
      code: issue.code,
      title: meta.title || issue.category,
      summary: meta.summary || `Signals detected in the ${issue.category} dimension.`,
      severity,
      badges: [`Severity ${severity.toFixed(1)}`],
    };
  });

  const suggestionDetails = (entity.suggestions || []).map((suggestion, index) => {
    const meta = codeDictionary?.suggestions?.[suggestion.code] || {};
    const bucket = priorityBucket(suggestion.priority);
    severityCounts[bucket] += 1;

    const badges = [];
    if (typeof suggestion.priority === 'number') {
      badges.push(`Priority ${(suggestion.priority * 100).toFixed(0)}%`);
    }
    if (typeof suggestion.impact === 'number') {
      badges.push(`Impact ${(suggestion.impact * 100).toFixed(0)}%`);
    }
    if (typeof suggestion.effort === 'number') {
      badges.push(`Effort ${(suggestion.effort * 100).toFixed(0)}%`);
    }

    return {
      ...suggestion,
      code: suggestion.code,
      title: meta.title || suggestion.refactoring_type,
      summary: meta.summary || suggestion.refactoring_type,
      badges,
    };
  });

  return {
    ...entity,
    type: 'entity',
    name: entity.name || simplifyEntityName(entity.entity_id || ''),
    lineRange: entity.lineRange ?? entity.line_range ?? null,
    line_range: entity.line_range ?? entity.lineRange ?? null,
    score: entityScore,
    issues: issueDetails,
    suggestions: suggestionDetails,
    children: [],
  };
}

function buildFileNode(group, codeDictionary) {
  const severityCounts = { critical: 0, high: 0, medium: 0, low: 0 };
  const entities = (group.entities || []).map((entity) =>
    buildEntityNode(entity, codeDictionary, severityCounts)
  );

  const normalizedPath = normalizePath(group.file_path || '');
  const name = group.file_name || path.basename(normalizedPath);
  const avgScore = roundTo(group.avg_score ?? group.avgScore ?? 0);

  return {
    ...group,
    file_path: normalizedPath,
    filePath: normalizedPath,
    file_name: name,
    fileName: name,
    name,
    path: normalizedPath,
    id: `file_${normalizedPath.replace(/[^a-zA-Z0-9_]/g, '_')}`,
    type: 'file',
    severityCounts,
    entities,
    children: entities,
    entity_count: group.entity_count ?? group.entityCount ?? entities.length,
    entityCount: group.entity_count ?? group.entityCount ?? entities.length,
    avg_score: avgScore,
    avgScore,
    total_issues: group.total_issues ?? group.totalIssues ?? 0,
    totalIssues: group.total_issues ?? group.totalIssues ?? 0,
    highest_priority: group.highest_priority ?? group.highestPriority ?? 'Low',
    highestPriority: group.highest_priority ?? group.highestPriority ?? 'Low',
  };
}

function buildDirectoryNode(dirPath, fileNodes) {
  const name = dirPath && dirPath !== '.' ? path.basename(dirPath) : '.';
  return {
    id: `directory_${dirPath.replace(/[^a-zA-Z0-9_]/g, '_')}`,
    type: 'folder',
    name,
    path: dirPath,
    children: fileNodes,
    entity_count: fileNodes.reduce((sum, node) => sum + (node.entity_count || node.entities?.length || 0), 0),
    file_count: fileNodes.length,
    health_score: 0,
    refactoring_needed: fileNodes.reduce((sum, node) => sum + (node.total_issues || 0), 0),
  };
}

function addFilesToHierarchy(baseHierarchy, groups, codeDictionary) {
  const filesByDir = groupFilesByDirectory(groups);

  const attach = (node) => {
    const pathValue = normalizePath(node.path || '');
    const childDirs = Array.isArray(node.children) ? node.children.map(attach) : [];
    const files = filesByDir.get(pathValue) || [];
    filesByDir.delete(pathValue);
    const fileNodes = files.map((group) => buildFileNode(group, codeDictionary));

    return {
      ...node,
      path: pathValue,
      children: [...childDirs, ...fileNodes],
    };
  };

  let hierarchy = Array.isArray(baseHierarchy) ? baseHierarchy.map((node) => attach(node)) : [];

  filesByDir.forEach((fileList, dirPath) => {
    if (!fileList || fileList.length === 0) {
      return;
    }
    const fileNodes = fileList.map((group) => buildFileNode(group, codeDictionary));
    hierarchy.push(buildDirectoryNode(dirPath, fileNodes));
  });

  return hierarchy;
}

function buildSummary(results) {
  const summary = results?.summary || {};
  const filesProcessed = summary.files_processed ?? summary.total_files ?? 0;
  const entitiesAnalyzed = summary.entities_analyzed ?? summary.total_entities ?? 0;
  const refactoringNeeded = summary.refactoring_needed ?? results?.refactoring_candidates?.length ?? 0;
  const codeHealth = summary.code_health_score ?? 0;

  return {
    files_processed: filesProcessed,
    entities_analyzed: entitiesAnalyzed,
    refactoring_needed: refactoringNeeded,
    code_health_score: codeHealth,
    total_files: summary.total_files ?? filesProcessed,
    total_issues: summary.total_issues ?? refactoringNeeded,
    high_priority: summary.high_priority ?? 0,
    critical: summary.critical ?? 0,
    avg_refactoring_score: summary.avg_refactoring_score ?? 0,
    complexity_score: Number(((summary.avg_refactoring_score ?? 0) * 100).toFixed(1)),
    maintainability_index: Number(((summary.code_health_score ?? 0) * 100).toFixed(1)),
  };
}

function buildTemplateData(results) {
  if (!results) {
    return {
      generated_at: new Date().toISOString(),
      tool_name: 'Valknut',
      version: 'dev',
      theme_css_url: 'sibylline.css',
      enable_animation: true,
      has_oracle_data: false,
      summary: buildSummary({}),
      results: {},
      refactoring_candidates: [],
      refactoring_candidates_by_file: [],
      file_count: 0,
      directory_health_tree: null,
      unified_hierarchy: [],
      coverage_packs: [],
      code_dictionary: { issues: {}, suggestions: {} },
      warnings: [],
    };
  }

  const cleanedCandidates = cleanRefactoringCandidates(results.refactoring_candidates);
  const cleanedGroups = cleanGroups(results.refactoring_candidates_by_file);
  const cleanedTree = cleanDirectoryTree(results.directory_health_tree);
  const summary = buildSummary(results);

  const dictionary = results.code_dictionary || { issues: {}, suggestions: {} };
  const hierarchyWithFiles = addFilesToHierarchy(
    Array.isArray(results.unified_hierarchy) ? results.unified_hierarchy : [],
    cleanedGroups,
    dictionary
  );
  const sortedHierarchy = sortHierarchy(hierarchyWithFiles);
  const graphInsights = buildGraphInsights(cleanedTree);
  const cloneAnalysis = normalizeCloneAnalysis(results.clone_analysis);

  return {
    generated_at: new Date().toISOString(),
    tool_name: 'Valknut',
    version: process.env.VALKNUT_VERSION || 'dev',
    theme_css_url: 'sibylline.css',
    enable_animation: true,
    results,
    summary,
    refactoring_candidates: cleanedCandidates,
    refactoring_candidates_by_file: cleanedGroups,
    file_count: cleanedGroups.length,
    directory_health_tree: cleanedTree,
    unified_hierarchy: cleanUnifiedHierarchy(sortedHierarchy),
    coverage_packs: results.coverage_packs || [],
    code_dictionary: dictionary,
    codeDictionary: dictionary,
    warnings: results.warnings || [],
    oracle_refactoring_plan: results.oracle_refactoring_plan || null,
    has_oracle_data: Boolean(results.oracle_refactoring_plan),
    health_metrics: results.health_metrics || null,
    graph_insights: graphInsights,
    clone_analysis: cloneAnalysis,
    clone_analysis_raw: results.clone_analysis || null,
  };
}

function registerHelpers() {
  handlebars.registerHelper('json', (context) => JSON.stringify(context, null, 2));

  handlebars.registerHelper('format', (value, formatStr = '0.1') => {
    const num = Number(value);
    if (Number.isNaN(num)) return '';
    switch (formatStr) {
      case '0.0':
        return num.toFixed(0);
      case '0.2':
        return num.toFixed(2);
      default:
        return num.toFixed(1);
    }
  });

  handlebars.registerHelper('percentage', (value, decimals = '0') => {
    const num = Number(value);
    if (Number.isNaN(num)) return '0';
    const multiplied = num * 100;
    const digits = parseInt(decimals, 10);
    if (Number.isNaN(digits) || digits <= 0) {
      return multiplied.toFixed(0);
    }
    return multiplied.toFixed(digits);
  });

  handlebars.registerHelper('multiply', (a, b) => {
    const numA = Number(a);
    const numB = Number(b);
    if (Number.isNaN(numA) || Number.isNaN(numB)) return '';
    return (numA * numB).toString();
  });

  handlebars.registerHelper('capitalize', (value) => {
    if (typeof value !== 'string' || value.length === 0) return value;
    return value.charAt(0).toUpperCase() + value.slice(1);
  });

  handlebars.registerHelper('replace', (value, search, replacement) => {
    if (typeof value !== 'string') return value;
    return value.split(search).join(replacement ?? '');
  });

  handlebars.registerHelper('subtract', (a, b) => {
    const numA = Number(a);
    const numB = Number(b);
    if (Number.isNaN(numA) || Number.isNaN(numB)) return '';
    return (numA - numB).toString();
  });

  handlebars.registerHelper('add', (a, b) => {
    const numA = Number(a);
    const numB = Number(b);
    if (Number.isNaN(numA) || Number.isNaN(numB)) return '';
    return (numA + numB).toString();
  });

  handlebars.registerHelper('gt', (a, b) => {
    const numA = Number(a);
    const numB = Number(b);
    if (Number.isNaN(numA) || Number.isNaN(numB)) return false;
    return numA > numB;
  });

  handlebars.registerHelper('length', (arr) => {
    if (!Array.isArray(arr)) return 0;
    return arr.length;
  });

handlebars.registerHelper('inline_css', (filePath) => inlineCss(filePath));
handlebars.registerHelper('inline_js', (filePath) => inlineJs(filePath));

  handlebars.registerHelper('logo_data_url', () => {
    const candidates = [
      path.join(ASSETS_DIR, 'webpage_files/valknut-large.webp'),
      path.join(ASSETS_DIR, 'logo.webp'),
    ];
    for (const candidate of candidates) {
      if (fs.existsSync(candidate)) {
        const content = fs.readFileSync(candidate);
        const base64 = content.toString('base64');
        return `data:image/webp;base64,${base64}`;
      }
    }
    return '';
  });
}

function inlineCss(filePath) {
  const candidates = [
    path.join(TEMPLATE_ROOT, 'themes', filePath),
    path.join(TEMPLATE_ROOT, '..', 'themes', filePath),
    path.join(TEMPLATE_ROOT, filePath),
    path.join(ASSETS_DIR, filePath),
    path.join(ASSETS_DIR, '.valknut', filePath),
  ];

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      const content = fs.readFileSync(candidate, 'utf8');
      return new handlebars.SafeString(content);
    }
  }

  if (filePath.includes('sibylline.css')) {
    const fallback = `:root {\n  --font-family-default: 'Inter', sans-serif;\n  --text: #f8fafc;\n  --background: #0f172a;\n}\nbody {\n  font-family: var(--font-family-default);\n  background: var(--background);\n  color: var(--text);\n}`;
    return new handlebars.SafeString(fallback);
  }

  console.warn(`[render-report] CSS asset not found: ${filePath}`);
  return new handlebars.SafeString('');
}

function inlineJs(filePath) {
  const candidates = [
    path.join(ASSETS_DIR, filePath),
    path.join(ASSETS_DIR, 'dist', filePath),
    path.join(ASSETS_DIR, '.valknut', filePath),
    path.join(ROOT_DIR, 'dist', filePath),
    path.join(ROOT_DIR, 'public', filePath),
  ];

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      const content = fs.readFileSync(candidate, 'utf8');
      const sanitized = content.replace(/<\/script/gi, '<\\/script');
      return new handlebars.SafeString(sanitized);
    }
  }

  console.warn(`[render-report] JS asset not found: ${filePath}`);
  return new handlebars.SafeString('');
}

function registerPartials() {
  if (!fs.existsSync(PARTIALS_DIR)) return;
  const files = fs.readdirSync(PARTIALS_DIR).filter((file) => file.endsWith('.hbs'));
  files.forEach((file) => {
    const partialName = path.basename(file, '.hbs');
    const partialContent = fs.readFileSync(path.join(PARTIALS_DIR, file), 'utf8');
    handlebars.registerPartial(partialName, partialContent);
  });
}

function loadAnalysisData() {
  if (fs.existsSync(ANALYSIS_JSON)) {
    const data = readJson(ANALYSIS_JSON);
    if (data) {
      return data;
    }
  }

  console.warn('[render-report] analysis.json not found or invalid, using fallback stub data.');
  return {
    summary: {
      files_processed: 0,
      entities_analyzed: 0,
      refactoring_needed: 0,
      code_health_score: 0,
    },
    refactoring_candidates: [],
    refactoring_candidates_by_file: [],
    unified_hierarchy: [],
    coverage_packs: [],
    code_dictionary: { issues: {}, suggestions: {} },
    warnings: ['analysis.json missing - rendered with stub data'],
  };
}

function render() {
  ensureDir(OUTPUT_DIR);
  copyStaticAssets();
  const analysis = loadAnalysisData();
  const templateData = buildTemplateData(analysis);

  registerHelpers();
  registerPartials();

  const templatePath = path.join(TEMPLATE_ROOT, 'report.hbs');
  const templateSource = fs.readFileSync(templatePath, 'utf8');
  const template = handlebars.compile(templateSource, { noEscape: true });

  const html = template(templateData);
  fs.writeFileSync(OUTPUT_HTML, html);
  console.log(`[render-report] Rendered report to ${OUTPUT_HTML}`);

  const frontendPayload = {
    unifiedHierarchy: templateData.unified_hierarchy,
    refactoringCandidatesByFile: templateData.refactoring_candidates_by_file,
    directoryHealthTree: templateData.directory_health_tree,
    coveragePacks: templateData.coverage_packs,
    code_dictionary: templateData.code_dictionary,
    codeDictionary: templateData.code_dictionary,
    graphInsights: templateData.graph_insights,
    cloneAnalysis: templateData.clone_analysis,
  };

  fs.writeFileSync(OUTPUT_DATA_JSON, JSON.stringify(frontendPayload, null, 2));
}

if (require.main === module) {
  render();
} else {
  module.exports = {
    render,
    renderReport: render,
    buildTemplateData,
    loadAnalysisData,
    registerHelpers,
    registerPartials,
  };
}
