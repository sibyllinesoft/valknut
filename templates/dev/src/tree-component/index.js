import React from 'react';
import ReactDOM from 'react-dom/client';
import { CodeAnalysisTree } from './CodeAnalysisTree.jsx';
import { transformTreeData, validateTreeData, getSeverityLevel, countSeverityLevels, generateNodeId, filterBySeverity } from './treeUtils.js';

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
    // Expose React and ReactDOM on window for template compatibility
    window.React = React;
    window.ReactDOM = ReactDOM;
    
    // Export the component with both names for compatibility
    window.CodeAnalysisTree = CodeAnalysisTree;
    window.ReactTreeBundle = CodeAnalysisTree;
    
    // Export utility functions
    window.transformTreeData = transformTreeData;
    window.validateTreeData = validateTreeData;
    window.getSeverityLevel = getSeverityLevel;
    window.countSeverityLevels = countSeverityLevels;
    window.generateNodeId = generateNodeId;
    window.filterBySeverity = filterBySeverity;
}