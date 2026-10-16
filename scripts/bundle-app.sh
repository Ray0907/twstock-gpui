#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

BUNDLE_DIR="${REPO_ROOT}/target/bundle"
APP_NAME="twstock-gpui.app"
APP_PATH="${BUNDLE_DIR}/${APP_NAME}"
CONTENTS_DIR="${APP_PATH}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"
PLIST_FILE="${CONTENTS_DIR}/Info.plist"

cd "${REPO_ROOT}"

echo "==> Building release binary with cargo..."
cargo build --release

RELEASE_BIN="${REPO_ROOT}/target/release/twstock-gpui"
if [[ ! -f "${RELEASE_BIN}" ]]; then
    echo "Error: Release binary not found at ${RELEASE_BIN}" >&2
    exit 1
fi

# Ensure AppIcon.icns exists, or generate it
ICNS_SRC="${REPO_ROOT}/assets/icon/AppIcon.icns"
if [[ ! -f "${ICNS_SRC}" ]]; then
    echo "==> AppIcon.icns missing, running make-icon.sh..."
    "${SCRIPT_DIR}/make-icon.sh"
fi

echo "==> Assembling ${APP_PATH}..."
rm -rf "${APP_PATH}"
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}"

# Copy executable
cp "${RELEASE_BIN}" "${MACOS_DIR}/twstock-gpui"
chmod +x "${MACOS_DIR}/twstock-gpui"

# Copy icon
cp "${ICNS_SRC}" "${RESOURCES_DIR}/AppIcon.icns"

# Generate Info.plist
cat << 'EOF' > "${PLIST_FILE}"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleName</key>
    <string>twstock-gpui</string>
    <key>CFBundleDisplayName</key>
    <string>twstock-gpui</string>
    <key>CFBundleIdentifier</key>
    <string>dev.local.twstock-gpui</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleExecutable</key>
    <string>twstock-gpui</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.finance</string>
</dict>
</plist>
EOF

echo "==> Validating Info.plist..."
plutil -lint "${PLIST_FILE}"

echo "==> Successfully created app bundle: ${APP_PATH}"
echo "    Note: First launch of an unsigned app on macOS may require right-click -> Open."
