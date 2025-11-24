import React from 'react';
import ReactDOM from 'react-dom/client';
import { CodeAnalysisTree } from './CodeAnalysisTree.jsx';
import { ClonePairsPanel } from './ClonePairsPanel.jsx';
import {
    transformTreeData,
    validateTreeData,
    getSeverityLevel,
    countSeverityLevels,
    generateNodeId,
    filterBySeverity,
} from './treeUtils.js';

// Main component export (default for bundle)
export default CodeAnalysisTree;

// Named exports for utility functions
export {
    CodeAnalysisTree,
    transformTreeData,
    validateTreeData,
    getSeverityLevel,
    countSeverityLevels,
    generateNodeId,
    filterBySeverity
};

// Global exports for browser compatibility (matching existing webpack setup)
if (typeof window !== 'undefined') {
    // Normalize global bundle container (guard against UMD assigning a function)
    if (typeof window.ReactTreeBundle === 'function') {
        window.ReactTreeBundle = { CodeAnalysisTree: window.ReactTreeBundle };
    } else {
        window.ReactTreeBundle = window.ReactTreeBundle || {};
    }

    // Expose React and ReactDOM on window for template compatibility
    window.React = React;
    window.ReactDOM = ReactDOM;
    
    // Export the component with both names for compatibility
    window.CodeAnalysisTree = CodeAnalysisTree;
    window.ReactTreeBundle.CodeAnalysisTree = CodeAnalysisTree;
    
    // Export utility functions
    window.transformTreeData = transformTreeData;
    window.validateTreeData = validateTreeData;
    window.getSeverityLevel = getSeverityLevel;
    window.countSeverityLevels = countSeverityLevels;
    window.generateNodeId = generateNodeId;
    window.filterBySeverity = filterBySeverity;

    // Mount helpers
    window.ReactTreeBundle.mountTree = (data, containerId = 'react-tree-root') => {
        const container = document.getElementById(containerId);
        if (!container) return;
        const root = ReactDOM.createRoot(container);
        root.render(React.createElement(CodeAnalysisTree, { data }));
    };

    window.ReactTreeBundle.mountClonePairs = (pairs, containerId = 'clone-pairs-root') => {
        const container = document.getElementById(containerId);
        if (!container) return;
        const root = ReactDOM.createRoot(container);
        root.render(React.createElement(ClonePairsPanel, { pairs }));
    };

    // Stable global hook that won't be overwritten by UMD reassignments
    if (typeof window.ValknutMountClonePairs !== 'function') {
        window.ValknutMountClonePairs = (pairs, containerId = 'clone-pairs-root') => {
            const container = document.getElementById(containerId);
            if (!container) return;
            const root = ReactDOM.createRoot(container);
            root.render(React.createElement(ClonePairsPanel, { pairs }));
        };
    }

    window.addEventListener('DOMContentLoaded', () => {
        const pairScript = document.getElementById('clone-pairs-data');
        const pairRoot = document.getElementById('clone-pairs-root');
        if (pairScript && pairRoot) {
            try {
                const pairs = JSON.parse(pairScript.textContent || '[]') || [];
                if (typeof window.ValknutMountClonePairs === 'function') {
                    window.ValknutMountClonePairs(pairs);
                } else {
                    window.ReactTreeBundle.mountClonePairs(pairs);
                }
            } catch (err) {
                console.error('[Valknut] Failed to parse clone pairs payload', err);
            }
        }
    });
}
