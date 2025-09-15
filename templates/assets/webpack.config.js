const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');

module.exports = (env, argv) => {
  const isProduction = argv.mode === 'production';
  
  return {
    mode: isProduction ? 'production' : 'development',
    entry: {
      'react-tree-bundle': './src/tree.js'
    },
    output: {
      path: path.resolve(__dirname, 'dist'),
      filename: isProduction ? '[name].min.js' : '[name].debug.js',
      library: {
        name: 'ReactTreeBundle',
        type: 'window',
        export: 'default'
      },
      globalObject: 'this',
      clean: true
    },
    plugins: [
      new HtmlWebpackPlugin({
        template: './src/template.html',
        filename: 'tree-component.html',
        inject: 'head'
      })
    ],
    optimization: {
      minimize: isProduction
    },
    devtool: isProduction ? false : 'source-map'
  };
};