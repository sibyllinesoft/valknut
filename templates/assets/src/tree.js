import React, { useState, useEffect, useCallback } from 'react';
import ReactDOM from 'react-dom/client';
import { Tree } from 'react-arborist';

const TreeNode = ({ node, style, innerRef, tree }) => {
    const data = node.data;
    const isFolder = data.type === 'folder';
    const isFile = data.type === 'file';
    const isEntity = data.type === 'entity';
    
    // Check for children using multiple approaches to ensure chevrons show
    // But entities (functions) should never have chevrons, even if they have children
    const hasChildren = !isEntity && (
        node.isInternal || 
        (node.children && node.children.length > 0) || 
        (data.children && data.children.length > 0) ||
        node.hasChildren
    );
    
    // Priority color mapping
    const getPriorityClass = (priority) => {
        switch(priority) {
            case 'critical': return 'tree-badge-Critical';
            case 'high': return 'tree-badge-High';
            case 'medium': return 'tree-badge-Medium';
            case 'low': return 'tree-badge-Low';
            default: return 'tree-badge-Low';
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
            className: 'tree-badge tree-badge-low',
            style: { 
                backgroundColor: getHealthColor(data.healthScore) + '20',
                color: getHealthColor(data.healthScore),
                marginLeft: '0.5rem'
            }
        }, 'Health: ' + (data.healthScore * 100).toFixed(0) + '%'));
    }
    
    // Priority badge
    if (data.priority || data.highestPriority) {
        children.push(React.createElement('div', {
            key: 'priority',
            className: `tree-badge ${getPriorityClass(data.priority || data.highestPriority)}`,
            style: { marginLeft: '0.5rem' }
        }, data.priority || data.highestPriority));
    }
    
    // Entity/file count
    if (data.entityCount || data.fileCount) {
        children.push(React.createElement('div', {
            key: 'count',
            className: 'tree-badge tree-badge-low',
            style: { marginLeft: '0.5rem' }
        }, `${data.entityCount || data.fileCount} ${data.entityCount ? 'entities' : 'files'}`));
    }
    
    // Average score for files
    if (data.avgScore) {
        children.push(React.createElement('div', {
            key: 'score',
            className: 'tree-badge tree-badge-low complexity-score',
            style: { marginLeft: '0.5rem' }
        }, `Avg: ${data.avgScore.toFixed(1)}`));
    }
    
    // Issue count for entities
    if (data.issueCount > 0) {
        children.push(React.createElement('div', {
            key: 'issues',
            className: 'tree-badge tree-badge-danger',
            style: { marginLeft: '0.5rem' }
        }, `${data.issueCount} issues`));
    }
    
    // Suggestion count for entities  
    if (data.suggestionCount > 0) {
        children.push(React.createElement('div', {
            key: 'suggestions',
            className: 'tree-badge tree-badge-info',
            style: { marginLeft: '0.5rem' }
        }, `${data.suggestionCount} suggestions`));
    }
    
    // Manual indentation calculation - ignore react-arborist's style to fix indentation
    const manualIndent = node.level * 24; // 24px per level

    return React.createElement('div', {
        ref: innerRef,
        style: {
            // Completely ignore react-arborist's style and use our own
            display: 'flex',
            alignItems: 'center',
            padding: '0.5rem 0.5rem 0.5rem 0px', // No left padding, we'll add it manually
            marginLeft: `${manualIndent}px`, // Use margin for indentation
            cursor: hasChildren ? 'pointer' : 'default',
            borderRadius: '4px',
            border: '1px solid transparent',
            minHeight: '32px',
            backgroundColor: node.isSelected ? 'rgba(0, 123, 255, 0.1)' : 'transparent',
            width: 'calc(100% - ' + manualIndent + 'px)' // Adjust width to account for margin
        },
        onClick: hasChildren ? () => tree.toggle(node.id) : undefined
    }, ...children.filter(Boolean));
};

// Main tree component
const CodeAnalysisTree = ({ data }) => {
    const [treeData, setTreeData] = useState([]);
    
    // Build tree structure from file paths and directory health
    const buildTreeData = useCallback((refactoringFiles, directoryHealth) => {
        
        const folderMap = new Map();
        const result = [];
        
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
                        folder = {
                            id: 'folder-' + currentPath,
                            name: String(part),
                            type: 'folder',
                            children: [],
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
            
            // Add file node
            const fileNode = {
                id: 'file-' + fileIndex,
                name: String(fileName),
                type: 'file',
                filePath: String(fileGroup.filePath),
                highestPriority: String(fileGroup.highestPriority || 'Low'),
                entityCount: typeof fileGroup.entityCount === 'number' ? fileGroup.entityCount : 0,
                avgScore: typeof fileGroup.avgScore === 'number' ? fileGroup.avgScore : 0,
                totalIssues: typeof fileGroup.totalIssues === 'number' ? fileGroup.totalIssues : 0,
                children: fileGroup.entities.map((entity, entityIndex) => {
                    // Clean up entity name - remove filename and :function: prefix
                    let cleanName = String(entity.name || 'Unknown Entity');
                    // Remove filename prefix (e.g., "./src/core/pipeline/pipeline_config.rs:function:")
                    const functionMatch = cleanName.match(/:function:(.+)$/);
                    if (functionMatch) {
                        cleanName = functionMatch[1];
                    }
                    
                    return {
                        id: `entity-${fileIndex}-${entityIndex}`,
                        name: cleanName,
                        type: 'entity',
                        priority: String(entity.priority || 'Low'),
                        score: typeof entity.score === 'number' ? entity.score : 0,
                        lineRange: entity.lineRange,
                        issueCount: Array.isArray(entity.issues) ? entity.issues.length : 0,
                        suggestionCount: Array.isArray(entity.suggestions) ? entity.suggestions.length : 0,
                        children: []
                    };
                })
            };
            
            parentFolder.push(fileNode);
            });
        }
        
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
        
        const sortedResult = sortNodes(result);
        
        return sortedResult;
    }, []);

    // Load data from props
    useEffect(() => {
        try {
            if (data && typeof data === 'object') {
                const treeStructure = buildTreeData(
                    data.refactoringCandidatesByFile || [],
                    data.directoryHealthTree
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
        openByDefault: false,
        width: '100%',
        height: 600,
        indent: 24, // Indentation per level
        rowHeight: 40,
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