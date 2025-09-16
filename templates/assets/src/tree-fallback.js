/**
 * Simple HTML/CSS tree fallback - no React dependencies
 * This provides a reliable tree view when React bundling fails
 */

function createSimpleTreeView(data) {
    if (!data || !Array.isArray(data) || data.length === 0) {
        return `
        <div class="valknut-tree-empty">
            <h3>No Refactoring Candidates Found</h3>
            <p>Your code is in excellent shape!</p>
        </div>
        `;
    }

    return `
    <div class="valknut-tree-simple">
        ${renderTreeLevel(data, 0)}
    </div>
    `;
}

function renderTreeLevel(nodes, level) {
    return nodes.map(node => {
        const hasChildren = node.children && Array.isArray(node.children) && node.children.length > 0;
        const nodeId = `node-${Math.random().toString(36).substr(2, 9)}`;
        const indent = level * 24;
        
        let nodeClass = 'tree-node';
        let icon = getNodeIcon(node.type);
        
        // Determine node styling
        if (node.type === 'folder') {
            nodeClass += ' tree-folder';
        } else if (node.type === 'file') {
            nodeClass += ' tree-file';
        } else if (node.type === 'entity') {
            nodeClass += ' tree-entity';
        } else if (node.type === 'issue-row') {
            nodeClass += ' tree-issue';
            icon = '⚠️';
        } else if (node.type === 'suggestion-row') {
            nodeClass += ' tree-suggestion';
            icon = '💡';
        } else if (node.type === 'info-row') {
            nodeClass += ' tree-info';
            icon = 'ℹ️';
        }

        // Generate badges
        let badges = '';
        
        // Health score for folders
        if (node.type === 'folder' && node.healthScore !== undefined) {
            const healthPercent = Math.round(node.healthScore * 100);
            const healthColor = node.healthScore >= 0.8 ? '#28a745' : 
                               node.healthScore >= 0.6 ? '#ffc107' : '#dc3545';
            badges += `<span class="tree-badge" style="background-color: ${healthColor}20; color: ${healthColor}; border: 1px solid ${healthColor}40;">Health: ${healthPercent}%</span>`;
        }
        
        // File count for folders
        if (node.type === 'folder' && node.fileCount) {
            badges += `<span class="tree-badge tree-badge-low">${node.fileCount} files</span>`;
        }
        
        // Entity count for folders
        if (node.type === 'folder' && node.entityCount) {
            badges += `<span class="tree-badge tree-badge-low">${node.entityCount} entities</span>`;
        }
        
        // Severity count badges
        if (node.severityCounts) {
            const counts = node.severityCounts;
            if (counts.critical > 0) {
                badges += `<span class="tree-badge tree-badge-critical">${counts.critical} critical</span>`;
            }
            if (counts.high > 0) {
                badges += `<span class="tree-badge tree-badge-high">${counts.high} high</span>`;
            }
            if (counts.medium > 0) {
                badges += `<span class="tree-badge tree-badge-medium">${counts.medium} medium</span>`;
            }
            if (counts.low > 0) {
                badges += `<span class="tree-badge tree-badge-low">${counts.low} low</span>`;
            }
        }
        
        // Priority badge
        if (node.priority || node.highestPriority) {
            const priority = node.priority || node.highestPriority;
            badges += `<span class="tree-badge tree-badge-${priority.toLowerCase()}">${priority}</span>`;
        }
        
        // Complexity score
        if (node.type === 'entity' && node.score) {
            badges += `<span class="tree-badge tree-badge-low">Complexity: ${node.score}</span>`;
        }
        
        // Average score for files
        if (node.type === 'file' && node.avgScore) {
            badges += `<span class="tree-badge tree-badge-low">Complexity: ${node.avgScore.toFixed(1)}</span>`;
        }
        
        // Line range for entities
        if (node.type === 'entity' && node.lineRange) {
            badges += `<span class="tree-badge tree-badge-low">L${node.lineRange[0]}-${node.lineRange[1]}</span>`;
        }

        const childrenHtml = hasChildren ? renderTreeLevel(node.children, level + 1) : '';
        
        return `
        <div class="${nodeClass}" data-level="${level}">
            <div class="tree-node-header" style="margin-left: ${indent}px;" ${hasChildren ? `onclick="toggleNode('${nodeId}')"` : ''}>
                ${hasChildren ? `<span class="tree-chevron" id="chevron-${nodeId}">▶</span>` : '<span class="tree-spacer"></span>'}
                <span class="tree-icon">${icon}</span>
                <span class="tree-label">${escapeHtml(node.name)}</span>
                <div class="tree-badges">${badges}</div>
            </div>
            ${hasChildren ? `<div class="tree-children" id="children-${nodeId}" style="display: none;">${childrenHtml}</div>` : ''}
        </div>
        `;
    }).join('');
}

