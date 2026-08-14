#!/bin/sh
# Install tlflow — https://tlflow.cc
#
# Downloads the release archive for this platform, verifies its checksum, and
# puts the binary somewhere on your PATH.
#
#   curl -fsSL https://raw.githubusercontent.com/DanielCarmingham/throughline/main/install.sh | sh
#
# Piping a script into a shell means running code you have not read. This one
# is short on purpose — read it first if you would rather:
#
#   curl -fsSL .../install.sh -o install.sh && less install.sh && sh install.sh
#
# Options (environment variables):
#   TLFLOW_VERSION   tag to install, e.g. v0.1.0   (default: latest)
#   TLFLOW_BIN_DIR   where to install              (default: ~/.local/bin)

set -eu

REPO="DanielCarmingham/throughline"
BIN_DIR="${TLFLOW_BIN_DIR:-$HOME/.local/bin}"
VERSION="${TLFLOW_VERSION:-latest}"

say() { printf '%s\n' "$*"; }
die() { printf 'install: %s\n' "$*" >&2; exit 1; }

need() {
  command -v "$1" >/dev/null 2>&1 || die "this script needs $1"
}

need uname
need mkdir
need tar

if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL "$1"; }
  fetch_to() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -qO- "$1"; }
  fetch_to() { wget -qO "$2" "$1"; }
else
  die "this script needs curl or wget"
fi

# --- which build ------------------------------------------------------------

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Darwin) os_part="apple-darwin" ;;
  Linux)  os_part="unknown-linux-gnu" ;;
  *)      die "unsupported OS: $os. Build from source: cargo install --path tlflow" ;;
esac

case "$arch" in
  arm64|aarch64) arch_part="aarch64" ;;
  x86_64|amd64)  arch_part="x86_64" ;;
  *)             die "unsupported architecture: $arch" ;;
esac

TARGET="${arch_part}-${os_part}"

# --- which version ----------------------------------------------------------

if [ "$VERSION" = "latest" ]; then
  # Resolve the tag rather than using /latest/download, so the version being
  # installed is printed and the checksum file can be fetched for the same tag.
  # Silence the transport's own error: a 404 here just means "no release yet",
  # which the message below says far more usefully.
  VERSION="$(fetch "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)"
  [ -n "$VERSION" ] || die "could not determine the latest version. Is there a release yet?"
fi

NAME="tlflow-${TARGET}"
BASE="https://github.com/$REPO/releases/download/$VERSION"

say "tlflow $VERSION  ($TARGET)"

# --- download and verify ----------------------------------------------------

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fetch_to "$BASE/$NAME.tar.gz" "$tmp/$NAME.tar.gz" \
  || die "no build for $TARGET in $VERSION"

if fetch_to "$BASE/SHA256SUMS" "$tmp/SHA256SUMS" 2>/dev/null; then
  if command -v shasum >/dev/null 2>&1; then
    sum="$(shasum -a 256 "$tmp/$NAME.tar.gz" | cut -d' ' -f1)"
  elif command -v sha256sum >/dev/null 2>&1; then
    sum="$(sha256sum "$tmp/$NAME.tar.gz" | cut -d' ' -f1)"
  else
    sum=""
    say "warning: no shasum or sha256sum; skipping verification"
  fi
  if [ -n "$sum" ]; then
    grep -q "$sum" "$tmp/SHA256SUMS" \
      || die "checksum mismatch for $NAME.tar.gz — refusing to install"
    say "checksum verified"
  fi
else
  say "warning: no SHA256SUMS published for $VERSION; skipping verification"
fi

tar -xzf "$tmp/$NAME.tar.gz" -C "$tmp"

# --- install ----------------------------------------------------------------

mkdir -p "$BIN_DIR"
mv "$tmp/$NAME/tlflow" "$BIN_DIR/tlflow"
chmod +x "$BIN_DIR/tlflow"

say "installed $BIN_DIR/tlflow"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    say ""
    say "$BIN_DIR is not on your PATH. Add it:"
    say "  echo 'export PATH=\"$BIN_DIR:\$PATH\"' >> ~/.zshrc"
    ;;
esac

# oh-my-zsh's tmux plugin defines `alias tl='tmux list-sessions'`, which is why
# this binary is not called tl. Warn if anything shadows tlflow too.
if command -v tlflow >/dev/null 2>&1; then
  found="$(command -v tlflow)"
  if [ "$found" != "$BIN_DIR/tlflow" ]; then
    say ""
    say "note: 'tlflow' currently resolves to $found, not the one just installed"
  fi
fi

say ""
say "  tlflow init      start a line in any repository"
say "  tlflow           open the terminal UI"
say "  tlflow --help    refs, examples, every flag"
