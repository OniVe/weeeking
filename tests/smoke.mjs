// Smoke test: boots the built server over stdio (no token needed) and checks
// the tool surface in both modes. Run: npm test
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import path from "node:path";

const SERVER = path.resolve("dist/server.mjs");

function runServer(env, stderrLog) {
  const child = spawn(process.execPath, [SERVER], {
    env: { ...process.env, ...env },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let buf = "";
  let nextId = 0;
  const pending = new Map();

  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    buf += chunk;
    let nl;
    while ((nl = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, nl).trim();
      buf = buf.slice(nl + 1);
      if (!line) continue;
      let msg;
      try {
        msg = JSON.parse(line);
      } catch {
        continue;
      }
      const waiter = msg.id ? pending.get(msg.id) : undefined;
      if (waiter) {
        pending.delete(msg.id);
        msg.error ? waiter.reject(new Error(JSON.stringify(msg.error))) : waiter.resolve(msg.result);
      }
    }
  });
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (d) => stderrLog.push(String(d)));
  child.on("exit", (code) => {
    for (const waiter of pending.values()) {
      waiter.reject(new Error(`server exited (${code}): ${stderrLog.join("").slice(-500)}`));
    }
    pending.clear();
  });

  const send = (method, params) =>
    new Promise((resolve, reject) => {
      const id = ++nextId;
      pending.set(id, { resolve, reject });
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    });

  const notify = (method) => child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method }) + "\n");

  return { child, send, notify };
}

async function bootstrap(env, stderrLog) {
  const s = runServer(env, stderrLog);
  await s.send("initialize", {
    protocolVersion: "2025-06-18",
    capabilities: {},
    clientInfo: { name: "weeeking-smoke", version: "1.0.0" },
  });
  s.notify("notifications/initialized");
  const { tools } = await s.send("tools/list", {});
  return { ...s, tools };
}

const guard = setTimeout(() => {
  console.error("smoke: TIMEOUT");
  process.exit(1);
}, 60000);

try {
  // 1. Default run: read-only, no token.
  const stderr1 = [];
  const ro = await bootstrap({ WEEEK_API_TOKEN: "", WEEEK_TOKEN: "", READ_ONLY: "", WEEEK_DISABLE_KEYCHAIN: "1" }, stderr1);
  const names = ro.tools.map((t) => t.name);

  for (const expected of ["weeek_context", "weeek_search_tasks", "weeek_get_task", "weeek_project", "weeek_board", "weeek_deals", "weeek_custom_fields", "weeek_board_column"]) {
    assert.ok(names.includes(expected), `read-only: missing tool ${expected} (have: ${names.join(", ")})`);
  }
  assert.ok(names.length >= 15, `read-only: expected >=15 tools, got ${names.length}`);
  assert.ok(!names.includes("weeek_create_task"), "read-only: write tool must be hidden");

  const project = ro.tools.find((t) => t.name === "weeek_project");
  assert.ok(project, "weeek_project tool missing");
  assert.ok(project.description.includes("get-project"), "weeek_project must describe get-project");
  assert.ok(!project.description.includes("create-project"), "read-only project tool must not offer create-project");

  const call = await ro.send("tools/call", { name: "weeek_context", arguments: {} });
  assert.equal(call.isError, true, "no-token call must return isError");
  assert.ok(call.content[0].text.includes("WEEEK_API_TOKEN"), "no-token error must mention WEEEK_API_TOKEN");
  ro.child.kill();

  // 2. Writable run: writes appear.
  const stderr2 = [];
  const rw = await bootstrap({ WEEEK_API_TOKEN: "", WEEEK_TOKEN: "", READ_ONLY: "false", WEEEK_DISABLE_KEYCHAIN: "1" }, stderr2);
  const rwNames = rw.tools.map((t) => t.name);
  for (const expected of ["weeek_create_task", "weeek_update_task", "weeek_move_task", "weeek_complete_task", "weeek_add_comment", "weeek_set_task_people", "weeek_task"]) {
    assert.ok(rwNames.includes(expected), `read-write: missing tool ${expected}`);
  }
  assert.ok(rwNames.length > names.length, "read-write must expose more tools than read-only");
  const projectRw = rw.tools.find((t) => t.name === "weeek_project");
  assert.ok(projectRw.description.includes("create-project"), "writable project tool must offer create-project");
  rw.child.kill();

  console.error(`smoke: OK — read-only ${names.length} tools, read-write ${rwNames.length} tools`);
  clearTimeout(guard);
  process.exit(0);
} catch (e) {
  console.error("smoke: FAILED —", e.message);
  clearTimeout(guard);
  process.exit(1);
}
