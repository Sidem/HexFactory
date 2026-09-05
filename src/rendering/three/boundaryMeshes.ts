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
  BoundaryAnchor,
  BoundaryDefinition,
  BoundaryPreview,
  BoundarySegment,
} from "../../core/types";
import { chordCorners, cornerHexes } from "../../core/lattice";
import { FOG_HEIGHT, heightAt, type TerrainCell } from "./terrainMeshes";

/**
 * The six corners of a hex of circumradius one, index 0 due north and then clockwise. The same
 * corners `lattice.ts` names in native's integers, in the trigonometry the rest of the diorama is
 * laid out with, so a post lands on the corner the hex mesh actually drew.
 */
const CORNER_OFFSETS: readonly (readonly [number, number])[] = [
  [0, -1],
  [Math.sqrt(3) / 2, -0.5],
  [Math.sqrt(3) / 2, 0.5],
  [0, 1],
  [-Math.sqrt(3) / 2, 0.5],
  [-Math.sqrt(3) / 2, -0.5],
];

/** One box vocabulary for posts, rails, gate braces and selection strips; no per-segment draw calls. */
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
  /** The anchor pins read as a decision the player made, so they stay lit rather than ghosted. */
  private readonly pin = new MeshStandardMaterial({
    color: "#ffd479",
    emissive: new Color("#6b4c00"),
    roughness: 0.4,
  });
  private built: InstancedMesh | null = null;
  private ghost: InstancedMesh | null = null;
  private pins: InstancedMesh | null = null;
  private boundaries: readonly Boundary[] | null = null;
  private terrain: ReadonlyMap<string, TerrainCell> | null = null;
  private preview: BoundaryPreview | null = null;
  private anchors: readonly BoundaryAnchor[] = [];

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
      Math.max(1, boundaries.length * 24),
    );
    let index = 0;
    for (const boundary of boundaries) {
      const definition = this.definitions.find(
        (d) => d.id === boundary.definition_id,
      );
      const { a, b } = this.ends(boundary);
      // A chord is one, two or four hex radii long depending on which of the fifteen it is, so
      // every rail, panel and brace is measured off the run rather than assumed to be a unit.
      const span = a.distanceTo(b);
      const direction = b.clone().sub(a).normalize();
      const wall = definition?.family === "wall";
      const wire = definition?.key.startsWith("wire") === true;
      const postHeight = wall ? 1.08 : 0.78;
      const tint = this.tint(definition, boundary.open);
      for (const p of [a, b]) {
        this.built.setMatrixAt(
          index,
          this.box(
            p.clone().add(new Vector3(0, postHeight / 2, 0)),
            new Vector3(wall ? 0.14 : 0.1, postHeight, wall ? 0.14 : 0.1),
          ),
        );
        this.built.setColorAt(index++, tint);
        for (const height of [0.09, postHeight]) {
          this.built.setMatrixAt(
            index,
            this.box(
              p.clone().add(new Vector3(0, height, 0)),
              new Vector3(wall ? 0.22 : 0.16, 0.08, wall ? 0.22 : 0.16),
            ),
          );
          this.built.setColorAt(index++, tint.clone().multiplyScalar(0.65));
        }
      }
      const railDirection = boundary.open
        ? direction.clone().applyAxisAngle(new Vector3(0, 1, 0), Math.PI / 2.4)
        : direction;
      const center = a.clone().addScaledVector(railDirection, span / 2);
      const rotation = new Quaternion().setFromUnitVectors(
        new Vector3(1, 0, 0),
        railDirection,
      );
      if (wall && !boundary.open) {
        this.built.setMatrixAt(
          index,
          this.box(
            center.clone().add(new Vector3(0, 0.54, 0)),
            new Vector3(span - 0.02, 0.96, 0.16),
            rotation,
          ),
        );
        this.built.setColorAt(index++, tint);
        for (const height of [0.12, 1.02]) {
          this.built.setMatrixAt(
            index,
            this.box(
              center.clone().add(new Vector3(0, height, 0)),
              new Vector3(span, 0.1, 0.22),
              rotation,
            ),
          );
          this.built.setColorAt(index++, tint.clone().multiplyScalar(0.78));
        }
        for (const offset of [-0.3, 0, 0.3]) {
          this.built.setMatrixAt(
            index,
            this.box(
              center
                .clone()
                .addScaledVector(direction, span * offset)
                .add(new Vector3(0, 0.54, 0)),
              new Vector3(0.065, 0.82, 0.2),
              rotation,
            ),
          );
          this.built.setColorAt(index++, tint.clone().multiplyScalar(0.85));
        }
      } else {
        const rails = wire
          ? [0.22, 0.44, 0.66]
          : wall
            ? [0.32, 0.72]
            : [0.28, 0.6];
        for (const height of rails) {
          this.built.setMatrixAt(
            index,
            this.box(
              center.clone().add(new Vector3(0, height, 0)),
              new Vector3(span - 0.05, wire ? 0.045 : 0.09, wire ? 0.04 : 0.07),
              rotation,
            ),
          );
          this.built.setColorAt(index++, tint);
        }
      }
      if (definition?.gate) {
        const braceDirection = railDirection
          .clone()
          .multiplyScalar(span * 0.85)
          .add(new Vector3(0, 0.32, 0));
        this.built.setMatrixAt(
          index,
          this.box(
            center.clone().add(new Vector3(0, wall ? 0.54 : 0.44, 0)),
            new Vector3(braceDirection.length(), wall ? 0.1 : 0.06, 0.07),
            new Quaternion().setFromUnitVectors(
              new Vector3(1, 0, 0),
              braceDirection.normalize(),
            ),
          ),
        );
        this.built.setColorAt(index++, tint);
      }
    }
    this.built.count = index;
    this.built.castShadow = true;
    this.built.receiveShadow = true;
    this.built.computeBoundingSphere();
    this.group.add(this.built);
    this.setPreview(this.preview);
    this.setAnchors(this.anchors);
    return true;
  }

  setPreview(preview: BoundaryPreview | null): void {
    this.preview = preview;
    this.drop(this.ghost);
    this.ghost = null;
    if (!preview?.segments.length) return;
    this.marker.color.set(preview.error ? "#ff8077" : "#79e7c0");
    this.ghost = new InstancedMesh(
      this.geometry,
      this.marker,
      preview.segments.length,
    );
    preview.segments.forEach((segment, index) => {
      const { a, b } = this.ends(segment);
      const span = a.distanceTo(b);
      const direction = b.clone().sub(a).normalize();
      this.ghost!.setMatrixAt(
        index,
        this.box(
          a
            .clone()
            .add(b)
            .multiplyScalar(0.5)
            .add(new Vector3(0, 0.87, 0)),
          new Vector3(span + 0.03, 0.09, 0.12),
          new Quaternion().setFromUnitVectors(new Vector3(1, 0, 0), direction),
        ),
      );
    });
    this.ghost.computeBoundingSphere();
    this.group.add(this.ghost);
  }

  /**
   * The vertices a selection is pinned to. Drawn whether or not a run resolves, because the first
   * click of a two-click selection has nothing to preview yet and still has to be visible.
   */
  setAnchors(anchors: readonly BoundaryAnchor[]): void {
    this.anchors = anchors;
    this.drop(this.pins);
    this.pins = null;
    if (!anchors.length) return;
    this.pins = new InstancedMesh(this.geometry, this.pin, anchors.length);
    anchors.forEach((anchor, index) => {
      const centre = axialToPixel(anchor, 1);
      const offset = CORNER_OFFSETS[((anchor.corner % 6) + 6) % 6]!;
      this.pins!.setMatrixAt(
        index,
        this.box(
          new Vector3(
            centre.x + offset[0]!,
            this.heightAt(anchor) + 0.62,
            centre.y + offset[1]!,
          ),
          new Vector3(0.16, 1.24, 0.16),
          new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), Math.PI / 4),
        ),
      );
    });
    this.pins.computeBoundingSphere();
    this.group.add(this.pins);
  }

  private tint(
    definition: BoundaryDefinition | undefined,
    open: boolean,
  ): Color {
    if (definition?.key.startsWith("brick")) return new Color("#b5563f");
    if (definition?.key.startsWith("concrete")) return new Color("#9aa0a4");
    if (definition?.key.startsWith("wire")) return new Color("#9fb0ba");
    if (definition?.family === "wall") return new Color("#8a6234");
    return new Color(open ? "#78bfa9" : "#e2c391");
  }

  /** The grade a vertex stands on: the highest of the three hexes that meet there. */
  private heightAt(anchor: BoundaryAnchor): number {
    const terrain = this.terrain;
    if (!terrain) return FOG_HEIGHT;
    return Math.max(
      ...cornerHexes(anchor).map((cell) => heightAt(terrain, cell.q, cell.r)),
    );
  }

  /**
   * Both ends of a chord. Three of the fifteen are hex edges and the rest cut across the hex's
   * interior, so the ends come from the corner pair rather than from an edge direction.
   */
  private ends(segment: BoundarySegment): { a: Vector3; b: Vector3 } {
    const [first, second] = chordCorners(segment.chord);
    const centre = axialToPixel(segment, 1);
    // One height for the whole chord: a rail is a straight bar, and a run that stepped mid-span
    // would leave a gap under it. The higher of the two ends is the one that keeps it above ground.
    const height = Math.max(
      this.heightAt({ ...segment, corner: first }),
      this.heightAt({ ...segment, corner: second }),
    );
    const at = (corner: number): Vector3 => {
      const offset = CORNER_OFFSETS[corner]!;
      return new Vector3(centre.x + offset[0]!, height, centre.y + offset[1]!);
    };
    return { a: at(first), b: at(second) };
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
    this.drop(this.pins);
    this.geometry.dispose();
    this.wood.dispose();
    this.marker.dispose();
    this.pin.dispose();
  }
}
