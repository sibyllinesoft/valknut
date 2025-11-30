import React from 'react';
import { CloneAnalysis } from './CloneAnalysis';
import './CloneAnalysis.css';

export default {
  title: 'Components/CloneAnalysis',
  component: CloneAnalysis,
  parameters: {
    layout: 'padded',
    backgrounds: {
      default: 'dark',
    },
  },
  argTypes: {
    data: {
      control: 'object',
      description: 'Clone analysis data with metrics and clone pairs',
    },
  },
};

const fullCloneData = {
  hasData: true,
  avgSimilarity: 0.78,
  maxSimilarity: 0.95,
  candidatesAfter: 24,
  clonePairs: [
    {
      file1: 'src/core/pipeline/pipeline_executor.rs',
      file2: 'src/core/pipeline/pipeline_stages.rs',
      similarity: 0.95,
      lines1: { start: 45, end: 89 },
      lines2: { start: 112, end: 156 },
    },
    {
      file1: 'src/detectors/complexity.rs',
      file2: 'src/detectors/cognitive.rs',
      similarity: 0.87,
      lines1: { start: 23, end: 67 },
      lines2: { start: 34, end: 78 },
    },
    {
      file1: 'src/io/reports/html.rs',
      file2: 'src/io/reports/markdown.rs',
      similarity: 0.72,
    },
    {
      file1: 'src/lang/javascript.rs',
      file2: 'src/lang/typescript.rs',
      similarity: 0.91,
      lines1: { start: 100, end: 250 },
      lines2: { start: 100, end: 255 },
    },
  ],
};

const minimalCloneData = {
  hasData: true,
  avgSimilarity: 0.65,
  maxSimilarity: 0.72,
  candidatesAfter: 3,
  clonePairs: [
    {
      file1: 'src/utils/helpers.rs',
      file2: 'src/utils/formatters.rs',
      similarity: 0.72,
    },
  ],
};

const highDuplicationData = {
  hasData: true,
  avgSimilarity: 0.89,
  maxSimilarity: 0.99,
  candidatesAfter: 47,
  clonePairs: [
    {
      file1: 'src/handlers/get_user.rs',
      file2: 'src/handlers/get_post.rs',
      similarity: 0.99,
      lines1: { start: 10, end: 45 },
      lines2: { start: 10, end: 45 },
    },
    {
      file1: 'src/handlers/create_user.rs',
      file2: 'src/handlers/create_post.rs',
      similarity: 0.97,
      lines1: { start: 15, end: 60 },
      lines2: { start: 15, end: 62 },
    },
    {
      file1: 'src/handlers/delete_user.rs',
      file2: 'src/handlers/delete_post.rs',
      similarity: 0.96,
    },
    {
      file1: 'src/handlers/update_user.rs',
      file2: 'src/handlers/update_post.rs',
      similarity: 0.94,
    },
    {
      file1: 'src/models/user.rs',
      file2: 'src/models/post.rs',
      similarity: 0.85,
    },
  ],
};

const metricsOnlyData = {
  hasData: true,
  avgSimilarity: 0.45,
  maxSimilarity: 0.58,
  candidatesAfter: 0,
  clonePairs: [],
};

const Template = (args) => <CloneAnalysis {...args} />;

export const Default = Template.bind({});
Default.args = {
  data: fullCloneData,
};

export const Minimal = Template.bind({});
Minimal.args = {
  data: minimalCloneData,
};

export const HighDuplication = Template.bind({});
HighDuplication.args = {
  data: highDuplicationData,
};

export const MetricsOnly = Template.bind({});
MetricsOnly.args = {
  data: metricsOnlyData,
};

export const Empty = Template.bind({});
Empty.args = {
  data: null,
};

export const NoData = Template.bind({});
NoData.args = {
  data: { hasData: false },
};
