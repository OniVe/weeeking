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
const version = binary.releaseTag().slice(1);

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

test("resolves supported platforms and asset names", () => {
  assert.equal(
    binary.resolveTarget({ platform: "win32", arch: "x64" }).asset(version),
    `weeeking-${version}-win-x64.exe`,
  );
  assert.equal(
    binary.resolveTarget({ platform: "linux", arch: "x64", libc: "gnu" }).asset(version),
    `weeeking-${version}-linux-x64`,
  );
  assert.equal(
    binary.resolveTarget({ platform: "linux", arch: "x64", libc: "musl" }).asset(version),
    `weeeking-${version}-linux-musl-x64`,
  );
  assert.equal(
    binary.resolveTarget({ platform: "linux", arch: "arm64", libc: "gnu" }).asset(version),
    `weeeking-${version}-linux-arm64`,
  );
  assert.equal(
    binary.resolveTarget({ platform: "darwin", arch: "arm64" }).asset(version),
    `weeeking-${version}-osx-arm64`,
  );
  assert.equal(
    binary.resolveTarget({ platform: "darwin", arch: "x64" }).asset(version),
    `weeeking-${version}-osx-x64`,
  );
  assert.equal(binary.resolveTarget({ platform: "freebsd", arch: "x64" }), null);
  assert.equal(binary.resolveTarget({ platform: "linux", arch: "ia32" }), null);
});

test("detects the libc flavor", () => {
  assert.equal(binary.detectLibc({ header: { glibcVersionRuntime: "2.39" } }), "gnu");
  assert.equal(binary.detectLibc({ header: {} }), "musl");
  assert.equal(binary.detectLibc(null), "musl");
});

test("parses sha256 files", () => {
  const hash = "a".repeat(64);
  assert.equal(binary.parseChecksum(`${hash}  weeeking-${version}-linux-x64\n`), hash);
  assert.equal(binary.parseChecksum(`${hash.toUpperCase()} *file`), hash);
  assert.throws(() => binary.parseChecksum("no hash here"), /контрольной суммы/);
});

test("downloads and verifies the binary for the current platform", async () => {
  const target = binary.resolveTarget();
  assert.ok(target, "тестовое окружение должно быть одной из поддерживаемых платформ");
  const assetName = target.asset(version);
  const payload = crypto.randomBytes(2048);
  const checksum = crypto.createHash("sha256").update(payload).digest("hex");
  const server = await startServer({
    [`/${assetName}`]: { body: payload },
    [`/${assetName}.sha256`]: { body: `${checksum}  ${assetName}\n` },
  });
  process.env.WEEEKING_DOWNLOAD_BASE = server.url;
  try {
    const location = await binary.ensureBinary();
    assert.equal(location, path.join(vendorDir, target.key, target.binary));
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
  const target = binary.resolveTarget();
  const assetName = target.asset(version);
  const payload = crypto.randomBytes(2048);
  const checksum = crypto.createHash("sha256").update(payload).digest("hex");
  const server = await startServer({
    [`/${assetName}`]: { body: payload },
    [`/${assetName}.sha256`]: { body: `${checksum}  ${assetName}\n` },
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
  const target = binary.resolveTarget();
  const assetName = target.asset(version);
  const payload = crypto.randomBytes(256);
  const wrong = crypto.randomBytes(256);
  const server = await startServer({
    [`/${assetName}`]: { body: payload },
    [`/${assetName}.sha256`]: {
      body: `${crypto.createHash("sha256").update(wrong).digest("hex")}  ${assetName}\n`,
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

test("reports unsupported platforms", async () => {
  const error = await binary
    .ensureBinary({ platform: "freebsd", arch: "x64" })
    .then(() => null, (problem) => problem);
  assert.ok(error, "ожидалась ошибка для неподдерживаемой платформы");
  assert.match(error.message, /не поддерживается/);
});
