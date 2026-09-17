#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
fixture() {
    local mode=$1 name=kfd_backend::tests::runtime_compute_pipeline_drop_aborts_for_every_live_logical_phase
    local children=5
    case "$mode" in missing) children=4 ;; extra) children=6 ;; foreign) name=foreign_parent ;; esac
    if [[ $mode == linux ]]; then
        name=queue_linux::tests::terminal_unpublished_cleanup_failure_aborts_instead_of_losing_custody
        children=1
    fi
    [[ $mode != unrelated_footer ]] || printf 'test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
    printf 'running 2 tests\n'
    [[ $mode != bare ]] || printf 'ok\n'
    printf 'test %s ... \n' "$name"
    for ((i=0; i<children; i++)); do printf '\nrunning 1 test\n'; done
    [[ $mode == unfinished ]] || printf 'ok\n'
    [[ $mode != duplicate ]] || printf 'ok\n'
    printf 'test ordinary::test ... ok\n'
    [[ $mode != truncated ]] || return 0
    case "$mode" in
        malformed_footer|unrelated_footer) printf 'test result: FAILED\n' ;;
        wrong_count) printf 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n' ;;
        *) printf 'test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n' ;;
    esac
}
fixture valid | awk -v count=2 -v passes=2 -v ignores=0 -f "$archive/parse-results.awk" > /dev/null
fixture linux | awk -v count=2 -v passes=2 -v ignores=0 -f "$archive/parse-results.awk" > /dev/null
for mode in missing extra foreign bare unfinished duplicate truncated malformed_footer unrelated_footer wrong_count; do
    if fixture "$mode" | awk -v count=2 -v passes=2 -v ignores=0 -f "$archive/parse-results.awk" > /dev/null; then
        printf 'incorrect acceptance: %s\n' "$mode" >&2
        exit 1
    fi
done
progress_fixture() {
    local mode=$1 name=ordinary::first
    [[ $mode != unmatched ]] || name=ordinary::missing
    printf 'running 2 tests\n'
    [[ $mode != late ]] || printf 'test ordinary::first ... ok\n'
    printf 'test %s has been running for over 60 seconds\n' "$name"
    [[ $mode != duplicate ]] || printf 'test %s has been running for over 60 seconds\n' "$name"
    [[ $mode == late ]] || printf 'test ordinary::first ... ok\n'
    printf 'test ordinary::second ... ok\n'
    printf 'test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 60.1s\n'
}
progress_fixture valid | awk -v count=2 -v passes=2 -v ignores=0 -v filtered=3 -v allow_progress=1 -f "$archive/parse-results.awk" > /dev/null
for mode in unmatched duplicate late; do
    if progress_fixture "$mode" | awk -v count=2 -v passes=2 -v ignores=0 -v filtered=3 -v allow_progress=1 -f "$archive/parse-results.awk" > /dev/null; then
        printf 'incorrect progress acceptance: %s\n' "$mode" >&2
        exit 1
    fi
done
if progress_fixture valid | awk -v count=2 -v passes=2 -v ignores=0 -v filtered=0 -v allow_progress=1 -f "$archive/parse-results.awk" > /dev/null; then
    printf 'incorrect filtered-count acceptance\n' >&2
    exit 1
fi
parsed=$(awk -v count=913 -v passes=896 -v ignores=17 -f "$archive/parse-results.awk" "$archive/raw/gnu-final-runtime.log")
diff -u <(sed -n 's/^\(.*\): test$/\1/p' "$archive/raw/gnu-runtime-roster.log") \
    <(printf '%s\n' "$parsed" | LC_ALL=C sort)
printf 'PASS: complete GNU runtime roster; valid child and filtered-progress transcripts; fourteen malformed transcript rejections.\n'
