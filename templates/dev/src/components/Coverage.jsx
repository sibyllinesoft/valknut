import React, { useState } from 'react';

/**
 * Coverage component
 * Displays AI-powered coverage analysis with expandable tree nodes
 */
export const Coverage = ({ data, projectRoot = '' }) => {
  const [expandedNodes, setExpandedNodes] = useState({});

  if (!data || !data.length) {
    return null;
  }

  const toggleNode = (index) => {
    setExpandedNodes(prev => ({
      ...prev,
      [index]: !prev[index]
    }));
  };

  const formatPercentage = (value, decimals = 1) => {
    if (value === null || value === undefined) return '—';
    return (value * 100).toFixed(decimals);
  };

  const normalizePath = (path) => {
    if (!path) return '';
    const normRoot = projectRoot.replace(/\\/g, '/').replace(/\/+$/, '');
    const normalized = String(path).replace(/\\/g, '/');
    if (normRoot && normalized.startsWith(normRoot)) {
      return normalized.slice(normRoot.length).replace(/^\\/?/, '');
    }
    // Fallback: basename
    const parts = normalized.split('/');
    return parts[parts.length - 1] || normalized;
  };

  const getScoreBadgeClass = (score) => {
    if (score > 0.8) return 'tree-badge-High';
    if (score > 0.6) return 'tree-badge-Medium';
    return 'tree-badge-Low';
  };

  return (
    <div className="coverage-section">
      <div className="coverage-content">
        <div className="analysis-tree" id="coverage-tree">
          {data.map((pack, index) => (
            <div key={index} className="tree-node" data-level="0">
              <div
                className={`tree-header tree-expandable ${expandedNodes[index] ? 'expanded' : ''}`}
                onClick={() => toggleNode(index)}
                role="button"
                tabIndex={0}
                aria-expanded={expandedNodes[index] || false}
                onKeyDown={(e) => e.key === 'Enter' && toggleNode(index)}
              >
                <div className="tree-chevron">
                  <svg className="chevron-icon" viewBox="0 0 24 24" fill="none">
                    <path d="M9 18l6-6-6-6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                  </svg>
                </div>
                <div className="tree-icon">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                    <polyline points="14 2 14 8 20 8"/>
                  </svg>
                </div>
                <div className="tree-label" title={pack.path}>{normalizePath(pack.path)}</div>
                <div className="tree-badge tree-badge-Medium">
                  {pack.gaps?.length || 0} gaps
                </div>
                <div className="tree-badge tree-badge-Low">
                  Current: {formatPercentage(pack.file_info?.coverage_before)}%
                </div>
                <div className="tree-badge tree-badge-High">
                  Target: {formatPercentage(pack.file_info?.coverage_after_if_filled)}%
                </div>
              </div>

              <div className={`tree-children ${expandedNodes[index] ? 'expanded' : 'collapsed'}`} style={{ display: expandedNodes[index] ? 'block' : 'none' }}>
                {/* File Info */}
                <div className="tree-leaf">
                  <div className="tree-leaf-header">
                    <div className="tree-leaf-title">Coverage Metrics</div>
                  </div>
                  <div className="tree-leaf-value">
                    <div className="coverage-metrics-grid">
                      <div className="stat-item">
                        <div className="stat-value">{pack.file_info?.loc || '—'}</div>
                        <div className="stat-label">Lines of Code</div>
                      </div>
                      <div className="stat-item">
                        <div className="stat-value">{formatPercentage(pack.file_info?.coverage_before)}%</div>
                        <div className="stat-label">Current Coverage</div>
                      </div>
                      <div className="stat-item">
                        <div className="stat-value">{formatPercentage(pack.file_info?.coverage_after_if_filled)}%</div>
                        <div className="stat-label">Target Coverage</div>
                      </div>
                      <div className="stat-item">
                        <div className="stat-value">+{formatPercentage(pack.value?.file_cov_gain)}%</div>
                        <div className="stat-label">Coverage Gain</div>
                      </div>
                    </div>
                  </div>
                </div>

                {/* Value/Effort Analysis */}
                <div className="tree-leaf">
                  <div className="tree-leaf-header">
                    <div className="tree-leaf-title">Priority Analysis</div>
                  </div>
                  <div className="tree-leaf-value">
                    <div className="priority-analysis-grid">
                      <div className="value-box">
                        <div className="box-title">Value</div>
                        <div className="box-content">
                          File Coverage: +{formatPercentage(pack.value?.file_cov_gain)}%<br/>
                          Repo Impact: +{formatPercentage(pack.value?.repo_cov_gain_est, 2)}%
                        </div>
                      </div>
                      <div className="effort-box">
                        <div className="box-title">Effort</div>
                        <div className="box-content">
                          Tests to Write: {pack.effort?.tests_to_write_est || '—'}<br/>
                          Mocks Needed: {pack.effort?.mocks_est || '—'}
                        </div>
                      </div>
                    </div>
                  </div>
                </div>

                {/* Coverage Gaps */}
                {pack.gaps?.map((gap, gapIndex) => (
                  <div key={gapIndex} className="tree-leaf">
                    <div className="tree-leaf-header">
                      <div className="tree-leaf-title">
                        Coverage Gap (Lines {gap.span?.start}-{gap.span?.end})
                      </div>
                      <div className={`tree-badge ${getScoreBadgeClass(gap.score)}`}>
                        Score: {gap.score?.toFixed(2) || '—'}
                      </div>
                    </div>
                    <div className="tree-leaf-value">
                      {/* Gap Features */}
                      <div className="gap-features-grid">
                        <div><strong>LOC:</strong> {gap.features?.gap_loc || '—'}</div>
                        <div><strong>Complexity:</strong> {gap.features?.cyclomatic_in_gap?.toFixed(1) || '—'}</div>
                        <div><strong>Callers:</strong> {gap.features?.fan_in_gap || '—'}</div>
                        <div><strong>Public API:</strong> {gap.features?.exports_touched ? 'Yes' : 'No'}</div>
                      </div>

                      {/* Symbols */}
                      {gap.symbols?.length > 0 && (
                        <div className="gap-symbols">
                          <div className="symbols-title">Functions/Methods:</div>
                          {gap.symbols.map((symbol, symIndex) => (
                            <div key={symIndex} className="symbol-item">
                              <div className="symbol-kind">{symbol.kind}: {symbol.name}</div>
                              <div className="symbol-signature">{symbol.signature}</div>
                            </div>
                          ))}
                        </div>
                      )}

                      {/* Code Preview */}
                      {gap.preview?.head?.length > 0 && (
                        <div className="gap-preview">
                          <div className="preview-title">Code Preview:</div>
                          <div className="preview-code">
                            <pre>{gap.preview.head.join('\n')}</pre>
                          </div>
                        </div>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};

export default Coverage;
