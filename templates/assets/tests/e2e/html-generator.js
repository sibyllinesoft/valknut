/**
 * HTML Generator - E2E Testing
 * 
 * This module replicates the complete HTML generation pipeline
 * used by valknut, from JSON analysis results to final HTML output.
 */

const fs = require('fs');
const path = require('path');
const TemplateCompiler = require('./template-compiler');

class HtmlGenerator {
    constructor() {
        this.compiler = new TemplateCompiler();
    }

    /**
     * Load and parse JSON analysis results
     */
    loadAnalysisResults(jsonPath) {
        try {
            const jsonContent = fs.readFileSync(jsonPath, 'utf8');
            return JSON.parse(jsonContent);
        } catch (error) {
            console.error(`Error loading analysis results from ${jsonPath}:`, error);
            throw error;
        }
    }

    /**
     * Transform valknut JSON output to template-ready format
     */
    transformDataForTemplate(analysisResults) {
        console.log('Transforming analysis results for template...');
        
        // Handle the real valknut JSON structure
        const summary = analysisResults.summary || {};
        const directoryTree = analysisResults.directory_health_tree || {};
        const unifiedHierarchy = analysisResults.unified_hierarchy || [];
        const refactoringCandidates = analysisResults.refactoring_candidates || [];
        const coveragePacks = analysisResults.coverage_packs || [];
        
        // Create summary compatible with template expectations
        const transformedSummary = {
            total_files: summary.files_processed || 0,
            analyzed_files: summary.files_processed || 0,
            overall_health: Math.round((summary.code_health_score || 0) * 100),
            refactoring_candidates: refactoringCandidates.length,
            entities_analyzed: summary.entities_analyzed || 0,
            avg_refactoring_score: summary.avg_refactoring_score || 0
        };

        // Extract file list from unified hierarchy or directory tree for compatibility
        const files = this.extractFilesFromHierarchy(unifiedHierarchy, directoryTree);
        
        // Build tree structure for tests
        const tree_data = this.buildTreeFromUnifiedHierarchy(unifiedHierarchy);
        
        // Pass the ORIGINAL valknut data structure to the template
        // This is what the tree.hbs template expects
        const transformed = {
            // Summary and basic info
            summary: transformedSummary,
            files: files, // For backward compatibility
            refactoring_candidates: refactoringCandidates,
            timestamp: new Date().toISOString(),
            version: analysisResults.version || 'valknut-analysis',
            
            // Tree structure for tests
            tree_data: tree_data,
            
            // Original valknut data for the tree template
            unified_hierarchy: JSON.stringify(unifiedHierarchy),
            directory_health_tree: directoryTree,
            coverage_packs: coveragePacks,
            
            // Statistics and warnings
            statistics: analysisResults.statistics || {},
            warnings: analysisResults.warnings || []
        };

        // Log key metrics for debugging
        console.log(`Transformed data summary:
- Total files: ${files.length}
- Refactoring candidates: ${refactoringCandidates.length}
- Overall health: ${transformedSummary.overall_health}%
- Unified hierarchy nodes: ${unifiedHierarchy.length}
- Directory tree: ${!!directoryTree.root}
- Coverage packs: ${coveragePacks.length}`);

        return transformed;
    }

    /**
     * Extract files from unified hierarchy structure
     */
    extractFilesFromHierarchy(unifiedHierarchy, directoryTree) {
        const files = [];
        
        const extractFromNode = (node, currentPath = '') => {
            if (node.type === 'file') {
                files.push({
                    path: node.name || currentPath,
                    complexity_score: node.score || 0,
                    structure_score: node.score || 0,
                    overall_health: Math.round((node.score || 0) * 10),
                    refactoring_candidates: node.children ? node.children.length : 0
                });
            }
            
            if (node.children && Array.isArray(node.children)) {
                node.children.forEach(child => {
                    const childPath = currentPath ? `${currentPath}/${child.name || ''}` : (child.name || '');
                    extractFromNode(child, childPath);
                });
            }
        };

        unifiedHierarchy.forEach(rootNode => {
            extractFromNode(rootNode);
        });

        // If no files found in unified hierarchy, try directory tree
        if (files.length === 0 && directoryTree.directories) {
            Object.values(directoryTree.directories).forEach(dir => {
                if (dir.path && dir.path.includes('.')) { // Simple heuristic for files
                    files.push({
                        path: dir.path,
                        complexity_score: 0,
                        structure_score: 0,
                        overall_health: Math.round((dir.health_score || 0) * 100),
                        refactoring_candidates: dir.refactoring_needed || 0
                    });
                }
            });
        }

        return files;
    }

