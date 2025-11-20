/**
 * Bun test setup file for valknut tree components
 * Configures React testing environment and mocks
 */

import { Window } from 'happy-dom';

// Provide a full DOM implementation for React testing utilities
const happyDom = new Window({ url: 'http://localhost/' });

globalThis.window = happyDom.window;
globalThis.document = happyDom.document;
globalThis.navigator = happyDom.navigator;
globalThis.location = happyDom.location;
globalThis.HTMLElement = happyDom.HTMLElement;
globalThis.Element = happyDom.Element;
globalThis.Node = happyDom.Node;
globalThis.CustomEvent = happyDom.CustomEvent;
globalThis.Event = happyDom.Event;
globalThis.MutationObserver = happyDom.MutationObserver;
globalThis.DOMRect = happyDom.DOMRect;
globalThis.ResizeObserver = happyDom.ResizeObserver || class {
  observe() {}
  unobserve() {}
  disconnect() {}
};
globalThis.IntersectionObserver = happyDom.IntersectionObserver || class {
  observe() {}
  unobserve() {}
  disconnect() {}
};
globalThis.getComputedStyle = happyDom.window.getComputedStyle.bind(happyDom.window);

// Set up testing environment
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

// Mock console methods for cleaner test output
const originalError = console.error;
const originalWarn = console.warn;

console.error = (...args) => {
  // Filter out known React/testing warnings that are expected
  const message = args[0];
  if (typeof message === 'string') {
    // Skip React 18 act() warnings in tests
    if (message.includes('ReactDOM.render is no longer supported')) return;
    if (message.includes('Warning: ReactDOM.render has been replaced')) return;
  }
  originalError.apply(console, args);
};

console.warn = (...args) => {
  const message = args[0];
  if (typeof message === 'string') {
    // Skip development mode warnings
    if (message.includes('ReactDOM.render is no longer supported')) return;
  }
  originalWarn.apply(console, args);
};

// Set up global test environment
globalThis.TEST_ENV = true;

// Mock performance API if not available
if (typeof performance === 'undefined') {
  globalThis.performance = {
    now: () => Date.now(),
    mark: () => {},
    measure: () => {},
    getEntriesByName: () => [],
    getEntriesByType: () => []
  };
}

// Mock Lucide icons for tests
window.lucide = {
  createIcons: () => {},
  createElement: (name) => {
    const element = document.createElement('div');
    element.setAttribute('data-lucide', name);
    element.textContent = name; // fallback text
    return element;
  }
};

// Additional browser APIs used by react-dom/testing-library
if (typeof window.matchMedia !== 'function') {
  window.matchMedia = () => ({
    matches: false,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false
  });
}

if (typeof window.requestAnimationFrame !== 'function') {
  window.requestAnimationFrame = (cb) => setTimeout(cb, 0);
}

if (typeof window.cancelAnimationFrame !== 'function') {
  window.cancelAnimationFrame = (id) => clearTimeout(id);
}

if (typeof window.scrollTo !== 'function') {
  window.scrollTo = () => {};
}

if (typeof globalThis.Image === 'undefined') {
  globalThis.Image = class ImageMock {};
}

// Extend Jest matchers with testing-library
import '@testing-library/jest-dom';

// Jest-like globals for compatibility (Bun test uses different names)
if (typeof jest === 'undefined') {
  globalThis.jest = {
    fn: () => () => {},
    mock: () => {},
    clearAllMocks: () => {},
    resetAllMocks: () => {},
    restoreAllMocks: () => {},
    doMock: () => {},
    spyOn: (obj, method) => {
      const original = obj[method];
      const spy = (...args) => {
        spy.calls.push(args);
        return original.apply(obj, args);
      };
      spy.calls = [];
      spy.mockImplementation = (fn) => {
        obj[method] = fn;
        return spy;
      };
      spy.mockRestore = () => {
        obj[method] = original;
      };
      obj[method] = spy;
      return spy;
    }
  };
}
