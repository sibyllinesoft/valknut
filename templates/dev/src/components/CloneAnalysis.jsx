import React from 'react';

/**
 * Clone Analysis component
 * Displays code duplication metrics and clone pairs
 */
export const CloneAnalysis = ({ data }) => {
  if (!data || !data.hasData) {
    return null;
  }

  const { avgSimilarity, maxSimilarity, candidatesAfter, clonePairs } = data;

  return (
    <div className="clone-section">
      {/* Metrics Grid */}
      <div className="clone-metric-grid">
        <div className="clone-metric-card" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'flex-end' }}>
          <span className="clone-metric-card__label">Average Similarity</span>
          <span className="clone-metric-card__value">
            {avgSimilarity != null ? avgSimilarity.toFixed(2) : '—'}
          </span>
        </div>
        <div className="clone-metric-card" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'flex-end' }}>
          <span className="clone-metric-card__label">Maximum Similarity</span>
          <span className="clone-metric-card__value">
            {maxSimilarity != null ? maxSimilarity.toFixed(2) : '—'}
          </span>
        </div>
        <div className="clone-metric-card" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'flex-end' }}>
          <span className="clone-metric-card__label">Candidates</span>
          <span className="clone-metric-card__value">
            {candidatesAfter ?? '—'}
          </span>
        </div>
      </div>

      {/* Clone Pairs */}
      {clonePairs && clonePairs.length > 0 && (
        <div className="clone-pairs" style={{ marginTop: '1rem', borderTop: '1px solid rgba(255,255,255,0.026)', paddingTop: '0.35rem', maxHeight: '600px', overflow: 'auto' }}>
          {clonePairs.map((pair, idx) => (
            <div
              key={idx}
              className="clone-pair-card"
              style={{
                background: 'transparent',
                border: '1px solid rgba(255,255,255,0.026)',
                borderRadius: 8,
                padding: 12,
                marginBottom: 8,
                display: 'flex',
                flexDirection: 'column',
                gap: 8,
              }}
            >
              {/* Similarity Badge */}
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontSize: 13, color: '#e5e7eb', fontWeight: 600 }}>
                  Clone Pair #{idx + 1}
                </span>
                <span style={{
                  fontSize: 11,
                  padding: '2px 8px',
                  borderRadius: 4,
                  background: pair.similarity >= 0.9 ? 'rgba(239,68,68,0.1)' : 'rgba(148,163,184,0.08)',
                  color: pair.similarity >= 0.9 ? '#ef4444' : '#94a3b8',
                }}>
                  {(pair.similarity * 100).toFixed(0)}% similar
                </span>
              </div>

              {/* File Paths */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                <code style={{ fontSize: 11, padding: '2px 6px', background: 'rgba(148,163,184,0.08)', borderRadius: 4, color: '#94a3b8' }}>
                  {pair.file1}
                </code>
                <code style={{ fontSize: 11, padding: '2px 6px', background: 'rgba(148,163,184,0.08)', borderRadius: 4, color: '#94a3b8' }}>
                  {pair.file2}
                </code>
              </div>

              {/* Line info if available */}
              {(pair.lines1 || pair.lines2) && (
                <div style={{ fontSize: 11, color: '#64748b' }}>
                  {pair.lines1 && <span>Lines {pair.lines1.start}-{pair.lines1.end}</span>}
                  {pair.lines1 && pair.lines2 && ' / '}
                  {pair.lines2 && <span>Lines {pair.lines2.start}-{pair.lines2.end}</span>}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default CloneAnalysis;
