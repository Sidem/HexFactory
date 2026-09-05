import {
  BoxGeometry,
  CylinderGeometry,
  TorusGeometry,
  type BufferGeometry,
} from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

/** Authored mechanical assemblies, merged once into the existing instancing vocabulary. */
function join(pieces: BufferGeometry[]): BufferGeometry {
  const result = mergeGeometries(pieces, false);
  for (const piece of pieces) piece.dispose();
  if (!result) throw new Error("Incompatible machine anatomy");
  return result;
}

export function vesselAssembly(): BufferGeometry {
  const pieces: BufferGeometry[] = [
    new CylinderGeometry(0.82, 0.82, 1.04, 12),
    new CylinderGeometry(0.48, 0.82, 0.24, 12).translate(0, 0.58, 0),
    new CylinderGeometry(0.98, 0.98, 0.12, 12).translate(0, -0.6, 0),
    new CylinderGeometry(0.32, 0.32, 0.08, 12).translate(0, 0.73, 0),
  ];
  for (const y of [-0.42, 0.38]) {
    pieces.push(
      new TorusGeometry(0.83, 0.055, 4, 12)
        .rotateX(Math.PI / 2)
        .translate(0, y, 0),
    );
  }
  for (let i = 0; i < 6; i++) {
    const angle = (i * Math.PI) / 3;
    pieces.push(
      new BoxGeometry(0.13, 0.92, 0.12)
        .translate(0, -0.09, 0.83)
        .rotateY(angle),
    );
  }
  // Proud service hatch and its recessed-looking concentric flange.
  pieces.push(
    new CylinderGeometry(0.25, 0.25, 0.13, 12)
      .rotateX(Math.PI / 2)
      .translate(0, 0, 0.84),
  );
  pieces.push(new TorusGeometry(0.22, 0.045, 4, 12).translate(0, 0, 0.93));
  return join(pieces);
}

export function chamberAssembly(): BufferGeometry {
  const pieces: BufferGeometry[] = [
    new BoxGeometry(1.42, 1.12, 1.34),
    new BoxGeometry(1.78, 0.14, 1.65).translate(0, -0.65, 0),
    new CylinderGeometry(0.75, 1.02, 0.24, 4)
      .rotateY(Math.PI / 4)
      .translate(0, 0.64, 0),
  ];
  for (const x of [-0.77, 0.77]) {
    for (const z of [-0.7, 0.7])
      pieces.push(new BoxGeometry(0.16, 1.22, 0.16).translate(x, -0.02, z));
  }
  // A panel surrounded by a substantial frame, with louvres on both side faces.
  for (const x of [-0.49, 0.49])
    pieces.push(new BoxGeometry(0.08, 0.78, 0.12).translate(x, -0.04, 0.72));
  for (const y of [-0.43, 0.35])
    pieces.push(new BoxGeometry(1.06, 0.08, 0.12).translate(0, y, 0.72));
  for (const x of [-0.74, 0.74]) {
    for (let row = 0; row < 4; row++)
      pieces.push(
        new BoxGeometry(0.1, 0.07, 0.8).translate(x, -0.28 + row * 0.18, 0),
      );
  }
  pieces.push(new BoxGeometry(0.2, 0.08, 0.1).translate(0.24, 0, 0.75));
  return join(pieces);
}

export function stackAssembly(): BufferGeometry {
  const pieces: BufferGeometry[] = [
    new CylinderGeometry(0.38, 0.5, 1.86, 10, 1, true),
  ];
  for (const y of [-0.85, -0.12, 0.87])
    pieces.push(
      new TorusGeometry(y < 0 ? 0.49 : 0.4, 0.09, 4, 10)
        .rotateX(Math.PI / 2)
        .translate(0, y, 0),
    );
  // Recessed throat leaves an actual opening visible from the diorama camera.
  pieces.push(new CylinderGeometry(0.3, 0.3, 0.04, 10).translate(0, 0.56, 0));
  return join(pieces);
}
