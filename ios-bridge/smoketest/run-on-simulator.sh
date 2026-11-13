#!/usr/bin/env bash
# Builds smoketest/main.c against the simulator slice of IosBridge.xcframework (built by
# ../build-xcframework.sh) and runs it under the iOS Simulator via `simctl spawn`, proving the
# .xcframework actually links and runs under the iphonesimulator platform — not just that it
# compiles for that target on the host, which `cargo test` already covers.
#
# Usage: run-on-simulator.sh [<simulator-udid-or-name>]  (default: "iPad (A16)")
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
CRATE_DIR="$(cd .. && pwd)"
XCFRAMEWORK="$CRATE_DIR/build/IosBridge.xcframework"
SIM_SLICE="$XCFRAMEWORK/ios-arm64-simulator"

if [[ ! -d "$SIM_SLICE" ]]; then
    echo "error: $SIM_SLICE not found — run ios-bridge/build-xcframework.sh first" >&2
    exit 1
fi

DEVICE="${1:-iPad (A16)}"
BOOT_STATE=$(xcrun simctl list devices | grep -F "$DEVICE" | grep -o '(Booted)' || true)
if [[ -z "$BOOT_STATE" ]]; then
    echo "==> booting simulator: $DEVICE"
    xcrun simctl boot "$DEVICE"
fi

BUILD_DIR="$CRATE_DIR/build/smoketest"
mkdir -p "$BUILD_DIR"
BIN="$BUILD_DIR/smoketest"

echo "==> compiling smoketest for iphonesimulator"
xcrun --sdk iphonesimulator clang \
    -arch arm64 -mios-simulator-version-min=17.0 \
    -I "$SIM_SLICE/Headers" \
    main.c "$SIM_SLICE/libios_bridge.a" \
    -o "$BIN"

DEMO="${2:-$CRATE_DIR/../samples/test_demo.dem}"
if [[ ! -f "$DEMO" ]]; then
    echo "error: demo not found at $DEMO (not committed to git; pass a path as the 2nd argument)" >&2
    exit 1
fi
OUT="$BUILD_DIR/out.json.gz"

echo "==> running under the simulator ($DEVICE)"
xcrun simctl spawn "$DEVICE" "$BIN" "$DEMO" "$OUT"
echo "==> output: $OUT ($(stat -f%z "$OUT") bytes)"
