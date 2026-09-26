#!/usr/bin/env bash
set -uo pipefail

evidence_directory="${OPTIFLOW_EVIDENCE_DIRECTORY:-target/adversarial-evidence}"
mkdir -p "${evidence_directory}"
summary="${evidence_directory}/summary.tsv"
printf "case\tstatus\n" > "${summary}"
overall_status=0

run_case() {
  local name="$1"
  shift
  local log="${evidence_directory}/${name}.log"

  set +e
  "$@" 2>&1 | tee "${log}"
  local status=${PIPESTATUS[0]}
  set -e

  if [[ ${status} -eq 0 ]]; then
    printf "%s\tpassed\n" "${name}" >> "${summary}"
  else
    printf "%s\tfailed:%s\n" "${name}" "${status}" >> "${summary}"
    overall_status=1
  fi
}

run_properties() {
  run_case "properties" cargo test --locked --test properties
}

run_faults() {
  run_case "fault-artifact-set" cargo test --locked --lib "artifact_set::tests::"
  run_case "fault-discovery" cargo test --locked --lib "discovery::tests::"
  run_case "fault-observation" cargo test --locked --lib "observation::tests::"
  run_case "fault-state" cargo test --locked --lib "state::tests::"
  run_case "fault-subprocess" cargo test --locked --lib "subprocess::tests::"
  run_case "fault-extensions" cargo test --locked --test extensions
}

run_fuzz() {
  local sanitizer_options="detect_leaks=0"
  if [[ -n "${ASAN_OPTIONS:-}" ]]; then
    sanitizer_options="${ASAN_OPTIONS}:detect_leaks=0"
  fi
  run_case "fuzz-config-document" env ASAN_OPTIONS="${sanitizer_options}" \
    cargo fuzz run config-document fuzz/corpus/config_document -- \
    -max_total_time=15 -timeout=5 -max_len=65536
  run_case "fuzz-artifact-set-reader" env ASAN_OPTIONS="${sanitizer_options}" \
    cargo fuzz run artifact-set-reader fuzz/corpus/artifact_set_reader -- \
    -max_total_time=15 -timeout=5 -max_len=65536
}

run_corpus() {
  run_case "filesystem-corpus" python3 scripts/filesystem-corpus.py --check --tier pr
}

if [[ $# -eq 0 ]]; then
  set -- properties faults corpus
fi

for suite in "$@"; do
  case "${suite}" in
    properties)
      run_properties
      ;;
    faults)
      run_faults
      ;;
    fuzz)
      run_fuzz
      ;;
    corpus)
      run_corpus
      ;;
    *)
      printf "unknown adversarial suite: %s\n" "${suite}" >&2
      exit 2
      ;;
  esac
done

exit "${overall_status}"
