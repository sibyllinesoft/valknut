import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { TreeNode } from './TreeNode.jsx';
import { getSeverityLevel } from './treeUtils.js';

/**
 * Main tree component for displaying code analysis results
 * Supports both unified hierarchy and legacy refactoring data formats
 */
export const CodeAnalysisTree = ({ data }) => {
    const SHOW_ENTITY_DETAIL_ROWS = false;
    const [treeData, setTreeData] = useState([]);
    const [expandedIds, setExpandedIds] = useState(new Set());
    const [projectRoot, setProjectRoot] = useState('');

    const codeDictionary = useMemo(() => {
        const source = (data && typeof data === 'object')
            ? (data.code_dictionary || data.codeDictionary || {})
            : {};
        return {
            issues: source.issues || {},
            suggestions: source.suggestions || {}
        };
    }, [data]);

    const priorityOrder = useMemo(() => ({
        critical: 0,
        high: 1,
        medium: 2,
        low: 3
    }), []);

    const getPriorityRank = useCallback((node) => {
        const raw = node?.priority ?? node?.highestPriority ?? '';
        const normalized = String(raw || '').trim().toLowerCase();
        return priorityOrder[normalized] ?? 999;
    }, [priorityOrder]);

    const sortNodesByPriority = useCallback((nodes) => {
        if (!Array.isArray(nodes) || nodes.length === 0) {
            return [];
        }

        const sorted = [...nodes].sort((a, b) => {
            const aType = a?.type;
            const bType = b?.type;

            if (aType === 'folder' && bType !== 'folder') return -1;
            if (bType === 'folder' && aType !== 'folder') return 1;

            if (aType === 'folder' && bType === 'folder') {
                const aHealth = typeof a?.healthScore === 'number' ? a.healthScore : 1;
                const bHealth = typeof b?.healthScore === 'number' ? b.healthScore : 1;
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
            children: sortNodesByPriority(node?.children || [])
        }));
    }, [getPriorityRank]);

    const groupCandidatesByFile = useCallback((candidates = []) => {
        const groups = new Map();

        candidates.forEach((candidate) => {
            if (!candidate || typeof candidate !== 'object') {
                return;
            }

            const filePath =
                candidate.file_path ||
                candidate.filePath ||
                candidate.path ||
                candidate.file ||
                '';

            if (!filePath) {
                return;
            }

            const normalizedPath = String(filePath);
            const existing = groups.get(normalizedPath) || [];
            existing.push(candidate);
            groups.set(normalizedPath, existing);
        });

        const pickHighestPriority = (entities = []) => {
            return entities
                .map((entity) => String(entity.priority || entity.priority_level || 'low'))
                .reduce((best, current) => {
                    return getPriorityRank({ priority: current }) <
                        getPriorityRank({ priority: best || 'low' })
                        ? current
                        : best;
                }, 'low');
        };

        return Array.from(groups.entries()).map(([filePath, entities]) => {
            const fileName = filePath.split(/[\\/]/).pop() || filePath;
            const entityCount = entities.length;
            const totalIssues = entities.reduce(
                (sum, entity) =>
                    sum + (Array.isArray(entity.issues) ? entity.issues.length : 0),
                0
            );
            const avgScore =
                entityCount > 0
                    ? entities.reduce((sum, entity) => sum + (entity.score || 0), 0) /
                      entityCount
                    : 0;

            return {
                filePath,
                fileName,
                entityCount,
                highestPriority: pickHighestPriority(entities),
                avgScore,
                totalIssues,
                entities,
            };
        });
    }, [getPriorityRank]);

    // Build tree structure from file paths and directory health
    const aggregateTreeMetrics = useCallback((nodes) => {
        if (!Array.isArray(nodes)) {
            return [];
        }

        const cloneNodes = JSON.parse(JSON.stringify(nodes));

        const toNumber = (value) => {
            if (value === null || value === undefined || value === '') {
                return null;
            }
            const numeric = Number(value);
            return Number.isFinite(numeric) ? numeric : null;
        };

        const getNumber = (obj, keys, fallback = 0) => {
            if (!obj) return fallback;
            for (const key of keys) {
                const numeric = toNumber(obj[key]);
                if (numeric != null) {
                    return numeric;
                }
            }
            return fallback;
        };

        const getSeverityCounts = (node) => {
            const counts = node?.severityCounts || node?.severity_counts || {};
            return {
                critical: getNumber(counts, ['critical'], 0),
                high: getNumber(counts, ['high'], 0),
                medium: getNumber(counts, ['medium'], 0),
                low: getNumber(counts, ['low'], 0),
            };
        };

        const bubble = (items) => {
            return items.map((node) => {
                if (!node || typeof node !== 'object') {
                    return node;
                }

                const processedChildren = bubble(node.children || []);

                const baseIssues = getNumber(node, [
                    'totalIssues',
                    'total_issues',
                    'refactoringNeeded',
                    'refactoring_needed',
                    'issueCount',
                    'issue_count',
                ], 0);

                const childIssueSum = processedChildren.reduce(
                    (sum, child) => sum + getNumber(child, ['totalIssues', 'total_issues'], 0),
                    0
                );
                const totalIssues = processedChildren.length > 0
                    ? Math.max(baseIssues, childIssueSum)
                    : baseIssues;

                const severityAggregate = { critical: 0, high: 0, medium: 0, low: 0 };
                const addSeverity = (counts) => {
                    if (!counts) return;
                    severityAggregate.critical += counts.critical || 0;
                    severityAggregate.high += counts.high || 0;
                    severityAggregate.medium += counts.medium || 0;
                    severityAggregate.low += counts.low || 0;
                };

                // For leaf nodes include their own severity counts
                if (processedChildren.length === 0 || node.type === 'entity') {
                    addSeverity(getSeverityCounts(node));
                }

                processedChildren.forEach((child) => addSeverity(getSeverityCounts(child)));

                const childEntityCount = processedChildren.reduce(
                    (sum, child) => sum + getNumber(child, ['entityCount', 'entity_count'], 0),
                    0
                );

                let entityCount = node.type === 'entity' ? 1 : getNumber(node, ['entityCount', 'entity_count'], 0);
                if (node.type === 'folder') {
                    entityCount = childEntityCount > 0 ? childEntityCount : entityCount;
                } else if (node.type === 'file' && entityCount === 0) {
                    entityCount = childEntityCount;
                }

                const childFileCount = processedChildren.reduce((sum, child) => {
                    if (child.type === 'file') {
                        return sum + 1;
                    }
                    return sum + getNumber(child, ['fileCount', 'file_count'], 0);
                }, 0);

                let fileCount = getNumber(node, ['fileCount', 'file_count'], 0);
                if (node.type === 'folder') {
                    fileCount = childFileCount > 0 ? childFileCount : fileCount;
                } else if (node.type === 'file') {
                    fileCount = 1;
                }

                const collectScore = (scores = []) => scores.find((entry) => toNumber(entry) != null);

                const nodeScore = collectScore([
                    node.score,
                    node.avgScore,
                    node.avg_score,
                    node.avgRefactoringScore,
                    node.avg_refactoring_score,
                ]);

                let scoreSum = 0;
                let scoreWeight = 0;

                const addScore = (score, weight = 1) => {
                    const normalized = toNumber(score);
                    if (normalized != null && weight > 0) {
                        scoreSum += normalized * weight;
                        scoreWeight += weight;
                    }
                };

                if (nodeScore != null) {
                    const weight = node.type === 'entity'
                        ? 1
                        : node.type === 'file'
                            ? (entityCount > 0 ? entityCount : 1)
                            : node.type === 'folder'
                                ? (childEntityCount > 0 ? childEntityCount : 1)
                                : 1;
                    addScore(nodeScore, weight);
                }

                processedChildren.forEach((child) => {
                    const childSum = toNumber(child._scoreSum);
                    const childWeight = toNumber(child._scoreCount);
                    if (childSum != null && childWeight != null) {
                        scoreSum += childSum;
                        scoreWeight += childWeight;
                    }
                });

                const avgScore = scoreWeight > 0 ? scoreSum / scoreWeight : toNumber(nodeScore);

                const updatedNode = {
                    ...node,
                    children: processedChildren,
                    totalIssues,
                    severityCounts: severityAggregate,
                    entityCount: node.type === 'entity' ? 1 : entityCount,
                    fileCount,
                    _scoreSum: scoreSum,
                    _scoreCount: scoreWeight,
                };

                updatedNode.total_issues = totalIssues;
                updatedNode.severity_counts = severityAggregate;
                updatedNode.entity_count = entityCount;
                updatedNode.file_count = fileCount;
                updatedNode.refactoringNeeded = updatedNode.refactoringNeeded ?? getNumber(node, ['refactoringNeeded', 'refactoring_needed'], 0);
                updatedNode.refactoring_needed = updatedNode.refactoringNeeded;

                if (node.type === 'folder') {
                    updatedNode.avgScore = avgScore != null ? avgScore : updatedNode.avgScore;
                    updatedNode.avgRefactoringScore = avgScore != null ? avgScore : updatedNode.avgRefactoringScore;
                    updatedNode.refactoringNeeded = totalIssues;
                    updatedNode.criticalIssues = severityAggregate.critical;
                    updatedNode.highPriorityIssues = severityAggregate.high + severityAggregate.critical;
                    updatedNode.critical_issues = updatedNode.criticalIssues;
                    updatedNode.high_priority_issues = updatedNode.highPriorityIssues;
                    updatedNode.avg_refactoring_score = updatedNode.avgRefactoringScore;
                } else if (node.type === 'file') {
                    updatedNode.avgScore = updatedNode.avgScore != null ? updatedNode.avgScore : avgScore;
                    updatedNode.refactoringNeeded = updatedNode.refactoringNeeded ?? totalIssues;
                    updatedNode.avg_refactoring_score = updatedNode.avg_refactoring_score ?? updatedNode.avgScore;
                } else if (node.type === 'entity') {
                    updatedNode.avgScore = updatedNode.score != null ? updatedNode.score : avgScore;
                    updatedNode.score = updatedNode.score != null ? updatedNode.score : avgScore;
                }

                updatedNode.totalIssues = updatedNode.refactoringNeeded ?? totalIssues;
                updatedNode.total_issues = updatedNode.totalIssues;

                return updatedNode;
            });
        };

        const cleanup = (items) => {
            return items.map((node) => {
                if (!node || typeof node !== 'object') {
                    return node;
                }

                const { _scoreSum, _scoreCount, severity_counts, ...rest } = node;
                rest.severityCounts = node.severityCounts || getSeverityCounts(node);

                if (Array.isArray(rest.children) && rest.children.length > 0) {
                    rest.children = cleanup(rest.children);
                }

                return rest;
            });
        };

        const aggregated = bubble(cloneNodes);
        return cleanup(aggregated);
    }, []);

    const buildTreeData = useCallback((refactoringFiles, directoryHealth, coveragePacks, docIssuesMap) => {
        const folderMap = new Map();
        const result = [];
        const directoryLookup = (directoryHealth && directoryHealth.directories) || {};
        const docIssues = docIssuesMap || {};

        const formatIssueCategories = (categories) => {
            if (!categories || typeof categories !== 'object') {
                return [];
            }

            return Object.values(categories)
                .map((category) => ({
                    category: String(category?.category || 'uncategorized'),
                    affectedEntities: category?.affected_entities ?? 0,
                    avgSeverity: category?.avg_severity ?? 0,
                    maxSeverity: category?.max_severity ?? 0,
                    healthImpact: category?.health_impact ?? 0,
                }))
                .sort((a, b) => (b.healthImpact ?? 0) - (a.healthImpact ?? 0));
        };

        // Create a map of coverage packs by file path for easy lookup
        const coverageMap = new Map();
        if (coveragePacks && Array.isArray(coveragePacks)) {
            coveragePacks.forEach(pack => {
                if (pack.path) {
                    coverageMap.set(pack.path, pack);
                }
            });
        }
        
        // Add directory health data first
        if (directoryHealth && directoryHealth.directories) {
            Object.entries(directoryHealth.directories).forEach(([rawPath, health]) => {
                const pathParts = rawPath.split(/[\\\/]/).filter(Boolean);
                let currentPath = '';
                let parentFolder = result;
                
                pathParts.forEach((part, index) => {
                    currentPath += '/' + part;
                    let folder = folderMap.get(currentPath);
                    
                    if (!folder) {
                        const folderChildren = [];
                        
                        folder = {
                            id: 'folder-' + currentPath,
                            name: String(part),
                            type: 'folder',
                            children: folderChildren,
                            healthScore: typeof health?.health_score === 'number' ? health.health_score : undefined,
                            fileCount: typeof health?.file_count === 'number' ? health.file_count : 0,
                            entityCount: typeof health?.entity_count === 'number' ? health.entity_count : 0,
                            refactoringNeeded: typeof health?.refactoring_needed === 'number' ? health.refactoring_needed : 0,
                            criticalIssues: typeof health?.critical_issues === 'number' ? health.critical_issues : 0,
                            highPriorityIssues: typeof health?.high_priority_issues === 'number' ? health.high_priority_issues : 0,
                            avgRefactoringScore: typeof health?.avg_refactoring_score === 'number' ? health.avg_refactoring_score : 0,
                            issueCategories: formatIssueCategories(health?.issue_categories),
                            primaryIssueCategory: health?.primary_issue_category || null
                        };
                        
                        folderMap.set(currentPath, folder);
                        parentFolder.push(folder);
                    } else {
                        // Ensure folder has expected structural fields even if created earlier without health data
                        folder.issueCategories = Array.isArray(folder.issueCategories) ? folder.issueCategories : [];
                        folder.primaryIssueCategory = folder.primaryIssueCategory || null;
                    }

                    const lookupKey = currentPath;
                    const lookupEntry = directoryLookup[lookupKey] || directoryLookup[rawPath];
                    const healthSource = lookupEntry || (index === pathParts.length - 1 ? health : null);

                    if (healthSource) {
                        const categories = formatIssueCategories(healthSource.issue_categories);
                        if (typeof healthSource.health_score === 'number') {
                            folder.healthScore = healthSource.health_score;
                        }
                        if (typeof healthSource.file_count === 'number') {
                            folder.fileCount = healthSource.file_count;
                        }
                        if (typeof healthSource.entity_count === 'number') {
                            folder.entityCount = healthSource.entity_count;
                        }
                        if (typeof healthSource.refactoring_needed === 'number') {
                            folder.refactoringNeeded = healthSource.refactoring_needed;
                        }
                        if (typeof healthSource.critical_issues === 'number') {
                            folder.criticalIssues = healthSource.critical_issues;
                        }
                        if (typeof healthSource.high_priority_issues === 'number') {
                            folder.highPriorityIssues = healthSource.high_priority_issues;
                        }
                        if (typeof healthSource.avg_refactoring_score === 'number') {
                            folder.avgRefactoringScore = healthSource.avg_refactoring_score;
                        }
                        if (Array.isArray(categories) && categories.length > 0) {
                            folder.issueCategories = categories;
                            folder.primaryIssueCategory = healthSource.primary_issue_category || categories[0]?.category || folder.primaryIssueCategory;
                        }
                    }
                    
                    parentFolder = folder.children;
                });
            });
        }
        
        // Add refactoring files
        if (refactoringFiles && refactoringFiles.length > 0) {
            refactoringFiles.forEach((fileGroup, fileIndex) => {
                if (!fileGroup || !fileGroup.filePath) {
                    console.warn('[CodeAnalysisTree] Skipping invalid file group:', fileGroup);
                    return;
                }
                
                const pathParts = fileGroup.filePath.split('/').filter(Boolean);
                const fileName = pathParts.pop();
                let currentPath = '';
                let parentFolder = result;
            
                // Navigate/create folder structure
                pathParts.forEach(part => {
                    currentPath += '/' + part;
                    let folder = folderMap.get(currentPath);
                    
                    if (!folder) {
                        folder = {
                            id: 'folder-' + currentPath,
                            name: String(part),
                            type: 'folder',
                            children: []
                        };
                        folderMap.set(currentPath, folder);
                        parentFolder.push(folder);
                    }
                    
                    parentFolder = folder.children;
                });
                
                // Add file node with synthetic banner child if it has metadata
                const fileNodeId = 'file-' + fileIndex;
                const includeDetailRows = SHOW_ENTITY_DETAIL_ROWS;
                const fileChildren = [];
                
                // Add entity children
                fileChildren.push(...fileGroup.entities.map((entity, entityIndex) => {
                    // Clean up entity name - remove filename and :function: prefix
                    let cleanName = String(entity.name || 'Unknown Entity');
                    // Remove filename prefix (e.g., "./src/core/pipeline/pipeline_executor.rs:function:")
                    const functionMatch = cleanName.match(/:function:(.+)$/);
                    if (functionMatch) {
                        cleanName = functionMatch[1];
                    }
                    
                    const entityNodeId = `entity-${fileIndex}-${entityIndex}`;
                    const entityChildren = [];
                    
                    // Count issues and suggestions by severity level
                    const severityCounts = { critical: 0, high: 0, medium: 0, low: 0 };
                    
                    // Count issues by severity
                    if (entity.issues && Array.isArray(entity.issues)) {
                        entity.issues.forEach(issue => {
                            const severity = getSeverityLevel(issue.priority, issue.severity);
                            severityCounts[severity]++;
                        });
                    }
                    
                    // Count suggestions by severity (using priority/effort/impact)
                    if (entity.suggestions && Array.isArray(entity.suggestions)) {
                        entity.suggestions.forEach(suggestion => {
                            const severity = getSeverityLevel(suggestion.priority, suggestion.impact);
                            severityCounts[severity]++;
                        });
                    }
                    
                    // Always process children - don't gate behind hasEntityMetadata
                    // This ensures issues and suggestions are always added as children
                    {
                        // Look up coverage pack for this file
                        const coveragePack = coverageMap.get(fileGroup.filePath);
                        const coverageInfo = coveragePack?.file_info || null;

                        // Create multiple child nodes instead of one big banner
                        // Each detail row gets a dedicated 40px slot in the tree
                        // Order: Issues first (alert-triangle), then suggestions (lightbulb), then info
                        if (includeDetailRows && entity.issues && Array.isArray(entity.issues)) {
                            entity.issues.forEach((issue, idx) => {
                                // Format issue text with proper capitalization and severity
                                const category = String(issue.category || 'issue');
                                const categoryCapitalized = category.charAt(0).toUpperCase() + category.slice(1);
                                
                                // Extract key metrics from contributing features
                                let metrics = [];
                                if (issue.contributing_features && Array.isArray(issue.contributing_features)) {
                                    issue.contributing_features.forEach(feat => {
                                        if (feat.feature_name === 'lines_of_code' && feat.value) {
                                            metrics.push(`${feat.value} LOC`);
                                        } else if (feat.feature_name === 'cognitive_complexity' && feat.value) {
                                            metrics.push(`Cognitive: ${feat.value}`);
                                        } else if (feat.feature_name === 'cyclomatic_complexity' && feat.value) {
                                            metrics.push(`Cyclomatic: ${feat.value}`);
                                        } else if (feat.feature_name === 'maintainability_index' && feat.value) {
                                            metrics.push(`Maintainability: ${feat.value.toFixed(1)}`);
                                        }
                                    });
                                }

                                const issueMeta = issue.code && codeDictionary.issues
                                    ? codeDictionary.issues[issue.code]
                                    : undefined;
                                const issueCode = issue.code || issueMeta?.code || categoryCapitalized.toUpperCase();
                                const issueTitle = issueMeta?.title || categoryCapitalized;
                                const issueSummary = issueMeta?.summary
                                    || `Signals flagged in the ${categoryCapitalized.toLowerCase()} dimension.`;
                                const severityBadge = typeof issue.severity === 'number'
                                    ? `Severity ${issue.severity.toFixed(1)}`
                                    : null;
                                const badges = [
                                    ...(severityBadge ? [severityBadge] : []),
                                    ...metrics
                                ];

                                const issueChild = {
                                    id: `issue:${entityNodeId}:${idx}`,
                                    name: `${issueCode} · ${issueTitle}`,
                                    type: 'issue-row',
                                    entityScore: entity.score,
                                    issueSeverity: issue.severity,
                                    issueCategory: issue.category,
                                    code: issueCode,
                                    title: issueTitle,
                                    summary: issueSummary,
                                    badges
                                };
                                entityChildren.push(issueChild);
                            });
                        }

                        // Suggestions as separate children - SECOND
                        if (includeDetailRows && entity.suggestions && Array.isArray(entity.suggestions)) {
                            entity.suggestions.forEach((suggestion, idx) => {
                                // Format suggestion text properly
                                const refType = suggestion.refactoring_type || suggestion.type || 'suggestion';
                                let suggestionText = '';

                                // Convert snake_case refactoring types to readable format
                                if (refType.includes('extract_class')) {
                                    suggestionText = 'Extract Class: Split large module into smaller, focused classes';
                                } else if (refType.includes('extract_method')) {
                                    suggestionText = 'Extract Method: Break down complex logic into smaller functions';
                                } else if (refType.includes('reduce_cognitive_complexity')) {
                                    const match = refType.match(/\d+/);
                                    const complexity = match ? match[0] : 'high';
                                    suggestionText = `Reduce Cognitive Complexity (${complexity}): Simplify nested conditions and logic flow`;
                                } else if (refType.includes('reduce_cyclomatic_complexity')) {
                                    const match = refType.match(/\d+/);
                                    const complexity = match ? match[0] : 'high';
                                    suggestionText = `Reduce Cyclomatic Complexity (${complexity}): Reduce branches and decision points`;
                                } else {
                                    // Fallback formatting
                                    const formatted = refType.replace(/_/g, ' ')
                                        .replace(/\b\w/g, l => l.toUpperCase());
                                    suggestionText = formatted;
                                }

                                const [fallbackTitleRaw, ...fallbackSummaryParts] = suggestionText.split(':');
                                const fallbackTitle = fallbackTitleRaw ? fallbackTitleRaw.trim() : suggestionText.trim();
                                const fallbackSummary = fallbackSummaryParts.join(':').trim();

                                // Add effort and impact if available
                                let metadata = [];
                                if (suggestion.effort) {
                                    const effortPercent = (suggestion.effort * 100).toFixed(0);
                                    metadata.push(`Effort ${effortPercent}%`);
                                }
                                if (suggestion.impact) {
                                    const impactPercent = (suggestion.impact * 100).toFixed(0);
                                    metadata.push(`Impact ${impactPercent}%`);
                                }
                                if (suggestion.priority) {
                                    const priorityVal = typeof suggestion.priority === 'number'
                                        ? suggestion.priority.toFixed(2)
                                        : suggestion.priority;
                                    metadata.push(`Priority ${priorityVal}`);
                                }

                                const suggestionMeta = suggestion.code && codeDictionary.suggestions
                                    ? codeDictionary.suggestions[suggestion.code]
                                    : undefined;
                                const suggestionCode = suggestion.code
                                    || suggestionMeta?.code
                                    || fallbackTitle.toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 8)
                                    || 'SUGGEST';
                                const suggestionTitle = suggestionMeta?.title || fallbackTitle || suggestionCode;
                                const suggestionSummary = suggestionMeta?.summary
                                    || fallbackSummary
                                    || fallbackTitle;

                                entityChildren.push({
                                    id: `suggestion:${entityNodeId}:${idx}`,
                                    name: `${suggestionCode} · ${suggestionTitle}`,
                                    type: 'suggestion-row',
                                    code: suggestionCode,
                                    title: suggestionTitle,
                                    summary: suggestionSummary,
                                    badges: metadata,
                                    priority: suggestion.priority,
                                    impact: suggestion.impact,
                                    effort: suggestion.effort,
                                    refactoringType: refType
                                });
                            });
                        }

                        const coverageSummary = coverageInfo
                            ? {
                                coverageBefore: typeof coverageInfo.coverage_before === 'number'
                                    ? coverageInfo.coverage_before
                                    : null,
                                coverageAfter: typeof coverageInfo.coverage_after_if_filled === 'number'
                                    ? coverageInfo.coverage_after_if_filled
                                    : null,
                                linesOfCode: typeof coverageInfo.loc === 'number' ? coverageInfo.loc : null,
                            }
                            : null;

                        const entityNode = {
                            id: entityNodeId,
                            entity_id: entityNodeId,
                            name: cleanName,
                            type: 'entity',
                            priority: String(entity.priority || 'Low'),
                            score: typeof entity.score === 'number' ? entity.score : 0,
                            lineRange: entity.lineRange,
                            issueCount: Array.isArray(entity.issues) ? entity.issues.length : 0,
                            suggestionCount: Array.isArray(entity.suggestions) ? entity.suggestions.length : 0,
                            totalIssues: Array.isArray(entity.issues) ? entity.issues.length : 0,
                            severityCounts: severityCounts,
                            issues: Array.isArray(entity.issues) ? entity.issues : [],
                            suggestions: Array.isArray(entity.suggestions) ? entity.suggestions : [],
                            coverage: coverageSummary,
                            children: includeDetailRows ? entityChildren : []
                        };

                        return entityNode;
                    }
                    
                }));
                
                // Aggregate severity counts from all entities in this file
                const fileSeverityCounts = { critical: 0, high: 0, medium: 0, low: 0 };
                fileGroup.entities.forEach(entity => {
                    // Count issues by severity
                    if (entity.issues && Array.isArray(entity.issues)) {
                        entity.issues.forEach(issue => {
                            const severity = getSeverityLevel(issue.priority, issue.severity);
                            fileSeverityCounts[severity]++;
                        });
                    }
                    
                    // Count suggestions by severity
                    if (entity.suggestions && Array.isArray(entity.suggestions)) {
                        entity.suggestions.forEach(suggestion => {
                            const severity = getSeverityLevel(suggestion.priority, suggestion.impact);
                            fileSeverityCounts[severity]++;
                        });
                    }
                });

                // Look up doc issues for this file path
                const lookupDocIssues = (filePath) => {
                    const normalizedPath = filePath.replace(/^\.?\/+/, '');
                    // Try exact match first
                    if (docIssues[normalizedPath] !== undefined) return docIssues[normalizedPath];
                    if (docIssues[filePath] !== undefined) return docIssues[filePath];
                    // Try matching by filename suffix
                    for (const [key, value] of Object.entries(docIssues)) {
                        if (normalizedPath.endsWith(key) || key.endsWith(normalizedPath)) {
                            return value;
                        }
                    }
                    return null;
                };

                const fileNode = {
                    id: fileNodeId,
                    name: String(fileName),
                    type: 'file',
                    filePath: String(fileGroup.filePath),
                    highestPriority: String(fileGroup.highestPriority || 'Low'),
                    entityCount: typeof fileGroup.entityCount === 'number' ? fileGroup.entityCount : 0,
                    avgScore: typeof fileGroup.avgScore === 'number' ? fileGroup.avgScore : 0,
                    totalIssues: typeof fileGroup.totalIssues === 'number' ? fileGroup.totalIssues : Object.values(fileSeverityCounts).reduce((acc, value) => acc + (value || 0), 0),
                    severityCounts: fileSeverityCounts,
                    docIssues: lookupDocIssues(fileGroup.filePath),
                    children: fileChildren
                };
                
                parentFolder.push(fileNode);
            });
        }
        
        // Bubble up severity counts from children to parents
        const bubbleUpMetrics = (nodes) => {
            return nodes.map((node) => {
                const processedChildren = bubbleUpMetrics(node.children || []);

                const baseIssues =
                    typeof node.totalIssues === 'number'
                        ? node.totalIssues
                        : typeof node.issueCount === 'number'
                            ? node.issueCount
                            : 0;
                const childIssueSum = processedChildren.reduce(
                    (sum, child) => sum + (child.totalIssues || 0),
                    0
                );
                let totalIssues = processedChildren.length > 0
                    ? Math.max(baseIssues, childIssueSum)
                    : baseIssues;

                const aggregatedSeverity = { critical: 0, high: 0, medium: 0, low: 0 };
                const addSeverity = (counts = {}) => {
                    aggregatedSeverity.critical += counts.critical || 0;
                    aggregatedSeverity.high += counts.high || 0;
                    aggregatedSeverity.medium += counts.medium || 0;
                    aggregatedSeverity.low += counts.low || 0;
                };
                if (node.severityCounts && (node.type === 'entity' || processedChildren.length === 0)) {
                    addSeverity(node.severityCounts);
                }
                processedChildren.forEach((child) => addSeverity(child.severityCounts));

                const childEntityCount = processedChildren.reduce(
                    (sum, child) => sum + (child.entityCount || 0),
                    0
                );
                let entityCount = node.type === 'entity' ? 1 : node.entityCount || 0;
                if (node.type === 'folder') {
                    entityCount = childEntityCount > 0 ? childEntityCount : (node.entityCount || 0);
                } else if (node.type === 'file' && entityCount === 0) {
                    entityCount = childEntityCount;
                } else if (node.type === 'entity') {
                    entityCount = 1;
                }

                const childFileCount = processedChildren.reduce((sum, child) => {
                    if (child.type === 'file') {
                        return sum + 1;
                    }
                    return sum + (child.fileCount || 0);
                }, 0);
                let fileCount = node.type === 'folder'
                    ? (childFileCount > 0 ? childFileCount : (node.fileCount || 0))
                    : node.fileCount || 0;
                if (node.type === 'file') {
                    fileCount = 1;
                }

                let scoreSum = 0;
                let scoreCount = 0;
                const addScore = (score, weight = 1) => {
                    if (typeof score === 'number' && Number.isFinite(score) && weight > 0) {
                        scoreSum += score * weight;
                        scoreCount += weight;
                    }
                };

                const nodeScore =
                    typeof node.score === 'number'
                        ? node.score
                        : typeof node.avgScore === 'number'
                            ? node.avgScore
                            : typeof node.avgRefactoringScore === 'number'
                                ? node.avgRefactoringScore
                                : null;

                if (nodeScore != null) {
                    const weight = node.type === 'entity'
                        ? 1
                        : node.type === 'file'
                            ? (node.entityCount && node.entityCount > 0 ? node.entityCount : 1)
                            : node.type === 'folder'
                                ? (childEntityCount > 0 ? childEntityCount : 1)
                                : 1;
                    addScore(nodeScore, weight);
                }

                processedChildren.forEach((child) => {
                    if (typeof child._scoreSum === 'number' && typeof child._scoreCount === 'number') {
                        scoreSum += child._scoreSum;
                        scoreCount += child._scoreCount;
                    }
                });

                const avgScore = scoreCount > 0 ? scoreSum / scoreCount : null;

                let updatedNode = {
                    ...node,
                    children: processedChildren,
                    totalIssues,
                    severityCounts: aggregatedSeverity,
                    entityCount: node.type === 'entity' ? 1 : entityCount,
                    fileCount: node.type === 'folder' ? fileCount : node.fileCount || (node.type === 'file' ? 1 : fileCount),
                    _scoreSum: scoreSum,
                    _scoreCount: scoreCount,
                };

                if (node.type === 'folder') {
                    updatedNode = {
                        ...updatedNode,
                        avgScore: avgScore != null ? avgScore : node.avgScore,
                        avgRefactoringScore: avgScore != null ? avgScore : node.avgRefactoringScore,
                        refactoringNeeded: totalIssues,
                        criticalIssues: aggregatedSeverity.critical,
                        highPriorityIssues: aggregatedSeverity.high + aggregatedSeverity.critical,
                    };
                } else if (node.type === 'file') {
                    updatedNode = {
                        ...updatedNode,
                        avgScore: node.avgScore != null ? node.avgScore : avgScore,
                        refactoringNeeded: node.refactoringNeeded ?? totalIssues,
                    };
                } else if (node.type === 'entity') {
                    updatedNode = {
                        ...updatedNode,
                        avgScore: node.score != null ? node.score : avgScore,
                        score: node.score != null ? node.score : avgScore,
                    };
                }

                return updatedNode;
            });
        };

        const cleanupAggregates = (nodes) => {
            return nodes.map((node) => {
                const { _scoreSum, _scoreCount, ...rest } = node;
                if (rest.children) {
                    rest.children = cleanupAggregates(rest.children);
                }
                return rest;
            });
        };

        // Apply aggregation before sorting
        const aggregatedResult = aggregateTreeMetrics(result);
        const sortedResult = sortNodesByPriority(aggregatedResult);

        return sortedResult;
    }, [aggregateTreeMetrics, codeDictionary, sortNodesByPriority]);

    // Normalize tree data by flattening legacy category wrappers under entities
    const normalizeTreeData = useCallback((nodes) => {
        if (!Array.isArray(nodes)) {
            return [];
        }

        const clonedNodes = JSON.parse(JSON.stringify(nodes));

        const normalizeEntity = (node) => {
            if (!node || typeof node !== 'object' || !Array.isArray(node.children)) {
                return;
            }

            node.children.forEach(normalizeEntity);

            if (node.type !== 'entity') {
                return;
            }

            const detailCandidates = [];
            node.children.forEach((child) => {
                if (child && child.type === 'category' && Array.isArray(child.children)) {
                    detailCandidates.push(...child.children);
                } else if (child) {
                    detailCandidates.push(child);
                }
            });

            const remainingChildren = [];
            const issueRows = [];
            const suggestionRows = [];
            const infoRows = [];
            const rawIssueTypes = new Set(['issue', 'issue-row']);
            const rawSuggestionTypes = new Set(['suggestion', 'suggestion-row']);
            const rawInfoTypes = new Set(['info', 'info-row']);

            detailCandidates.forEach((child) => {
                if (!child) return;
                const childType = String(child.type || '').toLowerCase();
                if (rawIssueTypes.has(childType)) {
                    issueRows.push(child);
                } else if (rawSuggestionTypes.has(childType)) {
                    suggestionRows.push(child);
                } else if (rawInfoTypes.has(childType)) {
                    infoRows.push(child);
                } else {
                    remainingChildren.push(child);
                }
            });

            if ((!Array.isArray(node.issues) || node.issues.length === 0) && issueRows.length > 0) {
                node.issues = issueRows.map((row) => ({
                    title: row.title || row.name,
                    code: row.code,
                    summary: row.summary,
                    severity: row.issueSeverity,
                    category: row.issueCategory,
                    badges: Array.isArray(row.badges) ? row.badges : row.badges == null ? [] : [row.badges].filter(Boolean),
                    contributing_features: Array.isArray(row.contributing_features)
                        ? row.contributing_features
                        : Array.isArray(row.badges)
                            ? row.badges
                                .map((badge) => {
                                    const [label, value] = String(badge).split(':').map((part) => part.trim());
                                    if (!label) return null;
                                    return {
                                        feature_name: label.replace(/\s+/g, '_').toLowerCase(),
                                        value: isNaN(Number(value)) ? value : Number(value),
                                    };
                                })
                                .filter(Boolean)
                            : [],
                }));
            }

            if ((!Array.isArray(node.suggestions) || node.suggestions.length === 0) && suggestionRows.length > 0) {
                node.suggestions = suggestionRows.map((row) => ({
                    title: row.title || row.name,
                    summary: row.summary,
                    code: row.code,
                    priority: row.priority,
                    impact: row.impact,
                    effort: row.effort,
                    refactoring_type: row.refactoringType || row.refactoring_type,
                    badges: Array.isArray(row.badges) ? row.badges : row.badges == null ? [] : [row.badges].filter(Boolean),
                }));
            }

            if (infoRows.length > 0) {
                const coverage = node.coverage || { coverageBefore: null, coverageAfter: null, linesOfCode: null };
                infoRows.forEach((row) => {
                    const label = String(row.name || '').toLowerCase();
                    const numeric = parseFloat(String(row.name || '').replace(/[^0-9.]/g, ''));
                    const toRatio = (value) => {
                        if (!Number.isFinite(value)) return null;
                        return value > 1 ? value / 100 : value;
                    };

                    if (label.includes('coverage before')) {
                        const ratio = toRatio(numeric);
                        if (ratio != null) coverage.coverageBefore = ratio;
                    } else if (label.includes('coverage after')) {
                        const ratio = toRatio(numeric);
                        if (ratio != null) coverage.coverageAfter = ratio;
                    } else if (label.includes('lines of code')) {
                        if (Number.isFinite(numeric)) {
                            coverage.linesOfCode = Math.round(numeric);
                        }
                    }
                });

                if (coverage.coverageBefore != null || coverage.coverageAfter != null || coverage.linesOfCode != null) {
                    node.coverage = coverage;
                }
            }

            // Strip remaining children to avoid rendering legacy detail rows in the tree UI.
            node.children = [];
        };

        clonedNodes.forEach(normalizeEntity);
        return clonedNodes;
    }, []);

    const annotateNodesWithDictionary = useCallback((nodes) => {
        if (!Array.isArray(nodes)) {
            return [];
        }

        const annotate = (node) => {
            if (!node || typeof node !== 'object') {
                return node;
            }

            const clone = { ...node };

            if (Array.isArray(clone.children)) {
                clone.children = clone.children.map(annotate).filter(Boolean);
            }

            if (clone.type === 'issue-row') {
                const meta = clone.code && codeDictionary.issues
                    ? codeDictionary.issues[clone.code]
                    : undefined;
                const category = clone.issueCategory || clone.category || '';
                const fallbackTitle = meta?.title || (category ? category.charAt(0).toUpperCase() + category.slice(1) : 'Issue');
                const summary = meta?.summary || clone.summary || '';
                const severityValue = typeof clone.issueSeverity === 'number'
                    ? clone.issueSeverity
                    : typeof clone.severity === 'number'
                        ? clone.severity
                        : undefined;
                const badges = Array.isArray(clone.badges)
                    ? clone.badges.filter(Boolean).map(String)
                    : [];

                if (severityValue !== undefined) {
                    const severityBadge = `Severity ${severityValue.toFixed(1)}`;
                    if (!badges.includes(severityBadge)) {
                        badges.unshift(severityBadge);
                    }
                }

                clone.code = clone.code || meta?.code || fallbackTitle.toUpperCase();
                clone.title = fallbackTitle;
                clone.summary = summary;
                clone.badges = Array.from(new Set(badges));
                if (clone.code && clone.title) {
                    clone.name = `${clone.code} · ${clone.title}`;
                }
            } else if (clone.type === 'suggestion-row') {
                const meta = clone.code && codeDictionary.suggestions
                    ? codeDictionary.suggestions[clone.code]
                    : undefined;
                const refType = clone.refactoring_type || clone.refactoringType || clone.title || 'suggestion';
                const formatted = String(refType)
                    .replace(/_/g, ' ')
                    .replace(/\b\w/g, (l) => l.toUpperCase());
                const fallbackTitle = meta?.title || clone.title || formatted;
                const summary = meta?.summary || clone.summary || '';
                const badges = Array.isArray(clone.badges)
                    ? clone.badges.filter(Boolean).map(String)
                    : [];

                const addBadge = (badge) => {
                    if (badge && !badges.includes(badge)) {
                        badges.push(badge);
                    }
                };

                if (typeof clone.priority === 'number') {
                    addBadge(`Priority ${clone.priority.toFixed(2)}`);
                } else if (clone.priority) {
                    addBadge(`Priority ${clone.priority}`);
                }
                if (typeof clone.impact === 'number') {
                    addBadge(`Impact ${(clone.impact * 100).toFixed(0)}%`);
                }
                if (typeof clone.effort === 'number') {
                    addBadge(`Effort ${(clone.effort * 100).toFixed(0)}%`);
                }

                clone.code = clone.code || meta?.code || fallbackTitle.toUpperCase().replace(/[^A-Za-z0-9]/g, '').slice(0, 8) || 'SUGGEST';
                clone.title = fallbackTitle;
                clone.summary = summary;
                clone.badges = badges;
                if (clone.code && clone.title) {
                    clone.name = `${clone.code} · ${clone.title}`;
                }
            }

            return clone;
        };

        return nodes.map(annotate).filter(Boolean);
    }, [codeDictionary]);

    // Annotate file nodes with documentation issues from the doc issues map
    const annotateDocIssues = useCallback((nodes, docIssuesMap) => {
        if (!Array.isArray(nodes) || !docIssuesMap || typeof docIssuesMap !== 'object') {
            return nodes;
        }

        const lookupDocIssues = (filePath) => {
            if (!filePath) return null;
            const normalizedPath = filePath.replace(/^\.?\/+/, '');
            // Try exact match first
            if (docIssuesMap[normalizedPath] !== undefined) return docIssuesMap[normalizedPath];
            if (docIssuesMap[filePath] !== undefined) return docIssuesMap[filePath];
            // Try matching by filename suffix
            for (const [key, value] of Object.entries(docIssuesMap)) {
                if (normalizedPath.endsWith(key) || key.endsWith(normalizedPath)) {
                    return value;
                }
            }
            return null;
        };

        const annotate = (node) => {
            if (!node || typeof node !== 'object') {
                return node;
            }

            const clone = { ...node };

            if (Array.isArray(clone.children)) {
                clone.children = clone.children.map(annotate).filter(Boolean);
            }

            // Attach doc issues to file nodes
            if (clone.type === 'file') {
                const filePath = clone.filePath || clone.file_path || clone.path || '';
                const docIssueCount = lookupDocIssues(filePath);
                if (docIssueCount != null) {
                    clone.docIssues = docIssueCount;
                }
            }

            return clone;
        };

        return nodes.map(annotate).filter(Boolean);
    }, []);

    // Load data from props
    useEffect(() => {
        try {
            if (data && typeof data === 'object') {
                // Store project root for VS Code links
                if (data.projectRoot) {
                    setProjectRoot(data.projectRoot);
                }

                const unifiedHierarchy = Array.isArray(data.unifiedHierarchy)
                    ? data.unifiedHierarchy
                    : Array.isArray(data.unified_hierarchy)
                        ? data.unified_hierarchy
                        : [];

                if (unifiedHierarchy.length > 0) {
                    const hierarchy = JSON.parse(JSON.stringify(unifiedHierarchy));
                    const aggregated = aggregateTreeMetrics(hierarchy);
                    const annotated = annotateNodesWithDictionary(aggregated);
                    const normalized = normalizeTreeData(annotated);
                    const aggregatedNormalized = aggregateTreeMetrics(normalized);
                    // Annotate with doc issues from documentation data
                    const docIssuesMap = data.documentation?.file_doc_issues
                        || data.documentation?.fileDocIssues
                        || {};
                    const withDocIssues = annotateDocIssues(aggregatedNormalized, docIssuesMap);
                    const sorted = sortNodesByPriority(withDocIssues);
                    console.info('[CodeAnalysisTree] using unifiedHierarchy; nodes:', sorted.length);
                    setTreeData(sorted);
                    return;
                }

                // Fallback 1: groups precomputed by backend
                const groups = Array.isArray(data.refactoring_candidates_by_file)
                    ? data.refactoring_candidates_by_file
                    : Array.isArray(data.refactoringCandidatesByFile)
                        ? data.refactoringCandidatesByFile
                        : null;

                const toCandidatesFromGroups = (fileGroups) => {
                    if (!Array.isArray(fileGroups)) return [];
                    const result = [];
                    fileGroups.forEach((group) => {
                        const filePath = group.file_path || group.filePath || group.path || '';
                        (group.entities || []).forEach((entity) => {
                            result.push({
                                ...entity,
                                file_path: filePath,
                                filePath: filePath,
                            });
                        });
                    });
                    return result;
                };

                let candidates = Array.isArray(data.refactoring_candidates)
                    ? data.refactoring_candidates
                    : Array.isArray(data.refactoringCandidates)
                        ? data.refactoringCandidates
                        : [];

                if ((!candidates || candidates.length === 0) && groups) {
                    candidates = toCandidatesFromGroups(groups);
                }

                const coveragePacks = Array.isArray(data.coverage_packs)
                    ? data.coverage_packs
                    : Array.isArray(data.coveragePacks)
                        ? data.coveragePacks
                        : [];

                const fileGroups = Array.isArray(groups) && groups.length > 0
                    ? groups.map((g) => ({
                        filePath: g.file_path || g.filePath || g.path || '',
                        fileName: g.file_name || g.fileName || (g.file_path || '').split(/[\\/]/).pop() || '',
                        entityCount: g.entity_count ?? g.entityCount ?? (g.entities ? g.entities.length : 0),
                        avgScore: g.avg_score ?? g.avgScore ?? 0,
                        totalIssues: g.total_issues ?? g.totalIssues ?? 0,
                        highestPriority: g.highest_priority ?? g.highestPriority ?? 'Low',
                        entities: g.entities || [],
                    }))
                    : groupCandidatesByFile(candidates);

                // Extract doc issues map from documentation
                const docIssuesMap = data.documentation?.file_doc_issues
                    || data.documentation?.fileDocIssues
                    || {};

                const treeStructure = buildTreeData(
                    fileGroups,
                    data.directory_health_tree || data.directoryHealthTree || null,
                    coveragePacks,
                    docIssuesMap
                );
                const annotated = annotateNodesWithDictionary(treeStructure);
                const normalized = normalizeTreeData(annotated);
                const aggregatedNormalized = aggregateTreeMetrics(normalized);
                const sorted = sortNodesByPriority(aggregatedNormalized);
                setTreeData(sorted);
            } else {
                setTreeData([]);
            }
        } catch (error) {
            console.error('❌ Failed to load tree data:', error);
            setTreeData([]);
        }
    }, [
        data,
        buildTreeData,
        normalizeTreeData,
        annotateNodesWithDictionary,
        annotateDocIssues,
        sortNodesByPriority,
        aggregateTreeMetrics,
        groupCandidatesByFile,
    ]);

    useEffect(() => {
        if (treeData.length === 0) {
            setExpandedIds(new Set());
            return;
        }

        const allIds = new Set();
        const defaultExpanded = new Set();

        const collect = (nodes, depth) => {
            nodes.forEach((node) => {
                allIds.add(node.id);
                const hasChildren = Array.isArray(node.children) && node.children.length > 0;
                if (hasChildren && depth < 4) {
                    defaultExpanded.add(node.id);
                }
                if (Array.isArray(node.children) && node.children.length > 0) {
                    collect(node.children, depth + 1);
                }
            });
        };

        collect(treeData, 0);

        setExpandedIds((prev) => {
            const next = new Set();
            prev.forEach((id) => {
                if (allIds.has(id)) {
                    next.add(id);
                }
            });
            defaultExpanded.forEach((id) => next.add(id));
            return next;
        });
    }, [treeData]);

    const toggleNode = useCallback((id) => {
        setExpandedIds((prev) => {
            const next = new Set(prev);
            if (next.has(id)) {
                next.delete(id);
            } else {
                next.add(id);
            }
            return next;
        });
    }, []);

    const flatNodes = useMemo(() => {
        const items = [];

        const traverse = (nodes, depth) => {
            nodes.forEach((node) => {
                const hasChildren = Array.isArray(node.children) && node.children.length > 0;
                const isExpanded = expandedIds.has(node.id);
                const wrapper = {
                    id: node.id,
                    data: node,
                    isOpen: hasChildren && isExpanded,
                    isLeaf: !hasChildren,
                    childCount: hasChildren ? node.children.length : 0,
                    level: depth,
                    isSelected: false,
                };
                items.push({ wrapper, hasChildren });
                if (hasChildren && isExpanded) {
                    traverse(node.children, depth + 1);
                }
            });
        };

        traverse(treeData, 0);
        return items;
    }, [treeData, expandedIds]);

    const parentRef = useRef(null);
    const shouldVirtualize = typeof window !== 'undefined' && flatNodes.length > 200;

    const rowVirtualizer = useVirtualizer({
        count: shouldVirtualize ? flatNodes.length : 0,
        getScrollElement: () => parentRef.current,
        estimateSize: () => 44,
        overscan: 10,
    });

    const treeApi = useMemo(() => ({
        toggle: (id) => toggleNode(id),
    }), [toggleNode]);

    if (treeData.length === 0) {
        return React.createElement('div', {
            style: {
                textAlign: 'center',
                padding: '2rem',
                color: 'var(--muted)'
            }
        }, 
            React.createElement('h3', { key: 'title' }, 'No Refactoring Candidates Found'),
            React.createElement('p', { key: 'desc' }, 'Your code is in excellent shape!')
        );
    }

    const renderRow = (wrapper) =>
        React.createElement(TreeNode, {
            key: wrapper.id,
            node: wrapper,
            style: {
                width: '100%',
                height: '100%'
            },
            innerRef: undefined,
            tree: treeApi,
            projectRoot: projectRoot
        });

    if (typeof window !== 'undefined') {
        window.__VALKNUT_TREE_DATA_SNAPSHOT = treeData;
    }

    const treeContainerProps = {
    className: 'tree-scroll-container',
    role: 'tree',
    'aria-label': 'Complexity analysis tree',
    style: {
      height: 600,
      width: '100%',
      overflow: 'auto',
      position: 'relative',
      borderRadius: '8px',
      backgroundColor: 'transparent',
      color: 'var(--tree-foreground, var(--color-text))'
    }
  };

    if (!shouldVirtualize) {
        return React.createElement('div', {
            className: 'valknut-analysis-tree',
            style: { display: 'flex', flexDirection: 'column', gap: '0.75rem' }
        },
            React.createElement('div', treeContainerProps,
                React.createElement('div', { className: 'tree-virtualizer-inner' },
                    flatNodes.map((item) =>
                        React.createElement('div', {
                            key: item.wrapper.id,
                            className: 'tree-virtual-row',
                            role: 'presentation'
                        },
                            renderRow(item.wrapper)
                        )
                    )
                )
            )
        );
    }

    const virtualItems = rowVirtualizer.getVirtualItems();

    return React.createElement('div', {
        className: 'valknut-analysis-tree',
        style: { display: 'flex', flexDirection: 'column', gap: '0.75rem' }
    },
        React.createElement('div', {
            ref: parentRef,
            ...treeContainerProps
        },
            React.createElement('div', {
                className: 'tree-virtualizer-inner',
                style: {
                    height: `${rowVirtualizer.getTotalSize()}px`,
                    width: '100%',
                    position: 'relative'
                }
            },
                virtualItems.map((virtualRow) => {
                    const item = flatNodes[virtualRow.index];
                    const wrapper = item.wrapper;
                    return React.createElement('div', {
                        key: wrapper.id,
                        className: 'tree-virtual-row',
                        role: 'presentation',
                        style: {
                            position: 'absolute',
                            top: 0,
                            left: 0,
                            width: '100%',
                            transform: `translateY(${virtualRow.start}px)`,
                            height: `${virtualRow.size}px`
                        }
                    },
                        renderRow(wrapper)
                    );
                })
            )
        )
    );
};
