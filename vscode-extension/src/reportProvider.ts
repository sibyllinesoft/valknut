import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';

export class ReportProvider implements vscode.TreeDataProvider<ReportItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<ReportItem | undefined | null | void> = new vscode.EventEmitter<ReportItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<ReportItem | undefined | null | void> = this._onDidChangeTreeData.event;

    constructor(private context: vscode.ExtensionContext) {}

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    hasReports(): boolean {
        const reports = this.getReports();
        return reports.length > 0;
    }

    getTreeItem(element: ReportItem): vscode.TreeItem {
        return element;
    }

    getChildren(element?: ReportItem): Thenable<ReportItem[]> {
        if (!element) {
            // Root level - show reports
            return Promise.resolve(this.getReports());
        } else if (element.contextValue === 'report') {
            // Report level - show file sections
            return Promise.resolve(this.getReportSections(element));
        } else if (element.contextValue === 'section') {
            // Section level - show files or issues
            return Promise.resolve(this.getSectionItems(element));
        } else {
            return Promise.resolve([]);
        }
    }

    private getReports(): ReportItem[] {
        const config = vscode.workspace.getConfiguration('valknut');
        const reportPath = config.get<string>('reportPath', '');
        
        const reports: ReportItem[] = [];
        
        // Look in configured report path
        if (reportPath && fs.existsSync(reportPath)) {
            this.addReportsFromDirectory(reportPath, reports);
        }
        
        // Look in workspace root
        if (vscode.workspace.workspaceFolders) {
            for (const folder of vscode.workspace.workspaceFolders) {
                const workspaceReportPath = path.join(folder.uri.fsPath, 'reports');
                if (fs.existsSync(workspaceReportPath)) {
                    this.addReportsFromDirectory(workspaceReportPath, reports);
                }
                
                // Also check for individual report files
                const files = fs.readdirSync(folder.uri.fsPath);
                for (const file of files) {
                    if (file.includes('valknut') && file.endsWith('.json')) {
                        const filePath = path.join(folder.uri.fsPath, file);
                        if (this.isValidReport(filePath)) {
                            reports.push(this.createReportItem(filePath));
                        }
                    }
                }
            }
        }
        
        return reports;
    }

    private addReportsFromDirectory(dirPath: string, reports: ReportItem[]) {
        try {
            const files = fs.readdirSync(dirPath);
            for (const file of files) {
                const filePath = path.join(dirPath, file);
                const stat = fs.statSync(filePath);
                
                if (stat.isFile() && file.endsWith('.json')) {
                    if (this.isValidReport(filePath)) {
                        reports.push(this.createReportItem(filePath));
                    }
                } else if (stat.isDirectory()) {
                    // Recursively search subdirectories
                    this.addReportsFromDirectory(filePath, reports);
                }
            }
        } catch (error) {
            console.error('Error reading report directory:', error);
        }
    }

    private isValidReport(filePath: string): boolean {
        try {
            const content = fs.readFileSync(filePath, 'utf8');
            const data = JSON.parse(content);
            
            // Check if it looks like a Valknut report
            return data && (
                data.files || 
                data.analysis_results || 
                data.tool === 'valknut' ||
                filePath.includes('valknut')
            );
        } catch {
            return false;
        }
    }

    private createReportItem(filePath: string): ReportItem {
        const fileName = path.basename(filePath, '.json');
        const stats = fs.statSync(filePath);
        
        let reportData: any = {};
        try {
            const content = fs.readFileSync(filePath, 'utf8');
            reportData = JSON.parse(content);
        } catch (error) {
            console.error('Error parsing report:', error);
        }

        const fileCount = reportData.files?.length || 0;
        const issueCount = reportData.files?.reduce((total: number, file: any) => 
            total + (file.issues?.length || 0), 0) || 0;

        const item = new ReportItem(
            fileName,
            vscode.TreeItemCollapsibleState.Collapsed,
            {
                command: 'valknut.openReport',
                title: 'Open Report',
                arguments: [vscode.Uri.file(filePath)]
            }
        );

        item.contextValue = 'report';
        item.tooltip = `${filePath}\nFiles: ${fileCount}, Issues: ${issueCount}\nModified: ${stats.mtime.toLocaleString()}`;
        item.description = `${fileCount} files, ${issueCount} issues`;
        item.iconPath = new vscode.ThemeIcon('file-code');
        item.resourceUri = vscode.Uri.file(filePath);

        return item;
    }

    private getReportSections(reportItem: ReportItem): ReportItem[] {
        if (!reportItem.resourceUri) {
            return [];
        }

        try {
            const content = fs.readFileSync(reportItem.resourceUri.fsPath, 'utf8');
            const data = JSON.parse(content);
            
            const sections: ReportItem[] = [];
            
            // Files section
            if (data.files && data.files.length > 0) {
                const filesItem = new ReportItem(
                    `Files (${data.files.length})`,
                    vscode.TreeItemCollapsibleState.Collapsed
                );
                filesItem.contextValue = 'section';
                filesItem.iconPath = new vscode.ThemeIcon('file-directory');
                filesItem.sectionType = 'files';
                filesItem.reportData = data;
                sections.push(filesItem);
            }
            
            // Issues section
            const totalIssues = data.files?.reduce((total: number, file: any) => 
                total + (file.issues?.length || 0), 0) || 0;
                
            if (totalIssues > 0) {
                const issuesItem = new ReportItem(
                    `Issues (${totalIssues})`,
                    vscode.TreeItemCollapsibleState.Collapsed
                );
                issuesItem.contextValue = 'section';
                issuesItem.iconPath = new vscode.ThemeIcon('warning');
                issuesItem.sectionType = 'issues';
                issuesItem.reportData = data;
                sections.push(issuesItem);
            }
            
            // Metrics section
            if (data.metrics || data.analysis_results) {
                const metricsItem = new ReportItem(
                    'Metrics',
                    vscode.TreeItemCollapsibleState.Collapsed
                );
                metricsItem.contextValue = 'section';
                metricsItem.iconPath = new vscode.ThemeIcon('graph');
                metricsItem.sectionType = 'metrics';
                metricsItem.reportData = data;
                sections.push(metricsItem);
            }
            
            return sections;
        } catch (error) {
            console.error('Error loading report sections:', error);
            return [];
        }
    }

    private getSectionItems(sectionItem: ReportItem): ReportItem[] {
        if (!sectionItem.reportData) {
            return [];
        }

        const data = sectionItem.reportData;
        
        switch (sectionItem.sectionType) {
            case 'files':
                return this.getFileItems(data.files || []);
            case 'issues':
                return this.getIssueItems(data.files || []);
            case 'metrics':
                return this.getMetricItems(data);
            default:
                return [];
        }
    }

    private getFileItems(files: any[]): ReportItem[] {
        return files.map(file => {
            const issueCount = file.issues?.length || 0;
            const fileName = path.basename(file.path || 'Unknown');
            
            const item = new ReportItem(
                fileName,
                vscode.TreeItemCollapsibleState.None,
                {
                    command: 'vscode.open',
                    title: 'Open File',
                    arguments: [vscode.Uri.file(file.path)]
                }
            );
            
            item.contextValue = 'file';
            item.tooltip = `${file.path}\nSize: ${file.size || 0} bytes\nIssues: ${issueCount}`;
            item.description = issueCount > 0 ? `${issueCount} issues` : '';
            item.iconPath = issueCount > 0 
                ? new vscode.ThemeIcon('warning', new vscode.ThemeColor('problemsWarningIcon.foreground'))
                : new vscode.ThemeIcon('file');
            
            return item;
        });
    }

    private getIssueItems(files: any[]): ReportItem[] {
        const issues: ReportItem[] = [];
        
        for (const file of files) {
            if (file.issues && file.issues.length > 0) {
                for (const issue of file.issues) {
                    const issueLabel = `${issue.type || 'Issue'}: ${issue.message || 'No description'}`;
                    const fileName = path.basename(file.path || 'Unknown');
                    
                    const item = new ReportItem(
                        issueLabel,
                        vscode.TreeItemCollapsibleState.None,
                        {
                            command: 'vscode.open',
                            title: 'Go to Issue',
                            arguments: [
                                vscode.Uri.file(file.path),
                                { selection: new vscode.Range(
                                    new vscode.Position((issue.line || 1) - 1, 0),
                                    new vscode.Position((issue.line || 1) - 1, 0)
                                )}
                            ]
                        }
                    );
                    
                    item.contextValue = 'issue';
                    item.tooltip = `${fileName}:${issue.line || 1}\n${issue.message || 'No description'}`;
                    item.description = `${fileName}:${issue.line || 1}`;
                    
                    // Set icon based on severity
                    const severity = issue.severity || 'info';
                    switch (severity) {
                        case 'error':
                            item.iconPath = new vscode.ThemeIcon('error', new vscode.ThemeColor('problemsErrorIcon.foreground'));
                            break;
                        case 'warning':
                            item.iconPath = new vscode.ThemeIcon('warning', new vscode.ThemeColor('problemsWarningIcon.foreground'));
                            break;
                        default:
                            item.iconPath = new vscode.ThemeIcon('info', new vscode.ThemeColor('problemsInfoIcon.foreground'));
                    }
                    
                    issues.push(item);
                }
            }
        }
        
        return issues;
    }

    private getMetricItems(data: any): ReportItem[] {
        const metrics: ReportItem[] = [];
        
        // Add basic file metrics
        const fileCount = data.files?.length || 0;
        const totalSize = data.files?.reduce((sum: number, file: any) => sum + (file.size || 0), 0) || 0;
        const avgComplexity = fileCount > 0 
            ? (data.files.reduce((sum: number, file: any) => sum + (file.complexity || 0), 0) / fileCount).toFixed(2)
            : '0.00';
        
        metrics.push(this.createMetricItem('Files Analyzed', fileCount.toString()));
        metrics.push(this.createMetricItem('Total Size', `${(totalSize / 1024).toFixed(1)} KB`));
        metrics.push(this.createMetricItem('Average Complexity', avgComplexity));
        
        // Add custom metrics if available
        if (data.metrics) {
            for (const [key, value] of Object.entries(data.metrics)) {
                if (typeof value === 'object') {
                    metrics.push(this.createMetricItem(key, JSON.stringify(value)));
                } else {
                    metrics.push(this.createMetricItem(key, value?.toString() || 'N/A'));
                }
            }
        }
        
        return metrics;
    }

    private createMetricItem(name: string, value: string): ReportItem {
        const item = new ReportItem(
            `${name}: ${value}`,
            vscode.TreeItemCollapsibleState.None
        );
        
        item.contextValue = 'metric';
        item.tooltip = `${name}: ${value}`;
        item.iconPath = new vscode.ThemeIcon('symbol-numeric');
        
        return item;
    }
}

export class ReportItem extends vscode.TreeItem {
    public sectionType?: string;
    public reportData?: any;

    constructor(
        public readonly label: string,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly command?: vscode.Command
    ) {
        super(label, collapsibleState);
    }
}