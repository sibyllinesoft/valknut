import React from 'react';

/**
 * Analysis Summary component
 * Displays high-level analysis metrics and health gauges
 */
export const Summary = ({ data }) => {
  if (!data) {
    return null;
  }

  const { summary, healthMetrics } = data;

  // Interpolate color for health gauges using same scale as treemap
  // Treemap uses severity (0=good, 100=bad): gray -> brown -> dark red
  // Health is inverted (100=good, 0=bad), so we map health to severity first
  const getHealthColor = (health) => {
    const severity = 100 - health;
    const t = Math.max(0, Math.min(1, severity / 100));

    // Colorscale: [0, '#6b7280'], [0.5, '#8b4513'], [1, '#991b1b']
    const lerp = (a, b, t) => Math.round(a + (b - a) * t);

    let r, g, b;
    if (t <= 0.5) {
      const localT = t * 2;
      r = lerp(107, 139, localT);
      g = lerp(114, 69, localT);
      b = lerp(128, 19, localT);
    } else {
      const localT = (t - 0.5) * 2;
      r = lerp(139, 153, localT);
      g = lerp(69, 27, localT);
      b = lerp(19, 27, localT);
    }

    return `rgb(${r},${g},${b})`;
  };

  const renderHealthGauge = (label, health) => {
    const color = getHealthColor(health);
    const size = 52;
    const strokeWidth = 5;
    const radius = (size - strokeWidth) / 2;
    const circumference = 2 * Math.PI * radius;
    const strokeDashoffset = circumference - (health / 100) * circumference;

    return (
      <div className="health-gauge" style={{ cursor: 'help' }}>
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '0.25rem' }}>
          <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
            <circle
              cx={size / 2}
              cy={size / 2}
              r={radius}
              fill="none"
              stroke="rgba(255,255,255,0.1)"
              strokeWidth={strokeWidth}
            />
            <circle
              cx={size / 2}
              cy={size / 2}
              r={radius}
              fill="none"
              stroke={color}
              strokeWidth={strokeWidth}
              strokeLinecap="round"
              strokeDasharray={circumference}
              strokeDashoffset={strokeDashoffset}
              transform={`rotate(-90 ${size / 2} ${size / 2})`}
              style={{ transition: 'stroke-dashoffset 0.5s ease' }}
            />
            <text
              x={size / 2}
              y={size / 2}
              textAnchor="middle"
              dominantBaseline="middle"
              fill="var(--text, #e5e7eb)"
              fontSize="11"
              fontWeight="600"
            >
              {Math.round(health)}
            </text>
          </svg>
          <span style={{ fontSize: '0.7rem', color: 'rgba(229,231,235,0.6)', textAlign: 'center' }}>
            {label}
          </span>
        </div>
      </div>
    );
  };

  const overallHealth = healthMetrics
    ? Object.values(healthMetrics).reduce((sum, v) => sum + v, 0) / Object.keys(healthMetrics).length
    : null;

  return (
    <div className="analysis-summary results-section">
      <h2>Analysis Summary</h2>

      {/* Stats Grid */}
      <div className="summary-stats clone-metric-grid" style={{ display: 'flex', justifyContent: 'space-around', flexWrap: 'wrap', gap: '12px' }}>
        <div className="clone-metric-card" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'flex-end' }}>
          <span className="clone-metric-card__label">Files Analyzed</span>
          <span className="clone-metric-card__value">{summary?.filesProcessed ?? '—'}</span>
        </div>
        <div className="clone-metric-card" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'flex-end' }}>
          <span className="clone-metric-card__label">Code Entities</span>
          <span className="clone-metric-card__value">{summary?.entitiesAnalyzed ?? '—'}</span>
        </div>
        <div className="clone-metric-card" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'flex-end' }}>
          <span className="clone-metric-card__label">Need Refactoring</span>
          <span className="clone-metric-card__value">{summary?.refactoringNeeded ?? '—'}</span>
        </div>
        <div className="clone-metric-card" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'flex-end' }}>
          <span className="clone-metric-card__label">Doc Issues</span>
          <span className="clone-metric-card__value">{summary?.docIssueCount ?? 0}</span>
        </div>
      </div>

      {/* Health Gauges */}
      {healthMetrics && (
        <div className="health-gauges" style={{ display: 'flex', justifyContent: 'space-around', flexWrap: 'wrap', gap: '1rem', marginTop: '1.5rem', padding: '1rem', borderRadius: '12px' }}>
          {/* Overall Health */}
          {overallHealth !== null && (
            <div className="health-gauge" style={{ cursor: 'help' }}>
              <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '0.25rem' }}>
                <div style={{ width: '52px', height: '52px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                  <span style={{ fontSize: '1.5rem', fontWeight: 700, color: 'var(--text, #e5e7eb)' }}>
                    {Math.round(overallHealth)}%
                  </span>
                </div>
                <span style={{ fontSize: '0.7rem', color: 'rgba(229,231,235,0.6)', textAlign: 'center' }}>Health</span>
              </div>
            </div>
          )}

          {/* Category Gauges */}
          {healthMetrics.maintainability !== undefined && renderHealthGauge('Maintainability', healthMetrics.maintainability)}
          {healthMetrics.complexity !== undefined && renderHealthGauge('Complexity', healthMetrics.complexity)}
          {healthMetrics.cognitive !== undefined && renderHealthGauge('Cognitive', healthMetrics.cognitive)}
          {healthMetrics.structure !== undefined && renderHealthGauge('Structure', healthMetrics.structure)}
          {healthMetrics.debt !== undefined && renderHealthGauge('Debt', healthMetrics.debt)}
          {healthMetrics.docs !== undefined && renderHealthGauge('Docs', healthMetrics.docs)}
        </div>
      )}
    </div>
  );
};

export default Summary;
