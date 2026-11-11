#!/usr/bin/env bash
# Regenerate every app icon from img/icone1.png (needs only cargo; no external tools).
#
# The outputs under assets/app-icon/ are committed, so building and packaging never need this. After
# running it, update the sha256 values in ATTRIBUTION.toml, then `cargo xtask assets --write`.
#
#   packaging/icons.sh
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
exec cargo xtask app-icons
