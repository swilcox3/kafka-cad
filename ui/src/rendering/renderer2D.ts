import { Rect2DMsg } from "../proto/geom_pb";

var fabric = require("./fabric.min").fabric;

var renderer2d: any = null;

export function initialize(canvas: string) {
    renderer2d = new fabric.Canvas(canvas);
    renderer2d.setBackgroundColor("white", renderer2d.renderAll.bind(renderer2d));
    window.addEventListener('resize', resizeCanvas, false);
    renderer2d.on('mouse:down', function (options) {
        console.log(options.target);
    });

    function resizeCanvas() {
        renderer2d.setHeight(document.getElementById('wrapper').offsetHeight);
        renderer2d.setWidth(document.getElementById('wrapper').offsetWidth);
        renderer2d.renderAll();
    }
    resizeCanvas();
}

export function createViewport(sheet_id: string, view_id: string, posX: number, posY: number, scale: number) {
    var viewport = new fabric.Group([], {
        left: posX, top: posY, transparentCorners: false
    });
    viewport.view_id = view_id;
    viewport.sheet_id = sheet_id;
    viewport.viewport_scale = scale;
    viewport.view_type = "top";
    renderer2d.add(viewport);
}

export function addObjectRep(shape: any) {
    renderer2d.getObjects("group").forEach(view => {
        var originalLeft = shape.left;
        var originalTop = shape.top;
        if (view.view_type === shape.view_type) {
            view.addWithUpdate(shape);
            shape.left = originalLeft;
            shape.top = originalTop;
            shape.setCoords();
        }
    });
}

export function test() {
    console.log("Are we not cached?");
    const id = "test id 1";
    //createSheet("Test Sheet", id, 2000, 1000);
    createViewport(id, "view id", 1000, 1000, 1);
    var rect = new fabric.Rect({ width: 500, height: 500, fill: "red", transparentCorners: false });
    rect.view_type = "top";
    addObjectRep(rect);
    console.log(renderer2d);
    renderer2d.requestRenderAll();
}