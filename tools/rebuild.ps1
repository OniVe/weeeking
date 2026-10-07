# Пересборка релизного бинарника, даже пока OpenCode держит текущий запущенным.
# Windows не разрешает перезаписывать работающий exe, но разрешает переименовать:
# старый файл уезжает в weeeking.exe.old, cargo собирает новый на его место,
# а .old удаляется, как только его отпустит старый процесс.
#
# Запуск из корня репозитория:  .\tools\rebuild.ps1
$ErrorActionPreference = "Stop"

$root = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $root "target\release\weeeking.exe"
$old = "$exe.old"

if (Test-Path $exe) {
    if (Test-Path $old) {
        Remove-Item $old -ErrorAction SilentlyContinue
        if (Test-Path $old) {
            # старый .old всё ещё залочен запущенным процессом — уводим его под уникальным именем
            $stamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
            Move-Item $old "$old.$stamp" -ErrorAction SilentlyContinue
            if (Test-Path $old) { throw "Не могу освободить $old — перезапустите OpenCode и повторите." }
        }
    }
    Move-Item $exe $old
    Write-Host "→ старый бинарник отложен: $old"
}

cargo build --release --manifest-path (Join-Path $root "Cargo.toml")
if ($LASTEXITCODE -ne 0) { throw "cargo build --release завершился с ошибкой" }

Remove-Item $old -ErrorAction SilentlyContinue
if (Test-Path $old) {
    Write-Host "→ $old остаётся до завершения старого процесса (удалите позже или перезапустите OpenCode)"
} else {
    Write-Host "→ готово: $exe обновлён, старый файл убран"
}
