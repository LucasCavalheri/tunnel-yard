#!/usr/bin/env bash
# Screenshots (and, with --video, recordings) of the real window at every size, theme and language.
#
#   scripts/screenshots.sh [--sizes all|1366x768,...] [--shots demo,editor] [--themes dark,light]
#                          [--locales en,pt-BR] [--video] [--wait SECONDS] [--binary PATH]
#                          [--out DIR] [--dry-run]
#
# Runs the app on a virtual X display (Xvfb, software Vulkan) with TUNNELYARD_SHOT: made-up
# profiles on .example hosts, nothing read from /etc/openfortivpn, no tunnel touched, no update
# check. HOME and the XDG dirs point at a temp dir and the session D-Bus is out of reach, so your
# settings, autostart, desktop entry and tray stay untouched: nothing appears on your desktop.
# --wait is how long to let the window settle after its first paint (default 3 seconds).
# Writes SHOT-THEME-LOCALE@WxH.png into DIR (default target/screenshots). --video also writes
# SHOT-THEME-LOCALE@WxH.mp4 (the first seconds, entrance motion included) and a -strip.png with
# 12 frames in one image, for an agent that can only read images.
# Needs xvfb and ffmpeg (Debian/Ubuntu: sudo apt install xvfb ffmpeg mesa-vulkan-drivers).
set -euo pipefail
cd "$(dirname "$0")/.."
# Decimal points, not commas, in awk and ffmpeg timings, whatever the desktop locale is.
export LC_ALL=C

all_sizes=(900x620 1024x680 1120x740 1366x768 1920x1080 2560x1440)
sizes=all
shots=demo,editor
themes=dark,light
locales=en,pt-BR
video=0
wait_s=3
binary=""
out=target/screenshots
dry_run=0

# Seconds of black at the very start of a recording, read from ffmpeg's blackdetect log on stdin:
# the end of a black stretch that starts at 0. No such stretch: 0.
black_lead() {
  awk '/black_start:/ {
    for (i = 1; i <= NF; i++) { split($i, kv, ":"); v[kv[1]] = kv[2] }
    if (v["black_start"] + 0 < 0.05) { printf "%.2f\n", v["black_end"]; found = 1; exit }
  } END { if (!found) print "0.00" }'
}

# Internal, for scripts/screenshots.test.sh: black_lead on stdin, then exit.
if [[ ${1:-} == --black-lead ]]; then black_lead; exit 0; fi

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
    -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
  if (( w > 7680 || h > 4320 )); then
    echo "size $s is above the largest capture 7680x4320" >&2; exit 2
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

[[ $wait_s =~ ^[0-9]+$ ]] && (( wait_s >= 1 )) || { echo "bad --wait: $wait_s (whole seconds, at least 1)" >&2; exit 2; }

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
rec_pid=""
cleanup() {
  [[ -n $rec_pid ]] && kill -INT "$rec_pid" 2>/dev/null || true
  [[ -n $app_pid ]] && kill "$app_pid" 2>/dev/null || true
  [[ -n $xvfb_pid ]] && kill "$xvfb_pid" 2>/dev/null || true
  rm -rf "$scratch"
}
trap cleanup EXIT

# Luma of the virtual screen: ~0 while it is still black, above 3 once the window has painted.
luma() {
  ffmpeg -loglevel error -f x11grab -video_size "$1" -i "$display" -frames:v 1 \
    -vf "signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=-" -f null - 2>/dev/null \
    | sed -n 's/.*YAVG=\([0-9.]*\).*/\1/p' | head -1
}

# Blocks until the app paints something, up to 30 s. Fails if the app dies first.
wait_for_paint() {
  local size=$1 deadline
  deadline=$(( $(date +%s) + 30 ))
  while (( $(date +%s) < deadline )); do
    kill -0 "$app_pid" 2>/dev/null || return 1
    local y
    y=$(luma "$size")
    if [[ -n $y ]] && awk -v y="$y" 'BEGIN { exit !(y > 3) }'; then return 0; fi
    sleep 0.2
  done
  return 1
}

