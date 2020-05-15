const path = require('path');
// Optimizes duplicates in splitted bundles 
const webpack = require('webpack');
// creates index.html file by a template index.ejs
const HtmlWebpackPlugin = require('html-webpack-plugin');
// cleans dist folder
const { CleanWebpackPlugin } = require('clean-webpack-plugin');
const TsConfigPathsPlugin = require("tsconfig-paths-webpack-plugin");

// output folder location
const distFolder = "./dist";

module.exports = {
  mode: 'development',
  entry: './index.ts',
  plugins: [
    new CleanWebpackPlugin(),
    new HtmlWebpackPlugin({
      template: './index.ejs'
    }),
  ],
  devtool: 'inline-source-map',
  devServer: {
    contentBase: distFolder
  },
  module: {
    rules: [
      {
        test: /\.tsx?$/,
        use: 'ts-loader',
        exclude: /node_modules/
      },
      {
        test: /\.css$/,
        use: [
          'style-loader',
          'css-loader'
        ]
      }
    ]
  },
  optimization: {
    splitChunks: {
      cacheGroups: {
        commons: {
          test: /[\\/]node_modules[\\/]/,
          name: "vendors",
          chunks: "all"
        }
      }
    }
  },
  resolve: {
    extensions: [".tsx", ".ts", ".js"],
    plugins: [
      new TsConfigPathsPlugin({})
    ]
  },
  output: {
    filename: '[name].bundle.js',
    path: path.resolve(__dirname, distFolder)
  }
};