    /**
     * Build tree structure from unified hierarchy
     */
    buildTreeFromUnifiedHierarchy(unifiedHierarchy) {
        if (!unifiedHierarchy || unifiedHierarchy.length === 0) {
            return null;
        }

        const tree = {
            name: 'root',
            type: 'directory',
            children: [],
            path: '',
            metrics: {
                complexity_score: 0,
                structure_score: 0,
                overall_health: 0
            }
        };

        const convertNode = (hierarchyNode) => {
            const node = {
                name: hierarchyNode.name || 'unnamed',
                type: hierarchyNode.type || 'directory',
                path: hierarchyNode.path || hierarchyNode.name || '',
                metrics: {
                    complexity_score: Math.round((hierarchyNode.score || 0) * 10),
                    structure_score: Math.round((hierarchyNode.score || 0) * 10),
                    overall_health: Math.round((hierarchyNode.score || 0) * 10)
                }
            };

            if (hierarchyNode.children && Array.isArray(hierarchyNode.children)) {
                node.children = hierarchyNode.children.map(convertNode);
            } else {
                node.children = [];
            }

            return node;
        };

        tree.children = unifiedHierarchy.map(convertNode);
        return tree;
    }

    /**
     * Build tree structure from directory health tree
     */
    buildTreeFromDirectoryHealth(directoryTree) {
        if (!directoryTree || !directoryTree.root) {
            return {
                name: 'root',
                type: 'directory',
                children: [],
                path: '',
                metrics: { complexity_score: 0, structure_score: 0, overall_health: 0 }
            };
        }

        const root = directoryTree.root;
        const directories = directoryTree.directories || {};

        const convertDirectoryNode = (dirPath, dirData) => {
            return {
                name: dirPath.split('/').pop() || dirPath,
                type: 'directory',
                path: dirPath,
                metrics: {
                    complexity_score: Math.round((dirData.avg_refactoring_score || 0) * 100),
                    structure_score: Math.round((dirData.health_score || 0) * 100),
                    overall_health: Math.round((dirData.health_score || 0) * 100)
                },
                children: (dirData.children || []).map(childPath => {
                    if (directories[childPath]) {
                        return convertDirectoryNode(childPath, directories[childPath]);
                    } else {
                        // Treat as file
                        return {
                            name: childPath.split('/').pop() || childPath,
                            type: 'file',
                            path: childPath,
                            metrics: {
                                complexity_score: 0,
                                structure_score: 0,
                                overall_health: 50
                            }
                        };
                    }
                })
            };
        };

        return convertDirectoryNode(root.path, root);
    }

    /**
     * Helper method to count tree nodes
     */
    countTreeNodes(node) {
        if (!node) return 0;
        let count = 1;
        if (node.children && Array.isArray(node.children)) {
            node.children.forEach(child => {
                count += this.countTreeNodes(child);
            });
        }
        return count;
    }

    /**
     * Build hierarchical tree structure from flat file list
     */
    buildTreeStructure(files) {
        const tree = {
            name: 'root',
            type: 'directory',
            children: [],
            path: '',
            metrics: {
                complexity_score: 0,
                structure_score: 0,
                overall_health: 0
            }
        };

        files.forEach(file => {
            this.addFileToTree(tree, file);
        });

        return tree;
    }

    /**
     * Add a file to the tree structure
     */
    addFileToTree(root, file) {
        const pathParts = file.path.split('/').filter(part => part.length > 0);
        let currentNode = root;

        // Navigate/create path
        for (let i = 0; i < pathParts.length - 1; i++) {
            const part = pathParts[i];
            let child = currentNode.children.find(c => c.name === part);
            
            if (!child) {
                child = {
                    name: part,
                    type: 'directory',
                    children: [],
                    path: pathParts.slice(0, i + 1).join('/'),
                    metrics: {
                        complexity_score: 0,
                        structure_score: 0,
                        overall_health: 0
                    }
                };
                currentNode.children.push(child);
            }
            
            currentNode = child;
        }

        // Add the file itself
        const fileName = pathParts[pathParts.length - 1];
        const fileNode = {
            name: fileName,
            type: 'file',
            path: file.path,
            metrics: {
                complexity_score: file.complexity_score || 0,
                structure_score: file.structure_score || 0,
                overall_health: file.overall_health || 0,
                refactoring_candidates: file.refactoring_candidates || 0
            },
            refactoring_opportunities: file.refactoring_opportunities || []
        };

        currentNode.children.push(fileNode);
    }

