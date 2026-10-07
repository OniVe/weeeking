// Bundles src/server.ts into dist/server.mjs (single file, ES module).
import { build } from "esbuild";

await build({
  entryPoints: ["src/server.ts"],
  outfile: "dist/server.mjs",
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node20",
  sourcemap: false,
  external: ["@napi-rs/keyring"],
  banner: { js: "#!/usr/bin/env node" },
  logLevel: "info",
});
