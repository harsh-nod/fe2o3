# Original Source Emission Checkpoint

This is #272 M1 investigation and regression coverage, not completed production
qualification. M0 is complete; M1-M7 remain incomplete; strict production-to-safe
GPU launch coverage remains **0/47**. Existing tutorial execution is a separate
claim. This source candidate has not been merged into public main.

## Candidate

Local source candidate `b7a9db2f5` contains diagnostic commit `0704252f1` and two
new original-index emission regressions, atop the prior candidate `10add0374`.
One mapper now attaches retained source coordinates and a fixed failure phase
to the two otherwise context-free execution-availability/CFG errors. It preserves
precise errors and resource failures, performs no new SSA queries or claims, and
does not change admission or accounting. An independent review checked the frame,
stack and borrow mappings, including idempotence when the actual function is zero.

The new positive fixtures retain the existing ThreadIndex1d producer and guarded
store. They insert a shared borrow, ThreadIndexGet and holder StorageDead before
moving the original witness into the existing store path. One fixture includes
an extra continuation edge. Original occurrences are captured from the final
source, never transplanted from another owner. Completion assertions join source
definitions, uses and kills with the emitted producer, original loan and reader
payload. These are constructed semantic-MIR/SSA emission fixtures, not genuine
rustc-capture receipts or CPU/GPU equivalence proofs.

## Tests

All local runs use the same pinned, locked/offline, one-job guarded runner as
[the runtime packaging checkpoint](compiler-runtime-packaging-20260930.md).
The worktree is unchanged throughout each run and all GPU visibility is disabled.

| Run | Scope | Result |
| --- | --- | --- |
| r112 | Source-reference, frame and stack controls | 319 passed, 3 failed |
| r113 | Corrected controls; new regression code also compiled | 322 passed, 0 failed |
| r114 | Two positive original-index borrow emission regressions | Both failed at the promoted borrow archive check |
| r115 | Actual Rust Policy10 context/nominal matrix | All 80 sessions failed before the required consumer; every failure now has source coordinates |

Two r112 failures were old context-free diagnostic assertions. They now require
function2, block0, the exact original statement and Assign/Store stage, while
retaining injection-reach and no-completion checks. The third was a stale
independent frame-size mirror: production already included `direct_call_inputs`,
but its test mirror omitted that 24-byte field. Only the mirror was corrected;
the size-derived production budget was not increased or bypassed.

r112 stable source snapshot:
`2ea7031a2393041d0e007c18d66810b3d547d777d85649547d4c2f96493e1fdb`.
Log SHA-256:
`8a322e5bcbeeccdcfa2c7978c98d918fdfbd0a58d9feec1a2ca6d47ba397c2fb`.
r113 and r114 stable source snapshot:
`84f3a56f76e0f629fc18beee785216d1462f3fb6b5525c26fa9d6682051472ef`.
r113 log SHA-256:
`7092744e78de58c6bfd545c9f6ca946bbf0a0111e3d488b9fc0abc77166e5817`.
r114 log SHA-256:
`940bb41eacd0cd03d836cc2daa4706aba27a028919812513d4fe18813fe9a24f`.

Both r114 fixtures fail at function0/block1/statement0 with
`source reference promoted borrow archive differs from source SSA`. They do not
reach the completion observer. They remain failing positive tests: there is no
`should_panic`, ignored-test annotation or reinterpretation as a successful
negative test. This reproduces the phase identified by the earlier actual-source
MIR2 runs, but does not yet identify which archive predicate failed.

## Actual Rust Matrix

r115 rebuilt the backend at `b7a9db2f50729d24ec15e648d601a914e1a8c38b` and ran
gfx942/gfx950, backend optimization 0/3, MIR optimization 0/2, five test modes and
two fresh sessions per combination. All80 failed before the required consumer:

- All40 MIR0 sessions now identify `execution availability during Assign`,
  with actual function/block/statement coordinates. This localizes the failure
  to statement lowering, but not yet to an individual assignment predicate.
- All40 MIR2 sessions still identify the promoted borrow archive check, before
  referent resolution and payload flattening.

The positive-case coordinates, identical within each pair of fresh sessions:

| Target | Backend Optimization | MIR0 Function/Block/Statement | MIR2 Function/Block/Statement |
| --- | --- | --- | --- |
| gfx942 | 0 | 1/1/1 | 0/7/0 |
| gfx942 | 3 | 1/2/1 | 0/2/1 |
| gfx950 | 0 | 1/1/1 | 0/7/0 |
| gfx950 | 3 | 0/2/1 | 1/3/1 |

Function coordinates alone do not identify a kernel versus helper. No ordering
relative to borrow lowering is inferred for MIR0. None of the negative modes
reached its named rejection boundary, so these are not successful negative tests.

The run terminated with status101 after a 7m05s backend build and a 271.21-second
matrix. Source/tool inventories were stable; its source snapshot equals r113's.
The pinned LLVM opt SHA-256 was
`13cb4c99d1810b4db40bca5db0759ca94c8efd3c437bb8f7fe1a94f5bda66203`.
r115 log SHA-256:
`8667bd2ee9f94a12d22971e331640b958dd710e3a77f3966520f44d04593aeb9`.
The private actual-source scratch directories were verified absent after exit.
The independent semantic repair was not included in this run.

## Repair Boundary

The isolated repair worktree has the same baseline tree as `b7a9db2f5`. Separate
owners are handling source-origin/carrier integration and loan-bound borrow/read
lowering, with independent review. No live #271 owner work is copied or changed.
A checked shared borrow may preserve its referent's existing producer origin;
it cannot erase a distinct mutable-borrow, AddressOf or unknown-alias invalidation.
A source-derived carrier recipe does not replace exact original-use, SSA archive,
loan, lifetime, index-metadata or resource-account checks. Reading a witness must
not manufacture owned index authority from an ordinary scalar.

The full actual-source matrix, reached mutation controls, finite resource
boundaries and both positive emission fixtures remain required. No semantic
repair, finalizer, proof, simulator or hardware success is claimed here.

## Runtime Observation

A primary-session read-only SSH check reached MI350-2 on 2026-09-30 and confirmed
the pinned native image still exists. All four production host paths were absent:
the V3 client profile, policy-v2, compiler-runtime-manifest-v1 and compiler-runtime-v1
directory. A worker-only DNS failure was not treated as host unavailability.
These reads did not start containers, install files or change the shared host.
The earlier r111 container and scratch had already been cleaned.
