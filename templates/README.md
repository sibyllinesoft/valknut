
# Valknut HTML Report Templates

This directory contains the Handlebars templates for generating HTML reports, along with assets and development tools.

## Structure

- **Root templates**: Main report templates (.hbs files)
  - `report.hbs`: Primary HTML report template
  - `csv_report.hbs`, `markdown_report.hbs`, `sonar_report.hbs`: Alternative format templates

- **partials/**: Reusable Handlebars partials
  - `head.hbs`, `header.hbs`, `footer.hbs`: Layout components
  - `summary.hbs`, `oracle.hbs`, `coverage.hbs`, `tree.hbs`: Content sections

- **assets/**: Built JavaScript bundles for interactive components
  - `react-tree-bundle.js`: Production bundle for the code analysis tree
  - `react-tree-bundle.debug.js`: Debug version for development

- **dev/**: Development and testing resources
  - `package.json`, `webpack.config.cjs`: Build configuration
  - `src/`: Source code for React tree component
  - `tests/`: Unit, integration, and E2E tests
  - Debug files, reports, and documentation

## Building the Bundle

To rebuild the JS bundle:

1. cd templates/dev
2. bun install
3. bun run build:bundle

This generates `react-tree-bundle.js`