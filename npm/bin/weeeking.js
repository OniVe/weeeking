#!/usr/bin/env node
"use strict";

const { spawn } = require("node:child_process");
const { ensureBinary } = require("../lib/binary.js");

/** POSIX-номера сигналов для конвенции 128+N (на Windows Node не передаёт signal). */
const SIGNAL_CODES = { SIGHUP: 1, SIGINT: 2, SIGQUIT: 3, SIGTERM: 15 };

async function main() {
  let binary;
  try {
    binary = await ensureBinary({ log: (line) => console.error(`weeeking: ${line}`) });
  } catch (error) {
    console.error(`weeeking: ${error.message}`);
    // Не рвём процесс мгновенно: даём stderr догрузиться, код выхода — 1.
    process.exitCode = 1;
    return;
  }

  const child = spawn(binary, process.argv.slice(2), {
    stdio: "inherit",
    windowsHide: true,
  });

  const forward = (signal) => {
    if (!child.killed) {
      child.kill(signal);
    }
  };
  process.on("SIGINT", () => forward("SIGINT"));
  process.on("SIGTERM", () => forward("SIGTERM"));

  child.on("error", (error) => {
    console.error(`weeeking: не удалось запустить ${binary}: ${error.message}`);
    process.exitCode = 1;
  });
  child.on("exit", (code, signal) => {
    if (code === null && signal && Object.hasOwn(SIGNAL_CODES, signal)) {
      process.exit(128 + SIGNAL_CODES[signal]);
    }
    process.exit(code === null ? 1 : code);
  });
}

main();
