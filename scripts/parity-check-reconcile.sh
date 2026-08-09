#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

die() {
  printf 'parity check reconciliation: %s\n' "$1" >&2
  exit 2
}

readonly PAGE_SIZE=100
readonly MAX_PAGES=10
readonly SERVER_CAP=1000
readonly MAX_TOTAL_RUNS=10000
readonly MAX_SUITE_BYTES=4194304
readonly MAX_RUN_BYTES=67108864

regular_bounded_file() {
  local path="$1"
  local max_bytes="$2"
  local label="$3"
  local size
  [[ -f "${path}" && ! -L "${path}" ]] || die "${label} must be a regular file"
  size="$(wc -c <"${path}")"
  [[ "${size}" =~ ^[0-9]+$ && "${size}" -le "${max_bytes}" ]] ||
    die "${label} exceeds its size bound"
}

suite_ids() {
  local suite_pages="$1"
  local suite_total
  local suite_raw_count
  local suite_unique_count
  regular_bounded_file "${suite_pages}" "${MAX_SUITE_BYTES}" 'check-suite pages'
  jq -e --argjson max_pages "${MAX_PAGES}" --argjson page_size "${PAGE_SIZE}" '
    type == "array" and length > 0 and length <= $max_pages and
    all(.[];
      type == "object" and
      (.total_count | type) == "number" and
      .total_count >= 0 and .total_count == (.total_count | floor) and
      (.check_suites | type) == "array" and
      (.check_suites | length) <= $page_size
    ) and
    ([.[].total_count] | unique | length) == 1
  ' "${suite_pages}" >/dev/null ||
    die 'check-suite pagination shape/count is malformed'
  suite_total="$(jq -er '.[0].total_count' "${suite_pages}")"
  ((suite_total < SERVER_CAP)) ||
    die 'check-suite inventory reached the GitHub server cap'
  jq -e --argjson page_size "${PAGE_SIZE}" --argjson total "${suite_total}" '
    (if $total == 0 then 1
     else (($total + $page_size - 1) / $page_size | floor)
     end) as $expected_pages |
    length == $expected_pages and
    (to_entries | all(.[];
      if .key < ($expected_pages - 1)
      then (.value.check_suites | length) == $page_size
      else (.value.check_suites | length) ==
        ($total - (($expected_pages - 1) * $page_size))
      end
    ))
  ' "${suite_pages}" >/dev/null ||
    die 'check-suite pagination is truncated or has invalid page boundaries'
  suite_raw_count="$(jq '[.[].check_suites[]] | length' "${suite_pages}")"
  suite_unique_count="$(jq '
    [.[].check_suites[].id |
      select(type == "number" and . > 0 and . == floor)] | unique | length
  ' "${suite_pages}")"
  [[ "${suite_raw_count}" == "${suite_total}" &&
    "${suite_unique_count}" == "${suite_total}" ]] ||
    die 'check-suite inventory is truncated or contains duplicate IDs'
  jq -r '[.[].check_suites[].id] | sort[]' "${suite_pages}"
}

