import * as BABYLON from 'babylonjs'
import * as mouse from '../ui/mouse_events'
import * as gui from '../ui/gui'
import { UIControllerSingleton } from '../ui/controller'

function getHoveredMesh(scene: BABYLON.Scene, ground: BABYLON.Mesh) {
    var pickinfo = scene.pick(scene.pointerX, scene.pointerY, mesh => { return mesh != ground });
    if (pickinfo.hit) {
        return pickinfo.pickedMesh;
    }
    return null;
}

export class Renderer {
    private _canvas: HTMLCanvasElement
    private _engine: BABYLON.Engine
    private _scene: BABYLON.Scene
    private _highlight: BABYLON.HighlightLayer
    private _transform: BABYLON.TransformNode

    createScene(canvas: HTMLCanvasElement, engine: BABYLON.Engine) {
        this._canvas = canvas;
        this._engine = engine;
        // This creates a basic Babylon Scene object (non-mesh)
        const scene = new BABYLON.Scene(engine);
        scene.debugLayer.show();
        this._scene = scene;
        this._transform = new BABYLON.TransformNode("rootTransform", scene);
        this._transform.rotation = new BABYLON.Vector3(Math.PI / 2, 0, 0);
        this._transform.scaling = new BABYLON.Vector3(1, 1, -1);
        this._highlight = new BABYLON.HighlightLayer("highlight1", this._scene);
        // This creates and positions a free camera (non-mesh)
        const camera = new BABYLON.ArcRotateCamera("camera1", -Math.PI / 2, 1.0, 500, new BABYLON.Vector3(0, 0, 0), scene);
        camera.panningSensibility = 50;
        camera.panningInertia = .7;
        // This attaches the camera to the canvas
        camera.attachControl(canvas, true);
        // This creates a light, aiming 0,1,0 - to the sky (non-mesh)
        const light = new BABYLON.HemisphericLight("light", new BABYLON.Vector3(0, 1, 0), this._scene);
        // Default intensity is 1. Let's dim the light a small amount
        light.intensity = 0.7;
        light.parent = camera;

        var ground = BABYLON.Mesh.CreateGround("ground", 10000, 10000, 0, this._scene, false);
        var groundMaterial = new BABYLON.StandardMaterial("ground", this._scene);
        groundMaterial.specularColor = BABYLON.Color3.Black();
        ground.material = groundMaterial;

        gui.guiInstance.init();

        var onPointerClick = (evt: MouseEvent) => {
            mouse.onPointerClick(this._scene, evt, ground)
        }

        var current_hover: BABYLON.Mesh = null;
        var onPointerMove = (evt: MouseEvent) => {
            var hovered = getHoveredMesh(this._scene, ground)
            var layer = this._scene.getHighlightLayerByName("highlight1");
            if (current_hover && hovered != current_hover) {
                layer.removeMesh(current_hover)
            }
            if (mouse.onPointerMove(this._scene, ground, hovered as BABYLON.Mesh)) {
                if (hovered) {
                    layer.addMesh(hovered as BABYLON.Mesh, BABYLON.Color3.Green());
                    current_hover = hovered as BABYLON.Mesh;
                }
            }
        }

        this._scene.onPointerObservable.add((pointerInfo) => {
            switch (pointerInfo.type) {
                case BABYLON.PointerEventTypes.POINTERDOWN:
                    break;
                case BABYLON.PointerEventTypes.POINTERUP:
                    break;
                case BABYLON.PointerEventTypes.POINTERMOVE:
                    onPointerMove(pointerInfo.event)
                    break;
                case BABYLON.PointerEventTypes.POINTERWHEEL:
                    break;
                case BABYLON.PointerEventTypes.POINTERPICK:
                    break;
                case BABYLON.PointerEventTypes.POINTERTAP:
                    onPointerClick(pointerInfo.event)
                    break;
                case BABYLON.PointerEventTypes.POINTERDOUBLETAP:
                    break;
            }
        });
    }
    initialize(canvas: HTMLCanvasElement) {
        const engine = new BABYLON.Engine(canvas, true, { stencil: true });
        this.createScene(canvas, engine);
        engine.runRenderLoop(() => {
            this._scene.render();
        });
        window.addEventListener('resize', function () {
            engine.resize();
        });
    }

    stop() {
        this._engine.stopRenderLoop();
    }

