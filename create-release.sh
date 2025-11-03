#!/usr/bin/env bash
set -euo pipefail

# Foundry Release Tarball Creator
# Based on .github/workflows/release.yml

# USAGE:
# VERSION_NAME="v1.4.4-inj" PROFILE="release" ./create-release.sh

# Configuration
PROFILE="${PROFILE:-maxperf}"
VERSION_NAME="${VERSION_NAME:-$(cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | select(.name == "foundry-cli") | .version')}"
PLATFORM="${PLATFORM:-linux}"
ARCH="${ARCH:-$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')}"

# Detect target triple
if [[ "$(uname -s)" == "Linux" ]]; then
    if [[ "$(uname -m)" == "x86_64" ]]; then
        TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
    elif [[ "$(uname -m)" == "aarch64" ]]; then
        TARGET="${TARGET:-aarch64-unknown-linux-gnu}"
    fi
elif [[ "$(uname -s)" == "Darwin" ]]; then
    PLATFORM="darwin"
    if [[ "$(uname -m)" == "x86_64" ]]; then
        TARGET="${TARGET:-x86_64-apple-darwin}"
    elif [[ "$(uname -m)" == "arm64" ]]; then
        TARGET="${TARGET:-aarch64-apple-darwin}"
        ARCH="arm64"
    fi
fi

TARGET="${TARGET:-$(rustc -vV | sed -n 's|host: ||p')}"
OUT_DIR="target/${TARGET}/${PROFILE}"

echo "Building Foundry release tarball..."
echo "  Version: ${VERSION_NAME}"
echo "  Platform: ${PLATFORM}"
echo "  Arch: ${ARCH}"
echo "  Target: ${TARGET}"
echo "  Profile: ${PROFILE}"
echo "  Output: ${OUT_DIR}"
echo ""

# Build flags based on CI configuration
BUILD_FLAGS=(
    --target "${TARGET}"
    --profile "${PROFILE}"
    --bins
    --no-default-features
    --features aws-kms,gcp-kms,cli,asm-keccak,js-tracer
)

# Add jemalloc for non-MSVC and non-aarch64-linux targets
if [[ "${TARGET}" != *msvc* && "${TARGET}" != "aarch64-unknown-linux-gnu" ]]; then
    BUILD_FLAGS+=(--features jemalloc)
fi

echo "Building binaries with cargo..."
cargo build "${BUILD_FLAGS[@]}"

echo ""
echo "Verifying binaries..."
BINS=(anvil cast chisel forge)
for bin in "${BINS[@]}"; do
    BIN_PATH="${OUT_DIR}/${bin}"
    echo "  ${bin}:"
    file "${BIN_PATH}" || true
    du -h "${BIN_PATH}" || true
    "${BIN_PATH}" --version || true
    echo ""
done

echo "Creating release tarball..."
TARBALL="foundry_${VERSION_NAME}_${PLATFORM}_${ARCH}.tar.gz"

if [[ "$(uname -s)" == "Darwin" ]]; then
    # macOS requires gtar for consistent archives
    if ! command -v gtar &> /dev/null; then
        echo "Warning: gtar not found, using tar (may have compatibility issues)"
        tar -czvf "${TARBALL}" -C "${OUT_DIR}" "${BINS[@]}"
    else
        gtar -czvf "${TARBALL}" -C "${OUT_DIR}" "${BINS[@]}"
    fi
else
    tar -czvf "${TARBALL}" -C "${OUT_DIR}" "${BINS[@]}"
fi

echo ""
echo "✓ Release tarball created: ${TARBALL}"
echo "  Size: $(du -h "${TARBALL}" | cut -f1)"
echo ""
echo "Contents:"
tar -tzf "${TARBALL}"
