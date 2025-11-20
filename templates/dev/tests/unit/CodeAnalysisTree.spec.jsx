import { describe, test, expect, afterEach } from 'bun:test';
import React from 'react';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import { CodeAnalysisTree } from '../../src/tree-component/CodeAnalysisTree.jsx';
import {
  sampleAnalysisData,
  sampleCleanAnalysisData,
  sampleInvalidData
} from '../fixtures/sampleAnalysisData.js';

afterEach(() => {
  cleanup();
});

describe('CodeAnalysisTree', () => {
  test('renders empty state when no data provided', async () => {
    render(<CodeAnalysisTree data={null} />);

    expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
    expect(screen.getByText('Your code is in excellent shape!')).toBeInTheDocument();
  });

  test('renders refactoring candidates', async () => {
    render(<CodeAnalysisTree data={sampleAnalysisData} />);

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: /pipeline_executor\.rs/i })).toBeInTheDocument();
    });
    expect(screen.getByText(/evaluate_quality_gates/i)).toBeInTheDocument();
  });

  test('groups flat candidates into folder hierarchy', async () => {
    render(<CodeAnalysisTree data={sampleAnalysisData} />);

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: /src/i })).toBeInTheDocument();
      expect(screen.getByRole('treeitem', { name: /api/i })).toBeInTheDocument();
      expect(screen.getByRole('treeitem', { name: /pipeline_executor\.rs/i })).toBeInTheDocument();
    });
  });

  test('ignores malformed inputs gracefully', async () => {
    render(<CodeAnalysisTree data={sampleInvalidData} />);

    await waitFor(() => {
      expect(screen.getByRole('tree')).toBeInTheDocument();
    });
  });

  test('updates when data changes', async () => {
    const { rerender } = render(<CodeAnalysisTree data={sampleCleanAnalysisData} />);

    rerender(<CodeAnalysisTree data={sampleAnalysisData} />);

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: /pipeline_executor\.rs/i })).toBeInTheDocument();
    });
  });
});
