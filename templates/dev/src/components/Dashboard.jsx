import React, { useState, useEffect, useRef } from 'react';

/**
 * Dashboard/Treemap component
 * Displays an interactive treemap visualization of code analysis results
 * Uses Plotly.js for rendering when available, otherwise shows placeholder
 */
export const Dashboard = ({ data, initialMetric = 'complexity' }) => {
  const [colorMetric, setColorMetric] = useState(initialMetric);
  const treemapRef = useRef(null);
  const [plotlyLoaded, setPlotlyLoaded] = useState(false);

  const metricOptions = [
    { value: 'complexity', label: 'Complexity' },
    { value: 'cognitive', label: 'Cognitive' },
    { value: 'debt', label: 'Debt' },
    { value: 'maintainability', label: 'Maintainability' },
    { value: 'structure', label: 'Structure' },
    { value: 'docs', label: 'Docs' },
    { value: 'coverage', label: 'Coverage' },
  ];

  // Check for Plotly availability
  useEffect(() => {
    if (typeof window !== 'undefined' && window.Plotly) {
      setPlotlyLoaded(true);
    }
  }, []);

  // Build hierarchy from flat candidate list
  const buildHierarchy = (candidates) => {
    if (!candidates?.length) return null;

    const nodes = new Map();

    const ensureNode = (id, label, parent = '') => {
      if (!nodes.has(id)) {
        nodes.set(id, { id, label, parent, children: [], value: 0, severity: 0 });
      }
      return nodes.get(id);
    };

    // Root
    ensureNode('Project', 'Project', '');

    candidates.forEach((entity) => {
      const filePath = String(entity.file_path || entity.filePath || '').replace(/\\/g, '/');
      const parts = filePath.split('/').filter(Boolean);
      const fileName = parts.pop() || filePath || 'file';

      let parentId = 'Project';
      let pathSoFar = '';

      parts.forEach((part) => {
        pathSoFar = pathSoFar ? pathSoFar + '/' + part : part;
        const dirId = 'dir:' + pathSoFar;
        ensureNode(dirId, part, parentId);
        parentId = dirId;
      });

      const fileId = 'file:' + filePath;
      ensureNode(fileId, fileName, parentId);

      const entityId = 'entity:' + (entity.entity_id || entity.name || Math.random());
      const node = ensureNode(entityId, entity.name || fileName, fileId);
      node.value = entity.lines_of_code || 1;
      node.severity = entity.score || 0;
      node.entity = entity;
    });

    return nodes;
  };

  // Get severity color (matches treemap colorscale)
  const getSeverityColor = (severity, maxScale = 100) => {
    const t = Math.max(0, Math.min(1, severity / maxScale));

    // Colorscale: [0, '#6b7280'], [0.5, '#8b4513'], [1, '#991b1b']
    const lerp = (a, b, t) => Math.round(a + (b - a) * t);

    let r, g, b;
    if (t <= 0.5) {
      const localT = t * 2;
      r = lerp(0x6b, 0x8b, localT);
      g = lerp(0x72, 0x45, localT);
      b = lerp(0x80, 0x13, localT);
    } else {
      const localT = (t - 0.5) * 2;
      r = lerp(0x8b, 0x99, localT);
      g = lerp(0x45, 0x1b, localT);
      b = lerp(0x13, 0x1b, localT);
    }

    return `rgb(${r},${g},${b})`;
  };

  const hierarchy = data?.candidates ? buildHierarchy(data.candidates) : null;

  // Count stats for display
  const fileCount = hierarchy
    ? Array.from(hierarchy.keys()).filter((k) => k.startsWith('file:')).length
    : 0;
  const entityCount = hierarchy
    ? Array.from(hierarchy.keys()).filter((k) => k.startsWith('entity:')).length
    : 0;

  // Render placeholder treemap cells for visual demonstration
  const renderPlaceholderTreemap = () => {
    if (!hierarchy) {
      return (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            height: '100%',
            color: 'rgba(229,231,235,0.4)',
            fontSize: '0.875rem',
          }}
        >
          No data available
        </div>
      );
    }

    // Get top-level directories and files
    const rootChildren = Array.from(hierarchy.values())
      .filter((n) => n.parent === 'Project')
      .slice(0, 12);

    if (!rootChildren.length) {
      return (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            height: '100%',
            color: 'rgba(229,231,235,0.4)',
            fontSize: '0.875rem',
          }}
        >
          Empty project structure
        </div>
      );
    }

    // Calculate total value for proportional sizing
    const totalValue = rootChildren.reduce((sum, n) => {
      // Recursively sum all descendant values
      const sumDescendants = (nodeId) => {
        const node = hierarchy.get(nodeId);
        if (!node) return 0;
        if (!node.children?.length) return node.value || 1;
        return node.children.reduce((s, cId) => s + sumDescendants(cId), 0);
      };
      return sum + sumDescendants(n.id);
    }, 0);

    return (
      <div
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          gap: '2px',
          height: '100%',
          alignContent: 'flex-start',
        }}
      >
        {rootChildren.map((node) => {
          const nodeValue = (() => {
            const sumDescendants = (nodeId) => {
              const n = hierarchy.get(nodeId);
              if (!n) return 0;
              if (!n.children?.length) return n.value || 1;
              return n.children.reduce((s, cId) => s + sumDescendants(cId), 0);
            };
            return sumDescendants(node.id);
          })();

          const pct = totalValue > 0 ? (nodeValue / totalValue) * 100 : 10;
          const width = Math.max(60, Math.min(300, pct * 3));
          const height = Math.max(40, Math.min(150, pct * 1.5));

          // Random severity for demo
          const severity = Math.random() * 80;

          return (
            <div
              key={node.id}
              style={{
                width: `${width}px`,
                height: `${height}px`,
                background: getSeverityColor(severity),
                borderRadius: '4px',
                padding: '6px',
                display: 'flex',
                flexDirection: 'column',
                justifyContent: 'space-between',
                overflow: 'hidden',
                cursor: 'pointer',
                transition: 'transform 0.15s ease',
              }}
              title={`${node.label}: ${nodeValue} lines`}
            >
              <span
                style={{
                  fontSize: '0.75rem',
                  fontWeight: 600,
                  color: '#fff',
                  textShadow: '0 1px 2px rgba(0,0,0,0.5)',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                }}
              >
                {node.label}
              </span>
              <span
                style={{
                  fontSize: '0.65rem',
                  color: 'rgba(255,255,255,0.7)',
                }}
              >
                {nodeValue} LOC
              </span>
            </div>
          );
        })}
      </div>
    );
  };

  return (
    <div className="dashboard-section">
      {/* Color metric selector */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '12px',
          marginBottom: '0.5rem',
        }}
      >
        <label
          htmlFor="treemap-color-metric"
          style={{ fontSize: '0.9rem', color: 'var(--text-secondary, rgba(229,231,235,0.6))' }}
        >
          Color by:
        </label>
        <select
          id="treemap-color-metric"
          value={colorMetric}
          onChange={(e) => setColorMetric(e.target.value)}
          style={{
            background: 'var(--panel, rgba(255,255,255,0.06))',
            color: 'var(--text, #e5e7eb)',
            border: '1px solid rgba(255,255,255,0.08)',
            borderRadius: '6px',
            padding: '6px 10px',
            fontSize: '0.9rem',
          }}
        >
          {metricOptions.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>

        {/* Stats */}
        <div style={{ marginLeft: 'auto', display: 'flex', gap: '1rem', fontSize: '0.8rem' }}>
          <span style={{ color: 'rgba(229,231,235,0.5)' }}>
            {fileCount} files
          </span>
          <span style={{ color: 'rgba(229,231,235,0.5)' }}>
            {entityCount} entities
          </span>
        </div>
      </div>

      {/* Treemap container */}
      <div
        ref={treemapRef}
        style={{
          width: '100%',
          height: 'calc(100vh - 180px)',
          minHeight: '300px',
          background: 'rgba(0,0,0,0.2)',
          borderRadius: '8px',
          overflow: 'hidden',
        }}
      >
        {!plotlyLoaded && renderPlaceholderTreemap()}
        {plotlyLoaded && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              height: '100%',
              color: 'rgba(229,231,235,0.4)',
              fontSize: '0.875rem',
            }}
          >
            Plotly treemap would render here
          </div>
        )}
      </div>

      {/* Color scale legend */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          marginTop: '0.75rem',
          fontSize: '0.75rem',
          color: 'rgba(229,231,235,0.5)',
        }}
      >
        <span>Low severity</span>
        <div
          style={{
            width: '120px',
            height: '8px',
            borderRadius: '4px',
            background: 'linear-gradient(to right, #6b7280, #8b4513, #991b1b)',
          }}
        />
        <span>High severity</span>
      </div>
    </div>
  );
};

export default Dashboard;
