"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const fsp = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");

const PACKAGE = require("../package.json");

/** GitHub repo that hosts the release assets. */
const REPO = "OniVe/weeeking";

/** Supported targets: node's `${platform}-${arch}` -> release asset and local binary name. */
const TARGETS = {
  "win32-x64": { asset: "weeeking-win32-x64.exe", binary: "weeeking.exe" },
  "linux-x64": { asset: "weeeking-linux-x64", binary: "weeeking" },
};

const DOWNLOAD_TIMEOUT_MS = 120_000;
const CHECKSUM_TIMEOUT_MS = 30_000;
const ATTEMPTS = 3;

function targetKey(platform = process.platform, arch = process.arch) {
  const key = `${platform}-${arch}`;
  return Object.hasOwn(TARGETS, key) ? key : null;
}

function releaseTag() {
  return `v${PACKAGE.version}`;
}

function downloadBaseUrl() {
  const override = process.env.WEEEKING_DOWNLOAD_BASE;
  if (override) {
    return override.replace(/\/+$/, "");
  }
  return `https://github.com/${REPO}/releases/download/${releaseTag()}`;
}

function packageRoot() {
  return path.resolve(__dirname, "..");
}

async function isFile(file) {
  try {
    return (await fsp.stat(file)).isFile();
  } catch {
    return false;
  }
}

/** Локальный бинарник доверяем только при совпадении с сайдкар-хэшем рядом. */
async function isVerifiedFile(candidate) {
  if (!(await isFile(candidate))) {
    return false;
  }
  try {
    const expected = parseChecksum(await fsp.readFile(`${candidate}.sha256`, "utf8"));
    return sha256(await fsp.readFile(candidate)) === expected;
  } catch {
    return false;
  }
}

async function fetchChecked(url, timeout) {
  let lastError;
  for (let attempt = 1; attempt <= ATTEMPTS; attempt += 1) {
    try {
      // node >=18: global fetch; follows redirects, honours NODE_USE_ENV_PROXY when set.
      const response = await fetch(url, { signal: AbortSignal.timeout(timeout) });
      if (!response.ok) {
        const error = new Error(`HTTP ${response.status} (${url})`);
        // 4xx — окончательная ошибка (нет смысла повторять 404/403).
        error.nonRetryable = response.status >= 400 && response.status < 500;
        throw error;
      }
      return Buffer.from(await response.arrayBuffer());
    } catch (error) {
      lastError = error;
      if (error && error.nonRetryable) {
        break;
      }
      if (attempt < ATTEMPTS) {
        await new Promise((resolve) => setTimeout(resolve, 300 * attempt));
      }
    }
  }
  throw new Error(`не удалось скачать ${url}: ${lastError && lastError.message ? lastError.message : lastError}`);
}

function parseChecksum(text) {
  const match = String(text).match(/\b([0-9a-fA-F]{64})\b/);
  if (!match) {
    throw new Error("некорректный файл контрольной суммы (.sha256)");
  }
  return match[1].toLowerCase();
}

function sha256(buffer) {
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

async function downloadBinary(target, destination, log) {
  const assetUrl = `${downloadBaseUrl()}/${target.asset}`;
  log(`скачиваю ${assetUrl}`);
  const checksum = parseChecksum(
    (await fetchChecked(`${assetUrl}.sha256`, CHECKSUM_TIMEOUT_MS)).toString("utf8"),
  );
  const payload = await fetchChecked(assetUrl, DOWNLOAD_TIMEOUT_MS);
  const actual = sha256(payload);
  if (actual !== checksum) {
    throw new Error(
      `sha256 не совпал для ${target.asset}: ожидался ${checksum}, получен ${actual}`,
    );
  }
  await fsp.mkdir(path.dirname(destination), { recursive: true });
  const temp = `${destination}.tmp-${process.pid}`;
  try {
    await fsp.writeFile(temp, payload, { mode: 0o755 });
    await fsp.rename(temp, destination);
  } finally {
    // rename перенёс файл — rm тихо ничего не сделает; при сбое подчистим temp.
    await fsp.rm(temp, { force: true }).catch(() => {});
  }
  await fsp.writeFile(
    `${destination}.sha256`,
    `${checksum}  ${path.basename(destination)}\n`,
  );
  if (process.platform !== "win32") {
    await fsp.chmod(destination, 0o755);
  }
  return destination;
}

async function writableDestination(vendor, cache) {
  try {
    await fsp.mkdir(path.dirname(vendor), { recursive: true });
    await fsp.access(path.dirname(vendor), fs.constants.W_OK);
    return vendor;
  } catch {
    await fsp.mkdir(path.dirname(cache), { recursive: true });
    return cache;
  }
}

/**
 * Resolves the native weeeking binary for the current platform.
 *
 * Order: `WEEEKING_BINARY` override -> `vendor/` inside the package ->
 * `~/.weeeking/<version>/` cache; downloads with sha256 verification on first use.
 */
async function ensureBinary({ log = () => {} } = {}) {
  const override = process.env.WEEEKING_BINARY;
  if (override) {
    return override;
  }
  const key = targetKey();
  if (!key) {
    throw new Error(
      `платформа ${process.platform}-${process.arch} не поддерживается (доступны Windows x64 и Linux x64); ` +
        "задайте WEEEKING_BINARY, чтобы использовать свой бинарник",
    );
  }
  const target = TARGETS[key];
  const vendor = path.join(packageRoot(), "vendor", target.binary);
  const cache = path.join(os.homedir(), ".weeeking", PACKAGE.version, target.binary);
  for (const candidate of [vendor, cache]) {
    if (await isVerifiedFile(candidate)) {
      return candidate;
    }
  }
  const destination = await writableDestination(vendor, cache);
  return downloadBinary(target, destination, log);
}

module.exports = {
  TARGETS,
  ensureBinary,
  targetKey,
  parseChecksum,
  releaseTag,
  downloadBaseUrl,
};
