import React, { useMemo, useRef, useState, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

const chunk = (items, size) => {
  const rows = [];
  for (let i = 0; i < items.length; i += size) {
    rows.push(items.slice(i, i + size));
  }
  return rows;
};

export const ClonePairsPanel = ({ pairs = [] }) => {
  const containerRef = useRef(null);
  const [columns, setColumns] = useState(3);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const compute = () => {
      const w = el.clientWidth || window.innerWidth;
      if (w >= 1500) setColumns(3);
      else if (w >= 1100) setColumns(2);
      else setColumns(1);
    };

    compute();
    const ro = new ResizeObserver(compute);
    ro.observe(el);
    window.addEventListener('resize', compute);
    return () => {
      ro.disconnect();
      window.removeEventListener('resize', compute);
    };
  }, []);

  const rows = useMemo(() => chunk(pairs, columns), [pairs, columns]);

  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => 140,
    overscan: 8,
  });

  const renderCard = (pair, key) => {
    const colWidth = columns === 1
      ? '100%'
      : `calc((100% - ${(columns - 1) * 8}px) / ${columns})`;

    const pathParts = (p) => {
      const path = p?.path || '';
      const split = path.split(/[/\\]/);
      return { file: split.pop() || path, dir: split.join('/') || '—' };
    };
    const rangeTxt = (rng) =>
      Array.isArray(rng) && rng.length > 1 ? `${rng[0]}–${rng[1]}` : '';
    const src = pathParts(pair.source || {});
    const tgt = pathParts(pair.target || {});
    const simPct = Math.round((pair.similarity ?? pair.verification?.similarity ?? 0) * 100);
    const vSim =
      pair.verification?.similarity != null
        ? Math.round((pair.verification.similarity || 0) * 100)
        : null;

    return (
      <div
        key={key}
        className="clone-card"
        style={{
          flex: '0 0 calc(50% - 4px)',
          minWidth: 'calc(50% - 4px)',
          boxSizing: 'border-box',
          padding: '8px 8px',
        }}
      >
        <div
          style={{
            background: 'transparent',
            border: '1px solid rgba(255,255,255,0.026)',
            borderRadius: 8,
            padding: 12,
            display: 'flex',
            flexDirection: 'column',
            gap: 4,
          }}
        >
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: '1fr 1fr',
              gridAutoRows: 'auto',
              gap: 10,
              width: '100%',
              marginBottom: 4,
            }}
          >
            <div style={{ display: 'flex', flexDirection: 'column', gap: 1, minWidth: 0 }}>
              <div style={{ fontSize: 13, textTransform: 'uppercase', letterSpacing: 0.6, color: '#60a5fa' }}>
                Source
              </div>
              <div style={{ fontWeight: 700, color: '#e5e7eb', fontSize: 13, lineHeight: 1.2, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{src.file}</div>
              <div style={{ color: '#94a3b8', fontSize: 12, lineHeight: 1.2, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{src.dir}</div>
              <div style={{ color: '#cbd5e1', fontSize: 12, lineHeight: 1.2, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                {(pair.source?.name || '—') + (rangeTxt(pair.source?.range) ? ` · ${rangeTxt(pair.source.range)}` : '')}
              </div>
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 1, minWidth: 0 }}>
              <div style={{ fontSize: 13, textTransform: 'uppercase', letterSpacing: 0.6, color: '#f43f5e' }}>
                Target
              </div>
              <div style={{ fontWeight: 700, color: '#e5e7eb', fontSize: 13, lineHeight: 1.2, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{tgt.file}</div>
              <div style={{ color: '#94a3b8', fontSize: 12, lineHeight: 1.2, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{tgt.dir}</div>
              <div style={{ color: '#cbd5e1', fontSize: 12, lineHeight: 1.2, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                {(pair.target?.name || '—') + (rangeTxt(pair.target?.range) ? ` · ${rangeTxt(pair.target.range)}` : '')}
              </div>
            </div>
          </div>

          <div style={{ display: 'flex', gap: 10, flexWrap: 'nowrap', fontSize: 12, color: '#94a3b8', justifyContent: 'space-between', alignItems: 'center' }}>
            {pair.verification?.edit_cost != null ? <span>Edit cost: {pair.verification.edit_cost}</span> : <span />}
            <span style={{ fontWeight: 600, whiteSpace: 'nowrap' }}>Similarity: {simPct}%</span>
            {Array.isArray(pair.verification?.node_counts) ? (
              <span>Nodes: {pair.verification.node_counts.join(' / ')}</span>
            ) : <span />}
            {pair.verification?.truncated ? <span style={{ color: '#f59e0b' }}>Truncated AST</span> : <span />}
            {vSim != null ? (
              <span style={{ fontWeight: 600, whiteSpace: 'nowrap' }}>APTED: {vSim}%</span>
            ) : <span />}
          </div>
        </div>
      </div>
    );
  };

  const virtualItems = rowVirtualizer.getVirtualItems();

  return (
    <div
      ref={containerRef}
      style={{
        position: 'relative',
        height: 520,
        overflowX: 'hidden',
        overflowY: 'auto',
        background: 'transparent',
        borderRadius: 12,
        padding: '4px 2px',
        scrollbarWidth: 'thin',
        scrollbarColor: 'rgba(148,163,184,0.45) transparent',
      }}
    >
      <div
        style={{
          height: `${rowVirtualizer.getTotalSize()}px`,
          width: '100%',
          position: 'relative',
          overflow: 'hidden',
        }}
      >
        {virtualItems.map((virtualRow) => {
          const row = rows[virtualRow.index];
          return (
            <div
              key={virtualRow.key}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                transform: `translateY(${virtualRow.start}px)`,
                height: `${virtualRow.size}px`,
                display: 'flex',
                gap: 6,
                flexWrap: 'nowrap',
                overflow: 'hidden',
                paddingRight: 4,
              }}
            >
              {row.map((pair, idx) => renderCard(pair, `${virtualRow.index}-${idx}`))}
            </div>
          );
        })}
      </div>
    </div>
  );
};
