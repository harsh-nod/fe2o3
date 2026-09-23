# Exact declared-target debugger readback — 2026-09-23

This additive CPU debugger/query/bridge/site path advances #281 V4. It is
not GPU target detection, native register capture or a hardware adapter.

## Implemented contract

Simulation bundle admission retains a sealed declaration with the original
envelope version/identity, subject identity, admitted module wire version and
full canonical digest/length. All six supported loaders bind it before
flattening the module. Raw KIR has no verified bundle declaration.
Mutable module or hash substitution cannot replace the retained owner.

The opt-in `inspect_declared_target` query binds the same backend/capture
owner, full cursor, configuration and revision as runtime observations.
Wrong owners/cursors and stale revisions refuse; disabled runtime observations
return InvalidState, and raw input returns explicit target Unavailable.
Replies are separately bounded to 4,096 bytes including LF. The stream writer
flushes every response; a regression covers buffering and flush errors.

The bridge admits one explicit read using its currently accepted runtime
binding. It cannot override the GPU target. The existing four/six-call
runtime/storage collector remains unchanged; this query is separate.
The site validates the full reply/session and hides prior values while pending.
Controls, changed selections, reconnects, late replies and uncertainty clear
derived target/model state.

The one-access LDS view retains the exact allocation/storage-slot/generation,
full invocation, operation producer and same-stop target binding. It applies
the existing independent bank-arithmetic model with an explicitly assumed
allocation-base residue. It does not coerce ResourceV2 into legacy generation0
or infer a native transaction from source-site equality. A first page remains
16 returned rows / 64 scans with no automatic continuation.
The model requires a complete access ≤256 bytes, ≤65 dwords and ≤64 banks.
Unsupported scopes/producers, stale joins or missing rows produce no grid.

These caps are protocol/profile limits, not RSS guarantees. A logical CPU wave
width is not the bundle's target identity. Bank footprints are arithmetic
under stated assumptions, not conflict/multicast, coalescing, timing or
performance measurements.

## Fresh CLI and HTTP evidence

All paths are under
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr` on mi350.
The selected freshly built debugger was frozen as
`target-milestones-phase28-debug-tools-r2/fe2o3-debug`:
61,138,376 bytes, SHA-256
`41c5075e3de0404f7e18ca1ef0da8c7441adb2527c3a1eba4d45d126b36b84b9`.
The earlier unflushed binary and failed attempts remain historical failures.

The CLI qualifier passed **70 real requests across four sessions**:
retained ordinary-source loop and workgroup bundles, raw input, and disabled
observations. It checked independently retained inspector identities,
owner/full-cursor substitution, repeated reads without execution mutation,
reverse/replay revisions, explicit unavailable and stale-revision behavior.
`phase28-declared-target-cli-r3/receipt.json`: 16,431 bytes, SHA-256
`d3b80a9b801f2f7b2d59fa380f4a1c954eb4d1946eaf4e6a00a9d9764374d82c`.

The separately run real loopback bridge retained request/reply bodies and
exact owned child cleanup, with all 14 current bridge modules pinned:

| Actual profile | Requests | Receipt bytes | Receipt SHA-256 |
| --- | ---: | ---: | --- |
| Loop | 81 | 88451 | `9574f7bc7df8544ca22933b0ab316c3c75457423ea321ba1d084d403f1e9ed1f` |
| Workgroup | 87 | 108495 | `bcc18fd7181a13a90f89129d8533193746eb89eeb69a7415fe8e93c29f7c34a7` |

Receipts are under
`logs/phase28-declared-target-http-{loop,workgroup}-r1/receipt.json`.
Historical bundle admissions were revalidated under their own selected binary
pins; they are not mislabeled as having used this newly built debugger.
No fresh source export is claimed by these HTTP tests.

The workgroup's first access page contains a real same-stop witness:
AFTER cursor event13/revision3, allocation/slot/generation 2/2/1,
activation1/attempt6, lane0/WG0, committed 4-byte write at offset0/event12,
actual operation site (0,5,2), target gfx942:xnack- from the same V5 bundle.
The page scanned13 rows, returned1 and had no token. The loop's selected first
page has no corresponding LDS access; no witness is invented for it.

The initial combined HTTP gate passed the loop but timed out waiting for closing
port state before running the workgroup. It was not a target-query failure.
The separate workgroup rerun used the existing bounded 90-second quiescence
check and passed; it did not overwrite the successful loop receipt.

## Unit and browser evidence boundary

The bridge suite passed 81 tests; the HTTP qualifier's 16 pure controls passed.
The complete affected Rust suites and Clippy completed in the
`compiler-release-target-final-r1` gate (receipt SHA-256
`a098120dccd8ca8d0862b5736f405af44c5976c375e29d6cc0969f58847d9153`).
Existing unrelated warnings remain.

The full companion site gate passed 1,382 unit tests, lint/types/build,
21 tutorial-lab tests, evidence validation and 180 desktop/mobile browser
tests with zero retries. Its target transport tests are explicitly synthetic
correlation/error controls; actual CLI/HTTP readback above is separate evidence.
It is not yet a claim that those HTTP sessions were operated through a real
browser. A separate actual-browser qualification will be recorded independently.

The [declared-target/bank tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/declared-target-bank-lab-v1.md)
documents operation, exact clearing rules, unavailable fields and assumptions.
The [same-export origin/native tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/same-export-origin-native-v1.md)
is a different read-only compiler/artifact observation route.

V4 remains open: this CPU one-access adapter does not supply supported physical
same-stop hardware cells or qualified native transaction/conflict grouping.
