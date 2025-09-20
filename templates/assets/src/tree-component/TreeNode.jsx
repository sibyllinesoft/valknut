import React, { useEffect, useRef } from 'react';

/**
 * Individual tree node component for React Arborist
 * Handles rendering of different node types: folder, file, entity, and info/issue/suggestion rows
 */
export const TreeNode = ({ node, style, innerRef, tree }) => {
    const data = node.data;
    const iconRefs = useRef([]);
    iconRefs.current = [];

    const registerIcon = (element, fallback) => {
        if (element) {
            iconRefs.current.push({ element, fallback });
        }
    };

    const isFolder = data.type === 'folder';
    const isFile = data.type === 'file';
    const isEntity = data.type === 'entity';
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
    if (isInfoRow || isIssueRow || isSuggestionRow) {
        const manualIndent = (node.level * 24) + 16; // Extra indent for banner rows
        let iconName = 'info';
        let iconColor = 'var(--text-secondary)';
        let backgroundColor = 'transparent';
        let iconFallbackSymbol = 'ℹ️';

        if (isIssueRow) {
            iconName = 'alert-triangle';
            iconColor = 'var(--danger, #dc3545)';
            backgroundColor = 'rgba(220, 53, 69, 0.05)';
            iconFallbackSymbol = '⚠️';
        } else if (isSuggestionRow) {
            iconName = 'lightbulb';
            iconColor = 'var(--info, #007acc)';
            backgroundColor = 'rgba(0, 123, 255, 0.05)';
            iconFallbackSymbol = '💡';
        } else if (isInfoRow) {
            iconName = 'info';
            iconColor = 'var(--success, #28a745)';
            backgroundColor = 'rgba(40, 167, 69, 0.05)';
            iconFallbackSymbol = 'ℹ️';
        }
        
        // Parse text and score for complexity/structure issues
        let displayText = data.name;
        let scoreElement = null;
        
        // Clean up text by removing emoji prefixes and category prefixes first
        displayText = displayText
            .replace(/^(⚠️|💡|ℹ️)\s*/, '')
            .replace(/^(complexity|structure):\s*/i, '')
            .trim();
        
        // For issue rows (alert-triangle), check if it's complexity or structure and extract score
        if (isIssueRow) {
            const isComplexityIssue = data.name.toLowerCase().includes('complexity');
            const isStructureIssue = data.name.toLowerCase().includes('structure');
            
            if (isComplexityIssue || isStructureIssue) {
                // For "very high complexity/structural" descriptions, we need to use the entity score
                // Extract from text like "score: X" or look for entity score context
                let score = null;
                const scoreMatch = data.name.match(/score:\s*(\d+(?:\.\d+)?)/);
                
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
                    // Use the same colors as the banner (background and left border)
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
                    }, score.toString());
                }
            }
        }
        
        const children = [
            React.createElement('i', {
                'data-lucide': iconName,
                key: 'icon',
                ref: (el) => registerIcon(el, iconFallbackSymbol),
                style: { 
                    width: '14px', 
                    height: '14px', 
                    marginRight: '0.5rem',
                    color: iconColor,
                    flexShrink: 0
                }
            }),
            React.createElement('span', {
                key: 'text',
                style: { flex: 1 }
            }, displayText),
            scoreElement
        ].filter(Boolean);
        
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
        }, ...children);
    }
    
    // Regular node rendering (folder, file, entity)
    // Check for children using multiple approaches to ensure chevrons show
    // Entities can now have children (issue/suggestion banners)
    const hasChildren = (
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
            ref: (el) => registerIcon(el, fallbackSymbol)
        }));
    } else {
        // Add spacing for nodes without children to align with expandable nodes
        children.push(React.createElement('div', {
            key: 'spacer',
            style: { width: '16px', marginRight: '0.25rem' }
        }));
    }
    
    // Icon
    let iconName = 'function-square'; // default for entities
    let iconFallbackSymbol = '🔧';
    if (isFolder) {
        iconName = 'folder';
        iconFallbackSymbol = '📁';
    } else if (isFile) {
        iconName = 'file-code';
        iconFallbackSymbol = '📄';
    }

    children.push(React.createElement('i', {
        'data-lucide': iconName,
        key: 'icon',
        ref: (el) => registerIcon(el, iconFallbackSymbol),
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
    
    // Manual indentation calculation - ignore react-arborist's style to fix indentation
    const manualIndent = node.level * 24; // 24px per level

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
