#!/usr/bin/env bash
# Argument handling of scripts/screenshots.sh, without a display. The capture itself runs in CI.
set -euo pipefail
cd "$(dirname "$0")/.."
script=scripts/screenshots.sh
fail=0

expect_exit() {
  local want=$1; shift
  set +e
  bash "$script" "$@" >/dev/null 2>&1
  local got=$?
  set -e
  if [[ $got != "$want" ]]; then
    echo "FAIL: screenshots.sh $* exited $got, expected $want" >&2
    fail=1
  fi
}

plan=$(bash "$script" --dry-run)
lines=$(wc -l <<< "$plan")
if [[ $lines != 48 ]]; then
  echo "FAIL: full plan has $lines screens, expected 6 sizes x 2 shots x 2 themes x 2 locales = 48" >&2
  fail=1
fi
grep -qx 'demo-dark-en@900x620' <<< "$plan" || { echo "FAIL: minimum size missing from the plan" >&2; fail=1; }
grep -qx 'editor-light-pt-BR@2560x1440' <<< "$plan" || { echo "FAIL: largest size missing from the plan" >&2; fail=1; }

one=$(bash "$script" --dry-run --sizes 1366x768 --shots demo --themes dark --locales pt-BR)
[[ $one == 'demo-dark-pt-BR@1366x768' ]] || { echo "FAIL: filtered plan was '$one'" >&2; fail=1; }

expect_exit 0 --help

# Timings must use a decimal point on a pt-BR desktop too (ffmpeg -ss rejects "1,30").
grep -qx 'export LC_ALL=C' "$script" || { echo "FAIL: screenshots.sh must pin LC_ALL=C" >&2; fail=1; }
expect_exit 2 --dry-run --sizes 800x600
expect_exit 2 --dry-run --sizes 1366
expect_exit 2 --dry-run --sizes 9000x5000
expect_exit 2 --dry-run --wait 0
expect_exit 2 --dry-run --wait soon
expect_exit 2 --dry-run --shots settings
expect_exit 2 --dry-run --themes blue
expect_exit 2 --dry-run --locales fr
expect_exit 2 --nope

if [[ $fail != 0 ]]; then exit 1; fi
echo "screenshots.sh: ok"
