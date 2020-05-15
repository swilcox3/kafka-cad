import * as gui from './gui'
import * as BABYLON from 'babylonjs'
import { Point3d } from '../utils/math'
import * as ops from "../operations/operations"

interface Tool {
    onMouseDown(pt: Point3d, hovered?: BABYLON.Mesh): void,
    onMouseMove(pt: Point3d, hovered?: BABYLON.Mesh): boolean,
    cancel(): void,
    finish(pt: Point3d, picked?: BABYLON.Mesh): void
}

class SelectionController {
    private selectedObjs: Set<BABYLON.Mesh>;
    ctrlPressed: boolean;
    constructor() {
        this.selectedObjs = new Set();
        this.ctrlPressed = false;
    }

    getSelectedObjs() {
        return this.selectedObjs
    }

    isSelected(mesh: BABYLON.Mesh) {
        return this.selectedObjs.has(mesh)
    }

    deselectAll() {
        this.selectedObjs.forEach((obj) => {
            var mat = obj.material as BABYLON.StandardMaterial;
            mat.diffuseColor = BABYLON.Color3.Gray();
        })
        this.selectedObjs.clear();
    }

    addObject(mesh: BABYLON.Mesh) {
        var mat = mesh.material as BABYLON.StandardMaterial;
        mat.diffuseColor = BABYLON.Color3.Green();
        this.selectedObjs.add(mesh)
    }

    removeObject(mesh: BABYLON.Mesh) {
        var mat = mesh.material as BABYLON.StandardMaterial;
        mat.diffuseColor = BABYLON.Color3.Gray();
        this.selectedObjs.delete(mesh);
    }

    async deleteSelected() {
        if (this.selectedObjs.size > 0) {
            var event = await ops.beginUndoEvent("Delete objs");
            this.selectedObjs.forEach((obj) => {
                ops.deleteObject(event, obj.name)
            });
            this.deselectAll()
        }
    }

    selectObj(mesh: BABYLON.Mesh) {
        if (!this.isSelected(mesh)) {
            if (!this.ctrlPressed) {
                this.deselectAll();
                this.addObject(mesh);
            }
            else {
                this.addObject(mesh)
            }
            gui.guiInstance.setObjectOverlay(this.selectedObjs)
        }
        else {
            if (this.ctrlPressed) {
                this.removeObject(mesh)
            }
        }
    }
}

class UIController {
    private activeTool: Tool
    private selection: SelectionController
    private shiftPt: Point3d;
    private shiftPressed: boolean;
    private clipboard: Array<string>;
    constructor() {
        this.activeTool = null;
        this.selection = new SelectionController();
        this.shiftPt = null;
        this.shiftPressed = false;
        this.clipboard = new Array<string>();
    }

    setActiveTool(tool: Tool) {
        if (this.activeTool != null) {
            this.activeTool.cancel()
        }
        this.activeTool = tool
    }

    leftClick(pt: Point3d, mesh: BABYLON.Mesh) {
        if (this.activeTool != null) {
            this.activeTool.onMouseDown(pt, mesh)
        }
        else if (mesh != null) {
            this.selection.selectObj(mesh)
        }
        else if (mesh == null) {
            this.selection.deselectAll();
            gui.guiInstance.clearObjectOverlay();
        }
    }

    rightClick(pt: Point3d, picked: BABYLON.Mesh) {
        if (this.activeTool != null) {
            this.activeTool.finish(pt, picked)
            this.activeTool = null
        }
        else if (picked == null) {
            this.selection.deselectAll();
        }
    }

    mouseMove(pt: Point3d, hovered: BABYLON.Mesh) {
        if (this.activeTool != null) {
            if (this.shiftPressed) {
                if (this.shiftPt) {
                    if (Math.abs(pt.x - this.shiftPt.x) > Math.abs(pt.y - this.shiftPt.y)) {
                        pt = new Point3d(pt.x, this.shiftPt.y, this.shiftPt.z);
                    }
                    else {
                        pt = new Point3d(this.shiftPt.x, pt.y, this.shiftPt.z);
                    }
                }
                else {
                    this.shiftPt = pt;
                }
            }
            else {
                this.shiftPt = null;
            }
            return this.activeTool.onMouseMove(pt, hovered)
        }
        return true;
    }

    ctrlDown() {
        this.selection.ctrlPressed = true;
    }

    ctrlUp() {
        this.selection.ctrlPressed = false;
    }

    shiftDown() {
        this.shiftPressed = true;
    }

    shiftUp() {
        this.shiftPressed = false;
    }

    onDeleteKey() {
        if (this.activeTool == null) {
            this.selection.deleteSelected();
        }
    }

    cancel() {
        if (this.activeTool != null) {
            this.activeTool.cancel()
            this.activeTool = null
        }
    }

    setClipboard() {
        if (this.activeTool == null) {
            this.clipboard = []
            this.selection.getSelectedObjs().forEach((mesh) => {
                this.clipboard.push(mesh.name)
            });
        }
    }
}

export class UIControllerSingleton {
    private static instance: UIController
    constructor() {
        if (!UIControllerSingleton.instance) {
            UIControllerSingleton.instance = new UIController()
        }
    }

    getInstance() {
        return UIControllerSingleton.instance
    }
}