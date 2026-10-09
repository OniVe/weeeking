"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const fsp = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");

const PACKAGE = require("../package.json");

/** GitHub repo that hosts the release assets. */
const REPO = "OniVe/weeeking";

const DOWNLOAD_TIMEOUT_MS = 120_000;
const CHECKSUM_TIMEOUT_MS = 30_000;
const ATTEMPTS = 3;

const SUPPORTED = "Windows x64/arm64, Linux x64 (gnu/musl), Linux arm64, macOS arm64/x64";

/** Linux x64 поставляется в двух вариантах libc; остальные — по platform-arch. */
function detectLibc(report) {
  let value = report;
  if (value === undefined) {
    value = process.report && typeof process.report.getReport === "function"
      ? process.report.getReport()
      : null;
  }
  const header = value && value.header ? value.header : {};
  return header.glibcVersionRuntime ? "gnu" : "musl";
}

/**
 * Цель загрузки для платформы: `{ key, asset(version), binary }`
 * или `null` для неподдерживаемой платформы.
 */
function resolveTarget({ platform = process.platform, arch = process.arch, libc } = {}) {
  const key = `${platform}-${arch}`;
  switch (key) {
    case "win32-x64":
      return { key, asset: (version) => `weeeking-${version}-win-x64.exe`, binary: "weeeking.exe" };
    case "win32-arm64":
      return { key, asset: (version) => `weeeking-${version}-win-arm64.exe`, binary: "weeeking.exe" };
    case "linux-x64": {
      const flavor = libc === undefined ? detectLibc() : libc;
      return flavor === "musl"
        ? {
            key: "linux-musl-x64",
            asset: (version) => `weeeking-${version}-linux-musl-x64`,
            binary: "weeeking",
          }
        : {
            key: "linux-x64",
            asset: (version) => `weeeking-${version}-linux-x64`,
            binary: "weeeking",
          };
    }
    case "linux-arm64":
      return { key, asset: (version) => `weeeking-${version}-linux-arm64`, binary: "weeeking" };
    case "darwin-arm64":
      return { key: "osx-arm64", asset: (version) => `weeeking-${version}-osx-arm64`, binary: "weeeking" };
    case "darwin-x64":
      return { key: "osx-x64", asset: (version) => `weeeking-${version}-osx-x64`, binary: "weeeking" };
    default:
      return null;
  }
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
  const assetName = target.asset(PACKAGE.version);
  const assetUrl = `${downloadBaseUrl()}/${assetName}`;
  log(`скачиваю ${assetUrl}`);
  const checksum = parseChecksum(
    (await fetchChecked(`${assetUrl}.sha256`, CHECKSUM_TIMEOUT_MS)).toString("utf8"),
  );
  const payload = await fetchChecked(assetUrl, DOWNLOAD_TIMEOUT_MS);
  const actual = sha256(payload);
  if (actual !== checksum) {
    throw new Error(
      `sha256 не совпал для ${assetName}: ожидался ${checksum}, получен ${actual}`,
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
 * `~/.weeeking/<version>/<key>/` cache; downloads with sha256 verification on first use.
 * Тесты могут подменить платформу (`platform`/`arch`/`libc`) без изменения окружения.
 */
async function ensureBinary({ log = () => {}, platform, arch, libc } = {}) {
  const override = process.env.WEEEKING_BINARY;
  if (override) {
    return override;
  }
  const target = resolveTarget({ platform, arch, libc });
  if (!target) {
    const shownPlatform = platform ?? process.platform;
    const shownArch = arch ?? process.arch;
    throw new Error(
      `платформа ${shownPlatform}-${shownArch} не поддерживается (доступны: ${SUPPORTED}); ` +
        "задайте WEEEKING_BINARY, чтобы использовать свой бинарник",
    );
  }
  const vendor = path.join(packageRoot(), "vendor", target.key, target.binary);
  const cache = path.join(os.homedir(), ".weeeking", PACKAGE.version, target.key, target.binary);
  for (const candidate of [vendor, cache]) {
    if (await isVerifiedFile(candidate)) {
      return candidate;
    }
  }
  const destination = await writableDestination(vendor, cache);
  return downloadBinary(target, destination, log);
}

module.exports = {
  ensureBinary,
  resolveTarget,
  detectLibc,
  parseChecksum,
  releaseTag,
  downloadBaseUrl,
};
