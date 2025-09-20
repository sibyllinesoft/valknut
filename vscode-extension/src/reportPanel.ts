import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';

export class ReportPanel {
    public static currentPanel: ReportPanel | undefined;
    private static readonly viewType = 'valknutReport';

    private readonly _panel: vscode.WebviewPanel;
    private readonly _extensionUri: vscode.Uri;
    private _disposables: vscode.Disposable[] = [];
    private _reportData: any = null;
    private _reportPath: string;

    public static createOrShow(extensionUri: vscode.Uri, reportPath: string) {
        const column = vscode.window.activeTextEditor
            ? vscode.window.activeTextEditor.viewColumn
            : undefined;

        // If we already have a panel, show it
        if (ReportPanel.currentPanel) {
            ReportPanel.currentPanel._panel.reveal(column);
            ReportPanel.currentPanel.loadReport(reportPath);
            return;
        }

        // Otherwise, create a new panel
        const panel = vscode.window.createWebviewPanel(
            ReportPanel.viewType,
            'Valknut Report',
            column || vscode.ViewColumn.One,
            {
                // Enable javascript in the webview
                enableScripts: true,
                // Restrict the webview to only loading content from our extension's `media` directory
                localResourceRoots: [
                    vscode.Uri.joinPath(extensionUri, 'media'),
                    vscode.Uri.joinPath(extensionUri, '..', 'themes'),
                    vscode.Uri.joinPath(extensionUri, '..', 'templates')
                ]
            }
        );

        ReportPanel.currentPanel = new ReportPanel(panel, extensionUri, reportPath);
    }

    public static refresh() {
        if (ReportPanel.currentPanel) {
            ReportPanel.currentPanel._update();
        }
    }

    public static updateConfiguration() {
        if (ReportPanel.currentPanel) {
            ReportPanel.currentPanel._update();
        }
    }

    public static dispose() {
        if (ReportPanel.currentPanel) {
            ReportPanel.currentPanel.dispose();
        }
    }

