# Primary Release Late-Selection CPU Qualification

CPU development above signed source commit
`8b0021ba740c337b2641ecd26ed6c9375613ed32`, bound to the exact selected
source map recorded before and after qualification. This packet does not alter
the accepted R125 boundary and is not native failure, formal, or performance
acceptance.

## Boundary

The native failure fixture no longer asks whether retained-primary release is
supported immediately after `RuntimeContextV1::shutdown`. That point still owns
normal persistent-control, dispatch-data, and SDMA-pool custody which production
`shutdown_native_v1` settles before it chooses primary-release strategy. The
premature predicate was therefore false on the exercised native profile.

The corrected test arms the existing production selection observer with the
fault. It requires the real shutdown path to select retained-primary release
after normal prefix settlement, then injects the same deterministic error or
panic only after the genuine queue is installed in the original preallocated
Box. Before terminal reentry it rearms the observer false and requires it remain
false, while retaining the prior exact Box, wrapper-entry, host/device account,
profiler, destroyed-event, and panic-payload assertions. Production behavior is
unchanged; only the ignored hardware-qualification fixture changed.

## Qualification

`qualify-final.sh` serializes the final CPU commands with the requested
low-memory settings and records exact command, UTC interval, full combined
output, and exit status. GNU and scoped musl runtime suites, runtime doctests,
strict all-target all-feature Clippy, no-default checking, workspace formatting,
and the unsafe-source policy passed. GNU and musl each report 1,102 passed and
20 ignored. The two native probes remain ignored and are not counted as passes.
All 46 runtime doctests passed: four compile and 42 compile-fail. The unsafe
policy reports five passed and one ignored. Musl uses
`FE2O3_HIP_SYS_DISABLE=1`.

The selected source map covers Cargo inputs, all crate sources, examples,
runtime benchmark sources, the unsafe inventory, and the runtime primary-release
document. It excludes `docs/evidence` and the untracked RAM-backed `target`
symlink. The verifier compares all selected identities with signed commit
`8b0021ba`; the sole permitted delta is
`retained_release_tests/primary_envelope.rs`. Evidence-only commits may change
the recorded HEAD field but not the qualified file map.

Commands are path-bound to this c4 worktree. Compiler caches, toolchain and
system inputs are identified by commands, versions, and resulting executable
hashes rather than a hermetic closure.

The first campaign passed both runtime suites, both harness builds, executable
identity capture, doctests, strict Clippy, and no-default checking, then stopped
at workspace formatting. Its sole reported delta was rustfmt wrapping the new
selection assertion. It did not run the unsafe-source policy or an after-source
snapshot. All 13 preliminary receipts remain intact, including the fmt exit one,
and all six initial helper scripts are copied byte-for-byte beneath
`preliminary-scripts/`. The final campaign has a distinct `-final` receipt
namespace and begins from the mechanically formatted fixture. Qualification
does not treat any preliminary result as a substitute for its final counterpart.
The final before/after source maps contain 5,553 identical file identities. GNU
and musl executable identities were also unchanged before and after execution.

The first archive verifier then failed because its receipt modeled the
archive-only script commands with absolute paths while they were recorded with
relative paths. Its exact source and failure are preserved as
`failed-verifier.py` and `raw/verify.*`. The corrected verifier models those
commands without changing any qualification result; a fresh lint, format,
shell-syntax, ShellCheck, and verifier set uses the `-final` suffix. Across both
cohorts and both archive-check sets there are 38 receipts: 36 exit zero and the
two preserved preliminary failures are workspace fmt and the path-model verifier.

## Limits

No ignored native test, SSH command, GPU workload, native ioctl fault, formal
solver, or performance measurement is part of this archive. The preceding
native failure and its receipts remain in a separate packet owned by the native
campaign agent; this archive neither rewrites nor accepts them.
