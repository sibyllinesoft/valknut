import React, { useState } from 'react';
import { TabPanel, TabBar } from './TabPanel';
import './TabPanel.css';

export default {
  title: 'Components/TabPanel',
  component: TabPanel,
  parameters: {
    layout: 'padded',
    backgrounds: {
      default: 'dark',
    },
  },
};

// Interactive example with TabBar
export const InteractiveTabs = () => {
  const [activeTab, setActiveTab] = useState('summary');

  const tabs = [
    { id: 'summary', title: 'Summary' },
    { id: 'oracle', title: 'Oracle' },
    { id: 'files', title: 'Files' },
    { id: 'dashboard', title: 'Dashboard' },
    { id: 'clones', title: 'Clones' },
  ];

  return (
    <div>
      <TabBar tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab} />

      <TabPanel id="summary" title="Summary" active={activeTab === 'summary'}>
        <div style={{ padding: '1rem', color: '#e5e7eb' }}>
          <h3 style={{ margin: '0 0 1rem 0', color: 'var(--accent, #20d4c0)' }}>Summary Content</h3>
          <p>Analysis summary with health gauges and metrics.</p>
        </div>
      </TabPanel>

      <TabPanel id="oracle" title="Oracle" active={activeTab === 'oracle'}>
        <div style={{ padding: '1rem', color: '#e5e7eb' }}>
          <h3 style={{ margin: '0 0 1rem 0', color: 'var(--accent, #20d4c0)' }}>Oracle Content</h3>
          <p>AI-generated refactoring recommendations.</p>
        </div>
      </TabPanel>

      <TabPanel id="files" title="Files" active={activeTab === 'files'}>
        <div style={{ padding: '1rem', color: '#e5e7eb' }}>
          <h3 style={{ margin: '0 0 1rem 0', color: 'var(--accent, #20d4c0)' }}>Files Content</h3>
          <p>Interactive file tree with complexity analysis.</p>
        </div>
      </TabPanel>

      <TabPanel id="dashboard" title="Dashboard" active={activeTab === 'dashboard'}>
        <div style={{ padding: '1rem', color: '#e5e7eb' }}>
          <h3 style={{ margin: '0 0 1rem 0', color: 'var(--accent, #20d4c0)' }}>Dashboard Content</h3>
          <p>Visual treemap of the codebase.</p>
        </div>
      </TabPanel>

      <TabPanel id="clones" title="Clones" active={activeTab === 'clones'}>
        <div style={{ padding: '1rem', color: '#e5e7eb' }}>
          <h3 style={{ margin: '0 0 1rem 0', color: 'var(--accent, #20d4c0)' }}>Clones Content</h3>
          <p>Code duplication analysis and clone pairs.</p>
        </div>
      </TabPanel>
    </div>
  );
};

// Single TabPanel example
export const SinglePanel = () => (
  <TabPanel id="example" title="Example Tab" subtitle="A sample tab panel">
    <div style={{ padding: '1rem', color: '#e5e7eb' }}>
      <p>This is the content of a single tab panel.</p>
      <p>It uses the consistent tab-panel structure from the report.</p>
    </div>
  </TabPanel>
);

// Inactive panel (hidden)
export const InactivePanel = () => (
  <div>
    <p style={{ color: '#94a3b8', marginBottom: '1rem' }}>
      The panel below is inactive (hidden by default in the report):
    </p>
    <TabPanel id="inactive" title="Inactive Tab" active={false}>
      <div style={{ padding: '1rem', color: '#e5e7eb' }}>
        <p>You shouldn't see this content.</p>
      </div>
    </TabPanel>
    <p style={{ color: '#94a3b8' }}>
      (Notice the panel above is not visible)
    </p>
  </div>
);

// TabBar only
export const TabBarOnly = () => {
  const [activeTab, setActiveTab] = useState('tab1');

  const tabs = [
    { id: 'tab1', title: 'First' },
    { id: 'tab2', title: 'Second' },
    { id: 'tab3', title: 'Third' },
  ];

  return (
    <div>
      <p style={{ color: '#94a3b8', marginBottom: '1rem' }}>
        TabBar component with active state: <strong>{activeTab}</strong>
      </p>
      <TabBar tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab} />
    </div>
  );
};