    private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri, reportPath: string) {
        this._panel = panel;
        this._extensionUri = extensionUri;
        this._reportPath = reportPath;

        // Load the report data
        this.loadReport(reportPath);

        // Set the webview's initial html content
        this._update();

        // Listen for when the panel is disposed
        this._panel.onDidDispose(() => this.dispose(), null, this._disposables);

        // Handle messages from the webview
        this._panel.webview.onDidReceiveMessage(
            message => {
                switch (message.command) {
                    case 'openFile':
                        this.openFile(message.filePath, message.line);
                        return;
                    case 'exportReport':
                        vscode.commands.executeCommand('valknut.exportReport');
                        return;
                    case 'refreshReport':
                        this.loadReport(this._reportPath);
                        this._update();
                        return;
                }
            },
            null,
            this._disposables
        );
    }

    private async openFile(filePath: string, line?: number) {
        try {
            // Resolve relative paths
            let fullPath = filePath;
            if (!path.isAbsolute(filePath)) {
                const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
                if (workspaceRoot) {
                    fullPath = path.resolve(workspaceRoot, filePath);
                }
            }

            // Check if file exists
            if (!fs.existsSync(fullPath)) {
                vscode.window.showWarningMessage(`File not found: ${filePath}`);
                return;
            }

            const document = await vscode.workspace.openTextDocument(fullPath);
            const editor = await vscode.window.showTextDocument(document);

            // Jump to specific line if provided
            if (line && line > 0) {
                const position = new vscode.Position(line - 1, 0);
                editor.selection = new vscode.Selection(position, position);
                editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenter);
            }
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to open file: ${error}`);
        }
    }

    private loadReport(reportPath: string) {
        try {
            if (!fs.existsSync(reportPath)) {
                vscode.window.showErrorMessage(`Report file not found: ${reportPath}`);
                return;
            }

            const reportContent = fs.readFileSync(reportPath, 'utf8');
            this._reportData = JSON.parse(reportContent);
            this._reportPath = reportPath;
            
            // Update panel title with report name
            const reportName = path.basename(reportPath, path.extname(reportPath));
            this._panel.title = `Valknut Report - ${reportName}`;
            
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to load report: ${error}`);
        }
    }

    public get reportPath(): string {
        return this._reportPath;
    }

    private _update() {
        const webview = this._panel.webview;
        this._panel.webview.html = this._getHtmlForWebview(webview);
    }

    private _getHtmlForWebview(webview: vscode.Webview): string {
        if (!this._reportData) {
            return this._getLoadingHtml();
        }

        const config = vscode.workspace.getConfiguration('valknut');
        const theme = config.get('theme', 'default');
        
        // Get theme CSS
        const themeUri = vscode.Uri.joinPath(this._extensionUri, '..', 'themes', `${theme}.css`);
        const themeCSS = webview.asWebviewUri(themeUri);

        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Valknut Report</title>
    <link href="${themeCSS}" rel="stylesheet">
    <style>
        /* VS Code integration styles */
        .toolbar {
            position: fixed;
            top: 10px;
            right: 10px;
            display: flex;
            gap: 8px;
            z-index: 1000;
        }
        
        .toolbar button {
            background: var(--primary-color);
            color: white;
            border: none;
            padding: 8px 12px;
            border-radius: 4px;
            cursor: pointer;
            font-size: 12px;
            transition: opacity 0.2s;
        }
        
        .toolbar button:hover {
            opacity: 0.8;
        }
        
        .status-bar {
            position: fixed;
            bottom: 0;
            left: 0;
            right: 0;
            background: var(--surface-color);
            border-top: 1px solid var(--border-color);
            padding: 8px 16px;
            font-size: 12px;
            color: var(--text-secondary);
        }
    </style>
</head>
<body>
    <div class="toolbar">
        <button onclick="refreshReport()">Refresh</button>
        <button onclick="exportReport()">Export</button>
    </div>
    
    <div class="container">
        <header class="header">
            <h1>Valknut Analysis Report</h1>
            <div class="meta">
                Report: ${path.basename(this._reportPath)} | Generated on ${new Date().toISOString()}
            </div>
        </header>
        
        <section class="summary">
            ${this.generateSummaryCards()}
        </section>
        
        <section class="results-section">
            <h2>File Analysis</h2>
            <div class="file-list">
                ${this.generateInteractiveFileList()}
            </div>
        </section>
        
        ${this.generateIssuesSection()}
        
        <details class="raw-data">
            <summary>Raw Data</summary>
            <pre><code>${JSON.stringify(this._reportData, null, 2)}</code></pre>
        </details>
    </div>
    
    <div class="status-bar">
        <span>Report loaded: ${this._reportPath}</span>
    </div>
    
    <script>
        const vscode = acquireVsCodeApi();
        
        function refreshReport() {
            vscode.postMessage({ command: 'refreshReport' });
        }
        
        function exportReport() {
            vscode.postMessage({ command: 'exportReport' });
        }
        
        function openFile(filePath, line) {
            vscode.postMessage({ 
                command: 'openFile',
                filePath: filePath,
                line: line
            });
        }
        
        // Add click handlers for file items
        document.addEventListener('DOMContentLoaded', function() {
            const fileItems = document.querySelectorAll('.file-item[data-file-path]');
            fileItems.forEach(item => {
                item.addEventListener('click', function() {
                    const filePath = this.dataset.filePath;
                    const line = parseInt(this.dataset.line || '1');
                    openFile(filePath, line);
                });
            });
            
            // Add click handlers for issue items
            const issueItems = document.querySelectorAll('.issue-item[data-line]');
            issueItems.forEach(item => {
                item.addEventListener('click', function(e) {
                    e.stopPropagation();
                    const filePath = this.closest('.file-item').dataset.filePath;
                    const line = parseInt(this.dataset.line || '1');
                    openFile(filePath, line);
                });
            });
        });
    </script>
</body>
</html>`;
    }

    private generateSummaryCards(): string {
        const files = this._reportData?.files || [];
        const totalFiles = files.length;
        const totalIssues = files.reduce((total: number, file: any) => {
            return total + (file.issues?.length || 0);
        }, 0);
        const avgComplexity = files.length > 0 
            ? (files.reduce((sum: number, file: any) => sum + (file.complexity || 0), 0) / files.length).toFixed(1)
            : '0.0';

        return `
            <div class="summary-card">
                <div class="value">${totalFiles}</div>
                <div class="label">Files Analyzed</div>
            </div>
            <div class="summary-card">
                <div class="value">${totalIssues}</div>
                <div class="label">Issues Found</div>
            </div>
            <div class="summary-card">
                <div class="value">${avgComplexity}</div>
                <div class="label">Avg Complexity</div>
            </div>
        `;
    }

    private generateInteractiveFileList(): string {
        const files = this._reportData?.files || [];
        
        return files.map((file: any) => {
            const issuesCount = file.issues?.length || 0;
            const badgeClass = issuesCount > 0 ? 'error' : 'success';
            const badgeText = issuesCount > 0 ? `${issuesCount} issues` : 'Clean';
            
            return `
                <div class="file-item" data-file-path="${file.path}" data-line="1">
                    <div class="file-header">
                        <div class="file-path">${file.path}</div>
                        <div class="file-badge">
                            <span class="badge ${badgeClass}">${badgeText}</span>
                        </div>
                    </div>
                    <div class="file-details">
                        <span>Size: ${file.size || 0} bytes</span>
                        ${file.complexity ? `<span>Complexity: ${file.complexity}</span>` : ''}
                        ${file.language ? `<span>Language: ${file.language}</span>` : ''}
                    </div>
                    ${this.generateIssuesPreview(file.issues)}
                </div>
            `;
        }).join('');
    }

    private generateIssuesPreview(issues: any[]): string {
        if (!issues || issues.length === 0) {
            return '';
        }

        return `
            <div class="issues-preview">
                ${issues.map(issue => `
                    <div class="issue-item" data-line="${issue.line || 1}">
                        <span class="issue-type ${issue.severity || 'info'}">${issue.type || 'Issue'}</span>
                        <span class="issue-message">${issue.message || 'No description'}</span>
                        ${issue.line ? `<span class="issue-location">Line ${issue.line}</span>` : ''}
                    </div>
                `).join('')}
            </div>
        `;
    }

    private generateIssuesSection(): string {
        const files = this._reportData?.files || [];
        const allIssues = files.flatMap((file: any) => 
            (file.issues || []).map((issue: any) => ({ ...issue, file: file.path }))
        );

        if (allIssues.length === 0) {
            return '';
        }

        return `
            <section class="results-section">
                <h2>All Issues (${allIssues.length})</h2>
                <div class="issues-list">
                    ${allIssues.map((issue: any) => `
                        <div class="issue-item-full" data-file-path="${issue.file}" data-line="${issue.line || 1}">
                            <div class="issue-header">
                                <span class="issue-type ${issue.severity || 'info'}">${issue.type || 'Issue'}</span>
                                <span class="issue-file">${issue.file}${issue.line ? `:${issue.line}` : ''}</span>
                            </div>
                            <div class="issue-message">${issue.message || 'No description'}</div>
                        </div>
                    `).join('')}
                </div>
            </section>
        `;
    }

    private _getLoadingHtml(): string {
        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Loading Report</title>
    <style>
        body {
            font-family: system-ui, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: var(--vscode-editor-background);
            color: var(--vscode-editor-foreground);
        }
        .loading {
            text-align: center;
        }
        .spinner {
            border: 3px solid rgba(255, 255, 255, 0.3);
            border-radius: 50%;
            border-top: 3px solid var(--vscode-progressBar-background);
            width: 40px;
            height: 40px;
            animation: spin 1s linear infinite;
            margin: 0 auto 1rem;
        }
        @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
        }
    </style>
</head>
<body>
    <div class="loading">
        <div class="spinner"></div>
        <p>Loading Valknut report...</p>
    </div>
</body>
</html>`;
    }

    public dispose() {
        ReportPanel.currentPanel = undefined;

        // Clean up our resources
        this._panel.dispose();

        while (this._disposables.length) {
            const x = this._disposables.pop();
            if (x) {
                x.dispose();
            }
        }
    }
}
