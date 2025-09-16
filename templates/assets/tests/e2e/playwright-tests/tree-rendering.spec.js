/**
 * Tree Rendering Tests
 * 
 * Comprehensive tests for the React tree component rendering
 * across different data scenarios and browser environments.
 */

import { test, expect } from '@playwright/test';
import fs from 'fs';
import path from 'path';

test.describe('Tree Component Rendering', () => {
  test('should render tree structure correctly', async ({ page }) => {
    const testFile = path.resolve('./test-results/normal-results.html');
    await page.goto(`file://${testFile}`);
    
    await page.waitForTimeout(2000);
    
    // Check for tree container
    const treeContainer = page.locator('[data-testid="tree-container"], #tree-container, .tree-container, [role="tree"]');
    await expect(treeContainer).toBeVisible();
    
    // Check for tree nodes
    const treeNodes = page.locator('[data-testid*="tree-node"], .tree-node, [role="treeitem"]');
    await expect(treeNodes.first()).toBeVisible();
    
    // Verify tree has hierarchical structure
    const nodeCount = await treeNodes.count();
    expect(nodeCount).toBeGreaterThan(0);
  });
  
  test('should display file and folder icons correctly', async ({ page }) => {
    const testFile = path.resolve('./test-results/normal-results.html');
    await page.goto(`file://${testFile}`);
    
    await page.waitForTimeout(2000);
    
    // Check for Lucide icons
    const icons = page.locator('[data-lucide]');
    const iconCount = await icons.count();
    
    if (iconCount > 0) {
      // Verify common icon types
      const folderIcon = page.locator('[data-lucide="folder"]');
      const fileIcon = page.locator('[data-lucide="file-code"], [data-lucide="file"]');
      const functionIcon = page.locator('[data-lucide="function-square"]');
      
      // At least one type should be present
      const hasIcons = (await folderIcon.count()) > 0 || 
                      (await fileIcon.count()) > 0 || 
                      (await functionIcon.count()) > 0;
      expect(hasIcons).toBeTruthy();
    }
  });
  
  test('should show complexity scores and badges', async ({ page }) => {
    const testFile = path.resolve('./test-results/normal-results.html');
    await page.goto(`file://${testFile}`);
    
    await page.waitForTimeout(2000);
    
    // Look for complexity-related elements
    const complexityElements = page.locator('.tree-badge, .complexity-score, [class*="complexity"]');
    
    if (await complexityElements.count() > 0) {
      // Verify complexity scores are displayed
      const complexityText = await complexityElements.first().textContent();
      expect(complexityText).toMatch(/complexity|score|health/i);
    }
  });
  
  test('should handle empty state gracefully', async ({ page }) => {
    const testFile = path.resolve('./test-results/empty-candidates.html');
    await page.goto(`file://${testFile}`);
    
    await page.waitForTimeout(2000);
    
    // Should show empty state message
    await expect(page.locator('text=No Refactoring Candidates Found')).toBeVisible();
    await expect(page.locator('text=Your code is in excellent shape!')).toBeVisible();
    
    // Should have proper styling
    const emptyContainer = page.locator('text=No Refactoring Candidates Found').locator('..');
    const styles = await emptyContainer.evaluate(el => getComputedStyle(el));
    
    // Should be center-aligned
    expect(styles.textAlign).toBe('center');
  });
  
  test('should be responsive on different viewport sizes', async ({ page }) => {
    const testFile = path.resolve('./test-results/normal-results.html');
    
    // Test desktop size
    await page.setViewportSize({ width: 1200, height: 800 });
    await page.goto(`file://${testFile}`);
    await page.waitForTimeout(1000);
    
    let treeContainer = page.locator('[data-testid="tree-container"], #tree-container, .tree-container');
    await expect(treeContainer).toBeVisible();
    
    // Test tablet size
    await page.setViewportSize({ width: 768, height: 1024 });
    await page.waitForTimeout(500);
    await expect(treeContainer).toBeVisible();
    
    // Test mobile size
    await page.setViewportSize({ width: 375, height: 667 });
    await page.waitForTimeout(500);
    await expect(treeContainer).toBeVisible();
  });
  
  test('should handle mixed results scenario', async ({ page }) => {
    const testFile = path.resolve('./test-results/mixed-results.html');
    await page.goto(`file://${testFile}`);
    
    await page.waitForTimeout(2000);
    
    // Should render tree if there are any files/results
    const bodyText = await page.textContent('body');
    
    // Should either show tree or empty state, but not crash
    const hasTree = await page.locator('[role="tree"], .tree-container, #tree-container').isVisible();
    const hasEmptyState = bodyText.includes('No Refactoring Candidates Found');
    
    expect(hasTree || hasEmptyState).toBeTruthy();
  });
  
  test('should not have accessibility violations', async ({ page }) => {
    const testFile = path.resolve('./test-results/normal-results.html');
    await page.goto(`file://${testFile}`);
    
    await page.waitForTimeout(2000);
    
    // Basic accessibility checks
    // Check for proper heading structure
    const headings = page.locator('h1, h2, h3, h4, h5, h6');
    if (await headings.count() > 0) {
      // Should have logical heading hierarchy
      const firstHeading = await headings.first().tagName();
      expect(['H1', 'H2', 'H3']).toContain(firstHeading);
    }
    
    // Check for alt text on images (if any)
    const images = page.locator('img');
    const imageCount = await images.count();
    for (let i = 0; i < imageCount; i++) {
      const alt = await images.nth(i).getAttribute('alt');
      expect(alt).not.toBeNull();
    }
    
    // Check for proper color contrast (basic check)
    const bodyStyles = await page.evaluate(() => {
      const body = document.body;
      const styles = getComputedStyle(body);
      return {
        color: styles.color,
        backgroundColor: styles.backgroundColor
      };
    });
    
    // Should have defined colors (not default/transparent)
    expect(bodyStyles.color).not.toBe('');
    expect(bodyStyles.backgroundColor).not.toBe('');
  });
});