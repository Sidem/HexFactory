import type { FactoryHost } from "../core/FactoryHost";
import { SAVE_VERSION, type CurrentBuild } from "../core/saveSlots";
import type { FactorySnapshot } from "../core/types";

/** The exact native/catalogue envelopes this browser build can load. */
export function currentBuild(
  host: FactoryHost,
  snapshot: FactorySnapshot,
): CurrentBuild {
  return {
    versions: {
      save: SAVE_VERSION,
      world: snapshot.world_version,
      definitions: host.definitions.version,
      technology: host.technologies.version,
    },
    scenarios: host.scenarios.scenarios.map((scenario) => ({
      key: scenario.key,
      name: scenario.name,
      version: scenario.version,
    })),
    worldPresets: host.worldPresets,
  };
}
