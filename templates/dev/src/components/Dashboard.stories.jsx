import React from 'react';
import { Dashboard } from './Dashboard';
import { TabPanel, TabBar } from './TabPanel';
import './Dashboard.css';
import './TabPanel.css';

export default {
  title: 'Components/Dashboard',
  component: Dashboard,
  parameters: {
    layout: 'padded',
    backgrounds: {
      default: 'dark',
    },
  },
  argTypes: {
    data: {
      control: 'object',
      description: 'Analysis data containing refactoring candidates',
    },
    initialMetric: {
      control: 'select',
      options: ['complexity', 'cognitive', 'debt', 'maintainability', 'structure', 'docs', 'coverage'],
      description: 'Initial color metric for the treemap',
    },
  },
};

// Sample candidate data matching the real analysis output structure
const sampleCandidates = [
  {
    entity_id: 'src/core/pipeline/pipeline_executor.rs::AnalysisPipeline',
    name: 'AnalysisPipeline',
    file_path: 'src/core/pipeline/pipeline_executor.rs',
    lines_of_code: 450,
    score: 72,
    issues: [
      {
        category: 'complexity',
        severity: 68,
        contributing_features: [
          { feature_name: 'cyclomatic_complexity', value: 25 },
          { feature_name: 'cognitive_complexity', value: 42 },
        ],
      },
      {
        category: 'maintainability',
        severity: 55,
        contributing_features: [
          { feature_name: 'maintainability_index', value: 45 },
        ],
      },
    ],
  },
  {
    entity_id: 'src/core/scoring.rs::FeatureNormalizer',
    name: 'FeatureNormalizer',
    file_path: 'src/core/scoring.rs',
    lines_of_code: 380,
    score: 65,
    issues: [
      {
        category: 'complexity',
        severity: 58,
        contributing_features: [
          { feature_name: 'cyclomatic_complexity', value: 18 },
        ],
      },
    ],
  },
  {
    entity_id: 'src/detectors/complexity/mod.rs::ComplexityDetector',
    name: 'ComplexityDetector',
    file_path: 'src/detectors/complexity/mod.rs',
    lines_of_code: 220,
    score: 45,
    issues: [
      {
        category: 'structure',
        severity: 42,
        contributing_features: [
          { feature_name: 'nesting_depth', value: 4 },
        ],
      },
    ],
  },
  {
    entity_id: 'src/detectors/graph/centrality.rs::CentralityAnalyzer',
    name: 'CentralityAnalyzer',
    file_path: 'src/detectors/graph/centrality.rs',
    lines_of_code: 180,
    score: 38,
    issues: [],
  },
  {
    entity_id: 'src/api/engine.rs::ValknutEngine',
    name: 'ValknutEngine',
    file_path: 'src/api/engine.rs',
    lines_of_code: 320,
    score: 52,
    issues: [
      {
        category: 'debt',
        severity: 48,
        contributing_features: [
          { feature_name: 'technical_debt_score', value: 35 },
        ],
      },
    ],
  },
  {
    entity_id: 'src/io/reports/html.rs::HtmlReporter',
    name: 'HtmlReporter',
    file_path: 'src/io/reports/html.rs',
    lines_of_code: 280,
    score: 58,
    issues: [
      {
        category: 'cognitive',
        severity: 55,
        contributing_features: [
          { feature_name: 'cognitive_complexity', value: 28 },
        ],
      },
    ],
  },
  {
    entity_id: 'src/lang/common/ast_utils.rs::AstParser',
    name: 'AstParser',
    file_path: 'src/lang/common/ast_utils.rs',
    lines_of_code: 150,
    score: 32,
    issues: [],
  },
  {
    entity_id: 'src/core/featureset.rs::FeatureVector',
    name: 'FeatureVector',
    file_path: 'src/core/featureset.rs',
    lines_of_code: 200,
    score: 28,
    issues: [],
  },
];

const fullDashboardData = {
  candidates: sampleCandidates,
  clone_pairs: [
    {
      source: { id: 'src/core/pipeline/pipeline_executor.rs::AnalysisPipeline' },
      target: { id: 'src/core/scoring.rs::FeatureNormalizer' },
      similarity: 0.72,
    },
  ],
  documentation: {
    file_doc_health: {
      'src/core/pipeline/pipeline_executor.rs': 65,
      'src/core/scoring.rs': 78,
      'src/api/engine.rs': 85,
    },
  },
};

// Minimal data
const minimalDashboardData = {
  candidates: [
    {
      entity_id: 'src/main.rs::main',
      name: 'main',
      file_path: 'src/main.rs',
      lines_of_code: 50,
      score: 15,
      issues: [],
    },
    {
      entity_id: 'src/lib.rs::init',
      name: 'init',
      file_path: 'src/lib.rs',
      lines_of_code: 30,
      score: 10,
      issues: [],
    },
  ],
};

