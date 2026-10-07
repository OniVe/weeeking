import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { loadConfig } from "./env.js";
import { WeeekClient } from "./http.js";
import { readKeychainToken } from "./token.js";
import { registerCuratedTools } from "./tools/curated.js";
import { registerGeneratedTools } from "./tools/generated.js";

const cfg = loadConfig();
let tokenSource: "env" | "keychain" | "none" = cfg.token ? "env" : "none";
if (!cfg.token) {
  const stored = await readKeychainToken();
  if (stored) {
    cfg.token = stored;
    tokenSource = "keychain";
  }
}

const client = new WeeekClient(cfg);
const server = new McpServer({ name: "weeeking", version: "0.1.0" });

const options = { readOnly: cfg.readOnly, maxChars: cfg.maxChars };
registerCuratedTools(server, client, options);
registerGeneratedTools(server, client, options);

await server.connect(new StdioServerTransport());
console.error(
  `[weeeking] готов. READ_ONLY=${cfg.readOnly}, токен: ${tokenSource}, API ${cfg.baseUrl}`,
);
