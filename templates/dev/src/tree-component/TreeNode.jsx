import React, { useEffect, useRef } from 'react';
import Tooltip from '../components/Tooltip.jsx';

/**
 * Individual node renderer for the Valknut analysis tree.
 * Handles folders, files, entities, and supporting detail rows.
 */
const renderChevronIcon = () => {
    const pathD = 'M9 6l6 6-6 6';
    return React.createElement('svg', {
        key: 'chevron-svg',
        xmlns: 'http://www.w3.org/2000/svg',
        width: 16,
        height: 16,
        viewBox: '0 0 24 24',
        fill: 'none',
        stroke: 'currentColor',
        strokeWidth: 2,
        strokeLinecap: 'round',
        strokeLinejoin: 'round',
        className: 'chevron-icon'
    },
        React.createElement('path', { d: pathD })
    );
};

const formatDecimal = (value, decimals = 1) => {
    const numeric = Number(value);
    if (!Number.isFinite(numeric)) {
        return null;
    }
    return numeric.toFixed(decimals);
};

// Build VS Code link for file/entity
const buildVSCodeLink = (filePath, lineRange, projectRoot, lineNumber) => {
    if (!filePath) return null;
    // Build absolute path using projectRoot if path is relative
    let absPath;
    if (filePath.startsWith('/')) {
        absPath = filePath;
    } else if (projectRoot) {
        // Join projectRoot with relative path
        absPath = projectRoot.endsWith('/')
            ? `${projectRoot}${filePath}`
            : `${projectRoot}/${filePath}`;
    } else {
        absPath = `/${filePath}`;
    }
    // Encode the path for URI (but keep slashes unencoded)
    const encodedPath = absPath.split('/').map(segment => encodeURIComponent(segment)).join('/');
    let uri = `vscode://file${encodedPath}`;
    // Add line number if available - check lineRange array first, then fallback to lineNumber
    if (Array.isArray(lineRange) && lineRange.length > 0 && lineRange[0] > 0) {
        uri += `:${lineRange[0]}`;
    } else if (typeof lineNumber === 'number' && lineNumber > 0) {
        uri += `:${lineNumber}`;
    }
    console.log('[VSCode Link]', { filePath, projectRoot, absPath, lineNumber, uri });
    return uri;
};

const getNumericValue = (source, keys, fallback = null) => {
    if (!source || typeof source !== 'object') {
        return fallback;
    }
    for (const key of keys) {
        const raw = source[key];
        if (raw === null || raw === undefined || raw === '') continue;
        const numeric = Number(raw);
        if (Number.isFinite(numeric)) {
            return numeric;
        }
    }
    return fallback;
};

const combineSeverityCounts = (base = {}, extra = {}) => ({
    critical: (base.critical || 0) + (extra.critical || 0),
    high: (base.high || 0) + (extra.high || 0),
    medium: (base.medium || 0) + (extra.medium || 0),
    low: (base.low || 0) + (extra.low || 0),
});

// Normalized metric helpers (100% = acceptable baseline)
const clampValue = (v, min, max) => Math.min(max, Math.max(min, v));
const fmtPct = (value) => {
    if (value === null || value === undefined || Number.isNaN(value)) return null;
    const rounded = Math.round(value);
    return `${rounded}%`;
};
const ccPct = (value) => value != null ? (value / 10) * 100 : null;
const cogPct = (value, lang = 'default') => {
    const base = ['c', 'cpp', 'c++', 'objc', 'objective-c'].includes((lang || '').toLowerCase()) ? 25 : 15;
    return value != null ? (value / base) * 100 : null;
};
const miPct = (value) => value != null ? (60 / clampValue(value, 10, 100)) * 100 : null;
const tdPct = (value) => value; // already percent
const THRESHOLDS = {
    cyclomatic_complexity: 10,
    cognitive_complexity: 15,
    technical_debt_score: 40,
};

const getComplexityRatio = (nodeLike) => {
    if (!nodeLike || typeof nodeLike !== 'object') return null;
    const issues = Array.isArray(nodeLike.issues) ? nodeLike.issues : [];
    let maxRatio = null;

    issues.forEach((issue) => {
        const feats = Array.isArray(issue.contributing_features)
            ? issue.contributing_features
            : [];
        feats.forEach((feat) => {
            const name = String(feat.feature_name || '').toLowerCase();
            const value = Number(feat.value);
            if (!Number.isFinite(value)) return;
            const thresholdEntry = Object.entries(THRESHOLDS).find(([key]) =>
                name.includes(key)
            );
            if (!thresholdEntry) return;
            const [, threshold] = thresholdEntry;
            if (threshold <= 0) return;
            const ratio = value / threshold;
            if (ratio > (maxRatio ?? -Infinity)) {
                maxRatio = ratio;
            }
        });
    });

    return maxRatio;
};

const formatAcceptableRatio = (ratio) => {
    if (!Number.isFinite(ratio)) return null;
    return `${(ratio * 100).toFixed(0)}%`;
};

const getMaxComplexityRatio = (node) => {
    let maxRatio = getComplexityRatio(node);
    if (Array.isArray(node?.children)) {
        node.children.forEach((child) => {
            const childRatio = getMaxComplexityRatio(child);
            if (childRatio != null && childRatio > (maxRatio ?? -Infinity)) {
                maxRatio = childRatio;
            }
        });
    }
    return maxRatio;
};

