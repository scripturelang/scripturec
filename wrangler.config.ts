import { defineWranglerConfig } from "wrangler/experimental-config";

// cf uses Wrangler only as the static asset build tool.
export default defineWranglerConfig({ assetsDirectory: "./web" });
