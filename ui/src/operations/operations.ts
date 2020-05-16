import { Point3d, Vector3d } from "../utils/math"
//@ts-ignore
import WebsocketAsPromised from "websocket-as-promised";
import * as BABYLON from 'babylonjs'
//@ts-ignore
import * as geom from "../proto/geom_pb"
import * as api from "../proto/api_pb"
import { ApiClient } from "../proto/api_grpc_web_pb"
//@ts-ignore
import * as updates from "../proto/representation_pb"

var user: string = null;
var connection: any = null;
var client: any = null;
var file_to_sym_def: Map<string, string> = new Map();

export function initialize() {
    client = new ApiClient("http://localhost:8080", undefined, undefined);
    user = "02d9b1a0-ed81-4123-accb-1f9ac44cb926";
    return user;
}

export async function setConnection(connection_url: string) {
    connection = new WebsocketAsPromised(connection_url, {
        unpackMessage: (data: any) => {
            if (data) {
                data.arrayBuffer().then(buffer => {
                    var deser = updates.UpdateChangeMsg.deserializeBinary(buffer);
                    handleUpdate(deser);
                    return deser
                })
            }
            else {
                console.log("msg " + data);
            }
        }
    });
    await connection.open();
}

import { Renderer } from '../rendering/renderer'
import { UpdateChangeMsg } from "../proto/representation_pb";

var renderer: Renderer = null;
var filename: string = "";

export interface DataObject {
    getTempRepr(): any
    moveObj(delta: Vector3d): void
    getObj(): any
    id(): string
}

function initRenderer(canvas: HTMLCanvasElement) {
    renderer = new Renderer()
    renderer.initialize(canvas)
}

export function initFile(canvas: HTMLCanvasElement, name: string, user: string) {
    filename = name;
    subToFile(name, user);
    initRenderer(canvas);
    openFile(filename)
}

export function subToFile(name: string, user: string) {
    var sub = {
        "Subscribe": {
            "filename": name,
            "user": user
        }
    };
    var msg = JSON.stringify(sub);
    console.log("Subscribing to " + msg);
    connection.send(msg);
}

export function openFile(file_id: string) {
    /*console.log("open file " + file_id);
    var fileInput = new api.OpenFileInput();
    fileInput.setFile(file_id);
    fileInput.setUser(user);
    var stream = client.openFile(fileInput);
    stream.on('status', function (status) {
        console.log(status.code);
        console.log(status.details);
        console.log(status.metadata);
    });
    stream.on('data', function (response) {
        console.log(response);
        handleUpdate(response.getUpdate());
    });
    stream.on('end', function (end) {
        console.log("end");
        // stream end signal
    });*/
}

export function beginUndoEvent(desc: string) {
    var eventInput = new api.BeginUndoEventInput();
    eventInput.setFile(filename);
    eventInput.setUser(user);
    return new Promise((resolve: (value: string) => void, reject: (value: any) => void) => {
        client.beginUndoEvent(eventInput, {}, function (err, response) {
            if (err) {
                reject(err)
            } else {
                resolve(response.getEvent())
            }
        })
    })
}

export function undoLatest() {
}

export function cancelEvent(event: string) {
}

export function redoLatest() {
}

export function deleteObject(event: string, id: string) {
}

function handleUpdate(msg: UpdateChangeMsg) {
    var file = msg.getFile();
    var id = msg.getObjId();
    var update = msg.getUpdate();
    var outputCase = update.getOutputCase();
    switch (outputCase) {
        case updates.UpdateOutputMsg.OutputCase.DELETE:
            renderer.deleteMesh(id);
            break;
        case updates.UpdateOutputMsg.OutputCase.FILE_REF:
            var fileId = update.getFileRef();
            renderer.createSymbolDef(id, fileId);
            file_to_sym_def[fileId] = id;
            openFile(fileId);
            subToFile(fileId, user);
            break;
        case updates.UpdateOutputMsg.OutputCase.MESH:
            var mesh = update.getMesh();
            var parent = null;
            if (file != filename) {
                console.log("Object coming in from another file");
                parent = file_to_sym_def[file];
            }
            renderer.renderMesh(id, mesh, parent);
            break;
        case updates.UpdateOutputMsg.OutputCase.INSTANCE:
            var instance = update.getInstance();
            renderer.createInstances(id, instance);
            break;
        case updates.UpdateOutputMsg.OutputCase.OTHER_JSON:
            //TODO
            break;
    }
}

export function joinAtPoints(event: string, id_1: string, id_2: string, pt: Point3d) {
}

export function canReferTo(id: string) {
    var mesh = renderer.getMesh(id);
    if (mesh) {
        if (mesh.metadata) {
            if (mesh.metadata.traits) {
                if (mesh.metadata.traits.includes("ReferTo")) {
                    return true;
                }
            }
        }
    }
    return false;
}

export function getClosestPoint(id: string, pt: Point3d) {
}

export function snapToPoint(event: string, id: string, snap_to_id: string, pt: Point3d) {
}

export function snapToLine(event: string, id: string, snap_to_id: string, pt: Point3d) {
}

export function moveObj(event: string, id: string, delta: Point3d) {
}

export function moveObjs(event: string, ids: Array<string>, delta: Point3d) {
}

export function getMeshByID(id: string) {
    return renderer.getMesh(id)
}

export function copyObjs(event: string, ids: Array<string>, delta: Point3d) {
}