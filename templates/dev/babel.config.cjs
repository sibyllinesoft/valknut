module.exports = {
  presets: [
    ['@babel/preset-env', {
      targets: {
        ie: '11',        // Target IE11 to force ES5 compilation
        chrome: '58'     // Older Chrome version without template literals
      }
    }],
    ['@babel/preset-react', {
      runtime: 'automatic'
    }]
  ]
};