count=0
for s in "${size_list[@]}"; do
  w=${s%x*}; h=${s#*x}
  # -displayfd: Xvfb picks a free display and writes its number once it accepts connections.
  : > "$scratch/display"
  Xvfb -displayfd 3 -screen 0 "${w}x${h}x24" -nolisten tcp 3>"$scratch/display" 2>"$scratch/xvfb.log" &
  xvfb_pid=$!
  for _ in $(seq 1 100); do [[ -s $scratch/display ]] && break; kill -0 "$xvfb_pid" 2>/dev/null || break; sleep 0.1; done
  if ! [[ -s $scratch/display ]] || ! kill -0 "$xvfb_pid" 2>/dev/null; then
    echo "Xvfb did not start for $s; its log:" >&2
    tail -20 "$scratch/xvfb.log" >&2
    exit 1
  fi
  display=":$(tr -dc '0-9' < "$scratch/display")"
  for shot in "${shot_list[@]}"; do for t in "${theme_list[@]}"; do for l in "${locale_list[@]}"; do
    name="$shot-$t-$l@$s"
    home="$scratch/home"
    rm -rf "$home"
    mkdir -p "$home/.config/tunnel-yard"
    printf '{"locale":"%s","theme":"%s","autoReconnect":true}\n' "$l" "$t" > "$home/.config/tunnel-yard/settings.json"
    # A session bus address that leads nowhere: no tray icon can reach your desktop.
    run_env=(env -u WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS="unix:path=$scratch/no-session-bus"
      DISPLAY="$display" HOME="$home"
      XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" XDG_CACHE_HOME="$home/.cache"
      TUNNELYARD_SHOT="$shot" TUNNELYARD_WINDOW="$s")
    [[ -n $lavapipe ]] && run_env+=(VK_ICD_FILENAMES="$lavapipe")

    rec_pid=""
    if [[ $video == 1 ]]; then
      # Record from launch; the first paint is found below and the black lead-in is cut off.
      ffmpeg -loglevel error -y -f x11grab -framerate 30 -video_size "${w}x${h}" -i "$display" \
        -c:v libx264 -preset ultrafast -pix_fmt yuv420p "$scratch/raw.mp4" </dev/null &
      rec_pid=$!
    fi
    "${run_env[@]}" "$binary" >"$scratch/app.log" 2>&1 &
    app_pid=$!
    if ! wait_for_paint "${w}x${h}"; then
      echo "$name: the window never painted (or the app exited); its log:" >&2
      tail -20 "$scratch/app.log" >&2
      exit 1
    fi
    sleep "$wait_s"
    ffmpeg -loglevel error -y -f x11grab -video_size "${w}x${h}" -i "$display" -frames:v 1 -update 1 "$out/$name.png"
    if [[ -n $rec_pid ]]; then
      kill -INT "$rec_pid" 2>/dev/null || true
      wait "$rec_pid" 2>/dev/null || true
      # Cut what is still black before the window shows (Xvfb and software Vulkan start slowly).
      lead=$(ffmpeg -loglevel info -i "$scratch/raw.mp4" -vf "blackdetect=d=0.1:pix_th=0.05" -an -f null - 2>&1 | black_lead)
      ffmpeg -loglevel error -y -ss "$lead" -i "$scratch/raw.mp4" -t "$wait_s" \
        -vf "pad=ceil(iw/2)*2:ceil(ih/2)*2" -c:v libx264 -pix_fmt yuv420p "$out/$name.mp4"
      frames=$(( wait_s * 30 ))
      every=$(( frames / 12 > 0 ? frames / 12 : 1 ))
      ffmpeg -loglevel error -y -i "$out/$name.mp4" \
        -vf "select='not(mod(n\,$every))',scale=640:-1,tile=4x3:padding=4:color=black" \
        -frames:v 1 -update 1 "$out/$name-strip.png"
      rm -f "$scratch/raw.mp4"
    fi
    kill "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
    app_pid=""
    count=$(( count + 1 ))
  done; done; done
  kill "$xvfb_pid" 2>/dev/null || true
  wait "$xvfb_pid" 2>/dev/null || true
  xvfb_pid=""
done
echo "$count screens in $out"
