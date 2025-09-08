# Valknut Template System

Valknut now supports customizable HTML report generation using the Handlebars template engine. This allows you to create beautiful, themed reports with full control over the presentation and styling.

## Overview

The template system consists of:
- **Handlebars templates** (`.hbs` files) for HTML structure
- **CSS themes** for styling and visual appearance  
- **Built-in themes** with professional designs
- **VS Code extension** for interactive report viewing

## Template Structure

### Default Template Location
- Templates: `templates/`
- Themes: `themes/`

### Template Engine
Valknut uses Handlebars for templating, providing:
- Variable interpolation: `{{variable}}`
- Conditionals: `{{#if condition}}...{{/if}}`
- Loops: `{{#each items}}...{{/each}}`
- Helpers for data formatting

## Available Templates

### Built-in Template: `report.hbs`
The default template provides:
- Responsive design
- Interactive file navigation
- Issue highlighting
- Semantic analysis display
- Metrics overview
- Raw data viewer

## Template Data Structure

Templates receive the following data:

```javascript
{
    "generated_at": "2024-01-15T10:30:00Z",
    "tool_name": "Valknut",
    "version": "0.1.0",
    "theme_css_url": "path/to/theme.css",
    "results": {
        "files": [
            {
                "path": "src/main.rs",
                "size": 1024,
                "language": "rust",
                "complexity": 3.2,
                "issues_count": 2,
                "issues": [
                    {
                        "type": "complexity",
                        "severity": "warning", 
                        "message": "High cyclomatic complexity",
                        "line": 45
                    }
                ]
            }
        ],
        "semantic_analysis": [
            {
                "name": "Code Quality",
                "score": 8.5,
                "suggestions": ["Reduce function complexity", "Add documentation"]
            }
        ],
        "metrics": [
            {
                "name": "Total Files",
                "value": 10,
                "description": "Number of analyzed files"
            }
        ]
    },
    "summary": {
        "total_files": 10,
        "total_issues": 5,
        "complexity_score": 4.2,
        "maintainability_index": 75
    }
}
```

## Available Themes

### Default Theme (`themes/default.css`)
- Clean, professional appearance
- Light background with blue accents
- Responsive design
- VS Code integration ready

### Dracula Theme (`themes/dracula.css`) 
- Dark theme with vibrant colors
- Dracula color palette
- Cyberpunk aesthetics
- Animated hover effects
- Glowing accents

### Creating Custom Themes

Create a new CSS file in the `themes/` directory:

```css
/* themes/my-theme.css */
:root {
    --primary-color: #your-color;
    --secondary-color: #your-secondary;
    --success-color: #your-success;
    --warning-color: #your-warning;
    --error-color: #your-error;
    --background-color: #your-background;
    --surface-color: #your-surface;
    --text-primary: #your-text;
    --text-secondary: #your-secondary-text;
    --border-color: #your-border;
}

/* Your custom styles here */
```

## Creating Custom Templates

### Basic Template Structure

Create a new `.hbs` file in the `templates/` directory:

```handlebars
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{tool_name}} Analysis Report</title>
    <link rel="stylesheet" href="{{theme_css_url}}">
</head>
<body>
    <div class="container">
        <header class="header">
            <h1>{{tool_name}} Analysis Report</h1>
            <div class="meta">
                Generated on {{generated_at}} | Version {{version}}
            </div>
        </header>
        
        <!-- Summary Section -->
        <section class="summary">
            {{#each summary}}
            <div class="summary-card">
                <div class="value">{{this}}</div>
                <div class="label">{{@key}}</div>
            </div>
            {{/each}}
        </section>
        
        <!-- Files Section -->
        <section class="files">
            <h2>Files</h2>
            {{#each results.files}}
            <div class="file-item" data-file-path="{{this.path}}">
                <div class="file-path">{{this.path}}</div>
                <div class="file-details">
                    Size: {{this.size}} bytes
                    {{#if this.complexity}}| Complexity: {{this.complexity}}{{/if}}
                </div>
                
                {{#if this.issues}}
                <div class="issues">
                    {{#each this.issues}}
                    <div class="issue {{this.severity}}" data-line="{{this.line}}">
                        <span class="type">{{this.type}}</span>
                        <span class="message">{{this.message}}</span>
                        {{#if this.line}}<span class="line">Line {{this.line}}</span>{{/if}}
                    </div>
                    {{/each}}
                </div>
                {{/if}}
            </div>
            {{/each}}
        </section>
    </div>
    
    <!-- VS Code Integration -->
    <script>
        const vscode = acquireVsCodeApi();
        
        document.querySelectorAll('.file-item').forEach(item => {
            item.addEventListener('click', function() {
                const filePath = this.dataset.filePath;
                vscode.postMessage({
                    command: 'openFile',
                    filePath: filePath
                });
            });
        });
        
        document.querySelectorAll('.issue').forEach(issue => {
            issue.addEventListener('click', function(e) {
                e.stopPropagation();
                const filePath = this.closest('.file-item').dataset.filePath;
                const line = this.dataset.line;
                vscode.postMessage({
                    command: 'openFile',
                    filePath: filePath,
                    line: parseInt(line)
                });
            });
        });
    </script>
</body>
</html>
```

