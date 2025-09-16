// Utility functions for tree data transformation
// These are extracted for easier testing and reusability

/**
 * Transforms tree data to ensure all nodes have required 'id' properties for React Arborist
 * @param {Array} data - Array of tree node objects
 * @param {string} parentId - Parent ID for ID generation
 * @returns {Array} Transformed data with guaranteed ID properties
 */
export const transformTreeData = (data, parentId = '') => {
  if (!Array.isArray(data)) {
    return [];
  }

  return data.map((node, index) => {
    if (!node || typeof node !== 'object') {
      console.warn('⚠️ Invalid node detected, skipping:', node);
      return null;
    }

    // Generate ID if missing, using available properties
    let nodeId = node.id;
    if (!nodeId) {
      if (node.entity_id) {
        nodeId = node.entity_id;
      } else if (node.name) {
        // Create safe ID from name
        const safeName = String(node.name).replace(/[^a-zA-Z0-9_-]/g, '_');
        nodeId = parentId ? `${parentId}_${safeName}` : safeName;
      } else {
        // Fallback to index-based ID
        nodeId = parentId ? `${parentId}_node_${index}` : `node_${index}`;
      }
    }

    // Recursively transform children
    const transformedChildren = node.children 
      ? transformTreeData(node.children, nodeId)
      : [];

    return {
      ...node,
      id: nodeId,
      children: transformedChildren.filter(Boolean) // Remove null nodes
    };
  }).filter(Boolean); // Remove null nodes from top level
};

/**
 * Validates that tree data has required ID properties for React Arborist
 * @param {Array} data - Tree data to validate
 * @returns {Object} Validation result with isValid boolean and errors array
 */
export const validateTreeData = (data) => {
  const errors = [];
  
  const validateNode = (node, path = 'root') => {
    if (!node || typeof node !== 'object') {
      errors.push(`Invalid node at ${path}: ${typeof node}`);
      return;
    }

    if (!node.id) {
      errors.push(`Missing 'id' property at ${path}`);
    }

    if (node.children && Array.isArray(node.children)) {
      node.children.forEach((child, index) => {
        validateNode(child, `${path}.children[${index}]`);
      });
    }
  };

  if (!Array.isArray(data)) {
    errors.push('Tree data must be an array');
  } else {
    data.forEach((node, index) => {
      validateNode(node, `data[${index}]`);
    });
  }

  return {
    isValid: errors.length === 0,
    errors
  };
};

/**
 * Helper function to get severity level from priority/severity values
 * @param {string|number} priority - Priority value
 * @param {string|number} severity - Severity value
 * @returns {string} Normalized severity level
 */
export const getSeverityLevel = (priority, severity) => {
  // Priority can be string like "critical", "high", etc
  if (typeof priority === 'string') {
    const p = priority.toLowerCase();
    if (p.includes('critical')) return 'critical';
    if (p.includes('high')) return 'high';
    if (p.includes('medium') || p.includes('moderate')) return 'medium';
    if (p.includes('low')) return 'low';
  }
  
  // Severity can be numeric (actual scale appears to be 0-20+) or string
  if (typeof severity === 'number') {
    if (severity >= 15) return 'critical';
    if (severity >= 10) return 'high';
    if (severity >= 5) return 'medium';
    return 'low';
  }
  
  // Fallback
  return 'low';
};

/**
 * Counts severity levels in an array of issues/suggestions
 * @param {Array} items - Array of issues or suggestions
 * @returns {Object} Counts by severity level
 */
export const countSeverityLevels = (items) => {
  const counts = { critical: 0, high: 0, medium: 0, low: 0 };
  
  if (!Array.isArray(items)) {
    return counts;
  }
  
  items.forEach(item => {
    const severity = getSeverityLevel(item.priority, item.severity || item.impact);
    counts[severity]++;
  });
  
  return counts;
};

/**
 * Generates unique IDs for tree nodes based on content
 * @param {Object} node - Tree node object
 * @param {string} parentId - Parent node ID
 * @param {number} index - Node index in parent
 * @returns {string} Generated unique ID
 */
export const generateNodeId = (node, parentId = '', index = 0) => {
  if (node.id) {
    return node.id;
  }
  
  if (node.entity_id) {
    return node.entity_id;
  }
  
  if (node.name) {
    const safeName = String(node.name).replace(/[^a-zA-Z0-9_-]/g, '_');
    return parentId ? `${parentId}_${safeName}` : safeName;
  }
  
  return parentId ? `${parentId}_node_${index}` : `node_${index}`;
};

/**
 * Filters tree data by severity levels
 * @param {Array} data - Tree data array
 * @param {Array} severityLevels - Array of severity levels to include
 * @returns {Array} Filtered tree data
 */
export const filterBySeverity = (data, severityLevels = []) => {
  if (!Array.isArray(data) || !Array.isArray(severityLevels) || severityLevels.length === 0) {
    return data;
  }
  
  const filterNode = (node) => {
    if (!node || typeof node !== 'object') {
      return null;
    }
    
    // Check if node has matching severity
    const hasSeverity = severityLevels.some(level => {
      if (node.priority && node.priority.toLowerCase().includes(level.toLowerCase())) {
        return true;
      }
      if (node.severityCounts && node.severityCounts[level.toLowerCase()] > 0) {
        return true;
      }
      return false;
    });
    
    // Recursively filter children
    const filteredChildren = node.children 
      ? node.children.map(filterNode).filter(Boolean)
      : [];
    
    // Include node if it matches severity or has matching children
    if (hasSeverity || filteredChildren.length > 0) {
      return {
        ...node,
        children: filteredChildren
      };
    }
    
    return null;
  };
  
  return data.map(filterNode).filter(Boolean);
};