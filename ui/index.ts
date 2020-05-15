import * as ops from './src/operations/operations';

window.addEventListener('DOMContentLoaded', () => {
  var connection = "ws://" + window.location.hostname + ":7000";
  var user = ops.initialize();
  console.log(connection);
  ops.setConnection(connection).then(() => {
    var file = "00000003-0003-0003-0003-000000000003";
    ops.initFile(document.getElementById('renderCanvas') as HTMLCanvasElement, file);
  });
});
