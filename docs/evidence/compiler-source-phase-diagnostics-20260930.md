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
boundaries and both positive emission fixtures remain required. The following
follow-up records the first repair tests; it does not complete this gate.

## Index-Witness Repair Follow-up

The independent repair was integrated locally at `85416a5c0`, followed by
compile corrections at `e43a4e0c2`. A finalized, exact shared loan can preserve
an existing producer origin; other invalidations remain. Non-reference index
carrier recipes come from the original capability resolver, not a type-only
lookup. Borrow transport compares the current original SSA definition and
archived witness metadata. The reader validates the complete original reference
holder and returns its scalar payload, never an owned witness reconstructed from
ordinary bits. Existing account, archive and loan checks remain in force.

| Run | Scope | Result |
| --- | --- | --- |
| r116 | First combined repair build | Compile failed: two missing type imports and an invalid test-only SSA constructor |
| r117 | Corrected build and 345 focused tests | 333 passed, 12 failed |
| r118 | One positive fixture with a backtrace | Failed in the final-emission test observer, not at the old borrow-archive check |

r116 ended with status101 and stable source/tool inventories. Its source
snapshot was
`118edeec00899dabab6e4760c1b372abf0dcf669655f9da534b1e1e27d70908a`;
log SHA-256:
`9b5b692c8876e869a92e8097394a9e0cef51b047a147fdda616bf099c5b3d897`.
The correction imports the actual model types and uses an original SSA occurrence
in the unclaimed-use control rather than inventing a definition.

r117 built in 6m50s and ran its tests in 16.13 seconds. The existing 322 focused
guards remained passing. The new reader substitution test passed all nine fault
cases in both same-block and continuation forms: wrong loan, scalar substitution,
payload identity/type, owner, source, disjointness, index space and missing loan
claim. Each case requires one reached reader and its exact diagnostic, with
the original account restored. This is actual constructed semantic-MIR emission
coverage, not genuine rustc capture, proof or GPU execution.

The twelve failures do not receive acceptance credit:

- Six origin controls failed fixture admission with `InvalidTypeLayout`, before
  their intended query or resource boundary.
- The two emission positives and exact-budget positive reached a test-observer
  panic. r118 traced it to collecting only statement definitions, omitting the
  original call-edge definitions of the producer and reader result.
- The dead-parent fixture failed admission because a callable was outside the
  reachable root closure, not at the intended liveness check.
- The equal-shaped alternate-loan test reached zero readers, so its intended
  substitution boundary was not tested successfully.
- The unwind control reached the reader, but expected an escaping panic where
  the existing production boundary instead returns its callback-panic refusal.

The observer correction must inspect exact original edge definitions, including
source block, successor ordinal, SSA identity, reachability and promotion. It
must retain the original producer, holder, loan, scalar-output and consuming-store
checks. Fixture corrections and both genuine disjoint-index reader positives
still require compilation and execution; none is inferred from static review.

r117/r118 stable source snapshot:
`f67e89f723374f504021587d19a94ac72a8960c6cfd7899cf9924f9aa700f54b`.
r117 log SHA-256:
`959afb3cfe59f58af2db4711b9b7246470f5966d2e296bf0592c9961008d3fa5`.
r118 log SHA-256:
`2aaec89650e56481548babce82b0d100768d86ec2bdb141982a841c8711dbfbd`.

M1 remains incomplete. No finalizer, protected proof, simulator or safe hardware
launch success follows from these tests. The source repair is not on public main.

## Actual Rust After Repair

r119 ran the same 80-session matrix on local candidate `a5e69bed7`, including
the witness repair and finer assignment/operand diagnostics. It completed with
status101, stable source/tool inventories, a 6m47s backend build and a
319.56-second matrix. All80 still failed before their required consumer:

- All40 MIR0 sessions now identify
  `execution destination definition differs from source SSA`, specifically at
  `bind_local_definition_v29`'s cursor `define` call. The individual cursor
  predicate still needs diagnosis.
