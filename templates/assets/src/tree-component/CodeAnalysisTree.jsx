import React from 'react';
import TreeNode from './TreeNode.jsx';

const CodeAnalysisTree = ({ data, onNodeSelect = () => {}, enableAnimation = false }) => {
  if (!data || !data.unifiedHierarchy) {
    return <div className="code-analysis-tree">No data available for tree visualization</div>;
  }

  const renderTree = (node, level = 0) => {
    return (
      <TreeNode
        key={node.id}
        node={node}
        level={level}
        onSelect={onNodeSelect}
        enableAnimation={enableAnimation}
      />
    );
  };

  return (
    <div className="code-analysis-tree">
      {data.unifiedHierarchy.map(node => renderTree(node, 0))}
    </div>
  );
};

export default CodeAnalysisTree;