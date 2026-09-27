# Indexed Asynchronous SDMA Publication

CPU developer evidence over scripted native owners. This is not native GPU
execution, formal unwind proof, complete #182/A1/A2 closure, or HIP/HSA parity or
performance evidence. The full runtime objective remains open.

Signed source: `7243932d177e03899c56493884cb4a382da26bea`.
Source tree: `98402a54162984dccd8960cbe54d673e3542a521`.
Baseline: `d3d0220ba0c40efd0d198d4a7112eb205ad73334`.
The source signature verified for `harmenon@amd.com`.

## Reproduced And Fixed

Immediate asynchronous publication and later flush-driven publication detached
the admitted descriptor before native submission. A scripted ProcessTeardown
return retained the native pair but lost the logical descriptor, leaving its
allocation custody, FIFO and completion reservation live without their indexed
owner. `publication-before-fix` reproduces the missing descriptor through public
H2D admission; the original regression source and baseline identity are retained.
This is not evidence of real GPU storage being freed while active.

Publication now accepts an indexed submission ID. The descriptor is installed
once at admission and retained through preparation, storage extraction, native
submission and restoration. Preparation sets its phase to Quarantined. Attempted
window metadata is retained before moving native owners. Successful handoff roots
the published native owner in place before indexing it or recording publication
history. Returned teardown custody is rooted before diagnostic formatting.

Terminal preparation/submission errors never release logical custody, including
after a completed prefix. The unwind guard poisons ingress and resumes the
original panic payload without overwriting an already-rooted published owner.
The scripted lower layer roots its pair before raising a submit panic; this does
not prove every opaque native publication unwind boundary.

Succeeded, Failed and Quiescent settlement share an ID-based, checked release
path. Dependencies, allocation custody, FIFO and result/reservation prerequisites
are checked before the callback-free release commit; the descriptor is removed
last. A clean later-window failure preserves partial-copy classification through
a quiescent marker, including planning failure. Retryable submission restores
the exact pair before settlement.

Missing endpoint records and unsupported post-admission kind pairs fail closed.
Quarantined settlement rejects in-flight storage. Unstarted Ready cleanup can
release its own reservation while a different predecessor still owns storage.
This distinction is necessary when a failed explicit dependency is observed
before the still-active FIFO predecessor. The checks rely on existing private
sorted-index/count invariants; they do not prove global conservation or detect
arbitrary corruption. Bounded dependency checks use stack-backed sorting, and
local owner/FIFO lookup uses binary search rather than whole-queue validation.
This is not a new complexity bound for every existing removal operation.

## Coverage

- Twelve immediate/flush publication cases cover H2D, D2H and same-device copies
  with ProcessTeardown or a lower-rooted submit panic. Flush cases use a genuinely
  admitted and completed same-stream predecessor. Checks include original owner
  IDs, descriptor/roster identity, request coordinates, exact teardown diagnostic,
  endpoint markers, logical retains, FIFO/tail, reservations and inert ingress.
- Twenty-four bounded-prefix cases cover those three copy kinds with terminal
  preparation corruption, failed planning, retryable submission, submit teardown,
  rooted submit panic, successful continuation, missing endpoint identity and
  unsupported retained kinds. Each prefix actually submits, completes and retires
  through the runtime; no completed-byte count is fabricated.
- Nonzero scripted source bytes and destination sentinels verify exactly the
  first eight bytes after prefix completion, the preserved tail after clean
  failure, and the first sixteen bytes after successful continuation. Terminal
  cases inspect exact native owner identities in the relevant retained location.
- A public two-stream regression admits D0 -> D and A -> B, with B explicitly
  depending on D before its implicit A dependency. D fails cleanly; B settles
  without consuming a native step while A remains published with its exact pair.
  A subsequently completes, and all scripted owners and retains are released.

The 36 subprocess scenarios include 27 terminal children requiring inspection
markers followed by real unrepaired Drop SIGABRT, with core dumps disabled, and
nine clean children requiring successful cleanup. No terminal fixture is disarmed.
Scalar/metadata snapshots do not themselves retain native buffer owners.

The bounded policy is test-only and changes window geometry, not production
completion state. It does not qualify full production window geometry or GPU
DMA. The planning fault is not an allocator-OOM experiment; the metadata faults
are deliberate private corruption after real admission. Clean scripted helpers
disable device scrub-on-release and establish scripted recycling, not production
scrub qualification. Existing directional/same-device and full-window tests remain
in the broader suite.

## Qualification

Final signed-source command, without skipped failing targets or concurrent
compilation/proof campaigns during Worker deadline tests:

```sh
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test \
  -p fe2o3-runtime -p fe2o3-runtime-model --all-features --no-fail-fast
```

- Runtime unit tests: 1,724 passed, 3 failed, 28 ignored.
- Model unit tests: 1,080 passed, 0 failed, 19 ignored.
- Runtime integration tests: 11 passed, 3 hardware tests ignored.
- Runtime doctests: 52 passed. Model doctests: 29 passed.
- Strict all-feature/all-target runtime/model Clippy passed.
- Runtime/model no-default-feature compilation and formatting checks passed.
- Final source continuity against the signed commit passed.
- The preceding focused KFD run passed 827 tests, with 28 ignored and no failures.

The full test command exits 101. The unchanged failures at
`authorized_execution.rs:1317` report `InspectSocket(PermissionDenied)`:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`.
They remain failed, not waived. No permission or escalation was requested.

The receipts preserve the initial missing-descriptor reproduction, intermediate
passing matrices, the expanded fixture's indexed-table insert compile error,
and the old unsupported-kind fixture failure. Three direct logical fixtures now
have actual endpoint allocations/a supported kind pair; the release helper also
reserves quiescent marker capacity as real admission does. The successful final
runs include these fixture corrections without weakening corruption assertions.
`final-*` receipts identify and qualify the signed source, while `source-patch`
retains the complete change and `review-notes.md` records read-only findings.

## Remaining Work

SDMA cancellation still detaches an unpublished descriptor before separate
logical cleanup; it is not covered by the new common settlement path. Full
queued-owner/arena invariants, release/disposal composition, opaque native unwind
boundaries and authenticated hardware qualification also remain open. No new
Verus proof or matched HIP/HSA measurement ran here. The wider Worker,
device-language, multi-device, memory, atomic/collective and profiling gates are
unchanged; see the [successor plan](../../runtime-successor-writer-plan-v1.md)
and [issue #182](https://github.com/harsh-nod/fe2o3/issues/182).

Source pushes to both requested GitHub remotes and MI300X SSH failed DNS
resolution. No remote jobs or files were created, and no permission escalation
was requested. Unrelated owner-inspection evidence was preserved. Evidence-commit
publication is attempted separately after freezing this packet.

`receipts.tar.xz` contains commands, exit statuses, timestamps, logs, source
identity/signature/patch, the original regression source and review notes. Its
companion SHA-256 identifies the frozen archive.
