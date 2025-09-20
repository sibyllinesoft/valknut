import { describe, test, expect, afterEach } from 'bun:test';
import React from 'react';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import { CodeAnalysisTree } from '../../src/tree-component/CodeAnalysisTree.jsx';
import {
  sampleAnalysisData,
  sampleUnifiedHierarchy,
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

  test('renders directory health when project is clean', async () => {
    render(<CodeAnalysisTree data={sampleCleanAnalysisData} />);

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: /src/i })).toBeInTheDocument();
    });

    expect(screen.getByText(/Health: 95%/)).toBeInTheDocument();
    expect(screen.getByText(/10 files/)).toBeInTheDocument();
  });

  test('renders legacy refactoring data', async () => {
    render(<CodeAnalysisTree data={sampleAnalysisData} />);

    await waitFor(() => {
      expect(screen.getByText(/Health: 45%/)).toBeInTheDocument();
    });
    expect(screen.getByText(/15 files/)).toBeInTheDocument();
    expect(screen.getByText(/critical/i)).toBeInTheDocument();
  });

  test('renders unified hierarchy format directly', async () => {
    render(<CodeAnalysisTree data={{ unifiedHierarchy: sampleUnifiedHierarchy }} />);

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: /src/i })).toBeInTheDocument();
      expect(screen.getByRole('treeitem', { name: /pipeline_executor\.rs/i })).toBeInTheDocument();
    });

    expect(screen.getByText(/evaluate_quality_gates/)).toBeInTheDocument();
  });

  test('ignores malformed inputs gracefully', async () => {
    render(<CodeAnalysisTree data={sampleInvalidData} />);

    await waitFor(() => {
      expect(screen.getByRole('tree')).toBeInTheDocument();
    });
  });

  test('updates when data changes', async () => {
    const { rerender } = render(<CodeAnalysisTree data={sampleCleanAnalysisData} />);

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: /src/i })).toBeInTheDocument();
    });

    rerender(<CodeAnalysisTree data={sampleAnalysisData} />);

    await waitFor(() => {
      expect(screen.getByRole('treeitem', { name: /pipeline_executor\.rs/i })).toBeInTheDocument();
    });
  });
});
