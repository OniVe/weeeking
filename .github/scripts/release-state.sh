#!/usr/bin/env bash
# Состояние GitHub-релиза текущего тега (для job publish-release):
#   missing | "ok <draft> <names>" | error
# Контракт: «не знаем» (error) никогда не путается с «неполный»; 404 — единственный признак
# missing; stderr отделён от значения (посторонний вывод gh не ломает разбор), поля валидируются.
# Вызывающий сам решает, что и когда удалять.

release_state() {
  local api err out draft names
  api="repos/${GITHUB_REPOSITORY}/releases/tags/${GITHUB_REF_NAME}"
  err=$(mktemp)
  if out=$(gh api "${api}" \
      --jq '"\(.draft)|\([.assets[].name] | sort | join(","))|"' 2>"${err}"); then
    rm -f "${err}"
    case "${out}" in
      "true|"*"|" | "false|"*"|") ;;
      *) echo "error"; return 0 ;;
    esac
    out=${out%|}
    case "${out}" in
      *"|"*) ;;
      *) echo "error"; return 0 ;;
    esac
    draft=${out%%|*}
    names=${out#*|}
    case "${names}" in
      *"|"* | *$'\n'* | *$'\r'* | *$'\t'*) echo "error"; return 0 ;;
    esac
    echo "ok ${draft} ${names}"
    return 0
  fi
  if grep -qF "(HTTP 404)" "${err}"; then
    echo "missing"
  else
    echo "error"
  fi
  rm -f "${err}"
}
