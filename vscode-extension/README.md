# Valknut VS Code Extension

A VS Code extension for viewing and navigating Valknut code analysis reports with interactive file navigation.

## Features

- **Interactive Report Viewing**: View Valknut analysis reports in a beautiful, themed interface
- **Click-to-Navigate**: Click on files and issues to jump directly to the source code
- **Theme Support**: Choose from multiple themes including Default, Dracula, and High Contrast
- **Report Management**: Browse and manage multiple reports through the integrated tree view
- **Workspace Integration**: Analyze your current workspace directly from VS Code
- **Auto-refresh**: Automatically refresh reports when files change

## Installation

### From VSIX (Development)

1. Build the extension: `npm run compile`
2. Package the extension: `vsce package`
3. Install the generated `.vsix` file in VS Code

### Requirements

- VS Code 1.74.0 or higher
- Valknut CLI tool installed and accessible in your PATH

## Usage

### Opening Reports

1. **Command Palette**: Open the command palette (`Ctrl+Shift+P`) and run "Valknut: Open Report"
2. **Context Menu**: Right-click on a JSON file and select "Open Valknut Report" (if it's a valid report)
3. **Tree View**: Use the Valknut Reports tree view in the Explorer panel

### Analyzing Code

1. **Current Workspace**: Run "Valknut: Analyze Current Workspace" from the command palette
2. **Context Menu**: Right-click on a folder in the Explorer and select "Analyze with Valknut"

### Navigation

- **Click on file paths** to open the file in the editor
- **Click on issue items** to jump to the specific line with the issue
- Use the **toolbar buttons** to refresh or export reports

## Configuration

Configure the extension through VS Code settings:

```json
{
    "valknut.reportPath": "/path/to/your/reports",
    "valknut.executablePath": "valknut",
    "valknut.theme": "dracula",
    "valknut.autoRefresh": true,
    "valknut.showLineNumbers": true,
    "valknut.maxFilePreview": 50
}
```

### Settings

- `valknut.reportPath`: Directory containing Valknut reports
- `valknut.executablePath`: Path to the Valknut executable
- `valknut.theme`: Report theme (`default`, `dracula`, `high-contrast`)
- `valknut.autoRefresh`: Auto-refresh reports when files change
- `valknut.showLineNumbers`: Show line numbers in reports
- `valknut.maxFilePreview`: Maximum files to preview in reports

## Available Themes

### Default Theme
Clean, professional theme with blue accents and light background.

### Dracula Theme
Dark theme with vibrant colors inspired by the Dracula color scheme.

### High Contrast Theme
Accessibility-focused theme with high contrast colors.

## Commands

- `valknut.openReport`: Open a Valknut report
- `valknut.analyzeWorkspace`: Analyze the current workspace
- `valknut.refreshReport`: Refresh the current report
- `valknut.exportReport`: Export report to HTML

## Report Format

The extension supports Valknut reports in JSON format with the following structure:

```json
{
    "files": [
        {
            "path": "src/main.rs",
            "size": 1024,
            "language": "rust",
            "complexity": 3.2,
            "issues": [
                {
                    "type": "complexity",
                    "severity": "warning",
                    "message": "Function has high complexity",
                    "line": 45
                }
            ]
        }
    ],
    "metrics": {
        "total_files": 10,
        "total_lines": 1500
    }
}
```

## Development

### Building

```bash
npm install
npm run compile
```

### Debugging

1. Open the project in VS Code
2. Press F5 to start debugging
3. A new Extension Development Host window will open

### Packaging

```bash
npm install -g vsce
vsce package
```

## License

MIT License - see LICENSE file for details