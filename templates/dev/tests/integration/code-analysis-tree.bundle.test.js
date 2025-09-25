import { describe, test, expect } from 'bun:test';
import React from 'react';
import { renderToString } from 'react-dom/server';

import CodeAnalysisTree, { CodeAnalysisTree as NamedExport } from '../../src/tree-component/index.js';

describe('CodeAnalysisTree bundle export', () => {
  test('exports component via ESM and attaches to window globals', () => {
    expect(typeof CodeAnalysisTree).toBe('function');
    expect(CodeAnalysisTree).toBe(NamedExport);

    const html = renderToString(React.createElement(CodeAnalysisTree, { data: {} }));
    expect(html).toContain('No Refactoring Candidates Found');

    if (typeof window !== 'undefined') {
      expect(window.CodeAnalysisTree).toBe(CodeAnalysisTree);
      expect(window.ReactTreeBundle).toBe(CodeAnalysisTree);
    }
  });
});
