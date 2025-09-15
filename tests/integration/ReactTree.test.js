/**
 * Integration tests for CodeAnalysisTree React component
 * Tests the full component rendering and React error #31 issues
 */

import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';

// Mock the tree data in DOM
const mockValidData = JSON.stringify(window.testData.validTreeData);
const mockEmptyData = JSON.stringify(window.testData.emptyTreeData);

describe('CodeAnalysisTree Component', () => {
  let CodeAnalysisTree;
  
  beforeAll(() => {
    // Load our component
    require('../../src/tree.js');
    CodeAnalysisTree = window.CodeAnalysisTree;
  });
  
  beforeEach(() => {
    // Clear any previous mocks
    document.getElementById = jest.fn();
    jest.clearAllMocks();
  });
  
  describe('with valid data', () => {
    beforeEach(() => {
      document.getElementById.mockReturnValue({
        textContent: mockValidData
      });
    });
    
    test('should render without crashing (React error #31 test)', async () => {
      const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});
      
      expect(() => {
        render(<CodeAnalysisTree />);
      }).not.toThrow();
      
      // Check for React error #31 specifically
      const reactErrors = consoleSpy.mock.calls.filter(call => 
        call.some(arg => typeof arg === 'string' && arg.includes('Error #31'))
      );
      
      expect(reactErrors).toHaveLength(0);
      
      consoleSpy.mockRestore();
    });
    
    test('should find tree-data script element', async () => {
      document.getElementById.mockReturnValue({
        textContent: mockValidData
      });
      
      render(<CodeAnalysisTree />);
      
      await waitFor(() => {
        expect(document.getElementById).toHaveBeenCalledWith('tree-data');
      });
    });
    
    test('should handle JSON parsing correctly', async () => {
      const consoleSpy = jest.spyOn(console, 'log').mockImplementation(() => {});
      
      render(<CodeAnalysisTree />);
      
      await waitFor(() => {
        const parseSuccessLogs = consoleSpy.mock.calls.filter(call =>
          call.some(arg => typeof arg === 'string' && arg.includes('JSON parsed successfully'))
        );
        expect(parseSuccessLogs.length).toBeGreaterThan(0);
      });
      
      consoleSpy.mockRestore();
    });
    
    test('should render tree structure with data', async () => {
      render(<CodeAnalysisTree />);
      
      // Should not show the "no data" message
      await waitFor(() => {
        expect(screen.queryByText('No Refactoring Candidates Found')).not.toBeInTheDocument();
      });
      
      // Should render the mock tree
      await waitFor(() => {
        expect(screen.getByTestId('mock-tree')).toBeInTheDocument();
      });
    });
  });
  
  describe('with empty data', () => {
    beforeEach(() => {
      document.getElementById.mockReturnValue({
        textContent: mockEmptyData
      });
    });
    
    test('should show empty state message', async () => {
      render(<CodeAnalysisTree />);
      
      await waitFor(() => {
        expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
        expect(screen.getByText('Your code is in excellent shape!')).toBeInTheDocument();
      });
    });
  });
  
  describe('with missing script element', () => {
    beforeEach(() => {
      document.getElementById.mockReturnValue(null);
    });
    
    test('should handle missing tree-data element gracefully', async () => {
      const consoleSpy = jest.spyOn(console, 'warn').mockImplementation(() => {});
      
      render(<CodeAnalysisTree />);
      
      await waitFor(() => {
        const warningLogs = consoleSpy.mock.calls.filter(call =>
          call.some(arg => typeof arg === 'string' && arg.includes('No tree-data script element found'))
        );
        expect(warningLogs.length).toBeGreaterThan(0);
      });
      
      await waitFor(() => {
        expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
      });
      
      consoleSpy.mockRestore();
    });
  });
  
  describe('with malformed JSON', () => {
    beforeEach(() => {
      document.getElementById.mockReturnValue({
        textContent: '{ invalid json }'
      });
    });
    
    test('should handle JSON parsing errors gracefully', async () => {
      const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});
      
      render(<CodeAnalysisTree />);
      
      await waitFor(() => {
        const errorLogs = consoleSpy.mock.calls.filter(call =>
          call.some(arg => typeof arg === 'string' && arg.includes('Failed to load tree data'))
        );
        expect(errorLogs.length).toBeGreaterThan(0);
      });
      
      await waitFor(() => {
        expect(screen.getByText('No Refactoring Candidates Found')).toBeInTheDocument();
      });
      
      consoleSpy.mockRestore();
    });
  });
});
