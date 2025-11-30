import React, { useState } from 'react';
import { Coverage } from './Coverage';
import { TabPanel, TabBar } from './TabPanel';
import './Coverage.css';
import './TabPanel.css';

export default {
  title: 'Components/Coverage',
  component: Coverage,
  parameters: {
    layout: 'padded',
    backgrounds: {
      default: 'dark',
    },
  },
  argTypes: {
    data: {
      control: 'object',
      description: 'Coverage packs data containing gaps, file info, and priority analysis',
    },
  },
};

// Sample coverage data matching the Rust CoveragePack structure
const fullCoverageData = [
  {
    path: 'src/core/pipeline/pipeline_executor.rs',
    file_info: {
      loc: 485,
      coverage_before: 0.42,
      coverage_after_if_filled: 0.78,
    },
    value: {
      file_cov_gain: 0.36,
      repo_cov_gain_est: 0.024,
    },
    effort: {
      tests_to_write_est: 12,
      mocks_est: 3,
    },
    gaps: [
      {
        span: { start: 124, end: 156 },
        score: 0.92,
        features: {
          gap_loc: 32,
          cyclomatic_in_gap: 8.5,
          fan_in_gap: 5,
          exports_touched: true,
        },
        symbols: [
          {
            kind: 'fn',
            name: 'execute_stage',
            signature: 'fn execute_stage(&mut self, stage: AnalysisStage) -> Result<StageOutput>',
          },
          {
            kind: 'fn',
            name: 'collect_results',
            signature: 'fn collect_results(&self) -> PipelineResults',
          },
        ],
        preview: {
          head: [
            'pub async fn execute_stage(&mut self, stage: AnalysisStage) -> Result<StageOutput> {',
            '    let config = self.config.clone();',
            '    match stage {',
            '        AnalysisStage::Discovery => self.run_discovery(&config).await,',
            '        AnalysisStage::Extraction => self.run_extraction(&config).await,',
          ],
        },
      },
      {
        span: { start: 280, end: 310 },
        score: 0.75,
        features: {
          gap_loc: 30,
          cyclomatic_in_gap: 5.2,
          fan_in_gap: 2,
          exports_touched: false,
        },
        symbols: [
          {
            kind: 'fn',
            name: 'aggregate_features',
            signature: 'fn aggregate_features(&self, vectors: Vec<FeatureVector>) -> AggregatedFeatures',
          },
        ],
        preview: {
          head: [
            'fn aggregate_features(&self, vectors: Vec<FeatureVector>) -> AggregatedFeatures {',
            '    let mut aggregated = AggregatedFeatures::default();',
            '    for vector in vectors {',
            '        aggregated.merge(vector);',
            '    }',
          ],
        },
      },
    ],
  },
  {
    path: 'src/detectors/complexity/analyzer.rs',
    file_info: {
      loc: 312,
      coverage_before: 0.55,
      coverage_after_if_filled: 0.88,
    },
    value: {
      file_cov_gain: 0.33,
      repo_cov_gain_est: 0.018,
    },
    effort: {
      tests_to_write_est: 8,
      mocks_est: 1,
    },
    gaps: [
      {
        span: { start: 45, end: 89 },
        score: 0.88,
        features: {
          gap_loc: 44,
          cyclomatic_in_gap: 12.3,
          fan_in_gap: 8,
          exports_touched: true,
        },
        symbols: [
          {
            kind: 'fn',
            name: 'calculate_cognitive_complexity',
            signature: 'pub fn calculate_cognitive_complexity(&self, ast: &SyntaxTree) -> f64',
          },
        ],
        preview: {
          head: [
            'pub fn calculate_cognitive_complexity(&self, ast: &SyntaxTree) -> f64 {',
            '    let mut complexity = 0.0;',
            '    let mut nesting_level = 0;',
            '    ',
            '    for node in ast.walk() {',
          ],
        },
      },
    ],
  },
];

// Minimal data for testing edge cases
const minimalCoverageData = [
  {
    path: 'src/lib.rs',
    file_info: {
      loc: 45,
      coverage_before: 0.85,
      coverage_after_if_filled: 0.95,
    },
    value: {
      file_cov_gain: 0.10,
      repo_cov_gain_est: 0.002,
    },
    effort: {
      tests_to_write_est: 2,
      mocks_est: 0,
    },
    gaps: [
      {
        span: { start: 30, end: 38 },
        score: 0.45,
        features: {
          gap_loc: 8,
          cyclomatic_in_gap: 2.0,
          fan_in_gap: 1,
          exports_touched: false,
        },
        symbols: [],
        preview: {
          head: [
            'fn init_logging() {',
            '    if std::env::var("RUST_LOG").is_err() {',
            '        std::env::set_var("RUST_LOG", "info");',
            '    }',
            '}',
          ],
        },
      },
    ],
  },
];

