var fabric = require("./fabric.min").fabric;

var renderer2d: any = null;

export function initialize(canvas: string) {
    renderer2d = new fabric.Canvas(canvas);
    fabric.Object.prototype.transparentCorners = false;
    renderer2d.setBackgroundColor("gray", renderer2d.renderAll.bind(renderer2d));
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

export function createSheet(name: string, id: string, width: number, height: number) {
    var sheetRect = new fabric.Rect({
        width: width, height: height, opacity: 1, fill: "white", selectable: false, left: 10, top: 10
    });
    var viewGroup = new fabric.Group([], {});
    var sheetGroup = new fabric.Group([sheetRect, viewGroup]);

    sheetGroup.name = name;
    sheetGroup.id = id;

    console.log("renderer add");
    renderer2d.add(sheetGroup);
}

export function createViewport(sheet_id: string, view_id: string, posX: number, posY: number, scale: number) {
    var viewport = new fabric.Group([], {
        left: posX, top: posY
    });
    viewport.id = view_id;
    viewport.sheet_id = sheet_id;
    viewport.scale = scale;
    renderer2d.getObjects("group").forEach(sheet => {
        if (sheet.id === sheet_id) {
            var viewGroup = sheet.item(1);
            viewGroup.addWithUpdate(viewport);
        }
    });
}

export function addObjectRep(shape: any) {
    renderer2d.getObjects("group").forEach(sheet => {
        console.log(sheet);
        var viewGroup = sheet.item(1);
        console.log(viewGroup);
        viewGroup.getObjects().forEach(view => {
            console.log(view);
            view.addWithUpdate(shape);
        })
    });
}

export function test() {
    console.log("Are we not cached?");
    const id = "test id 1";
    createSheet("Test Sheet", id, 2000, 1000);
    createViewport(id, "view id", 100, 100, 1);
    var rect = new fabric.Rect({ width: 100, height: 100, left: 0, top: 0, fill: "red" });
    addObjectRep(rect);
    console.log(renderer2d);
    renderer2d.requestRenderAll();
}