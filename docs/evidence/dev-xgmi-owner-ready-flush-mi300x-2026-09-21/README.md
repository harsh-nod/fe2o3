# Ordered XGMI Owner And Ready-Prefix Qualification

Signed source: `c44625e12a1567204e1cb186b9f4da867244f686`.
Implementation: `9acf0e3875fa3a216a423455274d0802b35df97f`; the subsequent
source commit removes a trailing blank line from the runner.
Release musl ELF SHA-256:
`a596e701aafb55186d7610470139f2658cef3de3082590703e1c8037dfd9c620`.

## Result

One native owner-engine trial passed on MI300X GPUs 5 and 6:

| Index | PCI BDF | Unique ID |
| --- | --- | --- |
| 5 | `0000:a6:00.0` | `0xb7baafd0fb173d8e` |
| 6 | `0000:c6:00.0` | `0x10a254ce4987e716` |

The version journal stayed enabled. A real pending producer-to-consumer request
was refused before consumer submission, with its destination unchanged. The
producer was then explicitly progressed to success before a fresh consumer was
submitted with that completed event dependency. This records the missing pending
versioned-input handoff, not support for it.

The accepted path passed pre-submission cancellation, timeout identity recovery,
dropped-observer completion without caller-driven GPU progress, two successful
65-descriptor lists in opposite directions, ordered overlaps and duplicates,
complete destination canaries, unchanged sources, metadata-credit release, and
explicit owner/native shutdown. `ordered_lists=2` excludes the separately refused
and cancelled requests. The source uses a bounded ready-prefix flush of at most
eight descriptor observations, retaining one native ticket and all currentness
checks. The native receipt does not measure observations per flush or speedup.

All six endpoint observations passed: both endpoints immediately before the
workload, after a two-second settling delay, and after a further twenty-second
delay. Other host work was present. Idle observations are not an exclusive
reservation. Raw receipts were collected byte-for-byte before exact owned-tree
cleanup; both path and owned-process absence were confirmed.

## CPU Scope

Pinned nightly `2026-04-03`, GNU and musl, all runtime features, optimization level
1, debug assertions and overflow checks: **1221 passed and 20 hardware-specific
tests ignored on each target**, plus all three owner-example tests on each.
No-default-features library checking, warnings-denied all-feature library/test/
owner-example Clippy, selected changed-module rustfmt, source brackets and
whitespace checks passed. The three focused R74 Rust model tests passed.
Release musl example tests and both three-test Python parser/controller suites
also passed without skips; parser tests exercised the actual release binary's
pre-device argument rejection.

This packet does not rerun the full KFD/model suites, doctests, unsafe-policy
inventory or Verus campaign. Existing R74 abstract transitions cover the serial
custody pattern, not executable Rust/native refinement or the eight-observation
scheduling bound. No performance, HIP/HSA parity, GPU-side dependency-packet,
fault-injection, wall-clock bound, or formal-refinement claim is made.

## Replay

From the evidence archive commit, with the signed source commit reachable:

```sh
python3 -I -B docs/evidence/dev-xgmi-owner-ready-flush-mi300x-2026-09-21/verify.py
python3 -I -B docs/evidence/dev-xgmi-owner-ready-flush-mi300x-2026-09-21/test_verify.py
```

Replay authenticates the source signature and all 5640 selected tracked source
files, the exact 16 local, 11 remote and 10 CPU command receipts, build and test
environments, endpoint identities/activity/process observations, ELF/source/tool
continuity, result parsing, collection chronology, cleanup and claim boundaries.
Mutation tests rehash disposable copies and require altered commands, sources,
build controls, admission, results, scope claims and cleanup to be rejected.
The inventory seal is an integrity check; authenticity depends on the signed
archive commit, not a self-authenticating hash file.

Replay requires seven current worktree helper files to match the signed source
blobs and the pinned signer artifact in the earlier evidence directory to remain
available. It fails closed if those inputs change; this is not a standalone
portable packet. The temporary qualification script and its unrecorded clean
status assertions are not archived. Its tracked-source `git diff` brackets and
all qualification command receipts are retained, and the campaign separately
binds the complete selected source-file inventory to signed Git blobs.

`finished.json.native_execution` means the whole campaign qualified, not merely
that a workload process ran. The successful archived workload receipt and all
postflight/collection receipts are independently required by replay.