// High priority coverage gaps
const highPriorityCoverageData = [
  {
    path: 'src/api/engine.rs',
    file_info: {
      loc: 620,
      coverage_before: 0.28,
      coverage_after_if_filled: 0.72,
    },
    value: {
      file_cov_gain: 0.44,
      repo_cov_gain_est: 0.052,
    },
    effort: {
      tests_to_write_est: 18,
      mocks_est: 6,
    },
    gaps: [
      {
        span: { start: 85, end: 145 },
        score: 0.98,
        features: {
          gap_loc: 60,
          cyclomatic_in_gap: 15.8,
          fan_in_gap: 12,
          exports_touched: true,
        },
        symbols: [
          {
            kind: 'fn',
            name: 'analyze_directory',
            signature: 'pub async fn analyze_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<AnalysisResults>',
          },
          {
            kind: 'fn',
            name: 'validate_config',
            signature: 'fn validate_config(&self) -> Result<()>',
          },
          {
            kind: 'fn',
            name: 'prepare_pipeline',
            signature: 'async fn prepare_pipeline(&mut self) -> Result<AnalysisPipeline>',
          },
        ],
        preview: {
          head: [
            'pub async fn analyze_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<AnalysisResults> {',
            '    self.validate_config()?;',
            '    let pipeline = self.prepare_pipeline().await?;',
            '    let files = pipeline.discover_files(path.as_ref()).await?;',
            '    ',
          ],
        },
      },
      {
        span: { start: 220, end: 280 },
        score: 0.85,
        features: {
          gap_loc: 60,
          cyclomatic_in_gap: 9.2,
          fan_in_gap: 4,
          exports_touched: true,
        },
        symbols: [
          {
            kind: 'fn',
            name: 'process_results',
            signature: 'async fn process_results(&self, raw: RawResults) -> Result<AnalysisResults>',
          },
        ],
        preview: {
          head: [
            'async fn process_results(&self, raw: RawResults) -> Result<AnalysisResults> {',
            '    let normalized = self.normalizer.normalize(raw.features)?;',
            '    let scored = self.scorer.score(&normalized)?;',
            '    ',
            '    Ok(AnalysisResults {',
          ],
        },
      },
      {
        span: { start: 380, end: 420 },
        score: 0.72,
        features: {
          gap_loc: 40,
          cyclomatic_in_gap: 6.5,
          fan_in_gap: 3,
          exports_touched: false,
        },
        symbols: [
          {
            kind: 'fn',
            name: 'handle_error',
            signature: 'fn handle_error(&self, err: ValknutError) -> Result<()>',
          },
        ],
        preview: {
          head: [
            'fn handle_error(&self, err: ValknutError) -> Result<()> {',
            '    match err.kind() {',
            '        ErrorKind::Validation => Err(err),',
            '        ErrorKind::IO => {',
            '            log::warn!("IO error: {}", err);',
          ],
        },
      },
    ],
  },
];

