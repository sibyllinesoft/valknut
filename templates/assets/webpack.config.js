const path = require('path');

module.exports = {
  mode: 'production',
  entry: './src/tree.js',
  output: {
    path: path.resolve(__dirname, 'dist'),
    filename: 'react-tree-bundle.min.js',
    library: 'ReactTreeBundle',
    libraryTarget: 'window'
  },
  externals: {
    'react': 'React',
    'react-dom': 'ReactDOM'
  },
  optimization: {
    minimize: true
  }
};