#!/usr/bin/env bash

set -Eeuo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
GENERATED_FILE=""

cleanup() {
  if [[ -n "${GENERATED_FILE}" ]]; then
    rm -f -- "${GENERATED_FILE}"
  fi
}

trap cleanup EXIT

usage() {
  cat <<'EOF'
Usage: scripts/parity-matrix.sh <check|generate> [status.tsv] [matrix.md]

The TSV file is authoritative for pinned commits, row IDs, and statuses.
"check" fails if the Markdown projection differs. "generate" updates it.
EOF
}

main() {
  local command="${1:-}"
  local status_file="${2:-${REPO_ROOT}/docs/cuda-oxide-parity-status.tsv}"
  local matrix_file="${3:-${REPO_ROOT}/docs/cuda-oxide-parity-matrix.md}"

  case "${command}" in
    check | generate) ;;
    -h | --help | help)
      usage
      return 0
      ;;
    *)
      usage >&2
      return 2
      ;;
  esac

  if [[ ! -f "${status_file}" ]]; then
    printf 'parity status file does not exist: %s\n' "${status_file}" >&2
    return 2
  fi
  if [[ ! -f "${matrix_file}" ]]; then
    printf 'parity matrix does not exist: %s\n' "${matrix_file}" >&2
    return 2
  fi

  GENERATED_FILE="$(mktemp "${TMPDIR:-/tmp}/fe2o3-parity-matrix.XXXXXX")"

  if ! LC_ALL=C awk -v mode="${command}" '
    BEGIN {
      FS = "\t"
      baseline_start = "<!-- parity-status:baseline:start -->"
      baseline_end = "<!-- parity-status:baseline:end -->"
      counts_start = "<!-- parity-status:counts:start -->"
      counts_end = "<!-- parity-status:counts:end -->"
    }

    function fail(message) {
      print "parity matrix: " message > "/dev/stderr"
      failed = 1
      exit 2
    }

    function valid_hash(value) {
      return length(value) == 40 && value ~ /^[0-9a-f]+$/
    }

    function valid_date(value, fields) {
      if (value !~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/) {
        return 0
      }
      split(value, fields, "-")
      return fields[2] >= 1 && fields[2] <= 12 &&
        fields[3] >= 1 && fields[3] <= 31
    }

    function valid_status(value) {
      return value == "Complete" || value == "Partial" ||
        value == "Missing" || value == "N/A"
    }

    function trim(value) {
      sub(/^[[:space:]]+/, "", value)
      sub(/[[:space:]]+$/, "", value)
      return value
    }

    function parse_source() {
      source_lines = FNR
      if (FNR == 1) {
        if (NF != 2 || $1 != "schema_version" || $2 != "1") {
          fail("status line 1 must be exactly schema_version<TAB>1")
        }
        return
      }
      if (FNR == 2) {
        if (NF != 2 || $1 != "cuda_oxide_commit" || !valid_hash($2)) {
          fail("status line 2 must contain a lowercase 40-digit cuda-oxide commit")
        }
        cuda_commit = $2
        return
      }
      if (FNR == 3) {
        if (NF != 2 || $1 != "cuda_oxide_date" || !valid_date($2)) {
          fail("status line 3 must contain a YYYY-MM-DD cuda-oxide date")
        }
        cuda_date = $2
        return
      }
      if (FNR == 4) {
        if (NF != 2 || $1 != "fe2o3_commit" || !valid_hash($2)) {
          fail("status line 4 must contain a lowercase 40-digit fe2o3 commit")
        }
        fe2o3_commit = $2
        return
      }
      if (FNR == 5) {
        if (NF != 3 || $1 != "kind" || $2 != "id" || $3 != "status") {
          fail("status line 5 must be exactly kind<TAB>id<TAB>status")
        }
        return
      }
      if (FNR < 6 || NF != 3) {
        fail("status line " FNR " must contain exactly three tab-separated fields")
      }
      if (!valid_status($3)) {
        fail("invalid status for " $2 ": " $3)
      }

      if ($1 == "normative") {
        if (source_supplemental != 0) {
          fail("normative ID " $2 " appears after supplemental rows")
        }
        source_normative++
        expected = sprintf("%02d", source_normative)
      } else if ($1 == "supplemental") {
        source_supplemental++
        expected = sprintf("S%02d", source_supplemental)
      } else {
        fail("invalid row kind for " $2 ": " $1)
      }

      if ($2 != expected) {
        fail("duplicate, missing, or out-of-order status ID: expected " expected ", found " $2)
      }
      if ($2 in source_status) {
        fail("duplicate status ID: " $2)
      }

      source_status[$2] = $3
      source_count[$1, $3]++
    }

    function finish_source() {
      if (source_finished) {
        return
      }
      source_finished = 1
      if (source_lines < 5) {
        fail("status file is truncated")
      }
      if (source_normative != 94) {
        fail("status file must contain normative IDs 01-94; found " source_normative)
      }
      if (source_supplemental != 15) {
        fail("status file must contain supplemental IDs S01-S15; found " source_supplemental)
      }
    }

    function print_baseline() {
      print "The fixed comparison point is the fetched cuda-oxide `origin/main` commit"
      print "`" cuda_commit "` from " cuda_date ". The primary"
      print "source is `cuda-oxide-book/appendix/supported-features.md` at that commit. Its"
      print "94 feature rows are reproduced below in the same category order, including"
      print "partial, experimental, planned, and N/A rows. The supplemental audit also"
      print "accounts for capabilities demonstrated elsewhere in the repository."
      print ""
      print "The fe2o3 status floor and default claim snapshot are based on commit"
      print "`" fe2o3_commit "`."
      print "Qualifying per-row evidence may name a landed descendant of that commit; this"
      print "projection does not claim that every change at current HEAD has qualifying parity"
      print "evidence."
    }

    function print_counts() {
      print "| Scope | Complete | Partial | Missing | N/A | Total |"
      print "|:--|--:|--:|--:|--:|--:|"
      print "| Normative | " (source_count["normative", "Complete"] + 0) " | " \
        (source_count["normative", "Partial"] + 0) " | " \
        (source_count["normative", "Missing"] + 0) " | " \
        (source_count["normative", "N/A"] + 0) " | " source_normative " |"
      print "| Supplemental | " (source_count["supplemental", "Complete"] + 0) " | " \
        (source_count["supplemental", "Partial"] + 0) " | " \
        (source_count["supplemental", "Missing"] + 0) " | " \
        (source_count["supplemental", "N/A"] + 0) " | " source_supplemental " |"
    }

    function join_cells(cells, count, result, i) {
      result = cells[1]
      for (i = 2; i <= count; i++) {
        result = result "|" cells[i]
      }
      return result
    }

    function process_row(line, cells, count, id, status_field, current, expected) {
      count = split(line, cells, /\|/)
      if (count < 3) {
        return line
      }
      id = trim(cells[2])
      if (id ~ /^[0-9][0-9]$/) {
        if (count != 9) {
          fail("normative Markdown row " id " must contain seven columns")
        }
        if (doc_supplemental != 0) {
          fail("normative Markdown row " id " appears after supplemental rows")
        }
        doc_normative++
        expected = sprintf("%02d", doc_normative)
        status_field = 6
      } else if (id ~ /^S[0-9][0-9]$/) {
        if (count != 8) {
          fail("supplemental Markdown row " id " must contain six columns")
        }
        doc_supplemental++
        expected = sprintf("S%02d", doc_supplemental)
        status_field = 5
      } else {
        return line
      }

      if (id != expected) {
        fail("duplicate, missing, or out-of-order Markdown ID: expected " expected ", found " id)
      }
      if (!(id in source_status)) {
        fail("Markdown contains unknown parity ID: " id)
      }
      if (id in doc_seen) {
        fail("Markdown contains duplicate parity ID: " id)
      }
      doc_seen[id] = 1

      current = trim(cells[status_field])
      if (!valid_status(current)) {
        fail("invalid Markdown status for " id ": " current)
      }
      cells[status_field] = " " source_status[id] " "
      return join_cells(cells, count)
    }

    function process_markdown(line) {
      if (index(line, "<!-- parity-status:") != 0) {
        if (line == baseline_start) {
          if (baseline_open || counts_open || baseline_starts != 0) {
            fail("duplicate or nested baseline marker")
          }
          baseline_starts++
          baseline_open = 1
          print line
          print_baseline()
          return
        }
        if (line == baseline_end) {
          if (!baseline_open || baseline_ends != 0) {
            fail("unmatched baseline end marker")
          }
          baseline_open = 0
          baseline_ends++
          print line
          return
        }
        if (line == counts_start) {
          if (baseline_open || counts_open || counts_starts != 0) {
            fail("duplicate or nested counts marker")
          }
          counts_starts++
          counts_open = 1
          print line
          print_counts()
          return
        }
        if (line == counts_end) {
          if (!counts_open || counts_ends != 0) {
            fail("unmatched counts end marker")
          }
          counts_open = 0
          counts_ends++
          print line
          return
        }
        fail("unknown parity-status marker: " line)
      }

      if (baseline_open || counts_open) {
        return
      }
      print process_row(line)
    }

    NR == FNR {
      parse_source()
      next
    }

    {
      if (!source_finished) {
        finish_source()
      }
      matrix_seen = 1
      process_markdown($0)
    }

    END {
      if (failed) {
        exit 2
      }
      finish_source()
      if (!matrix_seen) {
        fail("Markdown matrix is empty")
      }
      if (baseline_open || counts_open) {
        fail("unterminated generated block")
      }
      if (baseline_starts != 1 || baseline_ends != 1) {
        fail("Markdown must contain exactly one baseline generated block")
      }
      if (counts_starts != 1 || counts_ends != 1) {
        fail("Markdown must contain exactly one counts generated block")
      }
      if (doc_normative != source_normative) {
        fail("Markdown must contain IDs 01-94; found " doc_normative)
      }
      if (doc_supplemental != source_supplemental) {
        fail("Markdown must contain IDs S01-S15; found " doc_supplemental)
      }
    }
  ' "${status_file}" "${matrix_file}" >"${GENERATED_FILE}"; then
    return 2
  fi

  if [[ "${command}" == "check" ]]; then
    if ! cmp -s "${matrix_file}" "${GENERATED_FILE}"; then
      printf '%s\n' \
        'parity matrix is out of date; run scripts/parity-matrix.sh generate' >&2
      diff -u --label "${matrix_file}" --label generated \
        "${matrix_file}" "${GENERATED_FILE}" >&2 || true
      return 1
    fi
    printf 'parity matrix is current: 94 normative rows, 15 supplemental rows\n'
    return 0
  fi

  if cmp -s "${matrix_file}" "${GENERATED_FILE}"; then
    printf 'parity matrix already current: %s\n' "${matrix_file}"
  else
    cp -- "${GENERATED_FILE}" "${matrix_file}"
    printf 'updated parity matrix: %s\n' "${matrix_file}"
  fi
}

main "$@"