// Many files with coverage gaps
const manyCoveragePacksData = [
  ...fullCoverageData,
  {
    path: 'src/io/reports/generator.rs',
    file_info: {
      loc: 280,
      coverage_before: 0.62,
      coverage_after_if_filled: 0.85,
    },
    value: {
      file_cov_gain: 0.23,
      repo_cov_gain_est: 0.012,
    },
    effort: {
      tests_to_write_est: 6,
      mocks_est: 2,
    },
    gaps: [
      {
        span: { start: 100, end: 130 },
        score: 0.68,
        features: {
          gap_loc: 30,
          cyclomatic_in_gap: 4.5,
          fan_in_gap: 2,
          exports_touched: true,
        },
        symbols: [
          {
            kind: 'fn',
            name: 'generate_html',
            signature: 'pub fn generate_html(&self, results: &AnalysisResults) -> Result<String>',
          },
        ],
        preview: {
          head: [
            'pub fn generate_html(&self, results: &AnalysisResults) -> Result<String> {',
            '    let template = self.load_template("report.hbs")?;',
            '    let context = self.build_context(results)?;',
            '    handlebars.render(&template, &context)',
          ],
        },
      },
    ],
  },
  {
    path: 'src/lang/rust_lang.rs',
    file_info: {
      loc: 450,
      coverage_before: 0.48,
      coverage_after_if_filled: 0.75,
    },
    value: {
      file_cov_gain: 0.27,
      repo_cov_gain_est: 0.015,
    },
    effort: {
      tests_to_write_est: 10,
      mocks_est: 1,
    },
    gaps: [
      {
        span: { start: 180, end: 220 },
        score: 0.82,
        features: {
          gap_loc: 40,
          cyclomatic_in_gap: 11.0,
          fan_in_gap: 6,
          exports_touched: true,
        },
        symbols: [
          {
            kind: 'fn',
            name: 'extract_entities',
            signature: 'fn extract_entities(&self, source: &str) -> Vec<CodeEntity>',
          },
        ],
        preview: {
          head: [
            'fn extract_entities(&self, source: &str) -> Vec<CodeEntity> {',
            '    let tree = self.parser.parse(source, None)?;',
            '    let mut entities = Vec::new();',
            '    ',
            '    for node in tree.root_node().walk() {',
          ],
        },
      },
    ],
  },
];

const tabs = [
  { id: 'summary', title: 'Summary' },
  { id: 'oracle', title: 'Oracle' },
  { id: 'coverage', title: 'Coverage' },
  { id: 'files', title: 'Files' },
  { id: 'dashboard', title: 'Dashboard' },
  { id: 'clones', title: 'Clones' },
];

// Interactive template with working tab switching
const InteractiveTemplate = ({ coverageData }) => {
  const [activeTab, setActiveTab] = useState('coverage');

  return (
    <div>
      <TabBar tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab} />

      <TabPanel id="summary" title="Summary" active={activeTab === 'summary'}>
        <div style={{ padding: '1rem', color: '#e5e7eb', maxHeight: 'calc(100vh - 128px)', overflow: 'auto' }}>
          <p>Summary content would go here.</p>
        </div>
      </TabPanel>

      <TabPanel id="oracle" title="Oracle" active={activeTab === 'oracle'}>
        <div style={{ padding: '1rem', color: '#e5e7eb', maxHeight: 'calc(100vh - 128px)', overflow: 'auto' }}>
          <p>Oracle content would go here.</p>
        </div>
      </TabPanel>

      <TabPanel id="coverage" title="Coverage" active={activeTab === 'coverage'}>
        <Coverage data={coverageData} />
      </TabPanel>

      <TabPanel id="files" title="Files" active={activeTab === 'files'}>
        <div style={{ padding: '1rem', color: '#e5e7eb', maxHeight: 'calc(100vh - 128px)', overflow: 'auto' }}>
          <p>Files content would go here.</p>
        </div>
      </TabPanel>

      <TabPanel id="dashboard" title="Dashboard" active={activeTab === 'dashboard'}>
        <div style={{ padding: '1rem', color: '#e5e7eb', maxHeight: 'calc(100vh - 128px)', overflow: 'auto' }}>
          <p>Dashboard content would go here.</p>
        </div>
      </TabPanel>

      <TabPanel id="clones" title="Clones" active={activeTab === 'clones'}>
        <div style={{ padding: '1rem', color: '#e5e7eb', maxHeight: 'calc(100vh - 128px)', overflow: 'auto' }}>
          <p>Clones content would go here.</p>
        </div>
      </TabPanel>
    </div>
  );
};

// Default story with full data - interactive tab switching
export const Default = () => <InteractiveTemplate coverageData={fullCoverageData} />;

// Minimal coverage data
export const Minimal = () => <InteractiveTemplate coverageData={minimalCoverageData} />;

// High priority scenario
export const HighPriority = () => <InteractiveTemplate coverageData={highPriorityCoverageData} />;

// Many coverage packs
export const ManyPacks = () => <InteractiveTemplate coverageData={manyCoveragePacksData} />;

// Empty/No data
export const Empty = () => <InteractiveTemplate coverageData={null} />;

// Component only (without tab context) for isolated testing
export const ComponentOnly = () => (
  <div style={{ maxHeight: 'calc(100vh - 128px)', overflow: 'auto' }}>
    <Coverage data={fullCoverageData} />
  </div>
);
