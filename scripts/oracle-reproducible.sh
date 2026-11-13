#!/usr/bin/env bash
# Reproducible build of the oracle component, byte-for-byte what OutLayer's workers would build.
#
# OutLayer's script (vendor/near-outlayer/scripts/build_github_wasm.sh, pinned compiler image,
# amd64) builds one directory. The oracle depends on ../shared, so this script first stages
# oracle/ and a copy of shared/ into one directory with two fixed edits:
#   - oracle/Cargo.toml: flatrate-shared path "../shared" -> "shared", and the oracle workspace
#     excludes it (so shared's dev-dependencies are not resolved);
#   - shared/Cargo.toml: the [dev-dependencies] section is dropped (test-only, not in the wasm).
# Same commit -> same staged bytes -> same wasm sha256.
#
# Usage: bash scripts/oracle-reproducible.sh [--twice] [--keep]
# ORACLE_DIR=oracle-tokens builds the token-price oracle (extra 2) the same way (default: oracle).
# Prints the sha256 on the last line of stdout. Needs docker and the OutLayer script (cloned
# into vendor/near-outlayer, or set OUTLAYER_BUILD_SCRIPT).
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$(pwd)
SCRIPT="${OUTLAYER_BUILD_SCRIPT:-$ROOT/vendor/near-outlayer/scripts/build_github_wasm.sh}"
if [ ! -f "$SCRIPT" ]; then
  echo "OutLayer build script not found at $SCRIPT." >&2
  echo "Get it with: git clone https://github.com/fastnear/near-outlayer vendor/near-outlayer && git -C vendor/near-outlayer checkout 2d50bfab14389857e6e50e84e76d5652b944e1cc" >&2
  exit 2
fi

ORACLE_DIR="${ORACLE_DIR:-oracle}"
if [ "$ORACLE_DIR" = oracle ]; then STAGE="$ROOT/target/oracle-stage"; else STAGE="$ROOT/target/$ORACLE_DIR-stage"; fi
rm -rf "$STAGE"
mkdir -p "$STAGE/shared"
cp -r "$ORACLE_DIR/src" "$ORACLE_DIR/Cargo.lock" "$STAGE/"
cp "$ORACLE_DIR/Cargo.toml" "$STAGE/Cargo.toml"
cp -r shared/src "$STAGE/shared/"
# Edit 1: oracle points at ./shared and does not pull it into its workspace.
sed -i.bak -e 's|path = "../shared"|path = "shared"|' -e 's|^\[workspace\]$|[workspace]\nexclude = ["shared"]|' "$STAGE/Cargo.toml"
# Edit 2: shared without its test-only dev-dependencies.
awk '/^\[dev-dependencies\]/{skip=1; next} /^\[/{skip=0} !skip' shared/Cargo.toml > "$STAGE/shared/Cargo.toml"
rm -f "$STAGE/Cargo.toml.bak"
# Fixed timestamps do not matter to cargo, but strip anything untracked just in case.
find "$STAGE" -name '*.orig' -delete

bash "$SCRIPT" --dir "$STAGE" --target wasm32-wasip2 "$@"
