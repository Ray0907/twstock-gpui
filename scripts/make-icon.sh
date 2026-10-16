#!/usr/bin/env bash
# Builds assets/icon/AppIcon.icns from assets/icon/icon-1024.png using only the
# tools that ship with macOS (sips, iconutil).
#
# icon-1024.png is the source of truth: a 1024x1024 transparent PNG with the
# macOS icon body (824px) centred and its shadow baked in. It was generated
# with Gemini image generation, then cut out of its studio backdrop.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ICON_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)/assets/icon"
PNG_1024="${ICON_DIR}/icon-1024.png"
ICNS_FILE="${ICON_DIR}/AppIcon.icns"
ICONSET_DIR="${ICON_DIR}/AppIcon.iconset"

[[ -f "${PNG_1024}" ]] || { echo "Error: ${PNG_1024} not found." >&2; exit 1; }
[[ "$(sips -g pixelWidth -g pixelHeight "${PNG_1024}" | awk '/pixel/{print $2}' | tr '\n' x)" == "1024x1024x" ]] \
    || { echo "Error: ${PNG_1024} must be 1024x1024." >&2; exit 1; }

rm -rf "${ICONSET_DIR}"
mkdir -p "${ICONSET_DIR}"

for base in 16 32 128 256 512; do
    sips -z "${base}" "${base}" "${PNG_1024}" --out "${ICONSET_DIR}/icon_${base}x${base}.png" >/dev/null
    sips -z "$((base * 2))" "$((base * 2))" "${PNG_1024}" --out "${ICONSET_DIR}/icon_${base}x${base}@2x.png" >/dev/null
done

iconutil -c icns "${ICONSET_DIR}" -o "${ICNS_FILE}"
rm -rf "${ICONSET_DIR}"
echo "Generated ${ICNS_FILE}"
