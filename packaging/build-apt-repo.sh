#!/bin/bash
# Build / refresh a signed flat APT repo suitable for GitHub Pages.
# Usage: APT_SIGNING_KEY_ID=<fingerprint> build-apt-repo.sh <repo-root> <deb-file> [more.deb...]
set -euo pipefail

REPO_ROOT="${1:?repo root required}"
shift
if [ "$#" -lt 1 ]; then
  echo "usage: APT_SIGNING_KEY_ID=<fingerprint> $0 <repo-root> <deb> [more.deb...]" >&2
  exit 1
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
KEYRING_FILE="${APT_KEYRING_FILE:-$SCRIPT_DIR/my-vpns-archive-keyring.asc}"
SIGNING_KEY="${APT_SIGNING_KEY_ID:-}"

if [ -z "$SIGNING_KEY" ]; then
  echo "APT_SIGNING_KEY_ID is required to publish a signed repository" >&2
  exit 1
fi

for command in dpkg-deb dpkg-scanpackages apt-ftparchive gpg; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "missing required command: $command" >&2
    exit 1
  fi
done

if [ ! -f "$KEYRING_FILE" ]; then
  echo "APT public key not found: $KEYRING_FILE" >&2
  exit 1
fi

APT_ARCHITECTURES_NORMALIZED="${APT_ARCHITECTURES:-}"
if [ -n "$APT_ARCHITECTURES_NORMALIZED" ]; then
  APT_ARCHITECTURES_NORMALIZED="${APT_ARCHITECTURES_NORMALIZED//,/ }"
else
  APT_ARCHITECTURES_NORMALIZED="$({
    for deb in "$@"; do
      dpkg-deb -f "$deb" Architecture
    done
  } | sort -u | tr '\n' ' ' | sed 's/[[:space:]]*$//')"
fi
if [ -z "$APT_ARCHITECTURES_NORMALIZED" ]; then
  echo "could not determine package architectures (set APT_ARCHITECTURES)" >&2
  exit 1
fi
APT_ARCHITECTURES_DISPLAY="${APT_ARCHITECTURES_NORMALIZED// /,}"

mkdir -p "$REPO_ROOT"
for deb in "$@"; do
  case "$deb" in
    *.deb) cp -f "$deb" "$REPO_ROOT/" ;;
    *)
      echo "only .deb files can be published: $deb" >&2
      exit 1
      ;;
  esac
done

cp -f "$KEYRING_FILE" "$REPO_ROOT/my-vpns-archive-keyring.asc"

(
  cd "$REPO_ROOT"
  rm -f Release InRelease Release.gpg
  dpkg-scanpackages --multiversion . /dev/null > Packages
  gzip -9c Packages > Packages.gz
  apt-ftparchive \
    -o APT::FTPArchive::Release::Origin="My VPNs" \
    -o APT::FTPArchive::Release::Label="My VPNs" \
    -o APT::FTPArchive::Release::Architectures="$APT_ARCHITECTURES_NORMALIZED" \
    -o APT::FTPArchive::Release::Description="My VPNs Debian packages" \
    release . > Release
  gpg --batch --no-tty --yes \
    --local-user "$SIGNING_KEY" \
    --digest-algo SHA256 \
    --clearsign --output InRelease Release
  gpg --batch --no-tty --yes \
    --local-user "$SIGNING_KEY" \
    --digest-algo SHA256 \
    --armor --detach-sign --output Release.gpg Release
)

# Tiny index for humans
cat > "$REPO_ROOT/README.md" << 'EOF'
# My VPNs APT repository

The repository metadata is signed with the My VPNs archive key.
Fingerprint: `A9F137BEE74B623131071358FB0EC1D5A01262F0`

```bash
curl -fsSL https://lucascavalheri.github.io/my-vpns/apt/my-vpns-archive-keyring.asc \
  | sudo tee /usr/share/keyrings/my-vpns-archive-keyring.asc >/dev/null
echo 'deb [arch=__APT_ARCHITECTURES__ signed-by=/usr/share/keyrings/my-vpns-archive-keyring.asc] https://lucascavalheri.github.io/my-vpns/apt ./' \
  | sudo tee /etc/apt/sources.list.d/my-vpns.list
sudo apt update
sudo apt install my-vpns
```

After installation, upgrades come with `sudo apt upgrade`.
EOF
sed -i "s/__APT_ARCHITECTURES__/$APT_ARCHITECTURES_DISPLAY/" "$REPO_ROOT/README.md"

echo "Signed APT repo ready at $REPO_ROOT"
ls -lh "$REPO_ROOT"
