#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LANDING_SOURCE="$REPOSITORY_ROOT/web/landing"
ARCHITECTURE_SOURCE="$REPOSITORY_ROOT/web/architecture"
STAGING_ROOT="$REPOSITORY_ROOT/.site/staging"
LANDING_STAGE="$STAGING_ROOT/landing"
ARCHITECTURE_STAGE="$STAGING_ROOT/architecture"
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

for managed_path in "$STAGING_ROOT" "$LANDING_STAGE" "$ARCHITECTURE_STAGE" "$DOCS_STAGE" "$SCHEMAS_STAGE" "$COMPOSE_STAGE" "$DIST_DIR"; do
  assert_managed_path "$managed_path"
done

if [[ ! -f "$LANDING_SOURCE/index.html" ]]; then
  echo "landing source is missing index.html" >&2
  exit 1
fi

if [[ ! -f "$ARCHITECTURE_SOURCE/architecture.config.json" ]]; then
  echo "architecture source is missing architecture.config.json" >&2
  exit 1
fi

rm -rf "$LANDING_STAGE" "$ARCHITECTURE_STAGE" "$DOCS_STAGE" "$SCHEMAS_STAGE" "$COMPOSE_STAGE"
mkdir -p "$LANDING_STAGE" "$ARCHITECTURE_STAGE/assets" "$SCHEMAS_STAGE" "$STAGING_ROOT"
cp -R "$LANDING_SOURCE"/. "$LANDING_STAGE"/
cp -R "$ARCHITECTURE_SOURCE/assets"/. "$ARCHITECTURE_STAGE/assets"/
cp -R "$REPOSITORY_ROOT/schemas"/. "$SCHEMAS_STAGE"/

for reserved_mount in architecture docs schemas; do
  if [[ -e "$LANDING_STAGE/$reserved_mount" ]]; then
    echo "landing source collides with the reserved /$reserved_mount/ mount" >&2
    exit 1
  fi
done

cd "$REPOSITORY_ROOT"
uv run --frozen --only-group docs python scripts/site/generate_architecture.py \
  --repository-root "$REPOSITORY_ROOT" \
  --config "$ARCHITECTURE_SOURCE/architecture.config.json" \
  --output "$ARCHITECTURE_STAGE"
uv run --frozen --only-group docs zensical build --clean --strict

if [[ ! -f "$ARCHITECTURE_STAGE/index.html" || ! -f "$ARCHITECTURE_STAGE/architecture.json" ]]; then
  echo "architecture generator did not produce the expected portal" >&2
  exit 1
fi

if [[ ! -f "$DOCS_STAGE/index.html" ]]; then
  echo "Zensical did not produce the expected docs/index.html" >&2
  exit 1
fi

mkdir -p "$COMPOSE_STAGE"
cp -R "$LANDING_STAGE"/. "$COMPOSE_STAGE"/
mkdir -p "$COMPOSE_STAGE/architecture"
cp -R "$ARCHITECTURE_STAGE"/. "$COMPOSE_STAGE/architecture"/
mkdir -p "$COMPOSE_STAGE/docs"
cp -R "$DOCS_STAGE"/. "$COMPOSE_STAGE/docs"/
mkdir -p "$COMPOSE_STAGE/schemas"
cp -R "$SCHEMAS_STAGE"/. "$COMPOSE_STAGE/schemas"/
: > "$COMPOSE_STAGE/.nojekyll"

uv run --frozen --only-group docs python scripts/site/verify.py "$COMPOSE_STAGE"

rm -rf "$DIST_DIR"
mv "$COMPOSE_STAGE" "$DIST_DIR"

echo "site artifact composed at $DIST_DIR"
