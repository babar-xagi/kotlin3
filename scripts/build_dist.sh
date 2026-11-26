#!/usr/bin/env bash
set -euo pipefail

VERSION="0.7.0"
BASE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$BASE_DIR"
DIST_DIR="$BASE_DIR/dist"

echo "=== Building Kotlin3 v$VERSION Distribution Packages ==="
mkdir -p "$DIST_DIR"
rm -rf "$DIST_DIR"/*

echo "--> Compiling Linux x86_64 release binary..."
cargo build --release

if rustup target list --installed | grep -q x86_64-pc-windows-gnu; then
    echo "--> Compiling Windows x86_64 release binary..."
    cargo build --target x86_64-pc-windows-gnu --release
    if command -v wixl >/dev/null 2>&1; then
        wixl -o "$DIST_DIR/kotlin3-v$VERSION-windows-x64.msi" "$BASE_DIR/installer/windows/k3.wxs"
    fi
    if command -v zip >/dev/null 2>&1; then
        zip -j "$DIST_DIR/kotlin3-v$VERSION-windows-x64.zip" "$BASE_DIR/target/x86_64-pc-windows-gnu/release/k3.exe"
    fi
fi

if command -v dpkg-deb >/dev/null 2>&1; then
    echo "--> Building Debian / Ubuntu .deb package..."
    DEB_ROOT="$DIST_DIR/deb_root"
    rm -rf "$DEB_ROOT"
    mkdir -p "$DEB_ROOT/DEBIAN" "$DEB_ROOT/usr/local/bin"
    chmod 755 "$DEB_ROOT" "$DEB_ROOT/DEBIAN" "$DEB_ROOT/usr" "$DEB_ROOT/usr/local" "$DEB_ROOT/usr/local/bin"
    cat << EOF > "$DEB_ROOT/DEBIAN/control"
Package: kotlin3
Version: $VERSION
Section: devel
Priority: optional
Architecture: amd64
Maintainer: Kotlin3 Project <https://github.com/babar-xagi/kotlin3>
Description: Fast, zero-Gradle, zero-Maven Kotlin CLI, package manager, and runtime in Rust & Kotlin
EOF
    chmod 644 "$DEB_ROOT/DEBIAN/control"
    cp "$BASE_DIR/target/release/k3" "$DEB_ROOT/usr/local/bin/k3"
    chmod 755 "$DEB_ROOT/usr/local/bin/k3"
    dpkg-deb --build --root-owner-group "$DEB_ROOT" "$DIST_DIR/kotlin3_${VERSION}_amd64.deb"
    rm -rf "$DEB_ROOT"
fi

echo "--> Packaging Linux tar.gz archive..."
tar -czf "$DIST_DIR/kotlin3-v$VERSION-linux-x86_64.tar.gz" -C "$BASE_DIR/target/release" k3

echo "=== Distribution Packages Created in $DIST_DIR ==="
ls -lh "$DIST_DIR"