    /**
     * Generate complete HTML from analysis results
     */
    generateHtml(analysisResults) {
        console.log('Starting HTML generation...');
        
        // Transform data for template
        const templateData = this.transformDataForTemplate(analysisResults);
        
        // Generate tree HTML using the tree partial
        console.log('Compiling tree template...');
        const treeHtml = this.compiler.compileTreeTemplate(templateData);
        
        // Generate complete page
        console.log('Generating complete HTML page...');
        const completeHtml = this.generateCompletePage(templateData, treeHtml);
        
        return {
            treeHtml,
            completeHtml,
            templateData
        };
    }

    /**
     * Generate a complete HTML page with embedded styles and scripts
     */
    generateCompletePage(templateData, treeHtml) {
        const html = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Valknut Analysis Results - E2E Test</title>
    <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.1.3/dist/css/bootstrap.min.css" rel="stylesheet">
    <link href="https://cdn.jsdelivr.net/npm/lucide@latest/icons.css" rel="stylesheet">
    <style>
        .tree-view {
            font-family: 'Courier New', monospace;
        }
        .tree-node {
            margin: 2px 0;
            padding: 4px 8px;
            border-radius: 4px;
        }
        .tree-node:hover {
            background-color: #f8f9fa;
        }
        .tree-indent {
            width: 20px;
            display: inline-block;
        }
        .tree-icon {
            width: 16px;
            height: 16px;
            margin-right: 8px;
        }
        .badge {
            font-size: 0.75em;
            margin-left: 8px;
        }
        .refactoring-opportunity {
            background-color: #fff3cd;
            border: 1px solid #ffeaa7;
            padding: 8px;
            margin: 4px 0;
            border-radius: 4px;
        }
    </style>
</head>
<body>
    <div class="container-fluid py-4">
        <div class="row">
            <div class="col-12">
                <div class="card">
                    <div class="card-header">
                        <h2 class="mb-0">Valknut Analysis Results</h2>
                        <small class="text-muted">Generated: ${templateData.timestamp}</small>
                    </div>
                    <div class="card-body">
                        <div class="row mb-4">
                            <div class="col-md-3">
                                <div class="text-center">
                                    <h4>${templateData.summary.analyzed_files}</h4>
                                    <p class="text-muted">Files Analyzed</p>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="text-center">
                                    <h4>${Math.round(templateData.summary.overall_health)}%</h4>
                                    <p class="text-muted">Overall Health</p>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="text-center">
                                    <h4>${templateData.refactoring_candidates.length}</h4>
                                    <p class="text-muted">Refactoring Candidates</p>
                                </div>
                            </div>
                            <div class="col-md-3">
                                <div class="text-center">
                                    <h4>${templateData.version}</h4>
                                    <p class="text-muted">Version</p>
                                </div>
                            </div>
                        </div>
                        
                        <div class="tree-view">
                            <h5>Code Structure Tree</h5>
                            ${treeHtml}
                        </div>
                        
                        ${templateData.refactoring_candidates.length > 0 ? `
                        <div class="mt-4">
                            <h5>Refactoring Opportunities</h5>
                            ${templateData.refactoring_candidates.map(candidate => `
                                <div class="refactoring-opportunity">
                                    <strong>${candidate.file_path}</strong>
                                    <p>${candidate.description}</p>
                                    <small class="text-muted">Priority: ${candidate.priority}</small>
                                </div>
                            `).join('')}
                        </div>
                        ` : '<div class="alert alert-info mt-4">No refactoring candidates found.</div>'}
                    </div>
                </div>
            </div>
        </div>
    </div>

    <script src="https://cdn.jsdelivr.net/npm/bootstrap@5.1.3/dist/js/bootstrap.bundle.min.js"></script>
    <script>
        // Add any interactive behavior here
        console.log('HTML page loaded with data:', ${JSON.stringify(templateData, null, 2)});
    </script>
</body>
</html>`;

        return html;
    }

    /**
     * Generate HTML from JSON file path
     */
    generateFromJsonFile(jsonPath, outputPath = null) {
        console.log(`Loading analysis results from: ${jsonPath}`);
        
        const analysisResults = this.loadAnalysisResults(jsonPath);
        const result = this.generateHtml(analysisResults);
        
        if (outputPath) {
            console.log(`Writing HTML output to: ${outputPath}`);
            fs.writeFileSync(outputPath, result.completeHtml, 'utf8');
        }
        
        return result;
    }
}

module.exports = HtmlGenerator;