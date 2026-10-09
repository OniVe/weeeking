"use strict";

const { ensureBinary } = require("../lib/binary.js");

if (process.env.WEEEKING_SKIP_DOWNLOAD || process.env.WEEEKING_BINARY) {
  console.error(
    "weeeking: скачивание бинарника пропущено (WEEEKING_SKIP_DOWNLOAD / WEEEKING_BINARY)",
  );
  process.exit(0);
}

ensureBinary({ log: (line) => console.error(`weeeking: ${line}`) })
  .then((location) => {
    console.error(`weeeking: бинарник готов: ${location}`);
  })
  .catch((error) => {
    // Установку не валим: launcher повторит попытку при первом запуске.
    console.error(`weeeking: не удалось скачать бинарник при установке (${error.message}).`);
    console.error(
      "weeeking: повторю при первом запуске; зеркало — WEEEKING_DOWNLOAD_BASE, свой бинарник — WEEEKING_BINARY.",
    );
  });
