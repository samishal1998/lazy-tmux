#!/bin/sh
# Install ltm (lazy-tmux).
#
#   curl -fsSL https://raw.githubusercontent.com/samishal1998/lazy-tmux/main/install.sh | sh
#
# Set LTM_VERSION to pin a release (e.g. v0.1.0), LTM_BIN_DIR to choose where
# it lands, LTM_BASE_URL to install from a mirror or a local directory
# (anything curl can read, e.g. file:///path). `ltm update` runs this same script.
set -eu

REPO="samishal1998/lazy-tmux"
VERSION="${LTM_VERSION:-latest}"
BIN_DIR="${LTM_BIN_DIR:-$HOME/.local/bin}"
BIN_NAME="${LTM_BIN_NAME:-ltm}"   # `ltm update` sets this when the binary was renamed

say() { printf '%s\n' "$*"; }
die() { printf 'install: %s\n' "$*" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "this needs $1, which is not installed"; }
need uname
need tar
if command -v curl >/dev/null 2>&1; then fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then fetch() { wget -qO "$2" "$1"; }
else die "this needs curl or wget"; fi

# Pick the build. Static musl on Linux, so it runs on any distro.
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)  case "$arch" in
            x86_64|amd64)  target=x86_64-unknown-linux-musl ;;
            aarch64|arm64) target=aarch64-unknown-linux-musl ;;
            *) die "no build for $os/$arch yet. Build from source: cargo install --git https://github.com/$REPO lazytmux-cli" ;;
          esac ;;
  Darwin) case "$arch" in
            x86_64)        target=x86_64-apple-darwin ;;
            arm64)         target=aarch64-apple-darwin ;;
            *) die "no build for $os/$arch yet" ;;
          esac ;;
  *) die "$os is not supported. ltm targets Linux and macOS" ;;
esac

if [ -n "${LTM_BASE_URL:-}" ]; then base="$LTM_BASE_URL"
elif [ "$VERSION" = "latest" ]; then base="https://github.com/$REPO/releases/latest/download"
else base="https://github.com/$REPO/releases/download/$VERSION"; fi
url="$base/ltm-$target.tar.gz"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

say "  target   $target"
say "  from     $url"
say "  into     $BIN_DIR/$BIN_NAME"
say ""

fetch "$url" "$tmp/ltm.tar.gz" || die "could not download $url
  If this is a fresh repository, there may be no release yet.
  Build from source instead: cargo install --git https://github.com/$REPO lazytmux-cli"

# Verify against the release's own checksums when it publishes them. A
# mismatch is always fatal.
if fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" 2>/dev/null; then
  if command -v sha256sum >/dev/null 2>&1; then sum="$(sha256sum "$tmp/ltm.tar.gz" | cut -d' ' -f1)"
  elif command -v shasum >/dev/null 2>&1; then sum="$(shasum -a 256 "$tmp/ltm.tar.gz" | cut -d' ' -f1)"
  else sum=""; fi
  if [ -n "$sum" ]; then
    grep -F "ltm-$target.tar.gz" "$tmp/SHA256SUMS" | grep -q "^$sum" || die "checksum mismatch -- refusing to install this"
    say "  checksum ok"
  else
    say "  (no sha256sum or shasum here; skipping checksum)"
  fi
else
  say "  (release publishes no SHA256SUMS; skipping checksum)"
fi

tar -xzf "$tmp/ltm.tar.gz" -C "$tmp"
# Never replace a working binary with one that cannot run here.
chmod +x "$tmp/ltm"
"$tmp/ltm" --version >/dev/null 2>&1 || die "the downloaded binary does not run on this machine; nothing was changed"
mkdir -p "$BIN_DIR"
# Write beside, then rename: an interrupted install (or an update of the
# binary that is currently running) never leaves a half-written file.
cp "$tmp/ltm" "$BIN_DIR/.ltm.new"
chmod +x "$BIN_DIR/.ltm.new"
mv -f "$BIN_DIR/.ltm.new" "$BIN_DIR/$BIN_NAME"

say "Installed $("$BIN_DIR/$BIN_NAME" --version)"

# `ltm update` sets LTM_UPDATE: the PATH hint and next steps are for first installs.
if [ -z "${LTM_UPDATE:-}" ]; then
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) say ""
       say "$BIN_DIR is not on your PATH. Add it:"
       say "  export PATH=\"$BIN_DIR:\$PATH\"" ;;
  esac

  say ""
  say "Next:"
  say "  ltm                                 open the session manager"
  say "  ltm doctor --fix                    fix common tmux paper-cuts"
  say "  ltm completions <shell> --install   bash, zsh or fish tab completion"
fi
