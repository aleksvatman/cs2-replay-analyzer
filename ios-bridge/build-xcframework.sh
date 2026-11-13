#!/usr/bin/env bash
# Builds ios-bridge for the device and the Apple Silicon simulator, generates the C header with
# cbindgen, and packages both into ios-bridge/build/IosBridge.xcframework for the iPad app to
# link. Run from anywhere; paths are resolved relative to this script.
#
# Requires: rustup targets aarch64-apple-ios and aarch64-apple-ios-sim, and cbindgen
# (`cargo install cbindgen`).
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

CRATE_DIR="$(pwd)"
REPO_ROOT="$(cd .. && pwd)"
BUILD_DIR="$CRATE_DIR/build"
HEADERS_DIR="$BUILD_DIR/headers"
XCFRAMEWORK="$BUILD_DIR/IosBridge.xcframework"

rm -rf "$BUILD_DIR"
mkdir -p "$HEADERS_DIR"

echo "==> cbindgen: generating header"
cbindgen --config cbindgen.toml --crate ios-bridge --output "$HEADERS_DIR/ios_bridge.h"
cat > "$HEADERS_DIR/module.modulemap" <<'EOF'
module IosBridge {
    header "ios_bridge.h"
    export *
}
EOF

echo "==> cargo build: aarch64-apple-ios (device, release)"
(cd "$REPO_ROOT" && cargo build -p ios-bridge --release --target aarch64-apple-ios)

echo "==> cargo build: aarch64-apple-ios-sim (simulator, release)"
(cd "$REPO_ROOT" && cargo build -p ios-bridge --release --target aarch64-apple-ios-sim)

DEVICE_LIB="$REPO_ROOT/target/aarch64-apple-ios/release/libios_bridge.a"
SIM_LIB="$REPO_ROOT/target/aarch64-apple-ios-sim/release/libios_bridge.a"

echo "==> xcodebuild -create-xcframework"
rm -rf "$XCFRAMEWORK"
xcodebuild -create-xcframework \
    -library "$DEVICE_LIB" -headers "$HEADERS_DIR" \
    -library "$SIM_LIB" -headers "$HEADERS_DIR" \
    -output "$XCFRAMEWORK"

echo "==> done: $XCFRAMEWORK"
