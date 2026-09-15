import {
  BoxGeometry,
  CylinderGeometry,
  LatheGeometry,
  SphereGeometry,
  TorusGeometry,
  Vector2,
  type BufferGeometry,
} from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import type { ModelGeometry } from "../modelParts";

function joined(parts: BufferGeometry[]): BufferGeometry {
  const result = mergeGeometries(parts, false)!;
  for (const part of parts) part.dispose();
  return result;
}

/** Shared, deliberately plain solids, centred inside a unit cube. */
export function elementaryGeometry(kind: ModelGeometry): BufferGeometry {
  switch (kind) {
    case "box":
      return new BoxGeometry(1, 1, 1);
    case "cylinder":
      return new CylinderGeometry(0.5, 0.5, 1, 12);
    case "cone":
      return new CylinderGeometry(0.22, 0.5, 1, 9);
    case "dome":
      return new SphereGeometry(0.5, 12, 6, 0, Math.PI * 2, 0, Math.PI / 2)
        .scale(1, 2, 1)
        .translate(0, -0.5, 0);
    case "crucible":
      return new LatheGeometry(
        [
          [0, -0.5],
          [0.33, -0.5],
          [0.5, 0.39],
          [0.5, 0.5],
          [0.38, 0.5],
          [0.27, -0.32],
          [0, -0.32],
        ].map(([x, y]) => new Vector2(x!, y!)),
        12,
      );
    case "crate":
      return joined([
        new BoxGeometry(1, 0.12, 1).translate(0, -0.44, 0),
        ...[-0.44, 0.44].map((x) =>
          new BoxGeometry(0.12, 1, 1).translate(x, 0, 0),
        ),
        ...[-0.44, 0.44].map((z) =>
          new BoxGeometry(0.76, 1, 0.12).translate(0, 0, z),
        ),
      ]);
    case "arch":
      return joined([
        new BoxGeometry(1, 0.16, 1).translate(0, 0.42, 0),
        ...[-0.42, 0.42].map((z) =>
          new BoxGeometry(1, 0.84, 0.16).translate(0, -0.08, z),
        ),
      ]);
    case "wheel":
      return joined([
        new TorusGeometry(0.42, 0.08, 6, 12)
          .rotateX(Math.PI / 2)
          .scale(1, 4, 1),
        new CylinderGeometry(0.13, 0.13, 1, 8),
        new BoxGeometry(0.85, 0.5, 0.08),
        new BoxGeometry(0.08, 0.5, 0.85),
      ]);
    case "claw":
      return joined([
        new BoxGeometry(0.85, 0.2, 0.4).translate(0, 0.4, 0),
        ...[-1, 1].flatMap((side) => [
          new BoxGeometry(0.18, 0.8, 0.32)
            .rotateZ(side * -0.2)
            .translate(side * 0.35, 0, 0),
          new BoxGeometry(0.28, 0.16, 0.32).translate(side * 0.23, -0.42, 0),
        ]),
      ]);
  }
}
