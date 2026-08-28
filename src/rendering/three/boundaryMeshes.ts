import {
  BoxGeometry,
  Color,
  Group,
  InstancedMesh,
  Matrix4,
  MeshStandardMaterial,
  Quaternion,
  Vector3,
} from "three";
import { axialToPixel } from "@hexlife/embed/hex";
import type {
  Boundary,
  BoundaryDefinition,
  BoundaryEdge,
  BoundaryPreview,
} from "../../core/types";
import type { TerrainCell } from "./terrainMeshes";

/** One box vocabulary for posts, rails, gate braces and selection strips; no per-edge draw calls. */
export class BoundaryMeshes {
  readonly group = new Group();
  private readonly geometry = new BoxGeometry(1, 1, 1);
  private readonly wood = new MeshStandardMaterial({
    color: "#b98a51",
    roughness: 0.86,
  });
  private readonly marker = new MeshStandardMaterial({
    color: "#79e7c0",
    transparent: true,
    opacity: 0.7,
    depthWrite: false,
  });
  private built: InstancedMesh | null = null;
  private ghost: InstancedMesh | null = null;
  private boundaries: readonly Boundary[] | null = null;
  private terrain: ReadonlyMap<string, TerrainCell> | null = null;
  private preview: BoundaryPreview | null = null;

  constructor(private readonly definitions: readonly BoundaryDefinition[]) {}

  update(
    boundaries: readonly Boundary[],
    terrain: ReadonlyMap<string, TerrainCell>,
  ): boolean {
    if (this.boundaries === boundaries && this.terrain === terrain)
      return false;
    this.boundaries = boundaries;
    this.terrain = terrain;
    this.drop(this.built);
    this.built = new InstancedMesh(
      this.geometry,
      this.wood,
      Math.max(1, boundaries.length * 5),
    );
    let index = 0;
    for (const boundary of boundaries) {
      const { a, b } = this.ends(boundary);
      const direction = b.clone().sub(a).normalize();
      const gate = this.definitions.find(
        (d) => d.id === boundary.definition_id,
      )?.gate;
      for (const p of [a, b])
        this.built.setMatrixAt(
          index++,
          this.box(
            p.clone().add(new Vector3(0, 0.39, 0)),
            new Vector3(0.1, 0.78, 0.1),
          ),
        );
      const railDirection = boundary.open
        ? direction.clone().applyAxisAngle(new Vector3(0, 1, 0), Math.PI / 2.4)
        : direction;
      const center = a.clone().addScaledVector(railDirection, 0.5);
      const rotation = new Quaternion().setFromUnitVectors(
        new Vector3(1, 0, 0),
        railDirection,
      );
      for (const height of [0.28, 0.6]) {
        this.built.setMatrixAt(
          index,
          this.box(
            center.clone().add(new Vector3(0, height, 0)),
            new Vector3(0.95, 0.09, 0.07),
            rotation,
          ),
        );
        this.built.setColorAt(
          index++,
          new Color(boundary.open ? "#78bfa9" : "#e2c391"),
        );
      }
      if (gate) {
        const braceDirection = railDirection
          .clone()
          .multiplyScalar(0.85)
          .add(new Vector3(0, 0.32, 0));
        this.built.setMatrixAt(
          index++,
          this.box(
            center.clone().add(new Vector3(0, 0.44, 0)),
            new Vector3(braceDirection.length(), 0.06, 0.07),
            new Quaternion().setFromUnitVectors(
              new Vector3(1, 0, 0),
              braceDirection.normalize(),
            ),
          ),
        );
      }
    }
    this.built.count = index;
    this.built.castShadow = true;
    this.built.receiveShadow = true;
    this.built.computeBoundingSphere();
    this.group.add(this.built);
    this.setPreview(this.preview);
    return true;
  }

  setPreview(preview: BoundaryPreview | null): void {
    this.preview = preview;
    this.drop(this.ghost);
    this.ghost = null;
    if (!preview?.edges.length) return;
    this.marker.color.set(preview.error ? "#ff8077" : "#79e7c0");
    this.ghost = new InstancedMesh(
      this.geometry,
      this.marker,
      preview.edges.length,
    );
    preview.edges.forEach((edge, index) => {
      const { a, b } = this.ends(edge);
      const direction = b.clone().sub(a).normalize();
      this.ghost!.setMatrixAt(
        index,
        this.box(
          a
            .clone()
            .add(b)
            .multiplyScalar(0.5)
            .add(new Vector3(0, 0.87, 0)),
          new Vector3(1.03, 0.09, 0.12),
          new Quaternion().setFromUnitVectors(new Vector3(1, 0, 0), direction),
        ),
      );
    });
    this.ghost.computeBoundingSphere();
    this.group.add(this.ghost);
  }

  private ends(edge: BoundaryEdge): { a: Vector3; b: Vector3 } {
    const center = axialToPixel(edge, 1);
    const [dq, dr] = [
      [1, 0],
      [0, 1],
      [-1, 1],
    ][edge.direction]!;
    const height = Math.max(
      this.terrain?.get(`${edge.q},${edge.r}`)?.height ?? 0.07,
      this.terrain?.get(`${edge.q + dq!},${edge.r + dr!}`)?.height ?? 0.07,
    );
    const angle = (edge.direction * Math.PI) / 3;
    return {
      a: new Vector3(
        center.x + Math.cos(angle - Math.PI / 6),
        height,
        center.y + Math.sin(angle - Math.PI / 6),
      ),
      b: new Vector3(
        center.x + Math.cos(angle + Math.PI / 6),
        height,
        center.y + Math.sin(angle + Math.PI / 6),
      ),
    };
  }

  private box(
    position: Vector3,
    scale: Vector3,
    rotation = new Quaternion(),
  ): Matrix4 {
    return new Matrix4().compose(position, rotation, scale);
  }

  private drop(mesh: InstancedMesh | null): void {
    if (mesh) {
      this.group.remove(mesh);
      mesh.dispose();
    }
  }

  dispose(): void {
    this.drop(this.built);
    this.drop(this.ghost);
    this.geometry.dispose();
    this.wood.dispose();
    this.marker.dispose();
  }
}
