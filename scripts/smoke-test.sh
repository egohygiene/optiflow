#!/usr/bin/env sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
temporary_root="$(mktemp -d)"
trap 'rm -rf "$temporary_root"' EXIT HUP INT TERM

input_directory="$temporary_root/media with spaces"
state_directory="$temporary_root/state"
mkdir -p "$input_directory"

printf "%s" "identical fixture bytes" > "$input_directory/first-🌌.bin"
cp "$input_directory/first-🌌.bin" "$input_directory/second.bin"
printf "%s" "different fixture" > "$input_directory/unique.bin"

cd "$repository_root"
cargo build --quiet --locked

report_path="$temporary_root/report.json"
./target/debug/optiflow \
  --state-directory "$state_directory" \
  --json \
  scan \
  --no-probe \
  "$input_directory" > "$report_path"

jq --exit-status '.schema == "optiflow.command-result.v1"' "$report_path" >/dev/null
jq --exit-status '.outcome.exit_code == 0' "$report_path" >/dev/null
jq --exit-status '.result.summary.exact_duplicate_groups == 1' "$report_path" >/dev/null
jq --exit-status '.result.summary.reclaimable_bytes == 23' "$report_path" >/dev/null

run_id="$(jq --raw-output '.result.run.run_id' "$report_path")"
plan_path="$temporary_root/plan.json"
./target/debug/optiflow \
  --state-directory "$state_directory" \
  --json \
  plan exact-duplicates \
  --run "$run_id" \
  --output "$plan_path" >/dev/null

jq --exit-status '.safety.mutates_files == false' "$plan_path" >/dev/null
jq --exit-status '.summary.action_count == 1' "$plan_path" >/dev/null
test -f "$input_directory/first-🌌.bin"
test -f "$input_directory/second.bin"

printf "%s\n" "optiflow smoke test passed."
