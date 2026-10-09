#!/usr/bin/env python3
"""Единый источник списка CI-платформ (.github/platforms.json).

Использование:
  platforms.py matrix          — JSON-массив для strategy.matrix (job platforms)
  platforms.py names <версия>  — имена ассетов релиза, по одному в строке:
                                 архив, архив.sha256, сырой бинарник, бинарник.sha256
"""

import json
import pathlib
import sys

DATA = json.loads(
    (pathlib.Path(__file__).resolve().parent.parent / "platforms.json").read_text(encoding="utf-8")
)


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit("usage: platforms.py matrix | names <version>")
    command = sys.argv[1]
    if command == "matrix":
        print(json.dumps(DATA, separators=(",", ":")))
    elif command == "names":
        if len(sys.argv) < 3:
            raise SystemExit("usage: platforms.py names <version>")
        version = sys.argv[2]
        for platform in DATA:
            base = f"weeeking-{version}-{platform['key']}"
            raw = f"{base}{platform['raw']}"
            print(f"{base}.{platform['archive']}")
            print(f"{base}.{platform['archive']}.sha256")
            print(raw)
            print(f"{raw}.sha256")
    else:
        raise SystemExit(f"unknown command: {command}")


if __name__ == "__main__":
    main()
