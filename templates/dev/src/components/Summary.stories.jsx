import React from 'react';
import { Summary } from './Summary';
import './Summary.css';

export default {
  title: 'Components/Summary',
  component: Summary,
  parameters: {
    layout: 'padded',
    backgrounds: {
      default: 'dark',
    },
  },
  argTypes: {
    data: {
      control: 'object',
      description: 'Summary data containing analysis stats and health metrics',
    },
  },
};

// Sample data matching the report structure
const fullSummaryData = {
  summary: {
    filesProcessed: 847,
    entitiesAnalyzed: 3421,
    refactoringNeeded: 156,
    docIssueCount: 42,
  },
  healthMetrics: {
    maintainability: 78,
    complexity: 65,
    cognitive: 72,
    structure: 85,
    debt: 58,
    docs: 45,
  },
};

const healthyCodebaseData = {
  summary: {
    filesProcessed: 234,
    entitiesAnalyzed: 1256,
    refactoringNeeded: 12,
    docIssueCount: 5,
  },
  healthMetrics: {
    maintainability: 92,
    complexity: 88,
    cognitive: 91,
    structure: 95,
    debt: 89,
    docs: 78,
  },
};

const troubledCodebaseData = {
  summary: {
    filesProcessed: 1523,
    entitiesAnalyzed: 8742,
    refactoringNeeded: 847,
    docIssueCount: 312,
  },
  healthMetrics: {
    maintainability: 35,
    complexity: 22,
    cognitive: 28,
    structure: 45,
    debt: 18,
    docs: 12,
  },
};

const minimalData = {
  summary: {
    filesProcessed: 15,
    entitiesAnalyzed: 87,
    refactoringNeeded: 3,
    docIssueCount: 0,
  },
};

const Template = (args) => <Summary {...args} />;

export const Default = Template.bind({});
Default.args = {
  data: fullSummaryData,
};

export const HealthyCodebase = Template.bind({});
HealthyCodebase.args = {
  data: healthyCodebaseData,
};

export const TroubledCodebase = Template.bind({});
TroubledCodebase.args = {
  data: troubledCodebaseData,
};

export const StatsOnly = Template.bind({});
StatsOnly.args = {
  data: minimalData,
};

export const Empty = Template.bind({});
Empty.args = {
  data: null,
};
