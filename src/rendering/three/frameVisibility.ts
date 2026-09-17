import { Frustum, Matrix4, Sphere, type Camera } from "three";

/** Conservative presentation bounds. Nothing here affects native activity or snapshots. */
export class FrameVisibility {
  private readonly frustum = new Frustum();
  private readonly projection = new Matrix4();
  private readonly sphere = new Sphere();

  update(camera: Camera): void {
    camera.updateMatrixWorld();
    this.projection.multiplyMatrices(
      camera.projectionMatrix,
      camera.matrixWorldInverse,
    );
    this.frustum.setFromProjectionMatrix(this.projection);
  }

  includes(x: number, y: number, z: number, radius: number): boolean {
    this.sphere.center.set(x, y, z);
    this.sphere.radius = radius;
    return this.frustum.intersectsSphere(this.sphere);
  }
}
