#!/usr/bin/env bash
# Screenshots (and, with --video, recordings) of the real window at every size, theme and language.
#
#   scripts/screenshots.sh [--sizes all|1366x768,...] [--shots demo,editor] [--themes dark,light]
#                          [--locales en,pt-BR] [--video] [--wait SECONDS] [--binary PATH]
#                          [--out DIR] [--dry-run]
#
# Runs the app on a virtual X display (Xvfb, software Vulkan) with TUNNELYARD_SHOT: made-up
# profiles on .example hosts, nothing read from /etc/openfortivpn, no tunnel touched. HOME and
# the XDG dirs point at a temp dir, so your settings, autostart and desktop entry stay untouched.
# Writes SHOT-THEME-LOCALE@WxH.png into DIR (default target/screenshots). --video also writes
# SHOT-THEME-LOCALE@WxH.mp4 (the first seconds, entrance motion included) and a -strip.png with
# 12 frames in one image, for an agent that can only read images.
# Needs xvfb and ffmpeg (Debian/Ubuntu: sudo apt install xvfb ffmpeg mesa-vulkan-drivers).
set -euo pipefail
cd "$(dirname "$0")/.."

all_sizes=(900x620 1024x680 1120x740 1366x768 1920x1080 2560x1440)
sizes=all
shots=demo,editor
themes=dark,light
locales=en,pt-BR
video=0
wait_s=4
binary=""
out=target/screenshots
dry_run=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sizes) sizes="$2"; shift 2 ;;
    --shots) shots="$2"; shift 2 ;;
    --themes) themes="$2"; shift 2 ;;
    --locales) locales="$2"; shift 2 ;;
    --video) video=1; shift ;;
    --wait) wait_s="$2"; shift 2 ;;
    --binary) binary="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ $sizes == all ]]; then
  size_list=("${all_sizes[@]}")
else
  IFS=, read -r -a size_list <<< "$sizes"
fi
for s in "${size_list[@]}"; do
  if ! [[ $s =~ ^[0-9]+x[0-9]+$ ]]; then
    echo "bad size: $s (expected WIDTHxHEIGHT)" >&2; exit 2
  fi
  w=${s%x*}; h=${s#*x}
  if (( w < 900 || h < 620 )); then
    echo "size $s is below the window minimum 900x620" >&2; exit 2
  fi
done
IFS=, read -r -a shot_list <<< "$shots"
for s in "${shot_list[@]}"; do
  [[ $s == demo || $s == editor ]] || { echo "unknown shot: $s (demo, editor)" >&2; exit 2; }
done
IFS=, read -r -a theme_list <<< "$themes"
for t in "${theme_list[@]}"; do
  [[ $t == dark || $t == light ]] || { echo "unknown theme: $t (dark, light)" >&2; exit 2; }
done
IFS=, read -r -a locale_list <<< "$locales"
for l in "${locale_list[@]}"; do
  [[ $l == en || $l == pt-BR ]] || { echo "unknown locale: $l (en, pt-BR)" >&2; exit 2; }
done

if [[ $dry_run == 1 ]]; then
  for s in "${size_list[@]}"; do for shot in "${shot_list[@]}"; do
    for t in "${theme_list[@]}"; do for l in "${locale_list[@]}"; do
      echo "$shot-$t-$l@$s"
    done; done
  done; done
  exit 0
fi

for tool in Xvfb ffmpeg; do
  command -v "$tool" >/dev/null || { echo "$tool is missing (sudo apt install xvfb ffmpeg mesa-vulkan-drivers)" >&2; exit 1; }
done
if [[ -z $binary ]]; then
  cargo build -q --locked
  binary=target/debug/tunnel-yard
fi
lavapipe=$(find /usr/share/vulkan/icd.d -name 'lvp_icd*.json' 2>/dev/null | head -1)

mkdir -p "$out"
scratch=$(mktemp -d)
xvfb_pid=""
app_pid=""
cleanup() {
  [[ -n $app_pid ]] && kill "$app_pid" 2>/dev/null || true
  [[ -n $xvfb_pid ]] && kill "$xvfb_pid" 2>/dev/null || true
  rm -rf "$scratch"
}
trap cleanup EXIT

display=:$(( 90 + RANDOM % 100 ))
count=0
for s in "${size_list[@]}"; do
  w=${s%x*}; h=${s#*x}
  Xvfb "$display" -screen 0 "${w}x${h}x24" -nolisten tcp >/dev/null 2>&1 &
  xvfb_pid=$!
  sleep 1
  for shot in "${shot_list[@]}"; do for t in "${theme_list[@]}"; do for l in "${locale_list[@]}"; do
    name="$shot-$t-$l@$s"
    home="$scratch/home"
    rm -rf "$home"
    mkdir -p "$home/.config/tunnel-yard"
    printf '{"locale":"%s","theme":"%s","autoReconnect":true}\n' "$l" "$t" > "$home/.config/tunnel-yard/settings.json"
    run_env=(env -u WAYLAND_DISPLAY DISPLAY="$display" HOME="$home"
      XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" XDG_CACHE_HOME="$home/.cache"
      TUNNELYARD_SHOT="$shot" TUNNELYARD_WINDOW="$s")
    [[ -n $lavapipe ]] && run_env+=(VK_ICD_FILENAMES="$lavapipe")

    rec_pid=""
    if [[ $video == 1 ]]; then
      ffmpeg -loglevel error -y -f x11grab -framerate 30 -video_size "${w}x${h}" -t "$wait_s" \
        -i "$display" -vf "pad=ceil(iw/2)*2:ceil(ih/2)*2" -c:v libx264 -pix_fmt yuv420p \
        "$out/$name.mp4" &
      rec_pid=$!
    fi
    "${run_env[@]}" "$binary" >"$scratch/app.log" 2>&1 &
    app_pid=$!
    sleep "$wait_s"
    [[ -n $rec_pid ]] && wait "$rec_pid"
    if ! kill -0 "$app_pid" 2>/dev/null; then
      echo "$name: the app exited early; its log:" >&2
      tail -20 "$scratch/app.log" >&2
      exit 1
    fi
    ffmpeg -loglevel error -y -f x11grab -video_size "${w}x${h}" -i "$display" -frames:v 1 -update 1 "$out/$name.png"
    kill "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
    app_pid=""
    if [[ $video == 1 ]]; then
      frames=$(( wait_s * 30 ))
      every=$(( frames / 12 > 0 ? frames / 12 : 1 ))
      ffmpeg -loglevel error -y -i "$out/$name.mp4" \
        -vf "select='not(mod(n\,$every))',scale=640:-1,tile=4x3:padding=4:color=black" \
        -frames:v 1 -update 1 "$out/$name-strip.png"
    fi
    count=$(( count + 1 ))
  done; done; done
  kill "$xvfb_pid" 2>/dev/null || true
  wait "$xvfb_pid" 2>/dev/null || true
  xvfb_pid=""
done
echo "$count screens in $out"
