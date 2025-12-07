#!/bin/sh
set -e

# --- Configuration ---
REPO="isupervillain/purecode"
# Base URL for assets. We'll use the GitHub releases API structure.
# https://github.com/isupervillain/purecode/releases/download/vX.Y.Z/purecode-<target>.<ext>
GITHUB_URL="https://github.com/$REPO/releases/download"

# Allow overriding the install directory with PURECODE_UNMANAGED_INSTALL
INSTALL_DIR="${PURECODE_UNMANAGED_INSTALL:-$HOME/.local/bin}"
BIN_NAME="purecode"

# --- Detection ---
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        # Prefer musl if available, otherwise gnu?
        # Actually, let's stick to musl for broad compatibility as requested.
        # But we must ensure the user machine is x86_64 for musl artifact.
        if [ "$ARCH" = "x86_64" ]; then
            TARGET="x86_64-unknown-linux-musl"
        else
            # Fallback or error? For now, we only build musl for x86_64 in CI.
            # If aarch64 linux, we might not have a build yet.
            echo "Error: Unsupported Linux architecture: $ARCH. Only x86_64 is currently supported via this script."
            exit 1
        fi
        EXT="tar.gz"
        ;;
    Darwin)
        if [ "$ARCH" = "x86_64" ]; then
            TARGET="x86_64-apple-darwin"
        elif [ "$ARCH" = "arm64" ]; then
            TARGET="aarch64-apple-darwin"
        else
            echo "Error: Unsupported macOS architecture: $ARCH"
            exit 1
        fi
        EXT="tar.gz"
        ;;
    *)
        echo "Error: Unsupported OS: $OS"
        exit 1
        ;;
esac

# --- Fetch Latest Version ---
# We need to know the version to construct the URL.
# Strategy: fetch the latest release tag from GitHub API (public).
# Or just ask the user/hardcode? No, dynamic is better.
# Using a simple curl to get the latest release redirect URL is a common trick.
echo "Detecting latest version..."
LATEST_URL="https://github.com/$REPO/releases/latest"
# -I headers only, -o /dev/null discard body, -w redirect_url print final url
RELEASE_URL=$(curl -Ls -o /dev/null -w %{url_effective} "$LATEST_URL")
# Extract tag from URL (e.g. .../releases/tag/v0.3.0)
VERSION_TAG=$(basename "$RELEASE_URL")

if [ -z "$VERSION_TAG" ] || [ "$VERSION_TAG" = "latest" ]; then
    echo "Error: Could not detect latest version tag."
    exit 1
fi

echo "Latest version: $VERSION_TAG"

ASSET_NAME="purecode-${TARGET}.${EXT}"
DOWNLOAD_URL="${GITHUB_URL}/${VERSION_TAG}/${ASSET_NAME}"

# --- Download & Install ---
echo "Downloading $DOWNLOAD_URL ..."
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

curl -LsSf "$DOWNLOAD_URL" -o "$TMP_DIR/$ASSET_NAME"

echo "Installing to $INSTALL_DIR ..."
mkdir -p "$INSTALL_DIR"

tar -xzf "$TMP_DIR/$ASSET_NAME" -C "$TMP_DIR"
# The tarball contains the binary directly or in a folder?
# CI command: `tar -czf ... purecode` inside the release dir.
# So it should unpack as just `purecode`.
mv "$TMP_DIR/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
chmod +x "$INSTALL_DIR/$BIN_NAME"

echo "Successfully installed purecode to $INSTALL_DIR/$BIN_NAME"

# --- Path Check ---
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) echo "Warning: $INSTALL_DIR is not in your PATH. You may need to add it:"
       echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
       ;;
esac
