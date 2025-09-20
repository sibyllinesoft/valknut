import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';

export interface AnalyzeOptions {
    onProgress?: (message: string) => void;
    cancellationToken?: vscode.CancellationToken;
}

export class ValknutAnalyzer {
    private config = vscode.workspace.getConfiguration('valknut');

    async analyzeWorkspace(workspacePath: string, options: AnalyzeOptions = {}): Promise<string | null> {
        const executablePath = this.config.get<string>('executablePath', 'valknut');
        const reportDir = path.join(workspacePath, 'reports');
        const reportFile = path.join(reportDir, `valknut-report-${Date.now()}.json`);

        // Ensure reports directory exists
        if (!fs.existsSync(reportDir)) {
            fs.mkdirSync(reportDir, { recursive: true });
        }

        return new Promise((resolve, reject) => {
            options.onProgress?.('Starting Valknut analysis...');

            const args = [
                'analyze',
                workspacePath,
                '--output', reportFile,
                '--format', 'json'
            ];

            const process = cp.spawn(executablePath, args, {
                cwd: workspacePath,
                stdio: ['ignore', 'pipe', 'pipe']
            });

            let stdout = '';
            let stderr = '';

            process.stdout?.on('data', (data: Buffer) => {
                const output = data.toString();
                stdout += output;
                
                // Parse progress messages if any
                const lines = output.split('\n').filter(line => line.trim());
                for (const line of lines) {
                    if (line.includes('Analyzing') || line.includes('Processing')) {
                        options.onProgress?.(line.trim());
                    }
                }
            });

            process.stderr?.on('data', (data: Buffer) => {
                stderr += data.toString();
            });

            process.on('close', (code) => {
                if (options.cancellationToken?.isCancellationRequested) {
                    reject(new Error('Analysis cancelled'));
                    return;
                }

                if (code === 0) {
                    // Check if report file was created
                    if (fs.existsSync(reportFile)) {
                        options.onProgress?.('Analysis completed successfully');
                        resolve(reportFile);
                    } else {
                        reject(new Error('Analysis completed but no report was generated'));
                    }
                } else {
                    reject(new Error(`Analysis failed with code ${code}: ${stderr || stdout}`));
                }
            });

            process.on('error', (error) => {
                reject(new Error(`Failed to start Valknut: ${error.message}`));
            });

            // Handle cancellation
            options.cancellationToken?.onCancellationRequested(() => {
                process.kill();
            });
        });
    }

