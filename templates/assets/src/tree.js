import React, { useState, useEffect, useCallback } from 'react';
import ReactDOM from 'react-dom/client';
import { Tree } from 'react-arborist';

const TreeNode = ({ node, style, innerRef, tree }) => {
    const data = node.data;
    const isFolder = data.type === 'folder';
    const isFile = data.type === 'file';
    const isEntity = data.type === 'entity';
    const isInfoRow = data.type === 'info-row';
    const isIssueRow = data.type === 'issue-row';
    const isSuggestionRow = data.type === 'suggestion-row';
    
    // Handle info/issue/suggestion rows
    if (isInfoRow || isIssueRow || isSuggestionRow) {
        const manualIndent = node.level * 24; // 24px per level
        let iconColor = 'var(--text-secondary)';
        let backgroundColor = 'transparent';
        
        if (isIssueRow) {
            iconColor = 'var(--danger, #dc3545)';
            backgroundColor = 'rgba(220, 53, 69, 0.05)';
        } else if (isSuggestionRow) {
            iconColor = 'var(--info, #007acc)';
            backgroundColor = 'rgba(0, 123, 255, 0.05)';
        } else if (isInfoRow) {
            iconColor = 'var(--success, #28a745)';
            backgroundColor = 'rgba(40, 167, 69, 0.05)';
        }
        
        return React.createElement('div', {
            ref: innerRef,
            style: {
                ...style,
                display: 'flex',
                alignItems: 'center',
                padding: '0.4rem 0.5rem',
                marginLeft: `${manualIndent}px`,
                backgroundColor: backgroundColor,
                borderLeft: `3px solid ${iconColor}`,
                fontSize: '0.85rem',
                color: 'var(--text)',
                width: `calc(100% - ${manualIndent}px)`,
                boxSizing: 'border-box'
            }
        }, data.name);
    }
    
    // Regular node rendering (folder, file, entity)
    // Check for children using multiple approaches to ensure chevrons show
    // But entities (functions) should never have chevrons, even if they have children
    const hasChildren = !isEntity && (
        node.isInternal || 
        (node.children && node.children.length > 0) || 
        (data.children && data.children.length > 0) ||
        node.hasChildren
    );
    
    // Priority color mapping with actual styling
    const getPriorityStyle = (priority) => {
        switch(priority?.toLowerCase()) {
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
    
    // Expand/collapse arrow for nodes with children
    if (hasChildren) {
        console.log('🔽 RENDERING CHEVRON for', data.name, '- isOpen:', node.isOpen, 'chevron:', node.isOpen ? 'chevron-down' : 'chevron-right');
        const chevronIcon = node.isOpen ? 'chevron-down' : 'chevron-right';
        const fallbackSymbol = node.isOpen ? '▼' : '▶'; // Unicode fallback
        
        children.push(React.createElement('i', {
            'data-lucide': chevronIcon,
            key: 'chevron',
            className: 'tree-chevron-icon',
            style: { 
                width: '16px', 
                height: '16px', 
                marginRight: '0.25rem',
                cursor: 'pointer',
                transition: 'transform 0.2s ease',
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: 'var(--text-secondary, #666)',
                fontSize: '12px',
                userSelect: 'none'
            },
            onClick: (e) => {
                e.stopPropagation();
                tree.toggle(node.id);
            },
            // Force Lucide refresh and add fallback
            ref: (el) => {
                if (el) {
                    // Add fallback text in case Lucide doesn't render
                    if (!el.querySelector('svg')) {
                        el.textContent = fallbackSymbol;
                    }
                    
                    // Try to initialize Lucide
                    if (typeof window !== 'undefined' && window.lucide) {
                        setTimeout(() => {
                            window.lucide.createIcons();
                            // Check if Lucide worked, if not use fallback
                            if (!el.querySelector('svg')) {
                                el.textContent = fallbackSymbol;
                            }
                        }, 50);
                    }
                }
            }
        }));
    } else {
        console.log('❌ NO CHEVRON for', data.name, '- hasChildren is false');
        // Add spacing for nodes without children to align with expandable nodes
        children.push(React.createElement('div', {
            key: 'spacer',
            style: { width: '16px', marginRight: '0.25rem' }
        }));
    }
    
    // Icon
    let iconName = 'function-square'; // default for entities
    if (isFolder) iconName = 'folder';
    else if (isFile) iconName = 'file-code';
    
    children.push(React.createElement('i', {
        'data-lucide': iconName,
        key: 'icon',
        style: { width: '16px', height: '16px', marginRight: '0.5rem' }
    }));
    
    // Label
    children.push(React.createElement('span', {
        key: 'label',
        style: { flex: 1, fontWeight: isFolder ? '500' : 'normal' }
    }, data.name));
    
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
    
    // Critical issues for folders
    if (isFolder && data.criticalIssues) {
        children.push(React.createElement('div', {
            key: 'critical',
            className: 'tree-badge tree-badge-danger',
            style: { marginLeft: '0.5rem' }
        }, `${data.criticalIssues} critical`));
    }
    
    // High priority issues for folders
    if (isFolder && data.highPriorityIssues) {
        children.push(React.createElement('div', {
            key: 'high',
            className: 'tree-badge tree-badge-warning',
            style: { marginLeft: '0.5rem' }
        }, `${data.highPriorityIssues} high`));
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
    if ((isFolder || isFile) && data.severityCounts) {
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
    
    // Complexity score for files
    if (isFile && data.avgScore) {
        children.push(React.createElement('div', {
            key: 'score',
            className: 'tree-badge tree-badge-low complexity-score',
            style: { marginLeft: '0.5rem' }
        }, `Complexity: ${data.avgScore.toFixed(1)}`));
    }
    
    // Remove issue counts - user said no need for # issue count
    
    // Complexity score for entities
    if (isEntity && data.score) {
        children.push(React.createElement('div', {
            key: 'complexity',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `Complexity: ${data.score}`));
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
    
    // Remove issue and suggestion counts - user said no need for # issue count
    
    // Manual indentation calculation - ignore react-arborist's style to fix indentation
    const manualIndent = node.level * 24; // 24px per level

    // Log folder health for debugging
    if (isFolder) {
        console.log('📊 Folder health for', data.name, '- health:', data.healthScore, 'fileCount:', data.fileCount, 'entityCount:', data.entityCount);
    }

    // Header row (clickable part with icon, label, badges)
    return React.createElement('div', {
        ref: innerRef,
        className: 'tree-header-row',
        style: {
            ...style,
            display: 'flex',
            alignItems: 'center',
            cursor: hasChildren ? 'pointer' : 'default',
            padding: '0.5rem 0.5rem 0.5rem 0px',
            marginLeft: `${manualIndent}px`,
            borderRadius: '4px',
            border: '1px solid transparent',
            backgroundColor: node.isSelected ? 'rgba(0, 123, 255, 0.1)' : 'transparent',
            width: 'calc(100% - ' + manualIndent + 'px)',
            minHeight: '32px',
            gap: '0.5rem'
        },
        onClick: hasChildren ? () => tree.toggle(node.id) : undefined
    }, ...children.filter(Boolean));
};

// Main tree component
const CodeAnalysisTree = ({ data }) => {
    const [treeData, setTreeData] = useState([]);
    
    // Helper function to get severity from priority/severity values
    const getSeverityLevel = (priority, severity) => {
        // Priority can be string like "critical", "high", etc
        if (typeof priority === 'string') {
            const p = priority.toLowerCase();
            if (p.includes('critical')) return 'critical';
            if (p.includes('high')) return 'high';
            if (p.includes('medium') || p.includes('moderate')) return 'medium';
            if (p.includes('low')) return 'low';
        }
        
        // Severity can be numeric (0-1 scale) or string
        if (typeof severity === 'number') {
            if (severity >= 0.8) return 'critical';
            if (severity >= 0.6) return 'high';
            if (severity >= 0.4) return 'medium';
            return 'low';
        }
        
        // Fallback
        return 'low';
    };

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
                        
                        // No banners for folders - badges show all needed info
                        
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
            
            // No banners for files - badges show all needed info
            
            // Add entity children
            fileChildren.push(...fileGroup.entities.map((entity, entityIndex) => {
                // Clean up entity name - remove filename and :function: prefix
                let cleanName = String(entity.name || 'Unknown Entity');
                // Remove filename prefix (e.g., "./src/core/pipeline/pipeline_config.rs:function:")
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
                    
                    // Coverage info as separate children
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
                    
                    // Issues as separate children
                    if (entity.issues && Array.isArray(entity.issues)) {
                        entity.issues.forEach((issue, idx) => {
                            // Fix the score display - use the actual entity score, not the issue severity
                            let issueText = `⚠️ ${issue.category}: ${issue.description}`;
                            
                            // For complexity issues, show the actual entity score
                            if (issue.category?.toLowerCase().includes('complexity') && entity.score) {
                                issueText = `⚠️ ${issue.category}: ${issue.description.replace('score: 0.0', `score: ${entity.score}`)}`;
                            }
                            
                            entityChildren.push({
                                id: `issue:${entityNodeId}:${idx}`,
                                name: issueText,
                                type: 'issue-row',
                                children: []
                            });
                        });
                    }
                    
                    // Suggestions as separate children
                    if (entity.suggestions && Array.isArray(entity.suggestions)) {
                        entity.suggestions.forEach((suggestion, idx) => {
                            // Fix the score display in suggestions too
                            let suggestionText = `💡 ${suggestion.type}: ${suggestion.description}`;
                            
                            // For complexity suggestions, show the actual entity score
                            if (suggestion.description?.includes('score: 0.0') && entity.score) {
                                suggestionText = `💡 ${suggestion.type}: ${suggestion.description.replace('score: 0.0', `score: ${entity.score}`)}`;
                            }
                            
                            // For extract method suggestions, include the method name context
                            if (suggestion.type?.toLowerCase().includes('extract_method') || 
                                suggestion.type?.toLowerCase().includes('extract method')) {
                                suggestionText = `💡 Extract Method for ${cleanName}: ${suggestion.description}`;
                            }
                            
                            entityChildren.push({
                                id: `suggestion:${entityNodeId}:${idx}`,
                                name: suggestionText,
                                type: 'suggestion-row',
                                children: []
                            });
                        });
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
                const treeStructure = buildTreeData(
                    data.refactoringCandidatesByFile || [],
                    data.directoryHealthTree,
                    data.coveragePacks || []
                );
                setTreeData(treeStructure);
            } else {
                setTreeData([]);
            }
        } catch (error) {
            console.error('❌ Failed to load tree data:', error);
            setTreeData([]);
        }
    }, [data, buildTreeData]);
    
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
    
    return React.createElement(Tree, {
        data: treeData,
        openByDefault: true,
        width: '100%',
        height: 600,
        indent: 24, // Indentation per level
        rowHeight: 40, // Fixed height - now works since each info/issue/suggestion gets its own row
        overscanCount: 10, // Render extra rows for better scrolling
        disableEdit: true, // Disable inline editing
        disableDrop: true, // Disable drag and drop
        children: TreeNode
    });
};

// Export for webpack library
export default CodeAnalysisTree;

// Also export for global use (fallback) and expose React/ReactDOM
if (typeof window !== 'undefined') {
    // Expose React and ReactDOM on window for template compatibility
    window.React = React;
    window.ReactDOM = ReactDOM;
    
    // Export the component with both names for compatibility
    window.CodeAnalysisTree = CodeAnalysisTree;
    window.ReactTreeBundle = CodeAnalysisTree;
}