- All40 MIR2 sessions passed the previous borrow-archive failure and now stop at
  `pending global native changed exact access recipe`. This is a later
  source/output/native global-memory correspondence check, not completed memory
  safety, finalization or authority.

Positive MIR0 coordinates, identical within each pair of fresh sessions:

| Target | Backend Optimization | Function/Block/Statement |
| --- | --- | --- |
| gfx942 | 0 | 1/0/1 |
| gfx942 | 3 | 1/0/1 |
| gfx950 | 0 | 1/2/1 |
| gfx950 | 3 | 0/2/1 |

MIR2's combined recipe diagnostic does not yet identify which access, guard,
root, address or result comparison differs. None of the four negative modes
reached its named consumer boundary, so no negative acceptance is credited.
The Rust fixture covers logical context issuance/ABI; its indexing still uses
the existing thread API. Context-derived invocation remains a separate M1 item.

r119 source snapshot:
`95391269757d6dae79f8e46d044b9a63ebc7adf3f212ddb60c714293e3bfa908`.
Log SHA-256:
`323b91c1dbead47bfad26700fed0a78a110717b597f3a5db6cc6116a43d19c78`.
Pinned LLVM opt SHA-256 remains the r115 value above. No proof executor or GPU
was invoked. The source repair remains an unmerged candidate, not a public
production capability claim.

## Original-Witness Emission Result

r120 compiled candidate `4c20e28eb` and ran 347 focused tests: **338 passed,
9 failed**. It completed with status101 after an 8m19s build and 17.73 seconds of
tests. Source/tool inventories remained unchanged throughout the run.

All four original-producer positives now pass: ThreadIndex and DisjointIndex
shared-borrow readers, each in same-block and continuation forms. Their final
observers retain the original call-edge definitions, loan/payload correspondence,
scalar observation, and consuming guarded store. The dead-parent control now
reaches its intended live-loan storage-death rejection. The nine reader
substitution cases in both control-flow forms remain passing.

The nine remaining failures are not accepted negatives:

- Six origin controls now pass type-layout admission but stop at
  `source enum payload differs from its original guarded value`, before the
  intended authenticated query.
- The exact-budget positive completes both admission and construction replay;
  its assertion incorrectly expected a single reader invocation. Separate
  phase counts are required, not an unbounded count check.
- The unwind control reaches its injected reader panic, but expects the inner
  reference callback diagnostic instead of the actual containing function-frame
  boundary's `source reference availability construction or callback panicked`.
- The alternate equal-shaped loan fixture still stops before the reader, at
  `typed allocation identity or representation requires its exact source contract`.

r120 source snapshot:
`926f14f16b2790f12abcc8f8ee2e1190689e8979af02d4b630a3222dc576a907`.
Log SHA-256:
`81673ed8632550dbffcd78960f8434d78530b473096d6bc4ed1c64b64f1c5adf`.
These are constructed original semantic-MIR emission results. They do not
establish a passing genuine Rust matrix, proof, simulator or hardware launch.

## Runtime Observation

A primary-session read-only SSH check reached MI350-2 on 2026-09-30 and confirmed
the pinned native image still exists. All four production host paths were absent:
the V3 client profile, policy-v2, compiler-runtime-manifest-v1 and compiler-runtime-v1
directory. A worker-only DNS failure was not treated as host unavailability.
These reads did not start containers, install files or change the shared host.
The earlier r111 container and scratch had already been cleaned.

The remaining runtime gap is also implementation work, not only provisioning.
The public native compiler gate in
`crates/cargo-fe2o3/src/compiler_execution_boundary_native.rs` still refuses with
`RuntimeEnforcementUnavailable`: it requires enforcement retained from before
exec through completion. Root intake currently has refusal-only semantics, and
the native attempt deliberately cannot resume the compiler. Existing installed
inventory, staging, trace, proof-helper backing and retirement components do not
yet compose into an approved compiler execution with continuous source/code/output
binding. Packaging success is not that execution guarantee. M1 remains incomplete.