    async exportHtmlReport(destination: string, reportPath?: string): Promise<void> {
        const config = vscode.workspace.getConfiguration('valknut');
        const executablePath = config.get<string>('executablePath', 'valknut');

        const workspaceRoot =
            vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ??
            (reportPath ? path.dirname(reportPath) : undefined);

        if (!workspaceRoot) {
            throw new Error('Unable to resolve a workspace folder for export.');
        }

        const tempDir = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'valknut-export-'));
        const args = [
            'analyze',
            workspaceRoot,
            '--out',
            tempDir,
            '--format',
            'html',
            '--quiet',
        ];

        try {
            await this.runValknutProcess(executablePath, args, workspaceRoot);

            const htmlFile = await this.findNewestHtml(tempDir);
            if (!htmlFile) {
                throw new Error('Valknut did not produce an HTML report.');
            }

            const destinationDir = path.dirname(destination);
            await fs.promises.mkdir(destinationDir, { recursive: true });
            await fs.promises.copyFile(htmlFile, destination);

            const entries = await fs.promises.readdir(tempDir);
            for (const entry of entries) {
                const sourcePath = path.join(tempDir, entry);
                if (path.resolve(sourcePath) === path.resolve(htmlFile)) {
                    continue;
                }
                const targetPath = path.join(destinationDir, entry);
                await this.copyRecursive(sourcePath, targetPath);
            }
        } finally {
            await fs.promises.rm(tempDir, { recursive: true, force: true }).catch(() => undefined);
        }
    }

    async analyzeFile(filePath: string, options: AnalyzeOptions = {}): Promise<string | null> {
        const executablePath = this.config.get<string>('executablePath', 'valknut');
        const workspaceRoot = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(filePath))?.uri.fsPath;
        
        if (!workspaceRoot) {
            throw new Error('File is not in a workspace');
        }

        const reportDir = path.join(workspaceRoot, 'reports');
        const reportFile = path.join(reportDir, `valknut-file-${Date.now()}.json`);

        // Ensure reports directory exists
        if (!fs.existsSync(reportDir)) {
            fs.mkdirSync(reportDir, { recursive: true });
        }

        return new Promise((resolve, reject) => {
            options.onProgress?.('Analyzing file...');

            const args = [
                'analyze',
                filePath,
                '--output', reportFile,
                '--format', 'json'
            ];

            const process = cp.spawn(executablePath, args, {
                cwd: workspaceRoot,
                stdio: ['ignore', 'pipe', 'pipe']
            });

            let stdout = '';
            let stderr = '';

            process.stdout?.on('data', (data: Buffer) => {
                stdout += data.toString();
            });

            process.stderr?.on('data', (data: Buffer) => {
                stderr += data.toString();
            });

            process.on('close', (code) => {
                if (options.cancellationToken?.isCancellationRequested) {
                    reject(new Error('Analysis cancelled'));
                    return;
                }

                if (code === 0) {
                    if (fs.existsSync(reportFile)) {
                        options.onProgress?.('File analysis completed');
                        resolve(reportFile);
                    } else {
                        reject(new Error('Analysis completed but no report was generated'));
                    }
                } else {
                    reject(new Error(`Analysis failed with code ${code}: ${stderr || stdout}`));
                }
            });

            process.on('error', (error) => {
                if (error.message.includes('ENOENT')) {
                    reject(new Error(`Valknut executable not found at '${executablePath}'. Please check the 'valknut.executablePath' setting.`));
                } else {
                    reject(new Error(`Failed to start Valknut: ${error.message}`));
                }
            });

            // Handle cancellation
            options.cancellationToken?.onCancellationRequested(() => {
                process.kill();
            });
        });
    }

    async isValknutAvailable(): Promise<boolean> {
        const executablePath = this.config.get<string>('executablePath', 'valknut');

        return new Promise((resolve) => {
            const process = cp.spawn(executablePath, ['--version'], {
                stdio: 'ignore'
            });

            process.on('close', (code) => {
                resolve(code === 0);
            });

            process.on('error', () => {
                resolve(false);
            });
        });
    }

    async getVersion(): Promise<string | null> {
        const executablePath = this.config.get<string>('executablePath', 'valknut');

        return new Promise((resolve) => {
            const process = cp.spawn(executablePath, ['--version'], {
                stdio: ['ignore', 'pipe', 'ignore']
            });

            let version = '';
            process.stdout?.on('data', (data: Buffer) => {
                version += data.toString().trim();
            });

            process.on('close', (code) => {
                if (code === 0 && version) {
                    resolve(version);
                } else {
                    resolve(null);
                }
            });

            process.on('error', () => {
                resolve(null);
            });
        });
    }

    private runValknutProcess(executablePath: string, args: string[], cwd: string): Promise<void> {
        return new Promise((resolve, reject) => {
            const child = cp.spawn(executablePath, args, {
                cwd,
                stdio: ['ignore', 'pipe', 'pipe'],
            });

            let stderr = '';
            child.stderr?.on('data', (data: Buffer) => {
                stderr += data.toString();
            });

            child.on('error', (error) => {
                reject(
                    error.message.includes('ENOENT')
                        ? new Error(`Valknut executable not found at '${executablePath}'.`) : error,
                );
            });

            child.on('close', (code) => {
                if (code === 0) {
                    resolve();
                } else {
                    reject(new Error(`Valknut export failed with code ${code}: ${stderr.trim()}`));
                }
            });
        });
    }

    private async findNewestHtml(tempDir: string): Promise<string | null> {
        const entries = await fs.promises.readdir(tempDir);
        const htmlFiles: { path: string; mtime: number }[] = [];

        for (const entry of entries) {
            if (!entry.endsWith('.html')) {
                continue;
            }

            const fullPath = path.join(tempDir, entry);
            const stats = await fs.promises.stat(fullPath);
            htmlFiles.push({ path: fullPath, mtime: stats.mtimeMs });
        }

        if (htmlFiles.length === 0) {
            return null;
        }

        htmlFiles.sort((a, b) => b.mtime - a.mtime);
        return htmlFiles[0].path;
    }

    private async copyRecursive(source: string, destination: string): Promise<void> {
        const stats = await fs.promises.stat(source);

        if (stats.isDirectory()) {
            await fs.promises.mkdir(destination, { recursive: true });
            const children = await fs.promises.readdir(source);
            for (const child of children) {
                await this.copyRecursive(
                    path.join(source, child),
                    path.join(destination, child),
                );
            }
            return;
        }

        await fs.promises.mkdir(path.dirname(destination), { recursive: true });
        await fs.promises.copyFile(source, destination);
    }
}
