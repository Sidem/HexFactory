# Browser route

Start at `docs/AGENT-MAP.md` and load only the task route.

- `core/` transports commands and snapshots; it never simulates player or factory state.
- `ui/` owns reusable DOM controllers and keyed reconciliation.
- `main.ts` is composition and event wiring. New self-contained state or view behavior belongs in a
  focused module rather than another global and another render branch.
- `rendering/` consumes snapshots. Picking stays on the logical axial plane.
- Lists with controls use `syncChildren`; never rebuild a pressed control.
- Run the nearest Vitest file, then `npm run test:run` and `npm run typecheck`.
