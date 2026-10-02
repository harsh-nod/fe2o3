# Cooperative Multi GPU Peer Progress

Implementation baseline: `2bc47c614eaff9ba75cedfe42c9c8c030f3bfd21`.

## Scope

The normal async registered-stream, tracked-operation and drain lanes now use
`RuntimeContextV1::progress_stream_v1`. This additive SPI defaults to the existing
full flush, preserving other backends and negotiated Worker behavior without a
new wire operation. Explicit Context flush and graph publication retain their
full-publication contract. Context validation, reservations, journal custody and
failure handling share the same implementation for both operations.

The multi-device KFD override attempts at most one cooperative Read/Write leaf
per call, including recursive native peer and staged-copy progress. A scoped
guard resets after success, error or unwind. Strict flush uses the same stream
ordering with the guard disabled. This prevents a ready peer packet chain from
being drained in a single normal async stream selection. Metadata traversal and
child-native calls retain their existing bounds; there is no new wall-clock,
syscall-count or whole-tick bound. Async scheduling lanes still have independent
budgets.

The late gathered-consumer witness now seeds the oldest peer through this API.
Its v2 report identifies `single-peer-leaf-quantum` capture rather than a short
drain deadline. Actual retained/completed counters and every original peer's
Pending status are still checked before and after consumer admission. Ordered
shared-destination dependencies identify the oldest root; the public counter
does not directly expose paired child markers. Real consumer admission validates
that paired custody. Publication and retirement require separate peer leaves.

## Coverage

The final snapshot passes all 2,215 runtime library tests, with 32 existing native
hardware ignores, and all 13 witness tests. Exact test rosters match the captured
outcomes without filtering. Strict all-feature library/test Clippy and strict
witness Clippy pass, as do all 61 doctests. The 32 ignored tests are not counted
as native passes; the separate selected hardware matrix is described below.
The no-default-feature library check, pinned formatter check of every changed
Rust file and whitespace check also pass.

Seventeen new unit tests cover SPI delegation, validation, error handling,
borrowed/background/current-thread engine selection, tracked work and drain,
multi-packet fairness between disjoint GPU pairs, cancellation between native
publication and sampling, pending samples, late directed resource predecessors,
and paired custody on terminal errors and unwinds. Scripted success cases check
full source/destination guards, original owner identities, result release and
scrub/demotion/recycle cleanup without clearing native flags.

The existing sharded-ring owner test now observes bounded cooperative progress
until actual first publication rather than assuming publication on the first
tick. It requires exactly two child markers naming that root, no completed or
staged peer, and retains its original futures, expired-deadline resumption,
full-byte checks and cleanup assertions. The initial failing full-suite result
and earlier test-harness compile diagnostics remain in the raw packet.

## Native Qualification

All 18 selected MI300X cases pass: eight late gathered-consumer cases across
three/four GPUs, both device orders and disjoint/overlapping destination windows;
eight matching prequeued controls; and two direct late-admission controls.
The independent Python oracle reconstructs every source-D, gathered-C,
computed-D, returned-E and host buffer, including ordered overwrites and logical
guards. The exact report schemas, 136 callbacks and 60 logical native peer copies
match. Each case completes result release and explicit owned shutdown.

The final native image is 8,790,696 bytes with SHA-256
`466eb689bb8a92da6ad8f3550c26c8c14ba4b59acfde595e0a020fd427ba8035`.
An earlier eighteen-case campaign passed, but subsequent test-only lint cleanup,
signature formatting and source-check metadata refresh produced a different
rebuilt image. That earlier campaign cannot qualify the final image. Acceptance
uses the full eighteen-case rerun under `hardware-02`, captured by
`hardware-final-04`, from 21:32:25 to 21:41:41 UTC on 2026-10-02 without retries.
Its build, hardware and CPU receipts all name the exact same final source
inventory. No source-delta or binary-equivalence exception is used. Earlier
campaigns and rejected lint/format receipts remain in the raw packet as
superseded diagnostics.

Cards 4 through 7 received fresh UID/BDF, idle-use, VRAM, process-roster and
process-to-device attachment checks before and after every case, together with
host available-memory checks. These are point observations, not exclusive
reservations. Foreign GPU 0 work was untouched. The exact owned executable was
confirmed inactive, hash-checked and removed with its fresh scratch directory;
final directory absence and device/process observations are recorded. No reset,
foreign process termination or broad temporary-directory cleanup was used.

## Source Controls

All 32 checked-in no-solver source controls pass on the committed baseline and
again on the final snapshot.
Initial candidate failures were broad source-identity/count drift. The reviewed
refresh changes only 16 hash literals and seven inventory counts across nine
guard files. The strict proposer and applied AST audit preserve predicates,
assumptions, proof bodies, solver counts and producer-reader identity pins.
All 76 existing executable proof-closure files are byte-identical to baseline.
This refresh does not formally qualify the new quantum adapter or rerun Verus.

## Limits

Native witnesses use the unchanged finite R57 qualification authority, not a
general production application-kernel proof provider. General generated-kernel
authorization remains a separate compiler/runtime integration requirement.
Retained native custody is not proof of an active GPU fence or physical overlap.
Scripted error tests do not establish native fault isolation. Logical byte guards
do not inspect hidden physical pool padding. No matched HIP/HSA performance
campaign, fresh solver run, whole-adapter formal refinement or full parity is
claimed.

The raw packet preserves source inventories, commands, outputs, controller
identities, baseline comparison, metadata proposals, failed diagnostics and the
accepted qualification receipts. `raw-manifest.json` hashes archive members;
`SHA256SUMS` binds this README, manifest and archive. No native executable is
distributed.
