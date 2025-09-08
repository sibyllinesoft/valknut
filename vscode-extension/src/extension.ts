import * as vscode from 'vscode';
import { ReportPanel } from './reportPanel';
import { ReportProvider } from './reportProvider';
import { ValknutAnalyzer } from './analyzer';

export function activate(context: vscode.ExtensionContext) {
    console.log('Valknut extension is now active!');

    // Initialize report provider
    const reportProvider = new ReportProvider(context);
    
    // Register tree data provider
    vscode.window.registerTreeDataProvider('valknutReports', reportProvider);

    // Initialize analyzer
    const analyzer = new ValknutAnalyzer();

    // Command to open report
    const openReportCommand = vscode.commands.registerCommand('valknut.openReport', async (uri?: vscode.Uri) => {
        try {
            let reportPath: string;
            
            if (uri) {
                reportPath = uri.fsPath;
            } else {
                // Show file picker
                const result = await vscode.window.showOpenDialog({
                    canSelectFiles: true,
                    canSelectFolders: false,
                    canSelectMany: false,
                    filters: {
                        'JSON files': ['json'],
                        'All files': ['*']
                    },
                    title: 'Select Valknut Report'
                });

                if (!result || result.length === 0) {
                    return;
                }
                
                reportPath = result[0].fsPath;
            }

            await ReportPanel.createOrShow(context.extensionUri, reportPath);
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to open report: ${error}`);
        }
    });

    // Command to analyze current workspace
    const analyzeWorkspaceCommand = vscode.commands.registerCommand('valknut.analyzeWorkspace', async () => {
        try {
            if (!vscode.workspace.workspaceFolders || vscode.workspace.workspaceFolders.length === 0) {
                vscode.window.showWarningMessage('No workspace folder open');
                return;
            }

            const workspaceRoot = vscode.workspace.workspaceFolders[0].uri.fsPath;
            
            await vscode.window.withProgress({
                location: vscode.ProgressLocation.Notification,
                title: "Analyzing workspace with Valknut",
                cancellable: true
            }, async (progress, token) => {
                try {
                    progress.report({ message: "Running analysis..." });
                    
                    const reportPath = await analyzer.analyzeWorkspace(workspaceRoot, {
                        onProgress: (message: string) => {
                            progress.report({ message });
                        },
                        cancellationToken: token
                    });

                    if (reportPath) {
                        progress.report({ message: "Opening report..." });
                        await ReportPanel.createOrShow(context.extensionUri, reportPath);
                        reportProvider.refresh();
                    }
                } catch (error) {
                    if (error instanceof Error && error.message.includes('cancelled')) {
                        vscode.window.showInformationMessage('Analysis cancelled');
                    } else {
                        throw error;
                    }
                }
            });
        } catch (error) {
            vscode.window.showErrorMessage(`Analysis failed: ${error}`);
        }
    });

    // Command to refresh report
    const refreshReportCommand = vscode.commands.registerCommand('valknut.refreshReport', () => {
        reportProvider.refresh();
        ReportPanel.refresh();
    });

    // Command to export report
    const exportReportCommand = vscode.commands.registerCommand('valknut.exportReport', async () => {
        if (!ReportPanel.currentPanel) {
            vscode.window.showWarningMessage('No report currently open');
            return;
        }

        try {
            const result = await vscode.window.showSaveDialog({
                filters: {
                    'HTML files': ['html'],
                    'JSON files': ['json'],
                    'All files': ['*']
                },
                defaultUri: vscode.Uri.file('valknut-report.html')
            });

            if (result) {
                await ReportPanel.currentPanel.exportReport(result.fsPath);
                vscode.window.showInformationMessage('Report exported successfully');
            }
        } catch (error) {
            vscode.window.showErrorMessage(`Export failed: ${error}`);
        }
    });

    // Register all commands
    context.subscriptions.push(
        openReportCommand,
        analyzeWorkspaceCommand,
        refreshReportCommand,
        exportReportCommand
    );

    // Watch for configuration changes
    const configWatcher = vscode.workspace.onDidChangeConfiguration(e => {
        if (e.affectsConfiguration('valknut')) {
            ReportPanel.updateConfiguration();
        }
    });

    context.subscriptions.push(configWatcher);

    // Auto-refresh on file changes if enabled
    const config = vscode.workspace.getConfiguration('valknut');
    if (config.get('autoRefresh', true)) {
        const fileWatcher = vscode.workspace.createFileSystemWatcher('**/*.json');
        
        fileWatcher.onDidChange((uri) => {
            if (uri.fsPath.includes('valknut') || uri.fsPath.includes('report')) {
                reportProvider.refresh();
            }
        });

        context.subscriptions.push(fileWatcher);
    }

    // Set context for when reports are available
    reportProvider.onDidChangeTreeData(() => {
        vscode.commands.executeCommand('setContext', 'valknut.hasReports', reportProvider.hasReports());
    });
}

export function deactivate() {
    ReportPanel.dispose();
}