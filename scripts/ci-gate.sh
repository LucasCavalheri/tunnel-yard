#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 <quality> <test> <site> <security>" >&2
  exit 2
fi

names=(quality test site security)
failed=0

for index in "${!names[@]}"; do
  argument_index=$((index + 1))
  result="${!argument_index}"
  if [[ "$result" != "success" ]]; then
    printf 'required check %s was %s (expected success)\n' "${names[index]}" "$result" >&2
    failed=1
  fi
done

if (( failed != 0 )); then
  exit 1
fi
