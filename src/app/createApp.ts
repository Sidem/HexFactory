import "./coreView";
import "./buildController";
import "./inspectorOverview";
import "./inspectorControls";
import "./workspaceController";
import "./constructionInput";
import "./lifecycle";
import { bootstrap } from "./bootstrap";
import { workspaceWiring } from "./workspaceWiring";
import { buildWiring } from "./buildWiring";
import { inputWiring } from "./inputWiring";
import { lifecycleWiring } from "./lifecycleWiring";
import { Runtime } from "./runtime";

export async function createApp(): Promise<Runtime> {
  const app = new Runtime();
  await bootstrap(app);
  await workspaceWiring(app);
  await buildWiring(app);
  await inputWiring(app);
  await lifecycleWiring(app);
  return app;
}
