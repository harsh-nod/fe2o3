# Primary Release Failure Envelope CPU Qualification

CPU development above base `1890a64e1911a2a346c5a9e70a8ff5ad45b19231`,
bound to the exact final uncommitted source map in
`raw/source-before-final.log`. This packet does not change the accepted R125
boundary and is not native failure, formal or performance acceptance.

## Boundary

The retained ordinary-primary shutdown path now enters a private shared helper
that installs the genuine queue into the original preallocated
`PrimaryQueueReleaseCustodyV1` Box before calling the lower release operation.
The helper removes that root only after an exact `ComputeAqlQueueDestroyedV1`
result. A lower error terminalizes while retaining the Box; a lower panic
attempts the same terminalization and then resumes the original payload. The
destroyed profile event and final retirement remain after successful helper
return.

The test-only interception is compiled only with both `test` and
`hardware-qualification`; every other build calls the unchanged real lower
`release_in_place`. Two ignored native tests use the existing admitted vecadd
workflow to create a genuine retained primary queue, then inject deterministic
post-install Err or panic results. Their isolated child protocol requires exactly
one harness test and one case marker and replays the child transcript. The tests
assert original Box identity, terminal/non-retired state, exactly one wrapper
entry across inert reentry, absence of a destroyed event, unchanged retained
host/device accounts and profiler state across retry, and panic payload identity.
Terminal native custody remains rooted until child exit.

## Qualification

`qualify-final.sh` serialized the final Cargo commands with low-memory settings
and recorded their exact command, UTC interval, full combined output and exit
status. GNU and musl each pass 1,102 runtime tests with 20 ignored. The two
additions relative to the hash-pinned Worker V3 bootstrap rosters are exactly the
new ignored native tests; ignored tests are not passes. Musl uses
`FE2O3_HIP_SYS_DISABLE=1` and does not substitute another native backend.

The final GNU and musl test executables are bound by path and SHA-256 before and
after execution and to the current build outputs. All 46 runtime doctests pass:
four compile and 42 compile-fail. Strict all-feature/all-target runtime Clippy,
runtime no-default-feature checking, workspace formatting and the unsafe-source
policy pass. The unsafe-policy harness remains five passed and one ignored; this
packet adds no unsafe block.

The final before/after map covers 5,553 selected Cargo and repository source
files, including all crate sources, examples, runtime benchmark sources, the
unsafe inventory and the changed runtime document. It excludes `docs/evidence`
and the untracked `target` symlink. The verifier compares current file identities
to that final map without requiring HEAD to remain the recorded base after a
containing commit. Commands remain bound to this archive's original absolute c4
worktree path; the SHA-256 manifest itself is portable. Cargo caches, compiler
binaries, system libraries and headers are identified only by commands/tool
versions and are not a hermetic input closure.

All 38 receipts are required by `verify.py`. Thirty-six exit zero. The original
evidence-script format check and the verifier it preceded are preserved with
exit one: formatting was still needed, and that verifier correctly rejected a
source change before the final campaign. Fresh formatted-script checks and the
final verifier exit zero. The verifier parses complete harnesses through a
hash-pinned prior parser and checks exact old-roster preservation, both campaign
source maps, final executable identity, command chronology and raw receipt
closure before sealing.

The preliminary formatter/verifier commands are path-bound to the same script
names later corrected for the final checks. Their original script bytes were not
copied aside, so their preserved command, output, timestamps and exit status are
historical evidence rather than a replayable snapshot of those intermediate
scripts.

## Limits And History

The two native tests were not executed in this CPU campaign. Their deterministic
runtime fault point is not a native ioctl failure. No MI300X work, formal solver,
performance measurement, Worker V3 authority or protected application claim is
part of this archive.

The first recorded full campaign also exited zero, but review then found that its
fault-only counter could not detect an erroneous second unarmed wrapper entry.
The test gained an observation-scoped total entry counter plus exact post-retry
account/profiler comparisons; the runtime document gained the result paragraph
and tightened oracle description. The two recorded source maps differ in exactly
those two files. The first executable hashes remain recorded before/after that
campaign but were superseded on disk; only the final executable hashes are
checked against the current files.

Before the archive, focused compilation and adjacent shutdown tests passed. Two
earlier compile attempts were started in the wrong `r61` worktree during a disk
space/reconciliation mistake and were interrupted without a result; that
worktree was restored clean before either full campaign. Those exploratory
attempts have no receipts here and are not qualification evidence.
