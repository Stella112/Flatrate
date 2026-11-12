#!/usr/bin/env bash
# Windows helper: run a make target inside WSL with a Linux-side cargo target dir.
# Usage (from Git Bash/PowerShell on Windows): wsl.exe -d Debian --cd <repo> -- bash scripts/wsl.sh <make target>
set -euo pipefail
cd "$(dirname "$0")/.."
source "$HOME/.cargo/env"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/flatrate-target}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
make "$@"
