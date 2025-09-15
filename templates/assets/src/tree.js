import React, { useState, useEffect, useCallback } from 'react';
import { Tree } from 'react-arborist';

const TreeNode = ({ node, style, innerRef, tree }) => {
    const data = node.data;
    const isFolder = data.type === 'folder';
    const isFile = data.type === 'file';
    const isEntity = data.type === 'entity';
    
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
    
    const children = [
        // Icon
        React.createElement('i', {
            'data-lucide': isFolder ? 'folder' : (isFile ? 'file-code' : 'function-square'),
            key: 'icon',
            style: { width: '16px', height: '16px', marginRight: '0.5rem' }
        }),
        
        // Label
        React.createElement('span', {
            key: 'label',
            style: { flex: 1, fontWeight: isFolder ? '500' : 'normal' }
        }, data.name)
    ];
    
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
    
    return React.createElement('div', {
        ref: innerRef,
        style: {
            display: 'flex',
            alignItems: 'center',
            padding: '0.5rem',
            cursor: 'pointer',
            borderRadius: '4px',
            border: '1px solid transparent',
        },
        onClick: () => tree.toggle(node.id)
    }, ...children.filter(Boolean));
};

// Main tree component
const CodeAnalysisTree = ({ data }) => {
    const [treeData, setTreeData] = useState([]);
    
    // Build tree structure from file paths and directory health
    const buildTreeData = useCallback((refactoringFiles, directoryHealth) => {
        console.log('🏗️ buildTreeData called with:', {
            refactoringFilesCount: refactoringFiles?.length || 0,
            directoryHealthPresent: !!directoryHealth,
            directoryHealthDirs: directoryHealth?.directories ? Object.keys(directoryHealth.directories).length : 0
        });
        
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
        refactoringFiles.forEach((fileGroup, fileIndex) => {
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
                children: fileGroup.entities.map((entity, entityIndex) => ({
                    id: `entity-${fileIndex}-${entityIndex}`,
                    name: String(entity.name || 'Unknown Entity'),
                    type: 'entity',
                    priority: String(entity.priority || 'Low'),
                    score: typeof entity.score === 'number' ? entity.score : 0,
                    lineRange: entity.lineRange,
                    issueCount: Array.isArray(entity.issues) ? entity.issues.length : 0,
                    suggestionCount: Array.isArray(entity.suggestions) ? entity.suggestions.length : 0,
                    children: []
                }))
            };
            
            parentFolder.push(fileNode);
        });
        
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
        
        console.log('🌳 buildTreeData returning:', {
            resultLength: result.length,
            firstFewNodes: result.slice(0, 3).map(n => ({name: n.name, type: n.type, childrenCount: n.children?.length}))
        });
        
        return sortNodes(result);
    }, []);

    // Load data from props
    useEffect(() => {
        try {
            console.log('🔍 Loading tree data from props...');
            console.log('📊 Props data:', data);
            
            if (data && typeof data === 'object') {
                console.log('📊 Refactoring candidates:', data.refactoringCandidatesByFile?.length || 0);
                console.log('🏗️ Directory health tree:', data.directoryHealthTree ? 'present' : 'missing');
                
                const treeStructure = buildTreeData(
                    data.refactoringCandidatesByFile || [],
                    data.directoryHealthTree
                );
                console.log('🌳 Built tree structure, nodes:', treeStructure.length);
                setTreeData(treeStructure);
            } else {
                console.warn('⚠️ No valid data provided in props');
                setTreeData([]);
            }
        } catch (error) {
            console.error('❌ Failed to load tree data:', error);
            setTreeData([]);
        }
    }, [data, buildTreeData]);
    
    if (treeData.length === 0) {
        console.log('⚠️ Showing "no analysis data" message - treeData.length is 0');
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
        indent: 24,
        rowHeight: 40,
        children: TreeNode
    });
};

// Export for global use
window.CodeAnalysisTree = CodeAnalysisTree;