import * as BABYLON from 'babylonjs'

export class Point3d {
    public x: number
    public y: number
    public z: number
    constructor(x: number, y: number, z: number) {
        this.x = x;
        this.y = y;
        this.z = z;
    }
    toString() {
        return "{x: " + this.x + ", y: " + this.y + ", z: " + this.z + "}"
    }
}

export class Vector3d {
    public x: number
    public y: number
    public z: number
    constructor(x: number, y: number, z: number) {
        this.x = x;
        this.y = y;
        this.z = z;
    }
}

export interface CoordTriple {
    x: number,
    y: number,
    z: number
}

export function toBabylonVector3(point: CoordTriple) {
    return new BABYLON.Vector3(point.x, -point.z, point.y)
}