inventory() {
  local suite_pages="$1"
  local run_groups="$2"
  local suite_inventory
  local suite_total
  local run_total
  local run_unique_count
  suite_inventory="$(suite_ids "${suite_pages}")"
  if [[ -z "${suite_inventory}" ]]; then
    suite_total=0
  else
    suite_total="$(wc -l <<<"${suite_inventory}")"
  fi
  regular_bounded_file "${run_groups}" "${MAX_RUN_BYTES}" 'check-run pages'

  jq -e \
    --argjson max_pages "${MAX_PAGES}" \
    --argjson page_size "${PAGE_SIZE}" \
    --argjson suite_total "${suite_total}" '
      type == "array" and length == $suite_total and
      ([.[].suite_id] | length) == ([.[].suite_id] | unique | length) and
      all(.[];
        .suite_id as $suite_id |
        ($suite_id | type) == "number" and
        $suite_id > 0 and $suite_id == ($suite_id | floor) and
        (.pages | type) == "array" and
        (.pages | length) > 0 and (.pages | length) <= $max_pages and
        all(.pages[];
          type == "object" and
          (.total_count | type) == "number" and
          .total_count >= 0 and .total_count == (.total_count | floor) and
          (.check_runs | type) == "array" and
          (.check_runs | length) <= $page_size
        ) and
        ([.pages[].total_count] | unique | length) == 1
      )
    ' "${run_groups}" >/dev/null ||
    die 'check-run pagination shape/count is malformed'

  if jq -e --argjson server_cap "${SERVER_CAP}" '
    any(.[].pages[].total_count; . >= $server_cap)
  ' "${run_groups}" >/dev/null; then
    die 'check-run inventory reached the GitHub server cap'
  fi
  jq -e --argjson page_size "${PAGE_SIZE}" '
    all(.[];
      .suite_id as $suite_id |
      .pages[0].total_count as $total |
      (if $total == 0 then 1
       else (($total + $page_size - 1) / $page_size | floor)
       end) as $expected_pages |
      (.pages | length) == $expected_pages and
      (.pages | to_entries | all(.[];
        if .key < ($expected_pages - 1)
        then (.value.check_runs | length) == $page_size
        else (.value.check_runs | length) ==
          ($total - (($expected_pages - 1) * $page_size))
        end
      )) and
      ([.pages[].check_runs[]] | length) == $total and
      ([.pages[].check_runs[].id] | length) ==
        ([.pages[].check_runs[].id] | unique | length) and
      all(.pages[].check_runs[];
        (.id | type) == "number" and .id > 0 and .id == (.id | floor) and
        (.check_suite.id | type) == "number" and
        .check_suite.id == (.check_suite.id | floor) and
        .check_suite.id == $suite_id
      )
    )
  ' "${run_groups}" >/dev/null ||
    die 'check-run inventory is truncated, duplicated, or has invalid page boundaries'

  jq -e --slurpfile suites "${suite_pages}" '
    ([.[] | .suite_id] | sort) ==
      ([$suites[0][].check_suites[].id] | sort)
  ' "${run_groups}" >/dev/null ||
    die 'check-run inventory does not cover every check suite exactly once'
  run_total="$(jq '[.[].pages[].check_runs[]] | length' "${run_groups}")"
  ((run_total <= MAX_TOTAL_RUNS)) || die 'check-run inventory exceeds total bound'
  run_unique_count="$(jq '[.[].pages[].check_runs[].id] | unique | length' \
    "${run_groups}")"
  [[ "${run_unique_count}" == "${run_total}" ]] ||
    die 'check-run IDs are duplicated across check suites'
  jq -cS '{
    total_count: ([.[].pages[].check_runs[]] | length),
    check_runs: [.[].pages[].check_runs[]]
  }' "${run_groups}"
}

