import { after, beforeEach, test } from "node:test";
import assert from "node:assert/strict";
import crypto from "node:crypto";
import fsp from "node:fs/promises";
import http from "node:http";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const binary = (
  await import(pathToFileURL(path.join(here, "..", "lib", "binary.js")))
).default;
const vendorDir = path.resolve(here, "..", "vendor");

let savedBinaryEnv;

function startServer(routes) {
  let requests = 0;
  const server = http.createServer((request, response) => {
    requests += 1;
    const handler = routes[request.url];
    if (!handler) {
      response.writeHead(404, { "content-type": "text/plain" });
      response.end("not found");
      return;
    }
    const { body, status = 200 } = handler;
    response.writeHead(status, { "content-type": "application/octet-stream" });
    response.end(body);
  });
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      resolve({
        url: `http://127.0.0.1:${port}`,
        requests: () => requests,
        close: () => new Promise((done) => server.close(done)),
      });
    });
  });
}

beforeEach(async () => {
  savedBinaryEnv = process.env.WEEEKING_BINARY;
  delete process.env.WEEEKING_BINARY;
  await fsp.rm(vendorDir, { recursive: true, force: true });
});

after(async () => {
  if (savedBinaryEnv !== undefined) {
    process.env.WEEEKING_BINARY = savedBinaryEnv;
  }
  await fsp.rm(vendorDir, { recursive: true, force: true });
});

test("maps supported platforms only", () => {
  assert.equal(binary.targetKey("win32", "x64"), "win32-x64");
  assert.equal(binary.targetKey("linux", "x64"), "linux-x64");
  assert.equal(binary.targetKey("darwin", "arm64"), null);
  assert.equal(binary.targetKey("linux", "arm64"), null);
});

test("parses sha256 files", () => {
  const hash = "a".repeat(64);
  assert.equal(binary.parseChecksum(`${hash}  weeeking-linux-x64\n`), hash);
  assert.equal(binary.parseChecksum(`${hash.toUpperCase()} *file`), hash);
  assert.throws(() => binary.parseChecksum("no hash here"), /контрольной суммы/);
});

test("downloads and verifies the binary for the current platform", async () => {
  const key = binary.targetKey();
  assert.ok(key, "тестовое окружение должно быть win32-x64 или linux-x64");
  const target = binary.TARGETS[key];
  const payload = crypto.randomBytes(2048);
  const checksum = crypto.createHash("sha256").update(payload).digest("hex");
  const server = await startServer({
    [`/${target.asset}`]: { body: payload },
    [`/${target.asset}.sha256`]: { body: `${checksum}  ${target.asset}\n` },
  });
  process.env.WEEEKING_DOWNLOAD_BASE = server.url;
  try {
    const location = await binary.ensureBinary();
    assert.equal(location, path.join(vendorDir, target.binary));
    assert.deepEqual(await fsp.readFile(location), payload);
    if (process.platform !== "win32") {
      assert.notEqual((await fsp.stat(location)).mode & 0o111, 0, "бинарник должен быть исполняемым");
    }
  } finally {
    delete process.env.WEEEKING_DOWNLOAD_BASE;
    await server.close();
  }
});

test("trusts a verified local copy and heals a corrupted one", async () => {
  const key = binary.targetKey();
  const target = binary.TARGETS[key];
  const payload = crypto.randomBytes(2048);
  const checksum = crypto.createHash("sha256").update(payload).digest("hex");
  const server = await startServer({
    [`/${target.asset}`]: { body: payload },
    [`/${target.asset}.sha256`]: { body: `${checksum}  ${target.asset}\n` },
  });
  process.env.WEEEKING_DOWNLOAD_BASE = server.url;
  try {
    const first = await binary.ensureBinary();
    const afterFirst = server.requests();
    const second = await binary.ensureBinary();
    assert.equal(second, first, "повторный запуск использует тот же файл");
    assert.equal(server.requests(), afterFirst, "валидный кэш не перекачивается");

    await fsp.writeFile(first, "corrupted");
    const healed = await binary.ensureBinary();
    assert.deepEqual(await fsp.readFile(healed), payload, "битый файл перекачивается");
    assert.ok(server.requests() > afterFirst, "self-heal ходит в сеть");
  } finally {
    delete process.env.WEEEKING_DOWNLOAD_BASE;
    await server.close();
  }
});

test("fails closed on checksum mismatch", async () => {
  const key = binary.targetKey();
  const target = binary.TARGETS[key];
  const payload = crypto.randomBytes(256);
  const wrong = crypto.randomBytes(256);
  const server = await startServer({
    [`/${target.asset}`]: { body: payload },
    [`/${target.asset}.sha256`]: {
      body: `${crypto.createHash("sha256").update(wrong).digest("hex")}  ${target.asset}\n`,
    },
  });
  process.env.WEEEKING_DOWNLOAD_BASE = server.url;
  try {
    await assert.rejects(binary.ensureBinary(), /sha256 не совпал/);
  } finally {
    delete process.env.WEEEKING_DOWNLOAD_BASE;
    await server.close();
  }
});

test("reports missing release assets", async () => {
  const server = await startServer({});
  process.env.WEEEKING_DOWNLOAD_BASE = server.url;
  try {
    await assert.rejects(binary.ensureBinary(), /HTTP 404/);
  } finally {
    delete process.env.WEEEKING_DOWNLOAD_BASE;
    await server.close();
  }
});
