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

    let totalIssues = 0;
    if (!children.length) {
        totalIssues += getNumericValue(node, [
            'totalIssues',
            'total_issues',
            'refactoringNeeded',
            'refactoring_needed',
            'issueCount',
            'issue_count',
        ], 0) || 0;
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

export const TreeNode = ({ node, style, innerRef, tree }) => {
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
    
    // Health score color
    const getHealthColor = (score) => {
        if (score >= 0.8) return 'var(--success)';
        if (score >= 0.6) return 'var(--warning)';
        return 'var(--danger)';
    };
    
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
    
    // Icon
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

    children.push(React.createElement('i', {
        'data-lucide': iconName,
        key: 'icon',
        ref: (el) => registerIcon(el, iconFallbackSymbol),
        className: 'tree-icon',
        style: { marginRight: '0.5rem' }
    }));
    
    // Label
    const labelText = isFolder
        ? `${data.name} (issues: ${aggregates.totalIssues ?? 'n/a'}, avg: ${aggregates.avgScore != null ? formatDecimal(aggregates.avgScore) : 'n/a'})`
        : data.name;

    children.push(React.createElement('span', {
        key: 'label',
        style: { flex: 1, fontWeight: (isFolder || isCategory) ? '500' : 'normal', color: 'inherit' }
    }, labelText));
    
    // Health score for folders
    if (isFolder && data.healthScore) {
        children.push(React.createElement('div', {
            key: 'health',
            className: 'tree-badge',
            style: { 
                backgroundColor: getHealthColor(data.healthScore) + '20',
                color: getHealthColor(data.healthScore),
                border: `1px solid ${getHealthColor(data.healthScore)}40`,
                marginLeft: '0.5rem'
            }
        }, 'Health: ' + (data.healthScore * 100).toFixed(0) + '%'));
    }
    
    // File count for folders
    if (isFolder && data.fileCount) {
        children.push(React.createElement('div', {
            key: 'files',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `${data.fileCount} files`));
    }
    
    // Entity count for folders only (remove functions badge from files)
    if (isFolder && data.entityCount) {
        children.push(React.createElement('div', {
            key: 'entities',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `${data.entityCount} entities`));
    }

    if (isFolder && aggregates.totalIssues > 0) {
        children.push(React.createElement('div', {
            key: 'issues',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `${aggregates.totalIssues} issues`));
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

    // Severity count badges for folders and files (aggregate from children)
    if ((isFolder || isFile) && aggregates.severityCounts) {
        const counts = aggregates.severityCounts;
        
        // Critical issues badge
        if (counts.critical > 0) {
            children.push(React.createElement('div', {
                key: 'critical-count',
                className: 'tree-badge',
                style: { 
                    marginLeft: '0.5rem',
                    ...getPriorityStyle('critical')
                }
            }, `${counts.critical} critical`));
        }
        
        // High issues badge  
        if (counts.high > 0) {
            children.push(React.createElement('div', {
                key: 'high-count',
                className: 'tree-badge',
                style: { 
                    marginLeft: '0.5rem',
                    ...getPriorityStyle('high')
                }
            }, `${counts.high} high`));
        }
        
        // Medium issues badge
        if (counts.medium > 0) {
            children.push(React.createElement('div', {
                key: 'medium-count',
                className: 'tree-badge',
                style: { 
                    marginLeft: '0.5rem',
                    ...getPriorityStyle('medium')
                }
            }, `${counts.medium} medium`));
        }
        
        // Low issues badge
        if (counts.low > 0) {
            children.push(React.createElement('div', {
                key: 'low-count',
                className: 'tree-badge',
                style: { 
                    marginLeft: '0.5rem',
                    ...getPriorityStyle('low')
                }
            }, `${counts.low} low`));
        }
    }

    // Complexity score for files
    const formattedNodeAvgScore = formatDecimal(aggregates.avgScore ?? data.avgScore);
    if (isFile && formattedNodeAvgScore !== null) {
        children.push(React.createElement('div', {
            key: 'score',
            className: 'tree-badge tree-badge-low complexity-score',
            style: { marginLeft: '0.5rem' }
        }, `Complexity: ${formattedNodeAvgScore}`));
    }

    if (isFolder && formattedNodeAvgScore !== null) {
        children.push(React.createElement('div', {
            key: 'avg-score',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `Avg Score: ${formattedNodeAvgScore}`));
    }

    // Complexity score for entities
    const formattedEntityScore = formatDecimal(aggregates.avgScore ?? data.score);
    if (isEntity && formattedEntityScore !== null) {
        children.push(React.createElement('div', {
            key: 'complexity',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `Complexity: ${formattedEntityScore}`));
    }
    
    // Line range for entities
    if (isEntity && data.lineRange) {
        children.push(React.createElement('div', {
            key: 'lines',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `L${data.lineRange[0]}-${data.lineRange[1]}`));
    }
    
    // Severity count badges for entities
    if (isEntity && data.severityCounts) {
        const counts = data.severityCounts;
        
        // Critical issues badge
        if (counts.critical > 0) {
            children.push(React.createElement('div', {
                key: 'critical-count',
                className: 'tree-badge',
                style: { 
                    marginLeft: '0.5rem',
                    ...getPriorityStyle('critical')
                }
            }, `${counts.critical} critical`));
        }
        
        // High issues badge  
        if (counts.high > 0) {
            children.push(React.createElement('div', {
                key: 'high-count',
                className: 'tree-badge',
                style: { 
                    marginLeft: '0.5rem',
                    ...getPriorityStyle('high')
                }
            }, `${counts.high} high`));
        }
        
        // Medium issues badge
        if (counts.medium > 0) {
            children.push(React.createElement('div', {
                key: 'medium-count',
                className: 'tree-badge',
                style: { 
                    marginLeft: '0.5rem',
                    ...getPriorityStyle('medium')
                }
            }, `${counts.medium} medium`));
        }
        
        // Low issues badge
        if (counts.low > 0) {
            children.push(React.createElement('div', {
                key: 'low-count',
                className: 'tree-badge',
                style: { 
                    marginLeft: '0.5rem',
                    ...getPriorityStyle('low')
                }
            }, `${counts.low} low`));
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

    const shouldShowTooltip = isEntity || isFile || isFolder;

    if (!shouldShowTooltip) {
        return headerRow;
    }

    const formatIssue = (issue = {}) => {
        const title = issue.title || issue.category || issue.code || 'Issue';
        const severity = typeof issue.severity === 'number' ? issue.severity.toFixed(1) : '—';
        const summary = issue.summary || '';
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

        if (isFolder) {
            const totalFolderIssues = aggregates.totalIssues ?? data.totalIssues ?? data.refactoringNeeded ?? 0;
            const metrics = [
                {
                    label: 'Health',
                    value:
                        typeof data.healthScore === 'number'
                            ? `${Math.round(data.healthScore * 100)}%`
                            : '—',
                },
                { label: 'Files', value: aggregates.fileCount ?? data.fileCount ?? 0 },
                { label: 'Entities', value: aggregates.entityCount ?? data.entityCount ?? 0 },
                { label: 'Issues', value: totalFolderIssues },
                { label: 'Critical Issues', value: aggregates.severityCounts?.critical ?? data.criticalIssues ?? 0 },
                { label: 'High Priority', value: (aggregates.severityCounts?.high || 0) + (aggregates.severityCounts?.critical || 0) },
            ];

            if (aggregates.avgScore != null) {
                metrics.push({ label: 'Avg Score', value: aggregates.avgScore });
            }

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
                                        `Avg severity ${renderValue(category.avgSeverity)}, impact ${renderValue(category.healthImpact)}`
                                    )
                                )
                            )
                        )
                    )
            );
        }

        if (isFile) {
            const severityCounts = aggregates.severityCounts || {};
            const totalIssues = aggregates.totalIssues ?? data.totalIssues ?? Object.values(severityCounts).reduce((acc, value) => acc + (value || 0), 0);
            const metrics = [
                { label: 'Priority', value: data.highestPriority || data.priority || '—' },
                {
                    label: 'Entities',
                    value: aggregates.entityCount ?? data.entityCount ??
                        (data.children ? data.children.filter((child) => child.type === 'entity').length : 0),
                },
                { label: 'Issues', value: totalIssues },
                { label: 'Avg Score', value: aggregates.avgScore ?? data.avgScore ?? data.score ?? null },
            ];

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
                renderMetrics(metrics),
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
                                    `${item.label}: ${renderValue(item.value)}`
                                )
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
                                    entity.name,
                                    entity.severityCounts &&
                                        React.createElement(
                                            'div',
                                            { className: 'issue-summary' },
                                            `${renderValue(entity.severityCounts.critical || 0)} critical · ${renderValue(entity.severityCounts.high || 0)} high`
                                        )
                                )
                            )
                        )
                    )
            );
        }

        const issues = Array.isArray(data.issues) ? data.issues : [];
        const suggestions = Array.isArray(data.suggestions) ? data.suggestions : [];

        const topIssuesRaw = issues
            .slice()
            .sort((a, b) => (b?.severity ?? 0) - (a?.severity ?? 0))
            .slice(0, 3);
        const topIssues = topIssuesRaw.map(formatIssue);

        const formattedSuggestions = suggestions.slice(0, 2).map(formatSuggestion);

        const featureMap = new Map();
        topIssuesRaw.forEach((issue) => {
            const features = Array.isArray(issue?.contributing_features) ? issue.contributing_features : [];
            features.forEach((feature) => {
                const name = (feature?.feature_name || '').trim();
                if (!name) return;
                const normalizedName = name.replace(/_/g, ' ');
                const value = feature?.value;
                if (!featureMap.has(normalizedName)) {
                    featureMap.set(normalizedName, new Set());
                }
                if (value != null) {
                    featureMap.get(normalizedName).add(value);
                }
            });
        });

        const featureSummary = Array.from(featureMap.entries()).map(([name, values]) => ({
            name,
            values: Array.from(values).slice(0, 3),
        }));

        const highestSeverity = topIssuesRaw.length > 0 ? topIssuesRaw[0].severity ?? null : null;

        const coverage = data.coverage || {};

        const metrics = [
            { label: 'Priority', value: data.priority || data.highestPriority || '—' },
            { label: 'Score', value: data.score ?? formattedEntityScore },
            { label: 'Confidence', value: data.confidence != null ? `${Math.round(data.confidence * 100)}%` : '—' },
            { label: 'Issues', value: issues.length ?? 0 },
            { label: 'Suggestions', value: suggestions.length ?? 0 },
            { label: 'Peak Severity', value: highestSeverity },
        ];

        if (coverage.linesOfCode != null) {
            metrics.push({ label: 'Lines of Code', value: coverage.linesOfCode });
        }
        if (coverage.coverageBefore != null) {
            metrics.push({ label: 'Coverage Before', value: `${(coverage.coverageBefore * 100).toFixed(1)}%` });
        }
        if (coverage.coverageAfter != null) {
            metrics.push({ label: 'Coverage After', value: `${(coverage.coverageAfter * 100).toFixed(1)}%` });
        }

        return React.createElement(
            'div',
            null,
            React.createElement('div', { className: 'tooltip-name' }, data.name || 'Function'),
            renderMetrics(metrics),
            featureSummary.length > 0 &&
                React.createElement(
                    'div',
                    { className: 'tooltip-section' },
                    React.createElement('h4', null, 'Signals'),
                    React.createElement(
                        'table',
                        { className: 'feature-table' },
                        React.createElement(
                            'tbody',
                            null,
                            featureSummary.map(({ name, values }) =>
                                React.createElement(
                                    'tr',
                                    { key: name },
                                    React.createElement('th', null, name.replace(/_/g, ' ').replace(/\b\w/g, (char) => char.toUpperCase())),
                                    React.createElement('td', null, values.length ? values.map((val) => renderValue(val)).join(', ') : '—')
                                )
                            )
                        )
                    )
                ),
            topIssues.length > 0 &&
                React.createElement(
                    'div',
                    { className: 'tooltip-section' },
                    React.createElement('h4', null, 'Top Issues'),
                    React.createElement(
                        'ul',
                        { className: 'tooltip-section-list' },
                        topIssues.map((issue, idx) =>
                            React.createElement(
                                'li',
                                { key: idx },
                                React.createElement('div', { className: 'issue-heading' }, `${issue.title} (Severity ${issue.severity})`),
                                issue.summary && React.createElement('div', { className: 'issue-summary' }, issue.summary)
                            )
                        )
                    )
                ),
            formattedSuggestions.length > 0 &&
                React.createElement(
                    'div',
                    { className: 'tooltip-section' },
                    React.createElement('h4', null, 'Suggestions'),
                    React.createElement(
                        'ul',
                        { className: 'tooltip-section-list' },
                        formattedSuggestions.map((suggestion, idx) =>
                            React.createElement(
                                'li',
                                { key: idx, className: 'suggestion-item' },
                                suggestion.heading && React.createElement('div', { className: 'suggestion-summary' }, suggestion.heading),
                                suggestion.meta && React.createElement('div', { className: 'issue-summary' }, suggestion.meta),
                                suggestion.summary && React.createElement('div', { className: 'issue-summary' }, suggestion.summary)
                            )
                        )
                    )
                )
        );
    };

    return React.createElement(Tooltip, { content: tooltipContent, placement: 'bottom' }, headerRow);
};
