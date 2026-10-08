#!/usr/bin/env python3
"""Пересборка релизного бинарника, даже пока сервер запущен.

Windows не разрешает перезаписывать работающий exe, но разрешает
переименовать: старый файл уезжает в weeeking(.exe).old, cargo собирает
новый на его место, а .old удаляется, как только его отпустит старый
процесс.

Запуск из корня репозитория: python tools/rebuild.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXE_NAME = "weeeking.exe" if os.name == "nt" else "weeeking"
EXE = ROOT / "target" / "release" / EXE_NAME
OLD = EXE.with_name(EXE.name + ".old")


def try_remove(path: Path) -> bool:
    try:
        path.unlink()
        return True
    except FileNotFoundError:
        return True
    except OSError:
        return False


def stash_locked_old() -> None:
    """Убирает залежавшийся .old; если он залочен — уводит под уникальным именем."""
    if not OLD.exists():
        return
    if try_remove(OLD):
        return
    stamped = OLD.with_name(OLD.name + f".{int(time.time())}")
    try:
        os.replace(OLD, stamped)
        print(f"-> залоченный {OLD.name} переименован в {stamped.name}")
    except OSError as exc:
        raise SystemExit(
            f"не могу освободить {OLD} ({exc}); перезапустите клиент и повторите"
        )


def main() -> int:
    stash_locked_old()

    if EXE.exists():
        try:
            os.replace(EXE, OLD)
            print(f"-> старый бинарник отложен: {OLD}")
        except OSError as exc:
            raise SystemExit(f"не могу переименовать {EXE} ({exc})")

    result = subprocess.run(
        ["cargo", "build", "--release", "--locked"],
        cwd=ROOT,
        check=False,
    )
    if result.returncode != 0:
        if OLD.exists():
            try:
                os.replace(OLD, EXE)
                print("-> сборка не удалась, предыдущий бинарник возвращён на место")
            except OSError:
                print(f"-> сборка не удалась; предыдущий бинарник остался в {OLD}")
        raise SystemExit(f"cargo build --release завершился с ошибкой ({result.returncode})")

    if not EXE.exists():
        if OLD.exists():
            try:
                os.replace(OLD, EXE)
                print("-> сборка не создала бинарник, предыдущий возвращён на место")
            except OSError:
                print(f"-> сборка не создала бинарник; предыдущий остался в {OLD}")
        raise SystemExit("cargo завершился успешно, но целевой бинарник не найден")

    if try_remove(OLD):
        print(f"-> готово: {EXE} обновлён, старый файл убран")
    else:
        print(f"-> {OLD} остаётся до завершения старого процесса (удалится при следующей сборке)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