const computeAggregates = (node) => {
    if (!node || typeof node !== 'object') {
        return {
            totalIssues: 0,
            severityCounts: { critical: 0, high: 0, medium: 0, low: 0 },
            entityCount: 0,
            fileCount: 0,
            avgScore: null,
            scoreWeight: 0,
        };
    }

    if (node.__aggregateCache) {
        return node.__aggregateCache;
    }

    const children = Array.isArray(node.children) ? node.children : [];
    const isEntity = node.type === 'entity';
    const isFile = node.type === 'file';
    const isFolder = node.type === 'folder';

    const severityCounts = combineSeverityCounts(
        node.severityCounts || node.severity_counts,
        {}
    );

    const directIssues = Array.isArray(node.issues) ? node.issues.length : 0;
    let totalIssues = getNumericValue(node, [
        'totalIssues',
        'total_issues',
        'refactoringNeeded',
        'refactoring_needed',
        'issueCount',
        'issue_count',
    ], 0) || 0;
    if (totalIssues === 0 && directIssues > 0) {
        totalIssues = directIssues;
    } else if (directIssues > totalIssues) {
        totalIssues = directIssues;
    }

    let entityCount = 0;
    if (isEntity) {
        entityCount = 1;
    }

    let fileCount = 0;
    if (isFile) {
        fileCount = 1;
    }

    let scoreSum = 0;
    let scoreWeight = 0;

    const nodeScore = getNumericValue(node, ['avgScore', 'avg_score', 'score'], null);
    if (nodeScore != null) {
        if (isEntity) {
            scoreSum += nodeScore;
            scoreWeight += 1;
        } else if (isFile) {
            const entityWeight = getNumericValue(node, ['entityCount', 'entity_count'], 0) || children.length || 1;
            scoreSum += nodeScore * entityWeight;
            scoreWeight += entityWeight;
        } else if (!children.length) {
            scoreSum += nodeScore;
            scoreWeight += 1;
        }
    }

    children.forEach((child) => {
        const childAggregates = computeAggregates(child);
        totalIssues += childAggregates.totalIssues;
        entityCount += childAggregates.entityCount;
        fileCount += childAggregates.fileCount;
        scoreSum += childAggregates._scoreSum;
        scoreWeight += childAggregates.scoreWeight;
        Object.assign(
            severityCounts,
            combineSeverityCounts(severityCounts, childAggregates.severityCounts)
        );
    });

    if (isFolder && !children.length) {
        fileCount = getNumericValue(node, ['fileCount', 'file_count'], fileCount);
        entityCount = getNumericValue(node, ['entityCount', 'entity_count'], entityCount);
    }

    if (isFolder && scoreWeight === 0 && nodeScore != null) {
        scoreSum += nodeScore;
        scoreWeight += 1;
    }

    const avgScore = scoreWeight > 0 ? scoreSum / scoreWeight : nodeScore;

    const aggregates = {
        totalIssues,
        severityCounts,
        entityCount,
        fileCount,
        avgScore,
        scoreWeight,
        _scoreSum: scoreSum,
    };

    node.__aggregateCache = aggregates;
    return aggregates;
};

