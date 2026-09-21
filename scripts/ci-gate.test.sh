#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

expect_failure() {
  if bash "$SCRIPT_DIR/ci-gate.sh" "$@" >/dev/null 2>&1; then
    echo "expected CI gate to fail for: $*" >&2
    exit 1
  fi
}

bash "$SCRIPT_DIR/ci-gate.sh" success success success success

for status in failure skipped cancelled neutral; do
  expect_failure "$status" success success success
  expect_failure success "$status" success success
  expect_failure success success "$status" success
  expect_failure success success success "$status"
done

expect_failure success success success
echo "ci-gate tests passed"
