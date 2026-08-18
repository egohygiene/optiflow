#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LANDING_SOURCE="$REPOSITORY_ROOT/web/landing"
STAGING_ROOT="$REPOSITORY_ROOT/.site/staging"
LANDING_STAGE="$STAGING_ROOT/landing"
DOCS_STAGE="$STAGING_ROOT/docs"
SCHEMAS_STAGE="$STAGING_ROOT/schemas"
COMPOSE_STAGE="$REPOSITORY_ROOT/.site/compose"
DIST_DIR="$REPOSITORY_ROOT/dist"

assert_managed_path() {
  local candidate="$1"

  case "$candidate" in
    "$REPOSITORY_ROOT/.site"/*|"$REPOSITORY_ROOT/dist") ;;
    *)
      echo "refusing to manage unexpected path: $candidate" >&2
      exit 1
      ;;
  esac
}

for managed_path in "$STAGING_ROOT" "$LANDING_STAGE" "$DOCS_STAGE" "$SCHEMAS_STAGE" "$COMPOSE_STAGE" "$DIST_DIR"; do
  assert_managed_path "$managed_path"
done

if [[ ! -f "$LANDING_SOURCE/index.html" ]]; then
  echo "landing source is missing index.html" >&2
  exit 1
fi

rm -rf "$LANDING_STAGE" "$DOCS_STAGE" "$SCHEMAS_STAGE" "$COMPOSE_STAGE"
mkdir -p "$LANDING_STAGE" "$SCHEMAS_STAGE" "$STAGING_ROOT"
cp -R "$LANDING_SOURCE"/. "$LANDING_STAGE"/
cp -R "$REPOSITORY_ROOT/schemas"/. "$SCHEMAS_STAGE"/

for reserved_mount in docs schemas; do
  if [[ -e "$LANDING_STAGE/$reserved_mount" ]]; then
    echo "landing source collides with the reserved /$reserved_mount/ mount" >&2
    exit 1
  fi
done

cd "$REPOSITORY_ROOT"
uv run --frozen --only-group docs zensical build --clean --strict

if [[ ! -f "$DOCS_STAGE/index.html" ]]; then
  echo "Zensical did not produce the expected docs/index.html" >&2
  exit 1
fi

mkdir -p "$COMPOSE_STAGE"
cp -R "$LANDING_STAGE"/. "$COMPOSE_STAGE"/
mkdir -p "$COMPOSE_STAGE/docs"
cp -R "$DOCS_STAGE"/. "$COMPOSE_STAGE/docs"/
mkdir -p "$COMPOSE_STAGE/schemas"
cp -R "$SCHEMAS_STAGE"/. "$COMPOSE_STAGE/schemas"/
: > "$COMPOSE_STAGE/.nojekyll"

uv run --frozen --only-group docs python scripts/site/verify.py "$COMPOSE_STAGE"

rm -rf "$DIST_DIR"
mv "$COMPOSE_STAGE" "$DIST_DIR"

echo "site artifact composed at $DIST_DIR"
