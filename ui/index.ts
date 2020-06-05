import * as ops from './src/operations/operations';

var renderCanvas3d = document.getElementById('renderCanvas3d');
var renderCanvas2d = document.getElementById('renderCanvas2d');

swapCanvases();

document.getElementById("SwapView").onclick = function () {
  swapCanvases();
};

function swapCanvases() {
  if (renderCanvas3d.style.visibility == 'visible') {
    renderCanvas3d.style.visibility = 'hidden';
    renderCanvas2d.style.visibility = 'visible';
  } else {
    renderCanvas3d.style.visibility = 'visible';
    renderCanvas2d.style.visibility = 'hidden';
  }
}

window.addEventListener('DOMContentLoaded', () => {
  var connection = "ws://" + window.location.hostname + ":7000";
  var user = ops.initialize();
  console.log(connection);
  ops.setConnection(connection).then(() => {
    const urlParams = new URLSearchParams(window.location.search);
    var file = urlParams.get('file');
    console.log("Got file: " + file);
    ops.initFile(renderCanvas3d as HTMLCanvasElement, "renderCanvas2d", file, user);
  });
});
