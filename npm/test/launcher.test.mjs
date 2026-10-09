import { test } from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

const run = promisify(execFile);
const launcher = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "bin",
  "weeeking.js",
);

function invoke(args, env) {
  return run(process.execPath, [launcher, ...args], {
    env: { ...process.env, WEEEKING_BINARY: process.execPath, ...env },
    windowsHide: true,
  });
}

test("passes through the child exit code", async () => {
  const result = await invoke(["-e", "process.exit(7)"]).then(
    () => ({ code: 0 }),
    (error) => ({ code: error.code }),
  );
  assert.equal(result.code, 7);
});

test("keeps stdout clean for MCP", async () => {
  const { stdout, stderr } = await invoke(["-e", "process.stdout.write('payload')"]);
  assert.equal(stdout, "payload");
  assert.equal(stderr, "");
});

test("reports a missing binary clearly", async () => {
  const error = await run(process.execPath, [launcher], {
    env: {
      ...process.env,
      WEEEKING_BINARY: path.join(os.tmpdir(), "definitely-missing-weeeking-binary"),
    },
  }).then(
    () => null,
    (problem) => problem,
  );
  assert.ok(error, "ожидался ненулевой код возврата");
  assert.equal(error.code, 1);
  assert.match(error.stderr, /не удалось запустить/);
});

test(
  "maps child signals to POSIX exit codes",
  { skip: process.platform === "win32" ? "POSIX only (Node не передаёт signal на Windows)" : false },
  async () => {
    const { spawn } = await import("node:child_process");
    for (const [signal, expected] of [
      ["SIGINT", 130],
      ["SIGTERM", 143],
      ["SIGHUP", 129],
      ["SIGQUIT", 131],
    ]) {
      const child = spawn(
        process.execPath,
        [launcher, "-e", `setTimeout(() => process.kill(process.pid, "${signal}"), 50)`],
        {
          env: { ...process.env, WEEEKING_BINARY: process.execPath },
          stdio: "ignore",
        },
      );
      const code = await new Promise((resolve) => child.on("exit", (code) => resolve(code)));
      assert.equal(code, expected, `${signal} должен давать ${expected}, получено ${code}`);
    }
  },
);
