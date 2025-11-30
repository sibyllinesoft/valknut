import React from 'react';

/**
 * AI Refactoring Oracle component
 * Displays AI-generated architectural improvement suggestions with flat task cards
 */
export const Oracle = ({ data }) => {
  if (!data) {
    return null;
  }

  const { assessment, refactoring_roadmap } = data;

  const capitalize = (str) => {
    if (!str) return '';
    return str.charAt(0).toUpperCase() + str.slice(1);
  };

  const tasks = refactoring_roadmap?.tasks || [];

  return (
    <div className="oracle-section">
      <div className="oracle-content">
        {/* Assessment - inline style with architectural narrative */}
        {assessment && (
          <div className="oracle-assessment">
            <div className="assessment-header">
              <span className="assessment-label">Assessment:</span>
              {assessment.architectural_style && (
                <span style={{ color: '#e5e7eb', fontWeight: 600, fontSize: '0.875rem' }}>{assessment.architectural_style}</span>
              )}
            </div>

            {assessment.architectural_narrative && (
              <p className="assessment-narrative">{assessment.architectural_narrative}</p>
            )}
          </div>
        )}

        {/* Refactoring Tasks - flat cards */}
        {tasks.length > 0 && (
          <div className="oracle-roadmap">
            <div className="roadmap-header">
              <span className="roadmap-label">Roadmap</span>
              <div className="roadmap-stats">
                <span className="stat required-count">
                  {tasks.filter(t => t.required).length} required
                </span>
                <span className="stat optional-count">
                  {tasks.filter(t => !t.required).length} optional
                </span>
              </div>
            </div>

            <div className="roadmap-tasks">
              {tasks.map((task) => (
                <div
                  key={task.id}
                  className="task-card"
                  style={{
                    background: 'transparent',
                    border: '1px solid rgba(255,255,255,0.026)',
                    borderRadius: 8,
                    padding: 12,
                    display: 'flex',
                    flexDirection: 'column',
                  }}
                >
                  {/* Header row: title, then optional + category */}
                  <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
                    <span style={{ fontWeight: 600, color: '#e5e7eb', flex: 1, minWidth: 150 }}>
                      {task.title}
                    </span>
                    <span style={{ fontSize: 11, color: '#64748b' }}>
                      {!task.required && 'optional '}{task.category}
                    </span>
                  </div>

                  {/* Files - after title with bottom margin */}
                  {task.files?.length > 0 && (
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 8, marginBottom: 24 }}>
                      {task.files.map((file, i) => (
                        <code
                          key={i}
                          style={{
                            fontSize: 11,
                            padding: '2px 6px',
                            background: 'rgba(148,163,184,0.08)',
                            borderRadius: 4,
                            color: '#94a3b8',
                          }}
                        >
                          {file}
                        </code>
                      ))}
                    </div>
                  )}

                  {/* Description - regular text color */}
                  <p style={{ margin: 0, marginBottom: task.mitigation ? 4 : 24, color: '#e5e7eb', fontSize: 13, lineHeight: 1.5 }}>
                    {task.description}
                  </p>

                  {/* Mitigation note if present - margin after combined text block */}
                  {task.mitigation && (
                    <p style={{ margin: 0, marginBottom: 24, color: '#94a3b8', fontSize: 12, fontStyle: 'italic' }}>
                      {task.mitigation}
                    </p>
                  )}

                  {/* Bottom row: Impact, Effort, Risk - label/value pairs like analysis summary */}
                  <div style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                  }}>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                      <span style={{ fontSize: '0.75rem', textTransform: 'uppercase', letterSpacing: '0.08em', color: 'rgba(148, 163, 184, 0.78)' }}>Impact</span>
                      <span style={{ fontSize: '1.1rem', fontWeight: 600, color: '#e2e8f0' }}>{task.impact || '—'}</span>
                    </div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                      <span style={{ fontSize: '0.75rem', textTransform: 'uppercase', letterSpacing: '0.08em', color: 'rgba(148, 163, 184, 0.78)' }}>Effort</span>
                      <span style={{ fontSize: '1.1rem', fontWeight: 600, color: '#e2e8f0' }}>{task.effort || '—'}</span>
                    </div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                      <span style={{ fontSize: '0.75rem', textTransform: 'uppercase', letterSpacing: '0.08em', color: 'rgba(148, 163, 184, 0.78)' }}>Risk</span>
                      <span style={{ fontSize: '1.1rem', fontWeight: 600, color: '#e2e8f0' }}>{task.risk_level ? capitalize(task.risk_level) : '—'}</span>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default Oracle;