    applyNewMeshProps(mesh: BABYLON.Mesh, temp?: boolean) {
        if (!temp) {
            var objMaterial = new BABYLON.StandardMaterial("obj", this._scene)
            objMaterial.diffuseColor = BABYLON.Color3.Gray();
            objMaterial.backFaceCulling = false;
            mesh.material = objMaterial;
        }
        else {
            var objMaterial = new BABYLON.StandardMaterial("temp", this._scene);
            objMaterial.wireframe = true;
            objMaterial.backFaceCulling = false;
            mesh.material = objMaterial;
        }
    }

    createSymbolDef(id: string, file_id: string) {
        console.log("Creating symbol def " + id);
        var mesh = this._scene.getMeshByName(id) as BABYLON.Mesh;
        if (mesh) {
            mesh.dispose();
        }
        //This is just there to suppress BabylonJS warnings about instances of objects with no geometry.
        mesh = BABYLON.MeshBuilder.CreateBox(id, { height: 1 }, this._scene);
        mesh.parent = this._transform;
        mesh.isVisible = false;
        mesh.metadata = {
            "type": "SymbolDef",
            "traits": [],
            "obj": {
                "file_id": file_id
            }
        };
    }

    renderMesh(id: string, triangles: any, parent_id: string, temp?: boolean) {
        console.log("Updating object " + id + " with parent " + parent_id);
        var positions = triangles.getPositionsList();
        var indices = triangles.getIndicesList();
        var mesh = this._scene.getMeshByName(id) as BABYLON.Mesh
        if (!mesh) {
            mesh = new BABYLON.Mesh(id, this._scene);
            this.applyNewMeshProps(mesh, temp);
            if (parent_id) {
                var parent = this._scene.getMeshByName(parent_id) as BABYLON.Mesh;
                if (parent) {
                    console.log("Got parent");
                    mesh.setParent(parent);
                    mesh.isVisible = false;
                    parent.instances.forEach(instanceParent => {
                        var newChildInstance = mesh.createInstance(instanceParent.name + id);
                        newChildInstance.setParent(instanceParent);
                    });
                }
            } else {
                mesh.parent = this._transform;
            }
        }
        mesh.metadata = JSON.parse(triangles.getMetaJson());
        var vertexData = new BABYLON.VertexData();
        vertexData.positions = positions;
        vertexData.indices = indices;
        vertexData.applyToMesh(mesh);
    }

    point3MsgToVector3(pt: any) {
        return new BABYLON.Vector3(pt.getX(), pt.getY(), pt.getZ());
    }

    createInstances(id: string, instanceData: any) {
        console.log("Creating instances " + id);
        var parent = instanceData.getSource();
        console.log(parent);
        var mesh = this._scene.getMeshByName(parent) as BABYLON.Mesh;
        if (mesh) {
            var newContainer = mesh.createInstance(id);
            newContainer.isVisible = false;
            var matrix = BABYLON.Matrix.FromArray(instanceData.getTransformList());
            console.log(matrix.m);
            var translation = new BABYLON.Vector3(matrix.m[12], matrix.m[14], -matrix.m[13]);
            console.log("Translation: " + translation);
            newContainer.position = translation;
            console.log("Position: " + newContainer.position);
            newContainer.metadata = JSON.parse(instanceData.getMetaJson());
            mesh.getChildren().forEach(child_node => {
                var childMesh = child_node as BABYLON.Mesh;
                var newChildInstance = childMesh.createInstance(id + childMesh.name);
                newChildInstance.isVisible = true;
                newChildInstance.parent = newContainer;
            })
        } else {
            console.log("No symbol def at " + parent);
        }
    }

    showNormals(mesh: BABYLON.Mesh) {
        var normals = mesh.getVerticesData(BABYLON.VertexBuffer.NormalKind);
        var positions = mesh.getVerticesData(BABYLON.VertexBuffer.PositionKind);
        var color = BABYLON.Color3.White();
        var size = 1;

        var lines = [];
        for (var i = 0; i < normals.length; i += 3) {
            var v1 = BABYLON.Vector3.FromArray(positions, i);
            var v2 = v1.add(BABYLON.Vector3.FromArray(normals, i).scaleInPlace(size));
            lines.push([v1.add(mesh.position), v2.add(mesh.position)]);
        }
        var normalLines = BABYLON.MeshBuilder.CreateLineSystem("normalLines", { lines: lines }, this._scene);
        normalLines.color = color;
        return normalLines;
    }

    deleteMesh(id: string) {
        console.log("Deleting object: " + id);
        var mesh = this._scene.getMeshByName(id)
        if (mesh) {
            mesh.dispose()
        }
    }

    getMesh(id: string) {
        return this._scene.getMeshByName(id) as BABYLON.Mesh
    }
}