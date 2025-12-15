#!/usr/bin/env bash
#
# Build script for fastn-based WASM projects.
# Usage: ./scripts/fastn-build.sh <package-name>
#
# This script:
# 1. Builds the WASM package using wasm-pack
# 2. Optimizes with wasm-opt
# 3. Adds content hashes to filenames for cache busting
# 4. Generates index.html from template with correct references
# 5. Copies files to root for easy local serving
#
# Template: Uses ./index.html.tmpl if present, otherwise uses the default
# template from the fastn scripts directory.
# Placeholders: {{PKG}}, {{JS_FILE}}, {{WASM_FILE}}

set -e -x

PKG="${1:?Usage: $0 <package-name>}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd -P)"

# Build WASM
wasm-pack build "$PKG" --target web

# Optimize WASM
# We need --enable-bulk-memory for Rust 1.92+ which emits bulk memory operations.
# We need --enable-nontrapping-float-to-int for i32.trunc_sat_f64_u instructions.
wasm-opt "$PKG/pkg/${PKG}_bg.wasm" -o "$PKG/pkg/${PKG}_bg.wasm" -O \
  --enable-bulk-memory \
  --enable-nontrapping-float-to-int

# Generate content hashes for cache busting
# Use first 8 chars of sha256 hash
# Works on both macOS (shasum) and Linux (sha256sum)
if command -v sha256sum &> /dev/null; then
    WASM_HASH=$(sha256sum "$PKG/pkg/${PKG}_bg.wasm" | cut -c1-8)
    JS_HASH=$(sha256sum "$PKG/pkg/${PKG}.js" | cut -c1-8)
else
    WASM_HASH=$(shasum -a 256 "$PKG/pkg/${PKG}_bg.wasm" | cut -c1-8)
    JS_HASH=$(shasum -a 256 "$PKG/pkg/${PKG}.js" | cut -c1-8)
fi

# Create hashed filenames
WASM_FILE="${PKG}_bg.${WASM_HASH}.wasm"
JS_FILE="${PKG}.${JS_HASH}.js"

# Rename WASM file
mv "$PKG/pkg/${PKG}_bg.wasm" "$PKG/pkg/${WASM_FILE}"

# Update JS file to reference hashed WASM filename, then rename it
# sed -i works differently on macOS vs Linux, so use a temp file approach
sed "s/${PKG}_bg\.wasm/${WASM_FILE}/g" "$PKG/pkg/${PKG}.js" > "$PKG/pkg/${PKG}.js.tmp"
mv "$PKG/pkg/${PKG}.js.tmp" "$PKG/pkg/${JS_FILE}"
rm -f "$PKG/pkg/${PKG}.js"

# Generate index.html from template
# Use local template if present, otherwise use default from fastn scripts
if [[ -f "index.html.tmpl" ]]; then
    TEMPLATE="index.html.tmpl"
else
    TEMPLATE="$SCRIPT_DIR/index.html.tmpl"
fi

sed -e "s/{{PKG}}/${PKG}/g" \
    -e "s/{{JS_FILE}}/${JS_FILE}/g" \
    -e "s/{{WASM_FILE}}/${WASM_FILE}/g" \
    "$TEMPLATE" > "$PKG/pkg/index.html"

# Copy to root for easy local serving
cp "$PKG/pkg/${WASM_FILE}" .
cp "$PKG/pkg/${JS_FILE}" .
cp "$PKG/pkg/index.html" .

# Clean old hashed files from root and pkg
find . -maxdepth 1 -name "${PKG}_bg.*.wasm" ! -name "${WASM_FILE}" -delete
find . -maxdepth 1 -name "${PKG}.*.js" ! -name "${JS_FILE}" -delete
find "$PKG/pkg" -maxdepth 1 -name "${PKG}_bg.*.wasm" ! -name "${WASM_FILE}" -delete
find "$PKG/pkg" -maxdepth 1 -name "${PKG}.*.js" ! -name "${JS_FILE}" -delete

echo "Generated files:"
echo "  WASM: ${WASM_FILE}"
echo "  JS:   ${JS_FILE}"
echo "  HTML: index.html (from ${TEMPLATE})"
