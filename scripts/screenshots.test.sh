#!/usr/bin/env bash
# Argument handling and helpers of scripts/screenshots.sh, without a display. The capture itself
# runs in CI. Needs ffmpeg (the paint check is tested on real frames): sudo apt install ffmpeg.
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

# The black lead-in read from ffmpeg's blackdetect log, with a decimal point on a pt-BR desktop too.
log='[blackdetect @ 0x1] black_start:0 black_end:0.666667 black_duration:0.666667'
for locale in pt_BR.UTF-8 C; do
  got=$(LC_ALL=$locale LANG=$locale bash "$script" --black-lead <<< "$log" 2>/dev/null)
  [[ $got == 0.67 ]] || { echo "FAIL: black lead-in under $locale was '$got', expected 0.67" >&2; fail=1; }
done
got=$(bash "$script" --black-lead <<< '[blackdetect @ 0x1] black_start:2.5 black_end:3 black_duration:0.5')
[[ $got == 0.00 ]] || { echo "FAIL: black later in the clip is not a lead-in, got '$got'" >&2; fail=1; }
got=$(bash "$script" --black-lead < /dev/null)
[[ $got == 0.00 ]] || { echo "FAIL: no black at all must give 0.00, got '$got'" >&2; fail=1; }

# Every clip length fills the 12 tiles of the strip (ffmpeg keeps frames n where n % every == 0).
for count in 12 13 24 67 90 91 143 144 300; do
  every=$(bash "$script" --strip-every "$count")
  picked=$(( (count + every - 1) / every ))
  (( picked >= 12 )) || { echo "FAIL: $count frames, every $every, only $picked tiles" >&2; fail=1; }
done
[[ $(bash "$script" --strip-every 5) == 1 ]] || { echo "FAIL: a clip under 12 frames must keep every frame" >&2; fail=1; }

# The paint check on ffmpeg's luma scale (black is 16, not 0): a black screen is not a window.
if command -v ffmpeg >/dev/null; then
  frames=$(mktemp -d)
  ffmpeg -loglevel error -y -f lavfi -i color=black:s=64x64 -frames:v 1 "$frames/black.png"
  # As dark as the darkest real capture (YAVG ~31), so the 20-to-31 margin is what is tested.
  ffmpeg -loglevel error -y -f lavfi -i color=0x121417:s=64x64 -frames:v 1 "$frames/dark.png"
  ffmpeg -loglevel error -y -f lavfi -i color=0xf4f1ea:s=64x64 -frames:v 1 "$frames/light.png"
  [[ $(bash "$script" --painted "$frames/black.png") == blank ]] || { echo "FAIL: a black screen counted as painted" >&2; fail=1; }
  [[ $(bash "$script" --painted "$frames/dark.png") == painted ]] || { echo "FAIL: the dark theme did not count as painted" >&2; fail=1; }
  [[ $(bash "$script" --painted "$frames/light.png") == painted ]] || { echo "FAIL: the light theme did not count as painted" >&2; fail=1; }
  rm -rf "$frames"
else
  echo "FAIL: ffmpeg is needed to test the paint check" >&2
  fail=1
fi

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
