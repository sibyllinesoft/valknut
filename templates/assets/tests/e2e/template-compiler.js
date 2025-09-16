/**
 * Handlebars Template Compiler - E2E Testing
 * 
 * This module provides utilities for compiling Handlebars templates 
 * in the exact same way as the real valknut system, ensuring
 * our E2E tests replicate the actual rendering pipeline.
 */

const fs = require('fs');
const path = require('path');
const Handlebars = require('handlebars');

class TemplateCompiler {
    constructor() {
        this.handlebars = Handlebars.create();
        this.templatesDir = path.resolve(__dirname, '../../..');
        this.partialsDir = path.join(this.templatesDir, 'partials');
        
        this.setupHelpers();
        this.registerPartials();
    }

    /**
     * Register all Handlebars helpers used by valknut
     */
    setupHelpers() {
        // Register the same helpers as the real system
        this.handlebars.registerHelper('json', function(context) {
            return JSON.stringify(context);
        });

        this.handlebars.registerHelper('eq', function(a, b) {
            return a === b;
        });

        this.handlebars.registerHelper('gt', function(a, b) {
            return a > b;
        });

        this.handlebars.registerHelper('lt', function(a, b) {
            return a < b;
        });

        this.handlebars.registerHelper('and', function(a, b) {
            return a && b;
        });

        this.handlebars.registerHelper('or', function(a, b) {
            return a || b;
        });

        this.handlebars.registerHelper('not', function(a) {
            return !a;
        });

        this.handlebars.registerHelper('capitalize', function(str) {
            if (!str) return '';
            return str.charAt(0).toUpperCase() + str.slice(1);
        });

        this.handlebars.registerHelper('round', function(num, decimals = 2) {
            return Number(num).toFixed(decimals);
        });

        this.handlebars.registerHelper('percentage', function(num) {
            return (Number(num) * 100).toFixed(1) + '%';
        });

        this.handlebars.registerHelper('pluralize', function(count, singular, plural) {
            return count === 1 ? singular : plural;
        });

        // Tree-specific helpers
        this.handlebars.registerHelper('hasChildren', function(node) {
            return node && node.children && node.children.length > 0;
        });

        this.handlebars.registerHelper('getIcon', function(nodeType) {
            const icons = {
                'directory': 'folder',
                'file': 'file',
                'function': 'code',
                'class': 'box',
                'module': 'package'
            };
            return icons[nodeType] || 'file';
        });

        this.handlebars.registerHelper('getBadgeColor', function(score) {
            if (score >= 80) return 'success';
            if (score >= 60) return 'warning';
            return 'danger';
        });

        this.handlebars.registerHelper('formatScore', function(score) {
            if (typeof score !== 'number') return 'N/A';
            return Math.round(score);
        });
    }

    /**
     * Register all partials from the partials directory
     */
    registerPartials() {
        if (!fs.existsSync(this.partialsDir)) {
            console.warn(`Partials directory not found: ${this.partialsDir}`);
            return;
        }

        const partialFiles = fs.readdirSync(this.partialsDir)
            .filter(file => file.endsWith('.hbs'));

        partialFiles.forEach(file => {
            const partialName = path.basename(file, '.hbs');
            const partialPath = path.join(this.partialsDir, file);
            const partialContent = fs.readFileSync(partialPath, 'utf8');
            
            this.handlebars.registerPartial(partialName, partialContent);
            console.log(`Registered partial: ${partialName}`);
        });
    }

    /**
     * Compile a template file with the given data
     */
    compileTemplate(templatePath, data) {
        try {
            const fullTemplatePath = path.resolve(this.templatesDir, templatePath);
            
            if (!fs.existsSync(fullTemplatePath)) {
                throw new Error(`Template not found: ${fullTemplatePath}`);
            }

            const templateContent = fs.readFileSync(fullTemplatePath, 'utf8');
            const template = this.handlebars.compile(templateContent);
            
            return template(data);
        } catch (error) {
            console.error(`Error compiling template ${templatePath}:`, error);
            throw error;
        }
    }

    /**
     * Compile the main tree template with analysis data
     */
    compileTreeTemplate(analysisData) {
        return this.compileTemplate('partials/tree.hbs', analysisData);
    }

    /**
     * Compile the complete HTML page
     */
    compileFullPage(analysisData) {
        // First compile the tree
        const treeHtml = this.compileTreeTemplate(analysisData);
        
        // Then compile the full page template
        const pageData = {
            ...analysisData,
            tree_html: treeHtml,
            title: 'Valknut Analysis Results',
            timestamp: new Date().toISOString()
        };

        // Note: Adjust this path based on your main template structure
        // This assumes there's a main.hbs or similar
        return this.compileTemplate('main.hbs', pageData);
    }
}

module.exports = TemplateCompiler;