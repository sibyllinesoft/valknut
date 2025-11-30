import React, { useState } from 'react';
import { Oracle } from './Oracle';
import { TabPanel, TabBar } from './TabPanel';
import './Oracle.css';
import './TabPanel.css';

export default {
  title: 'Components/Oracle',
  component: Oracle,
  parameters: {
    layout: 'padded',
    backgrounds: {
      default: 'dark',
    },
  },
  argTypes: {
    data: {
      control: 'object',
      description: 'Oracle response data containing assessment, refactoring roadmap, and risk assessment',
    },
  },
};

// Sample data matching the new RefactoringOracleResponse Rust struct
const fullOracleData = {
  assessment: {
    architectural_narrative:
      'The codebase follows a well-structured pipeline architecture with clear separation between API, core logic, and I/O operations. The detector subsystem demonstrates good plugin-like extensibility, though the configuration system has grown organically and could benefit from decomposition.',
    architectural_style: 'Pipeline Architecture with Modular Detector Plugins',
    issues: [
      'Config complexity',
      'Implicit stage dependencies',
      'Scattered result aggregation',
    ],
  },
  refactoring_roadmap: {
    tasks: [
      {
        id: 'task-1',
        title: 'Introduce Configuration Domain Modules',
        description:
          'Split the monolithic ValknutConfig into domain-specific configuration structs. Each domain owns its validation logic.',
        category: 'structure',
        files: ['src/core/config.rs', 'src/api/config_types.rs'],
        risk_level: 'medium',
        impact: 'high',
        effort: 'medium',
        mitigation: 'Maintain backward compatibility with deprecation warnings',
        required: true,
        depends_on: [],
      },
      {
        id: 'task-2',
        title: 'Unify Detector Trait Interface',
        description:
          'Create a standardized Detector trait with default implementations for common functionality.',
        category: 'abstraction',
        files: ['src/detectors/mod.rs'],
        risk_level: 'low',
        impact: 'medium',
        effort: 'low',
        required: true,
        depends_on: ['task-1'],
      },
      {
        id: 'task-3',
        title: 'Explicit Pipeline Stage Dependencies',
        description:
          'Introduce a stage dependency graph that explicitly declares which stages depend on which outputs.',
        category: 'pattern',
        files: ['src/core/pipeline/pipeline_executor.rs'],
        risk_level: 'high',
        impact: 'high',
        effort: 'high',
        mitigation: 'Implement compile-time cycle detection and comprehensive tests',
        required: true,
        depends_on: ['task-1'],
      },
      {
        id: 'task-4',
        title: 'Add Detector Plugin Registry',
        description:
          'Create a plugin registry that allows detectors to self-register for lazy loading.',
        category: 'pattern',
        files: ['src/detectors/registry.rs'],
        risk_level: 'medium',
        impact: 'medium',
        effort: 'medium',
        required: false,
        depends_on: ['task-2'],
      },
      {
        id: 'task-5',
        title: 'Extract AST Caching Layer',
        description:
          'Introduce a caching layer for parsed ASTs to avoid re-parsing.',
        category: 'optimization',
        files: ['src/io/cache/ast_cache.rs'],
        risk_level: 'low',
        impact: 'high',
        effort: 'medium',
        required: false,
        depends_on: [],
      },
      {
        id: 'task-6',
        title: 'Consolidate Error Types',
        description:
          'Review and consolidate the error type hierarchy for consistency.',
        category: 'cleanup',
        files: ['src/core/errors.rs'],
        risk_level: 'low',
        impact: 'low',
        effort: 'low',
        required: false,
        depends_on: [],
      },
    ],
  },
};

// Minimal data for testing edge cases
const minimalOracleData = {
  assessment: {
    architectural_narrative: 'A simple, well-organized codebase with room for growth.',
    architectural_style: 'Clean Architecture',
    issues: ['Documentation gaps'],
  },
  refactoring_roadmap: {
    tasks: [
      {
        id: 'task-1',
        title: 'Add module documentation',
        description: 'Add rustdoc comments to all public modules and functions.',
        category: 'cleanup',
        files: ['src/lib.rs'],
        risk_level: 'low',
        impact: 'medium',
        effort: 'low',
        required: true,
        depends_on: [],
      },
    ],
  },
};

// High risk scenario
const highRiskOracleData = {
  assessment: {
    architectural_narrative:
      'The codebase has accumulated significant technical debt with unclear module boundaries. Immediate stabilization work is needed before any feature development.',
    architectural_style: 'Needs Direction - Recommend Hexagonal Architecture',
    issues: [
      'High complexity',
      'Tight coupling',
      'No test coverage',
      'Memory issues',
    ],
  },
  refactoring_roadmap: {
    tasks: [
      {
        id: 'task-critical-1',
        title: 'Fix memory leak in feature vector handling',
        description:
          'Feature vectors are not being cleared after pipeline stages complete, causing memory growth.',
        category: 'cleanup',
        files: ['src/core/featureset.rs'],
        risk_level: 'high',
        impact: 'high',
        effort: 'medium',
        mitigation: 'Implement behind feature flag with memory profiling tests',
        required: true,
        depends_on: [],
      },
      {
        id: 'task-critical-2',
        title: 'Introduce module boundary interfaces',
        description:
          'Define clear interface traits at module boundaries to break the tight coupling.',
        category: 'abstraction',
        files: ['src/core/mod.rs', 'src/detectors/mod.rs'],
        risk_level: 'high',
        impact: 'high',
        effort: 'high',
        mitigation: 'Comprehensive integration tests before migration',
        required: true,
        depends_on: ['task-critical-1'],
      },
      {
        id: 'task-critical-3',
        title: 'Add integration test harness',
        description:
          'Establish a comprehensive integration test suite for safety.',
        category: 'pattern',
        files: ['tests/integration.rs'],
        risk_level: 'medium',
        impact: 'high',
        effort: 'medium',
        required: true,
        depends_on: ['task-critical-2'],
      },
    ],
  },
};

