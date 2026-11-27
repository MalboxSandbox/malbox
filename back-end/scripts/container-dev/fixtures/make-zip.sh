#!/usr/bin/env bash
# Writes $STATE/sample.zip containing test-sample.sh, for `run.sh submit-auto`.
# STATE defaults to the same directory run.sh uses (MALBOX_DEV_STATE).
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
DEV=$(cd "$HERE/.." && pwd)
BACKEND=$(cd "$DEV/../.." && pwd)
STATE=${STATE:-${MALBOX_DEV_STATE:-$BACKEND/.data/container-dev}}

command -v zip >/dev/null || { echo "make-zip.sh: the zip CLI is required" >&2; exit 1; }
mkdir -p "$STATE"
rm -f "$STATE/sample.zip"
# -j stores the file without its directory path.
zip -q -j "$STATE/sample.zip" "$DEV/test-sample.sh"
echo "$STATE/sample.zip"
