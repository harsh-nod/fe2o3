# Parse one complete harness, including only source-checked abort-child transcripts.
function fail() { bad=1; exit 1 }
$0 == "running " count " tests" {
    if (active || found++) fail()
    active=1
    next
}
!active { if ($0 !~ /^[[:space:]]*$/) fail(); next }
/^test result:/ {
    if (pending || rows != count) fail()
    footer="test result: ok. " passes " passed; 0 failed; " ignores " ignored; 0 measured; " (filtered+0) " filtered out;"
    if (index($0, footer) != 1 || substr($0, length(footer)+1) !~ /^ finished in [0-9]+(\.[0-9]+)?s$/) fail()
    active=0
    complete=1
    next
}
/^$/ { next }
pending {
    if ($0 == "running 1 test") {
        if (++children > expected) fail()
    } else if ($0 == "ok" && children == expected) {
        print pending
        rows++
        passed++
        pending=""
    } else fail()
    next
}
/^test [a-zA-Z0-9_:]+ has been running for over 60 seconds$/ {
    if (allow_progress != 1 || finished[$2] || waiting[$2]++) fail()
    next
}
/^test [a-zA-Z0-9_:]+ \.\.\. (ok|ignored.*)$/ {
    if (finished[$2]++) fail()
    print $2
    rows++
    if ($4 == "ok") passed++; else skipped++
    next
}
/^test [a-zA-Z0-9_:]+ \.\.\. $/ {
    if (finished[$2]++) fail()
    if ($2 == "kfd_backend::tests::runtime_compute_pipeline_drop_aborts_for_every_live_logical_phase") expected=5
    else if ($2 == "kfd_backend::tests::scripted_sdma_drop_still_aborts_with_live_or_terminal_custody") expected=2
    else if ($2 == "queue_linux::tests::terminal_unpublished_cleanup_failure_aborts_instead_of_losing_custody" ||
             $2 == "queue_linux::tests::unpublished_custody_cleanup_failure_is_process_terminal" ||
             $2 == "queue_linux::tests::payload_release_failure_after_event_destroy_is_process_terminal") expected=1
    else fail()
    pending=$2
    children=0
    next
}
{ fail() }
END {
    if (bad || active || pending || found != 1 || !complete || passed != passes || skipped != ignores) exit 1
    for (name in waiting) if (finished[name] != 1) exit 1
}
