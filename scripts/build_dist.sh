#!/usr/bin/env bash
set -euo pipefail

VERSION="0.6.0"
BASE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$BASE_DIR"
DIST_DIR="$BASE_DIR/dist"

echo "=== Building Kotlin3 v$VERSION Distribution Packages ==="
mkdir -p "$DIST_DIR"
rm -f "$DIST_DIR"/*

# 1. Linux release binary
echo "--> Compiling Linux x86_64 release binary..."
cargo build --release

# 2. Windows x86_64 release binary
echo "--> Compiling Windows x86_64 release binary..."
cargo build --target x86_64-pc-windows-gnu --release

# 3. Windows MSI installer
echo "--> Generating Windows MSI installer..."
wixl -o "$DIST_DIR/kotlin3-v$VERSION-windows-x64.msi" "$BASE_DIR/installer/windows/k3.wxs"

# 4. Windows ZIP archive
echo "--> Packaging Windows ZIP archive..."
zip -j "$DIST_DIR/kotlin3-v$VERSION-windows-x64.zip" "$BASE_DIR/target/x86_64-pc-windows-gnu/release/k3.exe"

# 5. Linux DEB package
echo "--> Building Debian / Ubuntu .deb package..."
DEB_ROOT="$DIST_DIR/deb_root"
rm -rf "$DEB_ROOT"
mkdir -p "$DEB_ROOT/DEBIAN" "$DEB_ROOT/usr/local/bin"
cat << EOF > "$DEB_ROOT/DEBIAN/control"
Package: kotlin3
Version: $VERSION
Section: devel
Priority: optional
Architecture: amd64
Maintainer: Kotlin3 Project <https://github.com/babar-xagi/kotlin3>
Description: Ultra-fast Kotlin compiler and unified toolchain written in Rust & Zig
 Kotlin3 (k3) compiles Kotlin code up to 3,800x faster than JVM kotlinc
 and generates standalone native binaries with sub-millisecond startup.
EOF
cp "$BASE_DIR/target/release/k3" "$DEB_ROOT/usr/local/bin/k3"
chmod 755 "$DEB_ROOT/usr/local/bin/k3"
dpkg-deb --build --root-owner-group "$DEB_ROOT" "$DIST_DIR/kotlin3_${VERSION}_amd64.deb"
rm -rf "$DEB_ROOT"

# 6. Linux tar.gz archive
echo "--> Packaging Linux tar.gz archive..."
tar -czf "$DIST_DIR/kotlin3-v$VERSION-linux-x86_64.tar.gz" -C "$BASE_DIR/target/release" k3

echo "=== All Distribution Packages Created Successfully in $DIST_DIR ==="
ls -lh "$DIST_DIR"
