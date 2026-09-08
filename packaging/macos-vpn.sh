#!/bin/bash
# Invoked once per connection through the macOS administrator dialog.
set -u
SESSION="${1:?session directory required}"
BIN="${2:?openfortivpn path required}"
export PATH=/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin
export LANG=C LC_ALL=C
umask 077
PRIVATE=$(mktemp -d /private/tmp/tunnel-yard-root.XXXXXX) || exit 1
DNS_KEY="State:/Network/Service/TunnelYard-$(basename "$SESSION")/DNS"
CHILD=''
RESULT=1
cleanup() {
  if [ -n "$CHILD" ] && kill -0 "$CHILD" 2>/dev/null; then
    kill -INT "$CHILD" 2>/dev/null || true
    # Allow openfortivpn/pppd to restore routing before terminating the helper.
    wait "$CHILD" 2>/dev/null || true
  fi
  printf 'remove %s\n' "$DNS_KEY" | /usr/sbin/scutil >/dev/null
  rm -f "$PRIVATE/profile.conf"
  rmdir "$PRIVATE"
  printf '%s\n' "$RESULT" > "$SESSION/exit-code"
}
trap cleanup EXIT
trap 'RESULT=0; exit 0' INT TERM HUP
# Snapshot before inspecting or executing the profile. Never follow a changing
# user-owned .conf while running as root.
cp "$SESSION/profile.conf" "$PRIVATE/profile.conf" || exit 1
if ! awk '
  /^[[:space:]]*([#;]|$)/ { next }
  { key=$0; sub(/=.*/, "", key); gsub(/^[ \t]+|[ \t]+$/, "", key)
    if (index($0,"=")==0 || key !~ /^(host|port|username|user|password|trusted-cert|set-dns|set-routes|realm|persistent|ca-file|user-cert|user-key|otp|otp-prompt|otp-delay|pppd-use-peerdns|pppd-accept-remote|half-internet-routes|min-tls|cipher-list|seclevel|saml-login)$/) exit 1
  }' "$PRIVATE/profile.conf"; then
  echo 'ERROR: Unsupported or unsafe macOS profile option.' >> "$SESSION/stderr.log"
  exit 1
fi
WANT_DNS=$(awk -F= '/^[ \t]*set-dns[ \t]*=/ {v=$2; gsub(/[ \t\r]/,"",v)} END {print tolower(v)}' "$PRIVATE/profile.conf")
# macOS resolves via SystemConfiguration, not /etc/resolv.conf. TunnelYard owns
# only its supplemental DNS entry; never overwrite another network service.
"$BIN" -c "$PRIVATE/profile.conf" --set-dns=0 --pppd-use-peerdns=0 --persistent=0 > "$SESSION/stdout.log" 2> "$SESSION/stderr.log" &
CHILD=$!
READY=0
while kill -0 "$CHILD" 2>/dev/null; do
  NOW=$(date +%s)
  LAST=$(stat -f %m "$SESSION/heartbeat" 2>/dev/null || echo 0)
  if [ -f "$SESSION/stop" ] || [ $((NOW-LAST)) -gt 15 ]; then
    RESULT=0
    exit 0
  fi
  if [ "$READY" -eq 0 ] && grep -q 'Tunnel is up and running' "$SESSION/stdout.log" "$SESSION/stderr.log"; then
    if [[ ! "$WANT_DNS" =~ ^(0|false|no|off)$ ]]; then
      ADDRESSES=$(sed -n 's/.*Got addresses:.*ns \[\([^]]*\)\].*/\1/p' "$SESSION/stdout.log" "$SESSION/stderr.log" | tail -1)
      SERVERS=$(printf '%s' "$ADDRESSES" | tr ',' ' ' | awk '{for(i=1;i<=NF;i++) { n=split($i,a,"."); valid=(n==4 && $i!="0.0.0.0"); for(j=1;j<=n;j++) if(a[j]!~/^[0-9]+$/ || a[j]>255) valid=0; if(valid) printf "%s ",$i }}')
      SUFFIX=$(sed -n 's/.*ns_suffix \[\([A-Za-z0-9.-]*\)\].*/\1/p' "$SESSION/stdout.log" "$SESSION/stderr.log" | tail -1)
      if [ -n "$SERVERS" ]; then
        {
          echo 'd.init'
          printf 'd.add ServerAddresses * %s\n' "$SERVERS"
          echo 'd.add SupplementalMatchDomains * ""'
          [ -z "$SUFFIX" ] || printf 'd.add SearchDomains * %s\n' "$SUFFIX"
          printf 'set %s\n' "$DNS_KEY"
        } | /usr/sbin/scutil >> "$SESSION/stdout.log" 2>> "$SESSION/stderr.log"
        if ! printf 'show %s\n' "$DNS_KEY" | /usr/sbin/scutil | grep -q ServerAddresses; then
          echo 'ERROR: Could not register VPN DNS in macOS SystemConfiguration.' >> "$SESSION/stderr.log"
          exit 1
        fi
      else
        echo 'ERROR: set-dns=1 but no valid VPN DNS servers were received.' >> "$SESSION/stderr.log"
        exit 1
      fi
    fi
    echo 'TUNNELYARD_TUNNEL_UP' >> "$SESSION/stdout.log"
    READY=1
  fi
  sleep 1
done
wait "$CHILD"
RESULT=$?
CHILD=''
exit "$RESULT"
