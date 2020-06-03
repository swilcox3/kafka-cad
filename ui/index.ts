import * as ops from './src/operations/operations';

window.addEventListener('DOMContentLoaded', () => {
  var connection = "ws://" + window.location.hostname + ":7000";
  var user = ops.initialize();
  console.log(connection);
  ops.setConnection(connection).then(() => {
    const urlParams = new URLSearchParams(window.location.search);
    var file = urlParams.get('file');
    console.log("Got file: " + file);
    ops.initFile(document.getElementById('renderCanvas') as HTMLCanvasElement, file, user);
  });
});