select_check() {
  local checks_json="$1"
  local app_id="$2"
  local app_slug="$3"
  local app_owner="$4"
  local check_name="$5"
  local head_sha="$6"
  local external_id="$7"
  local owner_mismatch
  local owned_count
  local legacy_count
  local dedicated_count
  local check_id
  regular_bounded_file "${checks_json}" "${MAX_RUN_BYTES}" 'check-run inventory'
  [[ "${app_id}" =~ ^[1-9][0-9]*$ ]] || die 'App ID is malformed'
  [[ "${app_slug}" =~ ^[A-Za-z0-9][A-Za-z0-9-]*$ ]] || die 'App slug is malformed'
  [[ -z "${app_owner}" ||
    "${app_owner}" =~ ^[A-Za-z0-9]([A-Za-z0-9-]{0,37}[A-Za-z0-9])?$ ]] ||
    die 'App owner is malformed'
  [[ -n "${check_name}" && "${check_name}" != *$'\n'* ]] ||
    die 'check name is malformed'
  [[ "${head_sha}" =~ ^[0-9a-f]{40}$ && ! "${head_sha}" =~ ^0{40}$ ]] ||
    die 'head SHA is malformed or zero'
  [[ "${external_id}" == "fe2o3-parity-v1:${head_sha}" ]] ||
    die 'external ID is not deterministic for the head SHA'
  jq -e '
    type == "object" and (.check_runs | type == "array") and
    (.total_count | type) == "number" and .total_count >= 0 and
    .total_count == (.check_runs | length) and
    ([.check_runs[].id] | length) == ([.check_runs[].id] | unique | length)
  ' "${checks_json}" >/dev/null || die 'check-run inventory is malformed'

  owner_mismatch="$(jq \
    --arg app_id "${app_id}" --arg app_slug "${app_slug}" \
    --arg app_owner "${app_owner}" --arg name "${check_name}" \
    --arg head "${head_sha}" '[
      .check_runs[] |
      select(.name == $name and .head_sha == $head and
        ((.app.id | tostring) == $app_id) and .app.slug == $app_slug and
        $app_owner != "" and
        ((.app.owner.login // "" | ascii_downcase) != ($app_owner | ascii_downcase)))
    ] | length' "${checks_json}")"
  ((owner_mismatch == 0)) || die 'App owner does not match configured owner'

  owned_count="$(jq \
    --arg app_id "${app_id}" --arg app_slug "${app_slug}" \
    --arg app_owner "${app_owner}" --arg name "${check_name}" \
    --arg head "${head_sha}" '[
      .check_runs[] |
      select(.name == $name and .head_sha == $head and
        ((.app.id | tostring) == $app_id) and .app.slug == $app_slug and
        ($app_owner == "" or
          ((.app.owner.login // "" | ascii_downcase) == ($app_owner | ascii_downcase))))
    ] | length' "${checks_json}")"
  legacy_count="$(jq \
    --arg app_id "${app_id}" --arg app_slug "${app_slug}" \
    --arg app_owner "${app_owner}" --arg name "${check_name}" \
    --arg head "${head_sha}" --arg external_id "${external_id}" '[
      .check_runs[] |
      select(.name == $name and .head_sha == $head and
        ((.app.id | tostring) == $app_id) and .app.slug == $app_slug and
        ($app_owner == "" or
          ((.app.owner.login // "" | ascii_downcase) == ($app_owner | ascii_downcase))) and
        ((.external_id // "") != $external_id))
    ] | length' "${checks_json}")"
  ((legacy_count == 0)) ||
    die 'legacy App check blocks deterministic-check activation'
  dedicated_count=$((owned_count - legacy_count))
  if ((dedicated_count == 0)); then
    printf 'create\n'
    return 0
  fi
  ((dedicated_count == 1 && owned_count == 1)) ||
    die 'multiple deterministic App checks block reconciliation'
  check_id="$(jq -er \
    --arg app_id "${app_id}" --arg app_slug "${app_slug}" \
    --arg app_owner "${app_owner}" --arg name "${check_name}" \
    --arg head "${head_sha}" --arg external_id "${external_id}" '
      .check_runs[] |
      select(.name == $name and .head_sha == $head and
        ((.app.id | tostring) == $app_id) and .app.slug == $app_slug and
        ($app_owner == "" or
          ((.app.owner.login // "" | ascii_downcase) == ($app_owner | ascii_downcase))) and
        .external_id == $external_id) | .id
    ' "${checks_json}")"
  [[ "${check_id}" =~ ^[1-9][0-9]*$ ]] || die 'selected check ID is malformed'
  printf 'update\t%s\n' "${check_id}"
}

case "${1:-}" in
  suite-ids)
    [[ "$#" == 2 ]] || die 'usage: suite-ids SUITE_PAGES_JSON'
    suite_ids "$2"
    ;;
  inventory)
    [[ "$#" == 3 ]] || die 'usage: inventory SUITE_PAGES_JSON RUN_GROUPS_JSON'
    inventory "$2" "$3"
    ;;
  select)
    [[ "$#" == 8 ]] ||
      die 'usage: select CHECKS_JSON APP_ID APP_SLUG APP_OWNER CHECK_NAME HEAD_SHA EXTERNAL_ID'
    select_check "$2" "$3" "$4" "$5" "$6" "$7" "$8"
    ;;
  *)
    die 'mode must be suite-ids, inventory, or select'
    ;;
esac
