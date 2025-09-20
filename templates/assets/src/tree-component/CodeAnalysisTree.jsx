import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { Tree } from 'react-arborist';
import { TreeNode } from './TreeNode.jsx';
import { getSeverityLevel } from './treeUtils.js';

/**
 * Main tree component for displaying code analysis results
 * Supports both unified hierarchy and legacy refactoring data formats
 */
export const CodeAnalysisTree = ({ data }) => {
    const [treeData, setTreeData] = useState([]);
    const [filterText, setFilterText] = useState('');

    const filterTree = useCallback((nodes, query) => {
        if (!query) {
            return nodes;
        }

        const needle = query.toLowerCase();

        const filterNode = (node) => {
            if (!node) {
                return null;
            }

            const children = Array.isArray(node.children) ? node.children : [];
            const name = String(node.name || '').toLowerCase();
            const matches = name.includes(needle);

            if (matches) {
                return {
                    ...node,
                    children: children.map((child) => filterNode(child) || child),
                };
            }

            const filteredChildren = children.map(filterNode).filter(Boolean);
            if (filteredChildren.length > 0) {
                return {
                    ...node,
                    children: filteredChildren,
                };
            }

            return null;
        };

        return nodes.map(filterNode).filter(Boolean);
    }, []);

    const filteredData = useMemo(
        () => filterTree(treeData, filterText.trim()),
        [treeData, filterText, filterTree]
    );

    // Build tree structure from file paths and directory health
    const buildTreeData = useCallback((refactoringFiles, directoryHealth, coveragePacks) => {
        
        const folderMap = new Map();
        const result = [];
        
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
            Object.entries(directoryHealth.directories).forEach(([path, health]) => {
                const pathParts = path.split('/').filter(Boolean);
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
                            healthScore: typeof health?.health_score === 'number' ? health.health_score : 0,
                            fileCount: typeof health?.file_count === 'number' ? health.file_count : 0,
                            entityCount: typeof health?.entity_count === 'number' ? health.entity_count : 0,
                            refactoringNeeded: Boolean(health?.refactoring_needed),
                            criticalIssues: typeof health?.critical_issues === 'number' ? health.critical_issues : 0,
                            highPriorityIssues: typeof health?.high_priority_issues === 'number' ? health.high_priority_issues : 0,
                            avgRefactoringScore: typeof health?.avg_refactoring_score === 'number' ? health.avg_refactoring_score : 0
                        };
                        
                        folderMap.set(currentPath, folder);
                        parentFolder.push(folder);
                    }
                    
                    parentFolder = folder.children;
                });
            });
        }
        
        // Add refactoring files
        if (refactoringFiles && refactoringFiles.length > 0) {
            refactoringFiles.forEach((fileGroup, fileIndex) => {
                if (!fileGroup || !fileGroup.filePath) {
                    console.warn('⚠️ Skipping invalid file group:', fileGroup);
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
                
                // Add synthetic banner row for entity if it has metadata
                const hasEntityMetadata = entity.score || entity.lineRange || entity.priority || 
                                        (Array.isArray(entity.issues) && entity.issues.length > 0) ||
                                        (Array.isArray(entity.suggestions) && entity.suggestions.length > 0) ||
                                        entity.issueCategories || entity.suggestionTypes;
                if (hasEntityMetadata) {
                    // Look up coverage pack for this file
                    const coveragePack = coverageMap.get(fileGroup.filePath);
                    
                    // Create multiple child nodes instead of one big banner
                    // Each gets its own 40px row in react-arborist
                    // Order: Issues first (alert-triangle), then suggestions (lightbulb), then info
                    
                    // Issues as separate children - FIRST
                    if (entity.issues && Array.isArray(entity.issues)) {
                        entity.issues.forEach((issue, idx) => {
                            // Fix the score display - use the actual entity score, not the issue severity
                            let issueText = `${issue.category}: ${issue.description}`;
                            
                            // For complexity issues, show the actual entity score
                            if (issue.category?.toLowerCase().includes('complexity') && entity.score) {
                                issueText = `${issue.category}: ${issue.description.replace('score: 0.0', `score: ${entity.score}`)}`;
                            }
                            
                            const issueChild = {
                                id: `issue:${entityNodeId}:${idx}`,
                                name: issueText,
                                type: 'issue-row',
                                entityScore: entity.score, // Pass through entity score
                                issueSeverity: issue.severity, // Pass through issue severity
                                issueCategory: issue.category, // Pass through issue category
                                children: []
                            };
                            entityChildren.push(issueChild);
                        });
                    }
                    
                    // Suggestions as separate children - SECOND
                    if (entity.suggestions && Array.isArray(entity.suggestions)) {
                        entity.suggestions.forEach((suggestion, idx) => {
                            // Fix the score display in suggestions too
                            let suggestionText = `${suggestion.type}: ${suggestion.description}`;
                            
                            // For complexity suggestions, show the actual entity score
                            if (suggestion.description?.includes('score: 0.0') && entity.score) {
                                suggestionText = `${suggestion.type}: ${suggestion.description.replace('score: 0.0', `score: ${entity.score}`)}`;
                            }
                            
                            // For extract method suggestions, include the method name context
                            if (suggestion.type?.toLowerCase().includes('extract_method') || 
                                suggestion.type?.toLowerCase().includes('extract method')) {
                                suggestionText = `Extract Method for ${cleanName}: ${suggestion.description}`;
                            }
                            
                            entityChildren.push({
                                id: `suggestion:${entityNodeId}:${idx}`,
                                name: suggestionText,
                                type: 'suggestion-row',
                                children: []
                            });
                        });
                    }
                    
                    // Coverage info as separate children - LAST
                    if (coveragePack && coveragePack.file_info) {
                        if (coveragePack.file_info.coverage_before !== undefined) {
                            entityChildren.push({
                                id: `info:${entityNodeId}:coverage-before`,
                                name: `Coverage Before: ${(coveragePack.file_info.coverage_before * 100).toFixed(1)}%`,
                                type: 'info-row',
                                children: []
                            });
                        }
                        if (coveragePack.file_info.coverage_after_if_filled !== undefined) {
                            entityChildren.push({
                                id: `info:${entityNodeId}:coverage-after`,
                                name: `Coverage After: ${(coveragePack.file_info.coverage_after_if_filled * 100).toFixed(1)}%`,
                                type: 'info-row',
                                children: []
                            });
                        }
                        if (coveragePack.file_info.loc) {
                            entityChildren.push({
                                id: `info:${entityNodeId}:loc`,
                                name: `Lines of Code: ${coveragePack.file_info.loc}`,
                                type: 'info-row',
                                children: []
                            });
                        }
                    }
                }
                
                return {
                    id: entityNodeId,
                    name: cleanName,
                    type: 'entity',
                    priority: String(entity.priority || 'Low'),
                    score: typeof entity.score === 'number' ? entity.score : 0,
                    lineRange: entity.lineRange,
                    issueCount: Array.isArray(entity.issues) ? entity.issues.length : 0,
                    suggestionCount: Array.isArray(entity.suggestions) ? entity.suggestions.length : 0,
                    severityCounts: severityCounts,
                    children: entityChildren
                };
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

            const fileNode = {
                id: fileNodeId,
                name: String(fileName),
                type: 'file',
                filePath: String(fileGroup.filePath),
                highestPriority: String(fileGroup.highestPriority || 'Low'),
                entityCount: typeof fileGroup.entityCount === 'number' ? fileGroup.entityCount : 0,
                avgScore: typeof fileGroup.avgScore === 'number' ? fileGroup.avgScore : 0,
                totalIssues: typeof fileGroup.totalIssues === 'number' ? fileGroup.totalIssues : 0,
                severityCounts: fileSeverityCounts,
                children: fileChildren
            };
            
            parentFolder.push(fileNode);
            });
        }
        
        // Bubble up severity counts from children to parents
        const bubbleUpSeverityCounts = (nodes) => {
            return nodes.map(node => {
                // First, recursively process children
                const processedChildren = bubbleUpSeverityCounts(node.children || []);
                
                // If this is a folder, aggregate severity counts from all children
                if (node.type === 'folder') {
                    const folderSeverityCounts = { critical: 0, high: 0, medium: 0, low: 0 };
                    
                    const aggregateFromChild = (child) => {
                        if (child.severityCounts) {
                            folderSeverityCounts.critical += child.severityCounts.critical || 0;
                            folderSeverityCounts.high += child.severityCounts.high || 0;
                            folderSeverityCounts.medium += child.severityCounts.medium || 0;
                            folderSeverityCounts.low += child.severityCounts.low || 0;
                        }
                        // Recursively aggregate from grandchildren
                        (child.children || []).forEach(aggregateFromChild);
                    };
                    
                    processedChildren.forEach(aggregateFromChild);
                    
                    return {
                        ...node,
                        severityCounts: folderSeverityCounts,
                        children: processedChildren
                    };
                }
                
                return {
                    ...node,
                    children: processedChildren
                };
            });
        };

        // Sort function: directories first, then by health score/priority
        const sortNodes = (nodes) => {
            return nodes.sort((a, b) => {
                // Folders first
                if (a.type === 'folder' && b.type !== 'folder') return -1;
                if (b.type === 'folder' && a.type !== 'folder') return 1;
                
                // Sort by health score for folders (lower = more critical)
                if (a.type === 'folder' && b.type === 'folder') {
                    const aHealth = a.healthScore || 1;
                    const bHealth = b.healthScore || 1;
                    if (aHealth !== bHealth) return aHealth - bHealth;
                }
                
                // Sort by priority for files/entities
                const priorityOrder = { critical: 0, high: 1, medium: 2, low: 3 };
                const aPri = priorityOrder[a.priority || a.highestPriority] || 999;
                const bPri = priorityOrder[b.priority || b.highestPriority] || 999;
                if (aPri !== bPri) return aPri - bPri;
                
                // Finally by name
                return a.name.localeCompare(b.name);
            }).map(node => ({
                ...node,
                children: sortNodes(node.children || [])
            }));
        };
        
        // Apply severity count bubbling before sorting
        const bubblerResult = bubbleUpSeverityCounts(result);
        const sortedResult = sortNodes(bubblerResult);
        
        return sortedResult;
    }, []);

    // Load data from props
    useEffect(() => {
        try {
            if (data && typeof data === 'object') {
                // Use unifiedHierarchy if available, fallback to refactoringCandidatesByFile for backward compatibility
                const hierarchyData = data.unifiedHierarchy || data.refactoringCandidatesByFile || [];
                
                if (data.unifiedHierarchy) {
                    // New unified hierarchy format - use directly
                    setTreeData(hierarchyData);
                } else {
                    // Legacy format - build tree structure
                    const treeStructure = buildTreeData(
                        hierarchyData,
                        data.directoryHealthTree,
                        data.coveragePacks || []
                    );
                    setTreeData(treeStructure);
                }
            } else {
                setTreeData([]);
            }
        } catch (error) {
            console.error('❌ Failed to load tree data:', error);
            setTreeData([]);
        }
    }, [data, buildTreeData]);

    const handleFilterChange = useCallback((event) => {
        setFilterText(event.target.value);
    }, []);
    
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

    const hasMatchingResults = filteredData.length > 0;

    return React.createElement('div', {
        className: 'valknut-analysis-tree',
        style: { display: 'flex', flexDirection: 'column', gap: '0.75rem' }
    },
        React.createElement('div', {
            key: 'controls',
            className: 'valknut-analysis-tree__controls',
            style: {
                display: 'flex',
                gap: '0.5rem',
                alignItems: 'center'
            }
        },
            React.createElement('input', {
                key: 'search',
                type: 'search',
                value: filterText,
                onChange: handleFilterChange,
                placeholder: 'Filter by file, folder, or entity name…',
                style: {
                    flex: 1,
                    padding: '0.5rem 0.75rem',
                    borderRadius: '6px',
                    border: '1px solid var(--border, #e0e0e0)',
                    fontSize: '0.95rem'
                }
            }),
            filterText
                ? React.createElement('span', {
                      key: 'results',
                      style: {
                          color: 'var(--text-secondary)',
                          fontSize: '0.85rem'
                      }
                  }, `${hasMatchingResults ? filteredData.length : 0} matches`)
                : null
        ),
        hasMatchingResults
            ? React.createElement(Tree, {
                  key: 'tree',
                  data: filteredData,
                  openByDefault: (node) => {
                      // Open folders and files by default, but keep entities (functions) closed
                      return node.data.type === 'folder' || node.data.type === 'file';
                  },
                  width: '100%',
                  height: 600,
                  indent: 24,
                  rowHeight: 40,
                  overscanCount: 10,
                  disableEdit: true,
                  disableDrop: true,
                  children: TreeNode
              })
            : React.createElement('div', {
                  key: 'no-results',
                  style: {
                      textAlign: 'center',
                      padding: '2rem',
                      color: 'var(--muted)'
                  }
              },
                  React.createElement('h3', { key: 'title' }, 'No matches for that filter'),
                  React.createElement('p', { key: 'desc' }, 'Try a different keyword or clear the filter input')
              )
    );
};