// Many optional tasks scenario
const expansiveOracleData = {
  assessment: {
    architectural_narrative:
      'The codebase demonstrates solid fundamentals with a clear pipeline architecture. The foundation is strong enough to support significant enhancements.',
    architectural_style: 'Pipeline Architecture with Plugin Extensibility',
    issues: ['Config modularity', 'Performance', 'Plugin flexibility'],
  },
  refactoring_roadmap: {
    tasks: [
      {
        id: 'essential-1',
        title: 'Modularize configuration system',
        description: 'Split configuration into domain-specific modules.',
        category: 'structure',
        files: ['src/core/config.rs'],
        risk_level: 'medium',
        impact: 'high',
        effort: 'medium',
        required: true,
        depends_on: [],
      },
      {
        id: 'essential-2',
        title: 'Add validation layer',
        description: 'Introduce comprehensive validation for all inputs.',
        category: 'pattern',
        files: ['src/api/mod.rs'],
        risk_level: 'low',
        impact: 'medium',
        effort: 'low',
        required: true,
        depends_on: [],
      },
      {
        id: 'optional-1',
        title: 'Implement lazy detector loading',
        description: 'Load detectors on demand for faster initialization.',
        category: 'optimization',
        files: ['src/detectors/mod.rs'],
        risk_level: 'low',
        impact: 'medium',
        effort: 'low',
        required: false,
        depends_on: ['essential-1'],
      },
      {
        id: 'optional-2',
        title: 'Add detector result caching',
        description: 'Cache detector results for incremental analysis.',
        category: 'optimization',
        files: ['src/io/cache/mod.rs'],
        risk_level: 'medium',
        impact: 'high',
        effort: 'medium',
        required: false,
        depends_on: ['essential-1'],
      },
      {
        id: 'optional-3',
        title: 'Add WASM detector support',
        description: 'Allow detectors in any language that compiles to WASM.',
        category: 'abstraction',
        files: ['src/detectors/wasm/mod.rs'],
        risk_level: 'high',
        impact: 'high',
        effort: 'high',
        mitigation: 'Mark as experimental, keep native path as default',
        required: false,
        depends_on: ['optional-1'],
      },
    ],
  },
};

// Template with TabBar + matching TabPanels (so Oracle lives inside the tab stack)
const Template = (args) => {
  const tabs = [
    { id: 'summary', title: 'Summary' },
    { id: 'oracle', title: 'Oracle' },
    { id: 'files', title: 'Files' },
    { id: 'dashboard', title: 'Dashboard' },
    { id: 'clones', title: 'Clones' },
  ];

  const [activeTab, setActiveTab] = useState('oracle');

  return (
    <div>
      <TabBar tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab} />

      <TabPanel id="summary" title="Summary" active={activeTab === 'summary'}>
        <div style={{ padding: '1rem', color: '#e5e7eb' }}>
          <p>Summary content would go here.</p>
        </div>
      </TabPanel>

      <TabPanel id="oracle" title="Oracle" active={activeTab === 'oracle'}>
        <Oracle {...args} />
      </TabPanel>

      <TabPanel id="files" title="Files" active={activeTab === 'files'}>
        <div style={{ padding: '1rem', color: '#e5e7eb' }}>
          <p>Files content would go here.</p>
        </div>
      </TabPanel>

      <TabPanel id="dashboard" title="Dashboard" active={activeTab === 'dashboard'}>
        <div style={{ padding: '1rem', color: '#e5e7eb' }}>
          <p>Dashboard content would go here.</p>
        </div>
      </TabPanel>

      <TabPanel id="clones" title="Clones" active={activeTab === 'clones'}>
        <div style={{ padding: '1rem', color: '#e5e7eb' }}>
          <p>Clones content would go here.</p>
        </div>
      </TabPanel>
    </div>
  );
};

// Default story with full data
export const Default = Template.bind({});
Default.args = {
  data: fullOracleData,
};

// Minimal data story
export const Minimal = Template.bind({});
Minimal.args = {
  data: minimalOracleData,
};

// High risk scenario
export const HighRisk = Template.bind({});
HighRisk.args = {
  data: highRiskOracleData,
};

// Many optional tasks
export const Expansive = Template.bind({});
Expansive.args = {
  data: expansiveOracleData,
};

// Assessment only
export const AssessmentOnly = Template.bind({});
AssessmentOnly.args = {
  data: {
    assessment: fullOracleData.assessment,
  },
};

// Empty/No data
export const Empty = Template.bind({});
Empty.args = {
  data: null,
};

// Interactive exploration
export const Interactive = () => {
  const [data] = React.useState(fullOracleData);
  return (
    <div>
      <p style={{ color: 'var(--muted)', marginBottom: '1rem', fontSize: '0.875rem' }}>
        Explore the roadmap - tasks are ordered by safe execution sequence with dependencies shown.
        Required tasks have accent-colored numbers, optional tasks are dimmed.
      </p>
      <Oracle data={data} />
    </div>
  );
};
