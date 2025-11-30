import React from 'react';

/**
 * TabPanel wrapper component
 * Provides consistent tab panel structure for the report sections
 */
export const TabPanel = ({ id, title, subtitle, children, active = true }) => {
  return (
    <section
      className={`tab-panel ${active ? 'tab-active' : ''}`}
      data-tab={id}
      style={{ display: active ? 'block' : 'none' }}
    >
      <div className="tab-heading-row" style={{ display: 'none' }}>
        <h2 className="tab-heading">{title}</h2>
        {subtitle && <p className="tab-subtitle">{subtitle}</p>}
      </div>
      <div className="tab-body">
        {children}
      </div>
    </section>
  );
};

/**
 * TabBar component
 * Displays the tab buttons for switching between sections
 */
export const TabBar = ({ tabs, activeTab, onTabChange }) => {
  return (
    <div className="tabs">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          className={`tab-button ${activeTab === tab.id ? 'tab-active' : ''}`}
          onClick={() => onTabChange(tab.id)}
          data-tab-target={tab.id}
        >
          <h2 className="tab-heading">{tab.title}</h2>
        </button>
      ))}
    </div>
  );
};

export default TabPanel;
