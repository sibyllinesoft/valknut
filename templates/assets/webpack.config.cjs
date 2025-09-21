const path = require('path');

module.exports = {
  mode: 'production',
  entry: './src/tree-component/index.js',
  output: {
    filename: 'react-tree-bundle.js',
    path: path.resolve(__dirname, 'dist'),
    library: 'ReactTreeBundle',
    libraryTarget: 'umd',
    globalObject: 'this'
  },
  module: {
    rules: [
      {
        test: /\.jsx?$/,
        // Don't exclude node_modules to ensure React gets transpiled
        use: {
          loader: 'babel-loader',
          options: {
            presets: [
              ['@babel/preset-env', {
                targets: {
                  ie: '11',
                  chrome: '58'
                },
                // Force transpilation of all modern features
                forceAllTransforms: true
              }],
              ['@babel/preset-react', {
                runtime: 'automatic'
              }]
            ],
            // Cache to speed up builds
            cacheDirectory: true
          }
        }
      }
    ]
  },
  resolve: {
    extensions: ['.js', '.jsx']
  },
  // Bundle React instead of treating as external
  // externals: {
  //   'react': 'React',
  //   'react-dom': 'ReactDOM',
  //   'react-arborist': 'ReactArborist'
  // }
};