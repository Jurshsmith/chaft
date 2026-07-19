#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

python3 -m py_compile \
  "$SCRIPT_DIR/generate-platform-verification-receipt.py" \
  "$SCRIPT_DIR/generate-platform-verification-receipt-test.py"
python3 "$SCRIPT_DIR/generate-platform-verification-receipt-test.py"
