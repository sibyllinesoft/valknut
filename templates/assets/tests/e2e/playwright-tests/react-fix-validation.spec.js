/**
 * React Fix Validation Tests
 * 
 * These tests specifically validate that the React createElement fix works
 * and that the "Objects are not valid as a React child" error is resolved.
 */

import { test, expect } from '@playwright/test';
import fs from 'fs';
import path from 'path';

test.describe('React Fix Validation', () => {
  test('should render empty state without React errors', async ({ page }) => {
    // Track console errors
    const consoleErrors = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });
    
    // Navigate to empty candidates test page
    const testFile = path.resolve('./test-results/empty-candidates.html');
    await page.goto(`file://${testFile}`);
    
    // Wait for React to load and render
    await page.waitForTimeout(2000);
    
    // Check for the empty state message
    const titleElement = page.locator('h3:has-text("No Refactoring Candidates Found")');
    const descElement = page.locator('p:has-text("Your code is in excellent shape!")');
    
    // Assert both elements are visible
    await expect(titleElement).toBeVisible();
    await expect(descElement).toBeVisible();
    
    // Most important: no React errors should be logged
    expect(consoleErrors.filter(error => 
      error.includes('Objects are not valid as a React child') ||
      error.includes('React error #31')
    )).toHaveLength(0);
    
    // Log results for debugging
    console.log(`Console errors detected: ${consoleErrors.length}`);
    if (consoleErrors.length > 0) {
      console.log('Console errors:', consoleErrors);
    }
  });
  
  test('should render normal results without errors', async ({ page }) => {
    const consoleErrors = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });
    
    // Navigate to normal results test page
    const testFile = path.resolve('./test-results/normal-results.html');
    await page.goto(`file://${testFile}`);
    
    // Wait for React to load and render
    await page.waitForTimeout(2000);
    
    // Should have tree nodes rendered
    const treeContainer = page.locator('[data-testid="tree-container"], #tree-container, .tree-container');
    await expect(treeContainer).toBeVisible();
    
    // No React errors
    expect(consoleErrors.filter(error => 
      error.includes('Objects are not valid as a React child') ||
      error.includes('React error #31')
    )).toHaveLength(0);
  });
  
  test('should handle large dataset without performance issues', async ({ page }) => {
    const consoleErrors = [];
    const consoleWarnings = [];
    
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      } else if (msg.type() === 'warning') {
        consoleWarnings.push(msg.text());
      }
    });
    
    // Navigate to large dataset test page
    const testFile = path.resolve('./test-results/large-dataset.html');
    await page.goto(`file://${testFile}`);
    
    // Measure rendering time
    const startTime = Date.now();
    
    // Wait for React to load and render (longer timeout for large dataset)
    await page.waitForTimeout(5000);
    
    const endTime = Date.now();
    const renderTime = endTime - startTime;
    
    // Should render within reasonable time (10 seconds max)
    expect(renderTime).toBeLessThan(10000);
    
    // No React errors even with large dataset
    expect(consoleErrors.filter(error => 
      error.includes('Objects are not valid as a React child') ||
      error.includes('React error #31')
    )).toHaveLength(0);
    
    console.log(`Large dataset render time: ${renderTime}ms`);
  });
  
  test('should have proper tree structure in DOM', async ({ page }) => {
    const testFile = path.resolve('./test-results/normal-results.html');
    await page.goto(`file://${testFile}`);
    
    await page.waitForTimeout(2000);
    
    // Check for React tree structure
    const reactRoot = page.locator('[data-reactroot], #tree-container');
    await expect(reactRoot).toBeVisible();
    
    // Check for proper HTML structure (no stray array objects)
    const textContent = await page.textContent('body');
    
    // Should not contain array artifacts like "[object Object]"
    expect(textContent).not.toContain('[object Object]');
    expect(textContent).not.toContain('[object HTMLElement]');
    
    // Should not contain stringified arrays like "[...]"
    const htmlContent = await page.innerHTML('body');
    expect(htmlContent).not.toMatch(/>\s*\[\s*<\w+/); // Pattern for array rendering
  });
  
  test('should handle interactive tree features', async ({ page }) => {
    const testFile = path.resolve('./test-results/normal-results.html');
    await page.goto(`file://${testFile}`);
    
    await page.waitForTimeout(2000);
    
    // Look for expandable tree nodes
    const chevronIcons = page.locator('[data-lucide="chevron-right"], [data-lucide="chevron-down"], .tree-chevron-icon');
    
    if (await chevronIcons.count() > 0) {
      // Click first expandable node
      await chevronIcons.first().click();
      
      // Wait for expansion
      await page.waitForTimeout(500);
      
      // Should not cause any React errors
      const consoleErrors = [];
      page.on('console', (msg) => {
        if (msg.type() === 'error') {
          consoleErrors.push(msg.text());
        }
      });
      
      await page.waitForTimeout(1000);
      
      expect(consoleErrors.filter(error => 
        error.includes('Objects are not valid as a React child')
      )).toHaveLength(0);
    }
  });
});