export const TreeNode = ({ node, style, innerRef, tree, projectRoot }) => {
    if (!node || !node.data) {
        throw new Error('TreeNode props missing data');
    }
    const data = node.data;
    const iconRefs = useRef([]);
    iconRefs.current = [];

    if (typeof window !== 'undefined') {
        console.log('[TreeNode] render', {
            id: node.id,
            level: node.level,
            isOpen: node.isOpen,
            type: data.type,
            childCount: node.childCount
        });
    }

    const toggleNode = (id) => {
        if (tree && typeof tree.toggle === 'function') {
            tree.toggle(id);
        }
    };

    const registerIcon = (element, fallback) => {
        if (element) {
            iconRefs.current.push({ element, fallback });
        }
    };

    // Check node types - entities have entity_id, not type
    const isFolder = data.type === 'folder';
    const isFile = data.type === 'file';
    const isEntity = data.type === 'entity' || !!data.entity_id;
    const isCategory = data.type === 'category';
    const isInfoRow = data.type === 'info-row';
    const isIssueRow = data.type === 'issue-row';
    const isSuggestionRow = data.type === 'suggestion-row';

    useEffect(() => {
        const pendingIcons = [...iconRefs.current];

        const applyFallbacks = () => {
            pendingIcons.forEach(({ element, fallback }) => {
                if (!element) {
                    return;
                }
                const hasSvg = element.querySelector('svg');
                if (!hasSvg) {
                    element.textContent = fallback;
                }
            });
        };

        if (typeof window !== 'undefined' && window.lucide) {
            window.lucide.createIcons();
            window.requestAnimationFrame(applyFallbacks);
        } else {
            applyFallbacks();
        }
    }, [node.id, node.isOpen, data.type, data.name, data.priority]);
    
    // Handle info/issue/suggestion rows
    if (isIssueRow || isSuggestionRow) {
        let iconName = 'info';
        let iconColor = 'var(--tree-muted, var(--color-text-muted))';
        let backgroundColor = 'transparent';
        let iconFallbackSymbol = 'i';

        if (isIssueRow) {
            iconName = 'alert-triangle';
            iconColor = 'var(--danger, #dc3545)';
            backgroundColor = 'rgba(220, 53, 69, 0.12)';
            iconFallbackSymbol = '!';
        } else if (isSuggestionRow) {
            iconName = 'lightbulb';
            iconColor = 'var(--info, #007acc)';
            backgroundColor = 'rgba(0, 123, 255, 0.12)';
            iconFallbackSymbol = '?';
        }
        
        // Parse text and score for complexity/structure issues
        const rawName = String(data.name || '');
        let scoreElement = null;

        // Clean up text by removing decorative icon prefixes and category prefixes first
        const fallbackText = rawName
            .replace(/^[!\?\*\-]\s*/, '')
            .replace(/^(complexity|structure):\s*/i, '')
            .trim();

        // For issue rows (alert-triangle), check if it's complexity or structure and extract score
        if (isIssueRow) {
            const nameStr = rawName;
            const isComplexityIssue = nameStr.toLowerCase().includes('complexity');
            const isStructureIssue = nameStr.toLowerCase().includes('structure');

            if (isComplexityIssue || isStructureIssue) {
                // For "very high complexity/structural" descriptions, we need to use the entity score
                // Extract from text like "score: X" or look for entity score context
                let score = null;
                const scoreMatch = rawName.match(/score:\s*(\d+(?:\.\d+)?)/);

                if (scoreMatch) {
                    score = parseFloat(scoreMatch[1]);
                } else if (data.issueSeverity !== undefined && (isComplexityIssue || isStructureIssue)) {
                    // Use the issue's own severity score directly (it's already in the right scale)
                    score = data.issueSeverity;
                } else if (data.entityScore && (isComplexityIssue || isStructureIssue)) {
                    // Fallback to entity score if no issue severity
                    score = data.entityScore;
                }

                if (score !== null) {
                    const formattedScore = formatDecimal(score);
                    // Use the same colors as the banner (background and left border)
                    if (formattedScore !== null) {
                        scoreElement = React.createElement('div', {
                            key: 'score-badge',
                            style: {
                                marginLeft: 'auto',
                                padding: '2px 8px',
                                borderRadius: '4px',
                                fontSize: '11px',
                                fontWeight: '500',
                                backgroundColor: backgroundColor,
                                color: iconColor,
                                border: `1px solid ${iconColor}`
                            }
                        }, formattedScore);
                    }
                }
            }
        }

        const detailIndent = ((node.level ?? 0) + 1) * 24;
        const codeLabel = data.code ? String(data.code) : null;
        const titleLabel = data.title ? String(data.title) : null;
        const summaryText = data.summary ? String(data.summary) : '';
        const badgeValues = Array.isArray(data.badges)
            ? data.badges.filter(Boolean).map(String)
            : [];

        const badgeElements = badgeValues.map((badge, idx) =>
            React.createElement('span', {
                key: `badge-${idx}`,
                style: {
                    display: 'inline-flex',
                    alignItems: 'center',
                    padding: '1px 6px',
                    borderRadius: '999px',
                    fontSize: '0.7rem',
                    fontWeight: 600,
                    backgroundColor: 'rgba(255, 255, 255, 0.18)',
                    color: iconColor,
                    border: `1px solid ${iconColor}`
                }
            }, badge)
        );

        const primaryLabel = titleLabel || fallbackText || rawName;
        const primaryLineChildren = [];

        if (codeLabel) {
            primaryLineChildren.push(
                React.createElement('span', {
                    key: 'code-label',
                    style: {
                        fontWeight: 700,
                        textTransform: 'uppercase',
                        letterSpacing: '0.04em',
                        color: iconColor
                    }
                }, codeLabel)
            );
        }

        primaryLineChildren.push(
            React.createElement('span', {
                key: 'primary-text',
                style: { fontWeight: 500 }
            }, primaryLabel)
        );

        if (badgeElements.length > 0) {
            primaryLineChildren.push(...badgeElements);
        }

        const structuredContent = React.createElement('div', {
            key: 'label-block',
            style: {
                flex: 1,
                display: 'flex',
                flexDirection: 'column',
                gap: '0.2rem',
                minWidth: 0
            }
        }, [
            React.createElement('div', {
                key: 'primary-line',
                style: {
                    display: 'flex',
                    flexWrap: 'wrap',
                    alignItems: 'center',
                    gap: '0.5rem',
                    minWidth: 0
                }
            }, primaryLineChildren.filter(Boolean)),
            summaryText
                ? React.createElement('div', {
                    key: 'secondary-line',
                    style: {
                        fontSize: '0.75rem',
                        color: 'var(--tree-muted, var(--color-text-muted))',
                        lineHeight: 1.4,
                        marginTop: '0.1rem'
                    }
                }, summaryText)
                : null
        ].filter(Boolean));

        const labelNode = (codeLabel || titleLabel || summaryText || badgeValues.length > 0)
            ? structuredContent
            : React.createElement('span', {
                key: 'fallback-text',
                style: { flex: 1 }
            }, fallbackText);

        const children = [
            React.createElement('i', {
                'data-lucide': iconName,
                key: 'icon',
                ref: (el) => registerIcon(el, iconFallbackSymbol),
                'data-node-id': node.id,
                style: {
                    width: '14px',
                    height: '14px',
                    marginRight: '0.5rem',
                    color: iconColor,
                    flexShrink: 0
                }
            }),
            labelNode,
            scoreElement
        ].filter(Boolean);
        
        return React.createElement('div', {
            ref: innerRef ?? undefined,
            className: `tree-detail-row ${isIssueRow ? 'tree-detail-row--issue' : 'tree-detail-row--suggestion'}`,
            role: 'treeitem',
            'aria-level': (node.level ?? 0) + 1,
            'aria-expanded': false,
            style: {
                ...style,
                display: 'flex',
                alignItems: 'center',
                padding: '0.4rem 0.5rem',
                backgroundColor: backgroundColor,
                borderLeft: `3px solid ${iconColor}`,
                fontSize: '0.85rem',
                color: 'var(--tree-foreground, var(--color-text))',
                boxSizing: 'border-box',
                marginLeft: `${detailIndent}px`,
                paddingLeft: '1.5rem',
                width: `calc(100% - ${detailIndent}px)`
            }
        }, ...children);
    }

    // Regular node rendering (folder, file, entity, category)
    // The flattened virtualizer payload mirrors the fields produced by the
    // TanStack tree adapter (isLeaf/childCount), so we rely on those hints and
    // the raw children array to decide whether a node can expand.
    const hasChildren = (!node.isLeaf && (node.childCount ?? 0) > 0) || 
                       (Array.isArray(data.children) && data.children.length > 0);
    
    // Show chevrons for folders, files, entities, and categories that have children
    // Never show for issue/suggestion/info rows (they're always leaves)
    const shouldShowChevron = hasChildren && !isIssueRow && !isSuggestionRow && !isInfoRow;
    
    // Priority color mapping with actual styling
    const getPriorityStyle = (priority) => {
        const priorityStr = String(priority || '').toLowerCase();
        switch(priorityStr) {
            case 'critical': 
                return { 
                    backgroundColor: '#dc354520', 
                    color: '#dc3545',
                    border: '1px solid #dc354540' 
                };
            case 'high': 
                return { 
                    backgroundColor: '#fd7e1420', 
                    color: '#fd7e14',
                    border: '1px solid #fd7e1440'
                };
            case 'medium': 
                return { 
                    backgroundColor: '#ffc10720', 
                    color: '#ffc107',
                    border: '1px solid #ffc10740'
                };
            case 'low': 
                return { 
                    backgroundColor: '#6c757d20', 
                    color: '#6c757d',
                    border: '1px solid #6c757d40'
                };
            default: 
                return { 
                    backgroundColor: '#6c757d20', 
                    color: '#6c757d',
                    border: '1px solid #6c757d40'
                };
        }
    };

    const buildSeverityBar = (counts, keyPrefix, options = {}) => {
        if (!counts) return null;

        const total =
            (counts.critical || 0) +
            (counts.high || 0) +
            (counts.medium || 0) +
            (counts.low || 0);

        if (total <= 0) return null;

        const order = ['critical', 'high', 'medium', 'low'];
        const segments = order
            .map((severity) => {
                const value = counts[severity] || 0;
                if (!value) return null;
                const pct = (value / total) * 100;
                const color = getPriorityStyle(severity).color || 'var(--accent)';
                const label = `${severity.charAt(0).toUpperCase()}${severity.slice(1)} ${Math.round(pct)}% (${value})`;

                return React.createElement('div', {
                    key: `${keyPrefix}-${severity}`,
                    className: `severity-bar__segment severity-bar__segment--${severity}`,
                    style: { width: `${pct}%`, backgroundColor: color },
                    title: label,
                });
            })
            .filter(Boolean);

        if (!segments.length) return null;

        return React.createElement(
            'div',
            {
                key: `${keyPrefix}-bar`,
                className: 'severity-bar',
                style: { marginLeft: options.marginLeft ?? '0.5rem' },
                role: 'presentation',
                'aria-label': 'Severity mix',
            },
            segments
        );
    };
    
    // Health score color
    const getHealthColor = (score) => {
        if (score >= 0.8) return 'var(--success)';
        if (score >= 0.6) return 'var(--warning)';
        return 'var(--danger)';
    };

    const getHealthScore = (nodeLike) => getNumericValue(nodeLike, ['healthScore', 'health_score', 'health'], null);
    
    const children = [];

    const aggregates = computeAggregates(data);
    if (typeof window !== 'undefined') {
        window.__VALKNUT_TREE_NODE_DEBUG = window.__VALKNUT_TREE_NODE_DEBUG || [];
        if (window.__VALKNUT_TREE_NODE_DEBUG.length < 50) {
            window.__VALKNUT_TREE_NODE_DEBUG.push({
                id: node.id,
                type: data.type,
                name: data.name,
                aggregates,
                original: {
                    totalIssues: data.totalIssues ?? data.total_issues,
                    avgScore: data.avgScore ?? data.avg_score,
                    severityCounts: data.severityCounts ?? data.severity_counts,
                    entityCount: data.entityCount ?? data.entity_count,
                    fileCount: data.fileCount ?? data.file_count,
                },
            });
        }

        if (isFolder) {
            window.__VALKNUT_TREE_DIR_LOG = window.__VALKNUT_TREE_DIR_LOG || {};
            if (!window.__VALKNUT_TREE_DIR_LOG[node.id]) {
                window.__VALKNUT_TREE_DIR_LOG[node.id] = {
                    id: node.id,
                    name: data.name,
                    aggregates,
                    original: {
                        totalIssues: data.totalIssues ?? data.total_issues,
                        avgScore: data.avgScore ?? data.avg_score,
                        severityCounts: data.severityCounts ?? data.severity_counts,
                        entityCount: data.entityCount ?? data.entity_count,
                        fileCount: data.fileCount ?? data.file_count,
                    },
                };
                // eslint-disable-next-line no-console
                console.log('[TreeNode] folder aggregates', window.__VALKNUT_TREE_DIR_LOG[node.id]);
            }
        }
    }

    // Tooltip content builders (moved up so we can scope tooltip to label/icon only)
    const formatIssue = (issue = {}) => {
        const severity = typeof issue.severity === 'number' ? issue.severity.toFixed(1) : '—';

        // Pull feature helpers
        const getFeat = (key) => {
            const feats = Array.isArray(issue.contributing_features) ? issue.contributing_features : [];
            const match = feats.find((f) => String(f.feature_name || '').toLowerCase().includes(key));
            if (!match || match.value === undefined) return null;
            const v = Number(match.value);
            return Number.isFinite(v) ? v : null;
        };

        const category = (issue.category || issue.code || '').toString().toLowerCase();
        let title = issue.title || issue.category || issue.code || 'Issue';
        let summary = issue.summary || '';

        const cyclo = getFeat('cyclomatic_complexity');
        const cognitive = getFeat('cognitive_complexity');
        const mi = getFeat('maintainability_index');
        const debt = getFeat('technical_debt_score');

        if (category.includes('debt')) {
            title = 'Poor code organization';
            if (!summary) {
                summary = debt != null
                    ? `Technical debt score ${debt.toFixed(1)} — higher means more restructuring and cleanup needed`
                    : 'Organization/debt exceeds the acceptable baseline';
            }
        } else if (category.includes('maintain')) {
            title = 'Too much code coupling';
            if (!summary) {
                summary = mi != null
                    ? `Maintainability Index ${mi.toFixed(1)} — lower MI suggests tighter coupling; target ≥ 60`
                    : 'Maintainability/coupling exceeds the acceptable baseline';
            }
        } else if (category.includes('cognit')) {
            title = 'Too many code paths';
            if (!summary && cognitive != null) {
                summary = `Cognitive complexity ${cognitive.toFixed(0)} — target ≤ 15 (lower is better)`;
            }
        } else if (category.includes('complex')) {
            title = 'Too many branch points';
            if (!summary && cyclo != null) {
                summary = `Cyclomatic complexity ${cyclo.toFixed(0)} — target ≤ 10 (lower is better)`;
            }
        } else if (category.includes('struct')) {
            title = 'Optimize file layout';
            if (!summary) {
                summary = 'Large or entangled structure; consider splitting files or extracting modules';
            }
        }

        return {
            title,
            severity,
            summary,
        };
    };

    const formatSuggestion = (suggestion = {}) => {
        const code = suggestion.code || suggestion.refactoring_type || '';
        const baseTitle = suggestion.title || suggestion.summary || suggestion.refactoring_type || code || 'Suggestion';
        const heading = code ? `${code} · ${baseTitle}` : baseTitle;

        const metaParts = [];
        if (typeof suggestion.impact === 'number') {
            metaParts.push(`Impact ${(suggestion.impact * 100).toFixed(0)}%`);
        }
        if (typeof suggestion.effort === 'number') {
            metaParts.push(`Effort ${(suggestion.effort * 100).toFixed(0)}%`);
        }
        if (suggestion.priority !== undefined && suggestion.priority !== null && suggestion.priority !== '') {
            const priorityValue = typeof suggestion.priority === 'number'
                ? suggestion.priority.toFixed(2)
                : suggestion.priority;
            metaParts.push(`Priority ${priorityValue}`);
        }

        if (Array.isArray(suggestion.badges)) {
            suggestion.badges.forEach((badge) => {
                if (typeof badge === 'string' && badge.trim() && !metaParts.includes(badge.trim())) {
                    metaParts.push(badge.trim());
                }
            });
        }

        const summary = suggestion.summary && suggestion.summary !== baseTitle
            ? suggestion.summary
            : suggestion.description || '';

        return {
            heading,
            meta: metaParts.join(' · '),
            summary,
        };
    };

    const tooltipContent = () => {
        const renderValue = (value) => {
            if (value === null || value === undefined || value === '') {
                return '—';
            }
            const numeric = Number(value);
            if (Number.isFinite(numeric)) {
                const rounded = numeric.toFixed(1);
                return rounded.endsWith('.0') ? rounded.slice(0, -2) : rounded;
            }
            return value;
        };

        const renderMetrics = (metrics) =>
            React.createElement(
                'ul',
                { className: 'tooltip-metrics' },
                metrics.map(({ label, value }) =>
                    React.createElement(
                        'li',
                        { key: label },
                        React.createElement('span', { className: 'metric-label' }, label),
                        React.createElement('span', { className: 'metric-value' }, renderValue(value))
                    )
                )
            );

        const capitalize = (value) => {
            if (!value || typeof value !== 'string') {
                return value;
            }
            return value.charAt(0).toUpperCase() + value.slice(1);
        };

        const suggestionTexts = Array.isArray(data.suggestions)
            ? Array.from(
                new Set(
                    data.suggestions
                        .map(
                            (s) =>
                                s.summary ||
                                s.explanation ||
                                s.heading ||
                                s.refactoring_type ||
                                s.refactoringType ||
                                s.title ||
                                ''
                        )
                        .map((txt) => (txt || '').trim())
                        .filter(Boolean)
                )
            )
            : [];
        const suggestionText = suggestionTexts[0] || null;

        const shouldShowTooltip = isEntity || isFile;
        if (!shouldShowTooltip) {
            return null;
        }

        if (isFolder) {
            const totalFolderIssues = aggregates.totalIssues ?? data.totalIssues ?? data.refactoringNeeded ?? 0;
            const folderHealth = getHealthScore(data);
            const folderAcceptable = formatAcceptableRatio(
                (() => {
                    let maxRatio = null;
                    const visit = (n) => {
                        const r = getComplexityRatio(n);
                        if (r != null && r > (maxRatio ?? -Infinity)) maxRatio = r;
                        if (Array.isArray(n?.children)) n.children.forEach(visit);
                    };
                    visit(data);
                    return maxRatio;
                })()
            );
            const metrics = [
                {
                    label: 'Health',
                    value:
                        typeof folderHealth === 'number'
                            ? `${Math.round(folderHealth * 100)}%`
                            : '—',
                },
                { label: 'Files', value: aggregates.fileCount ?? data.fileCount ?? 0 },
                { label: 'Entities', value: aggregates.entityCount ?? data.entityCount ?? 0 },
                { label: 'Issues', value: totalFolderIssues },
                { label: 'Critical Issues', value: aggregates.severityCounts?.critical ?? data.criticalIssues ?? 0 },
                { label: 'High Priority', value: (aggregates.severityCounts?.high || 0) + (aggregates.severityCounts?.critical || 0) },
            ];

            if (folderAcceptable) {
                metrics.push({ label: 'Complexity', value: folderAcceptable });
            }

            // Show normalized complexity/debt metrics if available on a folder aggregate
            const normalized = [];
            if (data.cyclomatic_complexity != null) {
                const v = fmtPct(ccPct(data.cyclomatic_complexity));
                if (v) normalized.push({ label: 'Cyclomatic', value: v });
            }
            if (data.cognitive_complexity != null) {
                const v = fmtPct(cogPct(data.cognitive_complexity));
                if (v) normalized.push({ label: 'Cognitive', value: v });
            }
            if (data.maintainability_index != null) {
                const v = fmtPct(miPct(data.maintainability_index));
                if (v) normalized.push({ label: 'MI', value: v });
            }
            if (data.technical_debt_score != null) {
                const v = fmtPct(tdPct(data.technical_debt_score));
                if (v) normalized.push({ label: 'Debt', value: v });
            }
            metrics.push(...normalized);

            const categories = Array.isArray(data.issueCategories)
                ? data.issueCategories.slice(0, 5)
                : [];

            return React.createElement(
                'div',
                null,
                React.createElement('div', { className: 'tooltip-name' }, data.name || 'Directory'),
                renderMetrics(metrics),
                categories.length > 0 &&
                    React.createElement(
                        'div',
                        { className: 'tooltip-section' },
                        React.createElement('h4', null, 'Top Categories'),
                        React.createElement(
                            'ul',
                            { className: 'tooltip-section-list' },
                            categories.map((category, idx) =>
                                React.createElement(
                                    'li',
                                    { key: `${category.category}-${idx}` },
                                    React.createElement(
                                        'div',
                                        { className: 'issue-heading' },
                                        `${capitalize(category.category)} · ${renderValue(category.affectedEntities)} entities`
                                    ),
                                    React.createElement(
                                        'div',
                                        { className: 'issue-summary' },
                                        suggestionText ||
                                            `Avg severity ${renderValue(category.avgSeverity)}, impact ${renderValue(
                                                category.healthImpact
                                            )}`
                                    )
                                )
                            )
                        )
                    )
            );
        }

        if (isFile) {
            const severityCounts = aggregates.severityCounts || {};
            const docIssueCount = data.docIssues;

            const severityList = [
                { key: 'critical', label: 'Critical', value: severityCounts.critical || 0 },
                { key: 'high', label: 'High', value: severityCounts.high || 0 },
                { key: 'medium', label: 'Medium', value: severityCounts.medium || 0 },
                { key: 'low', label: 'Low', value: severityCounts.low || 0 },
            ].filter((item) => item.value > 0);

            const topEntities = (data.children || [])
                .filter((child) => child.type === 'entity')
                .slice(0, 3);

            return React.createElement(
                'div',
                null,
                React.createElement('div', { className: 'tooltip-name' }, data.name || 'File'),
                severityList.length > 0 &&
                    React.createElement(
                        'div',
                        { className: 'tooltip-section' },
                        React.createElement('h4', null, 'Severity Breakdown'),
                        React.createElement(
                            'ul',
                            { className: 'tooltip-section-list' },
                            severityList.map((item) =>
                                React.createElement(
                                    'li',
                                    { key: item.key },
                                    React.createElement('div', { className: 'issue-heading' }, `${item.label} · ${item.value}`)
                                )
                            )
                        )
                    ),
                docIssueCount != null && docIssueCount > 0 &&
                    React.createElement(
                        'div',
                        { className: 'tooltip-section' },
                        React.createElement('h4', null, 'Documentation'),
                        React.createElement(
                            'ul',
                            { className: 'tooltip-section-list' },
                            React.createElement(
                                'li',
                                { key: 'doc-issues' },
                                React.createElement('div', { className: 'issue-heading' }, `${docIssueCount} undocumented items`)
                            )
                        )
                    ),
                topEntities.length > 0 &&
                    React.createElement(
                        'div',
                        { className: 'tooltip-section' },
                        React.createElement('h4', null, 'Top Entities'),
                        React.createElement(
                            'ul',
                            { className: 'tooltip-section-list' },
                            topEntities.map((entity) =>
                                React.createElement(
                                    'li',
                                    { key: entity.id },
                                    React.createElement('div', { className: 'issue-heading' }, entity.name),
                                    suggestionText
                                        ? React.createElement('div', { className: 'issue-summary' }, suggestionText)
                                        : React.createElement('div', { className: 'issue-summary' }, `Score ${renderValue(entity.score)}`)
                                )
                            )
                        )
                    )
            );
        }

        // Entities
        const issues = Array.isArray(data.issues) ? data.issues.map(formatIssue) : [];
        const rawSuggestions = Array.isArray(data.suggestions) ? data.suggestions : [];
        const suggestions = rawSuggestions.map(formatSuggestion);

        // Show all issues sorted by severity; omit metric row for entities
        const listedIssues = issues
            .filter((issue) => issue.severity !== '—')
            .sort((a, b) => Number(b.severity) - Number(a.severity));

        const metrics = [];

        // Build tagged suggestions for better matching by issue type/feature
        const suggestionsWithTags = (() => {
            const deriveText = (s) => {
                const explicit =
                    s.summary ||
                    s.explanation ||
                    s.heading ||
                    s.title ||
                    s.refactoring_type ||
                    s.refactoringType ||
                    s.code ||
                    '';
                if (explicit && explicit.trim()) return explicit.trim();

                const refType = (s.refactoring_type || s.refactoringType || '').trim();
                if (refType) {
                    const pretty = refType.replace(/_/g, ' ');
                    const match = refType.match(/_(\\d+(?:\\.\\d+)?)$/);
                    if (match) {
                        return `${pretty.replace(match[0], '').trim()} (target ${match[1]})`;
                    }
                    return pretty;
                }
                return 'Suggestion';
            };

            const tagSuggestion = (s) => {
                const tags = new Set();
                const text = deriveText(s);
                const haystack = `${s.code || ''} ${s.category || ''} ${s.refactoring_type || s.refactoringType || ''} ${s.heading || ''} ${s.title || ''} ${s.summary || ''} ${s.explanation || ''}`.toLowerCase();

                const add = (...items) => items.forEach((t) => t && tags.add(t));

                if (haystack.match(/cognit/)) add('cognitive_complexity', 'complexity');
                if (haystack.match(/cyclo/)) add('cyclomatic_complexity', 'complexity');
                if (haystack.match(/maintain/)) add('maintainability');
                if (haystack.match(/debt/)) add('technical_debt');
                if (haystack.match(/struct/)) add('structure');
                if (haystack.match(/clone/)) add('clone', 'similarity');
                if (haystack.match(/complex/)) add('complexity');
                if (haystack.match(/refactor/)) add('refactor');
                if (haystack.match(/extract/)) add('structure', 'maintainability', 'technical_debt');
                if (haystack.match(/class/) || haystack.match(/module/)) add('structure', 'maintainability');
                if (haystack.match(/large/)) add('technical_debt', 'maintainability');

                // Include canonical tags from code/category
                add((s.code || '').toLowerCase(), (s.category || '').toLowerCase());

                return { text, tags, raw: s };
            };

            return rawSuggestions.map(tagSuggestion);
        })();

        const suggestionFallback =
            suggestionsWithTags[0]?.text || null;

        const normalize = (v) => (v || '').toString().toLowerCase();

        const canonicalizeFeature = (name = '') => {
            const n = normalize(name);
            if (n.includes('cogn')) return 'cognitive_complexity';
            if (n.includes('cycl')) return 'cyclomatic_complexity';
            if (n.includes('debt')) return 'technical_debt';
            if (n.includes('maintain')) return 'maintainability';
            if (n.includes('struct')) return 'structure';
            if (n.includes('clone')) return 'clone';
            if (n.includes('complex')) return 'complexity';
            return n || null;
        };

        const findSuggestionForIssue = (issue) => {
            const issueTags = new Set();

            issueTags.add(canonicalizeFeature(issue.code));
            issueTags.add(canonicalizeFeature(issue.category));
            issueTags.add(canonicalizeFeature(issue.title));

            if (Array.isArray(issue.contributing_features)) {
                issue.contributing_features.forEach((feat) => {
                    const tag = canonicalizeFeature(feat?.feature_name);
                    if (tag) issueTags.add(tag);
                });
            }

            // Prefer matches where any tag overlaps
            for (const tag of issueTags) {
                if (!tag) continue;
                const match = suggestionsWithTags.find((s) => s.tags.has(tag));
                if (match) return match.text;
                // Broader complexity grouping
                if (tag === 'complexity' || tag === 'cyclomatic_complexity') {
                    const alt = suggestionsWithTags.find((s) =>
                        s.tags.has('cyclomatic_complexity') || s.tags.has('complexity')
                    );
                    if (alt) return alt.text;
                }
                if (tag === 'cognitive_complexity') {
                    const alt = suggestionsWithTags.find((s) =>
                        s.tags.has('cognitive_complexity')
                    );
                    if (alt) return alt.text;
                }
            }

            // Maintainability/debt fallbacks
            const debt = suggestionsWithTags.find((s) => s.tags.has('technical_debt'));
            if (issueTags.has('technical_debt') && debt) return debt.text;

            const maintain = suggestionsWithTags.find((s) => s.tags.has('maintainability'));
            if (issueTags.has('maintainability') && maintain) return maintain.text;

            // Structure fallback
            const structure = suggestionsWithTags.find((s) => s.tags.has('structure'));
            if (issueTags.has('structure') && structure) return structure.text;

            // If maintainability/debt need a fallback and no direct match, prefer structural/extract style fixes
            if ((issueTags.has('maintainability') || issueTags.has('technical_debt')) && structure) {
                return structure.text;
            }
            if ((issueTags.has('maintainability') || issueTags.has('technical_debt')) && maintain) {
                return maintain.text;
            }
            if ((issueTags.has('maintainability') || issueTags.has('technical_debt')) && debt) {
                return debt.text;
            }

            // Generic fallback
            return suggestionFallback;
        };

        return React.createElement(
            'div',
            null,
            React.createElement('div', { className: 'tooltip-name' }, data.name || 'Entity'),
            metrics.length > 0 &&
                React.createElement('ul', { className: 'tooltip-metrics' },
                    metrics.map(({ label, value }) =>
                        React.createElement(
                            'li',
                            { key: label },
                            React.createElement('span', { className: 'metric-label' }, label),
                            React.createElement('span', { className: 'metric-value' }, renderValue(value))
                        )
                    )
                ),
            listedIssues.length > 0 &&
                React.createElement(
                    'div',
                    { className: 'tooltip-section' },
                    React.createElement('h4', null, 'Issues'),
                    React.createElement(
                        'ul',
                        { className: 'tooltip-section-list' },
                        listedIssues.map((issue, idx) =>
                            React.createElement(
                                'li',
                                { key: idx },
                                React.createElement('div', { className: 'issue-heading' }, `${issue.title} (Severity ${issue.severity})`)
                            )
                        )
                    )
                ),
            suggestionTexts.length > 0 &&
                React.createElement(
                    'div',
                    { className: 'tooltip-section' },
                    React.createElement('h4', null, 'Suggested Actions'),
                    React.createElement(
                        'ul',
                        { className: 'tooltip-section-list' },
                        suggestionTexts.map((txt, idx) =>
                            React.createElement(
                                'li',
                                { key: idx },
                                React.createElement('div', { className: 'issue-summary' }, txt)
                            )
                        )
                    )
                )
        );
    };

    // Expand/collapse arrow for nodes with children (but not for entities)
    // Only show chevron for nodes that actually can expand
    if (shouldShowChevron && hasChildren) {
        children.push(React.createElement('span', {
            key: 'chevron',
            className: 'tree-chevron',
            'data-expanded': node.isOpen ? 'true' : 'false',
            style: { 
                width: '16px',
                height: '16px',
                marginRight: '0.25rem',
                cursor: 'pointer',
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: 'var(--tree-muted, rgba(148,163,184,0.8))',
                fontSize: '12px',
                userSelect: 'none'
            },
            onClick: (e) => {
                e.stopPropagation();
                toggleNode(node.id);
            },
            role: 'presentation'
        }, renderChevronIcon()));
    } else {
        // Add spacing for nodes without children to align with expandable nodes
        children.push(React.createElement('div', {
            key: 'spacer',
            style: { width: '1rem', marginRight: '0.25rem' }
        }));
    }

    const shouldShowTooltip = isEntity || isFile;

    // Icon + label (tooltip only on entities and files, not folders)
    let iconName = 'function-square'; // default for entities
    let iconFallbackSymbol = '*';
    if (isFolder) {
        iconName = 'folder';
        iconFallbackSymbol = '[F]';
    } else if (isFile) {
        iconName = 'file-code';
        iconFallbackSymbol = '<>'; 
    } else if (isCategory) {
        iconName = 'layers';
        iconFallbackSymbol = '[C]';
    }

    const iconElement = React.createElement('i', {
        'data-lucide': iconName,
        key: 'icon',
        ref: (el) => registerIcon(el, iconFallbackSymbol),
        className: 'tree-icon',
        style: { marginRight: '0.5rem' }
    });
    
    const labelText = data.name;

    // Build VS Code link for files and entities
    const filePath = data.file_path || data.filePath || data.path;
    const lineRange = data.line_range || data.lineRange;
    // Also check for line_number/start_line fields (common in code_dictionary entities)
    const lineNumber = data.line_number || data.lineNumber || data.start_line || data.startLine;
    const vscodeLink = (isFile || isEntity) ? buildVSCodeLink(filePath, lineRange, projectRoot, lineNumber) : null;

    const labelElement = vscodeLink
        ? React.createElement('a', {
            key: 'label',
            href: vscodeLink,
            onClick: (e) => e.stopPropagation(), // Don't toggle node when clicking link
            style: {
                flex: 1,
                fontWeight: (isFolder || isCategory) ? '500' : 'normal',
                color: 'inherit',
                minWidth: 0,
                textDecoration: 'none',
                cursor: 'pointer'
            },
            title: `Open in VS Code: ${filePath || data.name}`
        }, labelText)
        : React.createElement('span', {
            key: 'label',
            style: { flex: 1, fontWeight: (isFolder || isCategory) ? '500' : 'normal', color: 'inherit', minWidth: 0 }
        }, labelText);

    if (shouldShowTooltip) {
        children.push(
            React.createElement(
                Tooltip,
                { key: 'label-tooltip', content: tooltipContent, placement: 'bottom' },
                React.createElement('span', {
                    className: 'tree-label-with-icon',
                    style: { display: 'inline-flex', alignItems: 'center', gap: '0.5rem', flex: 1, minWidth: 0 }
                }, [iconElement, labelElement])
            )
        );
    } else {
        children.push(iconElement);
        children.push(labelElement);
    }

    // Pre-compute all badge data before adding to children array
    const folderHealth = isFolder ? getHealthScore(data) : null;
    const fileHealth = isFile ? getHealthScore(data) : null;
    const folderComplexityRatio = isFolder ? getMaxComplexityRatio(data) : null;
    const folderAcceptable = formatAcceptableRatio(folderComplexityRatio);
    const formattedNodeAvgScore = formatDecimal(aggregates.avgScore ?? data.avgScore);
    const fileComplexityRatio = isFile ? getMaxComplexityRatio(data) : null;
    const formattedNodeAcceptable = formatAcceptableRatio(fileComplexityRatio);

    // Badge ordering: Entities → Issues → Health → Complexity → Priority → Severity bar (rightmost)

    // Entities badges
    if (isFolder && (aggregates.entityCount || data.entityCount)) {
        const count = aggregates.entityCount ?? data.entityCount ?? 0;
        children.push(React.createElement('div', {
            key: 'entities-folder',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `${count} entities`));
    }
    if (isFile && (aggregates.entityCount || data.entityCount)) {
        const count = aggregates.entityCount ?? data.entityCount ?? 0;
        children.push(React.createElement('div', {
            key: 'entities-file',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `${count} entities`));
    }

    // Issues badges
    if (isFolder && aggregates.totalIssues > 0) {
        children.push(React.createElement('div', {
            key: 'issues-folder',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `${aggregates.totalIssues} issues`));
    }
    if (isFile && aggregates.totalIssues > 0) {
        children.push(React.createElement('div', {
            key: 'issues-file',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `${aggregates.totalIssues} issues`));
    }

    // Health badges
    if (isFolder && folderHealth !== null) {
        children.push(React.createElement('div', {
            key: 'health-folder',
            className: 'tree-badge tree-badge-low complexity-score',
            style: { marginLeft: '0.5rem', color: getHealthColor(folderHealth) }
        }, `Health: ${(folderHealth * 100).toFixed(0)}%`));
    }
    if (isFile && fileHealth !== null) {
        children.push(React.createElement('div', {
            key: 'health-file',
            className: 'tree-badge tree-badge-low complexity-score',
            style: { marginLeft: '0.5rem', color: getHealthColor(fileHealth) }
        }, `Health: ${(fileHealth * 100).toFixed(0)}%`));
    }

    // Complexity badges
    if (isFolder && formattedNodeAvgScore !== null) {
        children.push(React.createElement('div', {
            key: 'avg-score',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' },
            title: 'Average complexity score across entities in this folder'
        }, folderAcceptable
            ? `Complexity: ${folderAcceptable}`
            : `Avg Score: ${formattedNodeAvgScore}`));
    }
    const formattedEntityScore = formatDecimal(aggregates.avgScore ?? data.score);
    const entityAcceptable = formatAcceptableRatio(getMaxComplexityRatio(data));
    if (isEntity && formattedEntityScore !== null) {
        children.push(React.createElement('div', {
            key: 'complexity',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, entityAcceptable ? `Complexity: ${entityAcceptable}` : `Complexity: ${formattedEntityScore}`));
    }

    // Priority badge with color coding
    if (data.priority || data.highestPriority) {
        const priority = data.priority || data.highestPriority;
        children.push(React.createElement('div', {
            key: 'priority',
            className: 'tree-badge',
            style: {
                marginLeft: '0.5rem',
                ...getPriorityStyle(priority)
            }
        }, priority));
    }

    // Severity mix badge (normalized percentages) — ensure last/rightmost
    let severityBar = null;
    if ((isFolder || isFile) && aggregates.severityCounts) {
        severityBar = buildSeverityBar(aggregates.severityCounts, `${node.id}-severity`);
    }
    if (severityBar) {
        children.push(severityBar);
    }

    // Ensure severity bar is last/rightmost
    if (severityBar) {
        const idx = children.indexOf(severityBar);
        if (idx >= 0 && idx !== children.length - 1) {
            children.splice(idx, 1);
            children.push(severityBar);
        }
    }
    
    // Line range for entities
    if (isEntity && data.lineRange) {
        children.push(React.createElement('div', {
            key: 'lines',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `L${data.lineRange[0]}-${data.lineRange[1]}`));
    }
    
    // Severity mix badge for entities
    if (isEntity && data.severityCounts) {
        const severityBar = buildSeverityBar(data.severityCounts, `${node.id}-entity-severity`);
        if (severityBar) {
            children.push(severityBar);
        }
    }
    
    // Manual indentation calculation for nested rows
    const manualIndent = node.level * 24; // 24px per level

    // Header row (clickable part with icon, label, badges)
    const headerRow = React.createElement('div', {
        ref: innerRef ?? undefined,
        className: `tree-header-row${node.isSelected ? ' tree-header-row--selected' : ''}`,
        role: 'treeitem',
        'aria-level': (node.level ?? 0) + 1,
        'aria-expanded': shouldShowChevron ? !!node.isOpen : undefined,
        style: {
            ...style,
            display: 'flex',
            alignItems: 'center',
            cursor: shouldShowChevron ? 'pointer' : 'default',
            padding: '0.5rem 0.5rem 0.5rem 0px',
            marginLeft: `${manualIndent}px`,
            borderRadius: '4px',
            border: 'none',
            backgroundColor: node.isSelected ? 'rgba(99, 102, 241, 0.18)' : 'transparent',
            width: 'calc(100% - ' + manualIndent + 'px)',
            minHeight: '32px',
            gap: '0.5rem'
        },
        onClick: shouldShowChevron ? () => toggleNode(node.id) : undefined
        }, ...children.filter(Boolean));

        return headerRow;
};
