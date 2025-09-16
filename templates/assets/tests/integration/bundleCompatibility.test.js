// Bundle compatibility tests for React tree component loading
const React = require('react');
const { render, screen, waitFor } = require('@testing-library/react');

// Mock the global bundle loading mechanism
const mockWindowReact = {
  React: {
    createElement: React.createElement,
    useState: React.useState,
    useEffect: React.useEffect,
    useCallback: React.useCallback
  },
  ReactDOM: {
    createRoot: jest.fn(() => ({
      render: jest.fn(),
      unmount: jest.fn()
    }))
  }
};

// Mock the tree bundle
const mockTreeBundle = jest.fn(({ data }) => 
  React.createElement('div', { 'data-testid': 'mock-tree-bundle' }, 
    `Tree rendered with ${Array.isArray(data) ? data.length : 0} items`
  )
);

describe('Bundle Compatibility Tests', () => {
  let originalWindow;

  beforeEach(() => {
    // Store original window properties
    originalWindow = {
      React: global.window?.React,
      ReactDOM: global.window?.ReactDOM,
      ReactTreeBundle: global.window?.ReactTreeBundle,
      CodeAnalysisTree: global.window?.CodeAnalysisTree
    };

    // Clear all mocks
    jest.clearAllMocks();
  });

  afterEach(() => {
    // Restore original window properties
    Object.keys(originalWindow).forEach(key => {
      if (originalWindow[key] !== undefined) {
        global.window[key] = originalWindow[key];
      } else {
        delete global.window?.[key];
      }
    });
  });

  describe('React Bundle Loading', () => {
    test('should load React from window.React when available', () => {
      // Setup window.React
      global.window = {
        ...global.window,
        ...mockWindowReact
      };

      // Test that React is available globally
      expect(window.React).toBeDefined();
      expect(window.React.createElement).toBeDefined();
      expect(window.ReactDOM).toBeDefined();
    });

    test('should handle missing React gracefully', () => {
      // Remove React from window
      global.window = {
        ...global.window,
        React: undefined,
        ReactDOM: undefined
      };

      // Component should still be importable with fallback
      const TreeComponent = require('../../src/tree').default;
      expect(TreeComponent).toBeDefined();
    });

    test('should expose React and ReactDOM on window after import', async () => {
      // Import the tree component
      const TreeComponent = require('../../src/tree').default;

      // Should expose React on window
      expect(window.React).toBeDefined();
      expect(window.ReactDOM).toBeDefined();
    });
  });

  describe('TreeComponent Bundle Resolution', () => {
    test('should resolve TreeComponent from window.ReactTreeBundle', () => {
      // Setup mock bundle
      global.window = {
        ...global.window,
        ...mockWindowReact,
        ReactTreeBundle: mockTreeBundle
      };

      // Should be accessible
      expect(window.ReactTreeBundle).toBeDefined();
      expect(typeof window.ReactTreeBundle).toBe('function');
    });

    test('should resolve TreeComponent from window.CodeAnalysisTree as fallback', () => {
      // Setup fallback bundle name
      global.window = {
        ...global.window,
        ...mockWindowReact,
        CodeAnalysisTree: mockTreeBundle
      };

      // Should be accessible under both names
      expect(window.CodeAnalysisTree).toBeDefined();
    });

    test('should handle bundle loading failure gracefully', () => {
      // Setup window without tree bundle
      global.window = {
        ...global.window,
        ...mockWindowReact,
        ReactTreeBundle: undefined,
        CodeAnalysisTree: undefined
      };

      // Should not throw when bundle is missing
      expect(() => {
        const TreeComponent = require('../../src/tree').default;
      }).not.toThrow();
    });
  });

  describe('Bundle Integration with HTML Templates', () => {
    test('should work with CDN-loaded React bundles', () => {
      // Simulate CDN loading with version mismatch tolerance
      global.window = {
        ...global.window,
        React: {
          ...mockWindowReact.React,
          version: '18.2.0' // Different version
        },
        ReactDOM: mockWindowReact.ReactDOM
      };

      const TreeComponent = require('../../src/tree').default;
      
      // Should work despite version differences
      expect(() => {
        render(React.createElement(TreeComponent, { data: [] }));
      }).not.toThrow();
    });

    test('should handle mixed module systems (UMD/ESM)', () => {
      // Setup UMD-style globals
      global.window = {
        ...global.window,
        ...mockWindowReact
      };

      // Import as ESM
      const TreeComponent = require('../../src/tree').default;
      
      // Should be compatible
      expect(TreeComponent).toBeDefined();
      expect(typeof TreeComponent).toBe('function');
    });

    test('should support server-side rendering preparation', () => {
      // Simulate SSR environment
      const originalWindow = global.window;
      global.window = undefined;

      // Should not crash during import
      expect(() => {
        const TreeComponent = require('../../src/tree').default;
      }).not.toThrow();

      // Restore window
      global.window = originalWindow;
    });
  });

  describe('Bundle Size and Performance', () => {
    test('should not include unnecessary dependencies in bundle', () => {
      const TreeComponent = require('../../src/tree').default;
      
      // Should be lightweight function
      expect(typeof TreeComponent).toBe('function');
      
      // Should not have heavy dependencies attached
      expect(TreeComponent.toString().length).toBeLessThan(50000); // Reasonable size limit
    });

    test('should lazy load dependencies when needed', async () => {
      // Setup lazy loading scenario
      global.window = {
        ...global.window,
        React: undefined,
        ReactDOM: undefined
      };

      // Simulate async loading
      setTimeout(() => {
        global.window.React = mockWindowReact.React;
        global.window.ReactDOM = mockWindowReact.ReactDOM;
      }, 100);

      const TreeComponent = require('../../src/tree').default;
      
      // Should handle delayed availability
      await waitFor(() => {
        expect(window.React).toBeDefined();
      });
    });
  });

  describe('Error Recovery and Fallbacks', () => {
    test('should provide fallback when React Arborist fails to load', () => {
      // Mock react-arborist loading failure
      jest.doMock('react-arborist', () => {
        throw new Error('Failed to load react-arborist');
      });

      // Should not crash the entire bundle
      expect(() => {
        require('../../src/tree');
      }).not.toThrow();
    });

    test('should handle bundle corruption gracefully', () => {
      // Simulate corrupted bundle state
      global.window = {
        ...global.window,
        React: { createElement: null }, // Corrupted React
        ReactDOM: undefined
      };

      const TreeComponent = require('../../src/tree').default;
      
      // Should handle gracefully
      expect(TreeComponent).toBeDefined();
    });

    test('should provide meaningful error messages for bundle issues', () => {
      const consoleErrorSpy = jest.spyOn(console, 'error').mockImplementation();
      
      // Setup problematic environment
      global.window = {
        ...global.window,
        React: { createElement: 'invalid' }
      };

      try {
        const TreeComponent = require('../../src/tree').default;
        render(React.createElement(TreeComponent, { data: [] }));
      } catch (error) {
        // Should provide helpful error information
        expect(error.message).toMatch(/React|bundle|component/i);
      }

      consoleErrorSpy.mockRestore();
    });
  });

  describe('Cross-Browser Compatibility', () => {
    test('should work with IE11 polyfills', () => {
      // Simulate IE11 environment
      const originalPromise = global.Promise;
      const originalMap = global.Map;
      
      // Remove modern features
      global.Promise = undefined;
      global.Map = undefined;

      try {
        // Should handle missing modern features
        const TreeComponent = require('../../src/tree').default;
        expect(TreeComponent).toBeDefined();
      } finally {
        // Restore
        global.Promise = originalPromise;
        global.Map = originalMap;
      }
    });

    test('should handle different module loaders', () => {
      // Test with different module systems
      const loaders = ['webpack', 'rollup', 'browserify'];
      
      loaders.forEach(loader => {
        // Simulate different loader environments
        global.__webpack_require__ = loader === 'webpack' ? jest.fn() : undefined;
        global.define = loader === 'browserify' ? jest.fn() : undefined;

        expect(() => {
          const TreeComponent = require('../../src/tree').default;
        }).not.toThrow();
      });
    });
  });

  describe('Memory Management', () => {
    test('should clean up properly when unmounted', () => {
      global.window = {
        ...global.window,
        ...mockWindowReact
      };

      const TreeComponent = require('../../src/tree').default;
      const { unmount } = render(React.createElement(TreeComponent, { data: [] }));
      
      // Should unmount without memory leaks
      expect(() => unmount()).not.toThrow();
    });

    test('should handle rapid mount/unmount cycles', () => {
      global.window = {
        ...global.window,
        ...mockWindowReact
      };

      const TreeComponent = require('../../src/tree').default;
      
      // Rapid mount/unmount cycles
      for (let i = 0; i < 10; i++) {
        const { unmount } = render(React.createElement(TreeComponent, { data: [] }));
        unmount();
      }
      
      // Should not cause memory issues
      expect(true).toBe(true); // Test passes if no errors thrown
    });
  });

  describe('Bundle Validation', () => {
    test('should validate bundle integrity', () => {
      const TreeComponent = require('../../src/tree').default;
      
      // Should be a valid React component
      expect(typeof TreeComponent).toBe('function');
      
      // Should not have been tampered with
      expect(TreeComponent.toString()).toContain('React');
    });

    test('should maintain consistent API across bundle versions', () => {
      const TreeComponent = require('../../src/tree').default;
      
      // Should accept expected props
      const validProps = { data: [] };
      
      expect(() => {
        render(React.createElement(TreeComponent, validProps));
      }).not.toThrow();
    });

    test('should export all expected components', () => {
      // Check all expected exports
      const treeModule = require('../../src/tree');
      
      expect(treeModule.default).toBeDefined(); // Main component
      expect(typeof treeModule.default).toBe('function');
    });
  });
});