// High severity scenario
const highSeverityData = {
  candidates: [
    {
      entity_id: 'src/legacy/monolith.rs::GodClass',
      name: 'GodClass',
      file_path: 'src/legacy/monolith.rs',
      lines_of_code: 2500,
      score: 95,
      issues: [
        {
          category: 'complexity',
          severity: 92,
          contributing_features: [
            { feature_name: 'cyclomatic_complexity', value: 85 },
            { feature_name: 'cognitive_complexity', value: 120 },
          ],
        },
        {
          category: 'maintainability',
          severity: 88,
          contributing_features: [
            { feature_name: 'maintainability_index', value: 12 },
          ],
        },
        {
          category: 'debt',
          severity: 85,
          contributing_features: [
            { feature_name: 'technical_debt_score', value: 78 },
          ],
        },
      ],
    },
    {
      entity_id: 'src/legacy/utils.rs::do_everything',
      name: 'do_everything',
      file_path: 'src/legacy/utils.rs',
      lines_of_code: 800,
      score: 82,
      issues: [
        {
          category: 'structure',
          severity: 78,
          contributing_features: [
            { feature_name: 'nesting_depth', value: 12 },
          ],
        },
      ],
    },
    {
      entity_id: 'src/legacy/handlers.rs::handle_request',
      name: 'handle_request',
      file_path: 'src/legacy/handlers.rs',
      lines_of_code: 600,
      score: 75,
      issues: [
        {
          category: 'cognitive',
          severity: 72,
          contributing_features: [
            { feature_name: 'cognitive_complexity', value: 65 },
          ],
        },
      ],
    },
  ],
};

// Large codebase scenario
const largeDashboardData = {
  candidates: [
    ...sampleCandidates,
    {
      entity_id: 'src/detectors/lsh/minhash.rs::MinHash',
      name: 'MinHash',
      file_path: 'src/detectors/lsh/minhash.rs',
      lines_of_code: 240,
      score: 42,
      issues: [],
    },
    {
      entity_id: 'src/detectors/refactoring/opportunities.rs::RefactoringFinder',
      name: 'RefactoringFinder',
      file_path: 'src/detectors/refactoring/opportunities.rs',
      lines_of_code: 310,
      score: 55,
      issues: [],
    },
    {
      entity_id: 'src/io/cache/mod.rs::CacheManager',
      name: 'CacheManager',
      file_path: 'src/io/cache/mod.rs',
      lines_of_code: 180,
      score: 35,
      issues: [],
    },
    {
      entity_id: 'src/bin/cli/args.rs::CliArgs',
      name: 'CliArgs',
      file_path: 'src/bin/cli/args.rs',
      lines_of_code: 120,
      score: 22,
      issues: [],
    },
    {
      entity_id: 'src/bin/cli/output.rs::OutputFormatter',
      name: 'OutputFormatter',
      file_path: 'src/bin/cli/output.rs',
      lines_of_code: 200,
      score: 38,
      issues: [],
    },
  ],
};

// Template with TabBar showing Dashboard as active tab
const Template = (args) => {
  const tabs = [
    { id: 'summary', title: 'Summary' },
    { id: 'oracle', title: 'Oracle' },
    { id: 'files', title: 'Files' },
    { id: 'dashboard', title: 'Dashboard' },
    { id: 'clones', title: 'Clones' },
  ];

  return (
    <div>
      <TabBar tabs={tabs} activeTab="dashboard" onTabChange={() => {}} />
      <Dashboard {...args} />
    </div>
  );
};

// Default story with full data
export const Default = Template.bind({});
Default.args = {
  data: fullDashboardData,
  initialMetric: 'complexity',
};

// Minimal data story
export const Minimal = Template.bind({});
Minimal.args = {
  data: minimalDashboardData,
  initialMetric: 'complexity',
};

// High severity codebase
export const HighSeverity = Template.bind({});
HighSeverity.args = {
  data: highSeverityData,
  initialMetric: 'complexity',
};

// Large codebase
export const LargeCodebase = Template.bind({});
LargeCodebase.args = {
  data: largeDashboardData,
  initialMetric: 'complexity',
};

// Different metrics
export const ColorByDebt = Template.bind({});
ColorByDebt.args = {
  data: fullDashboardData,
  initialMetric: 'debt',
};

export const ColorByMaintainability = Template.bind({});
ColorByMaintainability.args = {
  data: fullDashboardData,
  initialMetric: 'maintainability',
};

export const ColorByCognitive = Template.bind({});
ColorByCognitive.args = {
  data: fullDashboardData,
  initialMetric: 'cognitive',
};

// Empty state
export const Empty = Template.bind({});
Empty.args = {
  data: null,
  initialMetric: 'complexity',
};

// Interactive exploration
export const Interactive = () => {
  const tabs = [
    { id: 'summary', title: 'Summary' },
    { id: 'oracle', title: 'Oracle' },
    { id: 'files', title: 'Files' },
    { id: 'dashboard', title: 'Dashboard' },
    { id: 'clones', title: 'Clones' },
  ];

  return (
    <div>
      <TabBar tabs={tabs} activeTab="dashboard" onTabChange={() => {}} />
      <p style={{ color: 'var(--muted)', marginBottom: '1rem', fontSize: '0.875rem' }}>
        The treemap shows code entities sized by lines of code and colored by the selected severity metric.
        In the full HTML report, hovering shows detailed tooltips with issues and suggestions.
      </p>
      <Dashboard data={largeDashboardData} initialMetric="complexity" />
    </div>
  );
};