function getNodeIcon(type) {
    switch (type) {
        case 'folder': return '📁';
        case 'file': return '📄';
        case 'entity': return '🔧';
        case 'issue-row': return '⚠️';
        case 'suggestion-row': return '💡';
        case 'info-row': return 'ℹ️';
        default: return '📄';
    }
}

function escapeHtml(text) {
    if (!text) return '';
    return String(text)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#039;');
}

// JavaScript for interactive tree functionality
function toggleNode(nodeId) {
    const chevron = document.getElementById(`chevron-${nodeId}`);
    const children = document.getElementById(`children-${nodeId}`);
    
    if (children && chevron) {
        if (children.style.display === 'none') {
            children.style.display = 'block';
            chevron.textContent = '▼';
        } else {
            children.style.display = 'none';
            chevron.textContent = '▶';
        }
    }
}

// CSS styles
const TREE_STYLES = `
.valknut-tree-simple {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    font-size: 14px;
    line-height: 1.5;
    color: var(--text, #333);
    background-color: var(--bg, #fff);
    border: 1px solid var(--border, #e0e0e0);
    border-radius: 8px;
    padding: 1rem;
    max-height: 600px;
    overflow-y: auto;
}

.valknut-tree-empty {
    text-align: center;
    padding: 2rem;
    color: var(--muted, #666);
}

.tree-node {
    margin-bottom: 2px;
}

.tree-node-header {
    display: flex;
    align-items: center;
    padding: 0.4rem 0.5rem;
    border-radius: 4px;
    cursor: pointer;
    transition: background-color 0.2s ease;
    min-height: 32px;
    gap: 0.5rem;
}

.tree-node-header:hover {
    background-color: var(--hover, rgba(0, 123, 255, 0.05));
}

.tree-folder .tree-node-header {
    font-weight: 500;
}

.tree-issue .tree-node-header {
    background-color: rgba(220, 53, 69, 0.05);
    border-left: 3px solid var(--danger, #dc3545);
}

.tree-suggestion .tree-node-header {
    background-color: rgba(0, 123, 255, 0.05);
    border-left: 3px solid var(--info, #007acc);
}

.tree-info .tree-node-header {
    background-color: rgba(40, 167, 69, 0.05);
    border-left: 3px solid var(--success, #28a745);
}

.tree-chevron {
    width: 16px;
    height: 16px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    user-select: none;
    color: var(--text-secondary, #666);
    cursor: pointer;
    transition: transform 0.2s ease;
}

.tree-spacer {
    width: 16px;
    height: 16px;
    display: inline-block;
}

.tree-icon {
    width: 16px;
    height: 16px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    flex-shrink: 0;
}

.tree-label {
    flex: 1;
    margin-right: 0.5rem;
}

.tree-badges {
    display: flex;
    gap: 0.25rem;
    align-items: center;
}

.tree-badge {
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 500;
    white-space: nowrap;
}

.tree-badge-low {
    background-color: #6c757d20;
    color: #6c757d;
    border: 1px solid #6c757d40;
}

.tree-badge-critical {
    background-color: #dc354520;
    color: #dc3545;
    border: 1px solid #dc354540;
}

.tree-badge-high {
    background-color: #fd7e1420;
    color: #fd7e14;
    border: 1px solid #fd7e1440;
}

.tree-badge-medium {
    background-color: #ffc10720;
    color: #ffc107;
    border: 1px solid #ffc10740;
}

.tree-children {
    margin-top: 2px;
}
`;

// Initialize the fallback tree
function initializeTree(containerId, data) {
    const container = document.getElementById(containerId);
    if (!container) {
        console.error('Tree container not found:', containerId);
        return;
    }
    
    // Add styles if not already present
    if (!document.getElementById('tree-fallback-styles')) {
        const styleElement = document.createElement('style');
        styleElement.id = 'tree-fallback-styles';
        styleElement.textContent = TREE_STYLES;
        document.head.appendChild(styleElement);
    }
    
    // Render the tree
    container.innerHTML = createSimpleTreeView(data);
    
    console.log('✅ Simple tree view initialized successfully');
}

// Export for use
if (typeof window !== 'undefined') {
    window.initializeTree = initializeTree;
    window.createSimpleTreeView = createSimpleTreeView;
    window.toggleNode = toggleNode;
}

if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
        initializeTree,
        createSimpleTreeView,
        toggleNode,
        TREE_STYLES
    };
}