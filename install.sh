#!/bin/bash
set -e

REPO="your-org/ec2-connect"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin) OS="darwin" ;;
  linux) OS="linux" ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="arm64" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Get latest version
VERSION=$(curl -sL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
if [ -z "$VERSION" ]; then
  echo "Failed to get latest version"
  exit 1
fi

echo "Installing ec2-connect $VERSION for $OS-$ARCH..."

# Download
FILENAME="ec2-connect-${OS}-${ARCH}.tar.gz"
URL="https://github.com/$REPO/releases/download/$VERSION/$FILENAME"

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

curl -sL "$URL" -o "$TMPDIR/$FILENAME"

# Verify checksum
curl -sL "$URL.sha256" -o "$TMPDIR/$FILENAME.sha256"
cd "$TMPDIR"
if command -v sha256sum &> /dev/null; then
  sha256sum -c "$FILENAME.sha256"
elif command -v shasum &> /dev/null; then
  shasum -a 256 -c "$FILENAME.sha256"
fi

# Extract and install
tar -xzf "$FILENAME"

if [ -w "$INSTALL_DIR" ]; then
  mv ec2-connect "$INSTALL_DIR/"
else
  sudo mv ec2-connect "$INSTALL_DIR/"
fi

echo "✓ ec2-connect installed to $INSTALL_DIR/ec2-connect"
echo "Run 'ec2-connect --help' to get started"