### Template Helpers

Common Handlebars patterns for Valknut templates:

#### File Lists
```handlebars
{{#each results.files}}
<div class="file-item" data-file-path="{{this.path}}">
    <h3>{{this.path}}</h3>
    <p>{{this.size}} bytes</p>
</div>
{{/each}}
```

#### Conditional Content
```handlebars
{{#if results.semantic_analysis}}
<section class="semantic-analysis">
    <h2>Semantic Analysis</h2>
    {{#each results.semantic_analysis}}
    <div class="analysis-item">
        <h3>{{this.name}}</h3>
        <div class="score">{{this.score}}</div>
    </div>
    {{/each}}
</section>
{{/if}}
```

#### Issue Display
```handlebars
{{#each this.issues}}
<div class="issue issue--{{this.severity}}" data-line="{{this.line}}">
    <span class="issue__type">{{this.type}}</span>
    <span class="issue__message">{{this.message}}</span>
    {{#if this.line}}
    <span class="issue__location">Line {{this.line}}</span>
    {{/if}}
</div>
{{/each}}
```

## VS Code Integration

Templates must include VS Code integration for click-to-navigate functionality:

```javascript
<script>
    const vscode = acquireVsCodeApi();
    
    // File navigation
    document.querySelectorAll('[data-file-path]').forEach(element => {
        element.addEventListener('click', function() {
            const filePath = this.dataset.filePath;
            const line = this.dataset.line || 1;
            vscode.postMessage({
                command: 'openFile',
                filePath: filePath,
                line: parseInt(line)
            });
        });
    });
</script>
```

## Usage Examples

### Generate HTML Report with Custom Template
```bash
valknut analyze ./src --format html --template custom-template --theme dracula
```

### Using in Rust Code
```rust
use valknut_rs::io::reports::{ReportGenerator, ReportError};

let generator = ReportGenerator::new()
    .with_templates_dir("./templates")?;

generator.generate_report(
    &analysis_results,
    "report.html",
    ReportFormat::Html
)?;
```

### VS Code Integration
1. Install the Valknut VS Code extension
2. Open a Valknut report JSON file
3. Right-click and select "Open Valknut Report"
4. The report opens with click-to-navigate functionality

## Best Practices

### Template Design
- Keep templates responsive and accessible
- Use semantic HTML structure
- Include proper meta tags and viewport settings
- Test with different data sizes

### Theme Development  
- Use CSS custom properties for easy customization
- Support both light and dark themes
- Ensure sufficient color contrast
- Test with colorblind-friendly palettes

### Performance
- Minimize CSS and JavaScript
- Use efficient Handlebars patterns
- Avoid deeply nested loops
- Consider file size for large reports

### Accessibility
- Include proper ARIA labels
- Support keyboard navigation  
- Use sufficient color contrast ratios
- Test with screen readers

## Troubleshooting

### Template Not Loading
- Check file path and permissions
- Verify Handlebars syntax
- Check for missing template variables

### Theme Not Applied
- Verify CSS file exists and is valid
- Check CSS custom property names
- Ensure theme is correctly linked

### VS Code Integration Issues
- Verify `acquireVsCodeApi()` is called
- Check message format and commands
- Test click handlers and event listeners

For more examples and advanced usage, see the `examples/` directory in the Valknut repository.