#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

die() {
  printf 'parity check reconciliation: %s\n' "$1" >&2
  exit 2
}

[[ "$#" == 7 ]] ||
  die 'usage: CHECKS_JSON APP_ID APP_SLUG CHECK_NAME HEAD_SHA EXTERNAL_ID MODE'

readonly CHECKS_JSON="$1"
readonly APP_ID="$2"
readonly APP_SLUG="$3"
readonly CHECK_NAME="$4"
readonly HEAD_SHA="$5"
readonly EXTERNAL_ID="$6"
readonly MODE="$7"

[[ -f "${CHECKS_JSON}" && ! -L "${CHECKS_JSON}" ]] ||
  die 'check-run response must be a regular file'
[[ "${APP_ID}" =~ ^[1-9][0-9]*$ ]] || die 'App ID is malformed'
[[ "${APP_SLUG}" =~ ^[A-Za-z0-9][A-Za-z0-9-]*$ ]] ||
  die 'App slug is malformed'
[[ -n "${CHECK_NAME}" && "${CHECK_NAME}" != *$'\n'* ]] ||
  die 'check name is malformed'
[[ "${HEAD_SHA}" =~ ^[0-9a-f]{40}$ && ! "${HEAD_SHA}" =~ ^0{40}$ ]] ||
  die 'head SHA is malformed or zero'
[[ "${EXTERNAL_ID}" == "fe2o3-parity-v1:${HEAD_SHA}" ]] ||
  die 'external ID is not deterministic for the head SHA'
[[ "${MODE}" == select ]] || die 'unsupported reconciliation mode'

jq -e '
  type == "object" and (.check_runs | type == "array") and
  (.total_count | type == "number") and .total_count >= 0 and
  .total_count == (.check_runs | length)
' "${CHECKS_JSON}" >/dev/null ||
  die 'check-run response is malformed, truncated, or ambiguous'

selection="$({
  jq -cer \
    --arg app_id "${APP_ID}" \
    --arg app_slug "${APP_SLUG}" \
    --arg name "${CHECK_NAME}" \
    --arg head "${HEAD_SHA}" \
    --arg external_id "${EXTERNAL_ID}" '
      [
        .check_runs[] |
        select(
          (.id | type) == "number" and .id > 0 and
          .name == $name and .head_sha == $head and
          ((.app.id | tostring) == $app_id) and
          .app.slug == $app_slug
        )
      ] as $owned |
      [$owned[] | select((.external_id // "") == $external_id)] as $dedicated |
      if ($dedicated | length) == 1 then
        "update\t\($dedicated[0].id)"
      elif ($dedicated | length) > 1 then
        error("duplicate deterministic App checks")
      elif ($owned | length) == 0 then
        "create"
      elif ($owned | length) == 1 then
        "update\t\($owned[0].id)"
      else
        error("ambiguous legacy App checks")
      end
    ' "${CHECKS_JSON}"
} 2>/dev/null)" || die 'App check selection is ambiguous'

printf '%s\n' "${selection}"
