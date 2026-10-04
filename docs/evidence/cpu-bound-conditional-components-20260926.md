# CPU-Bound Conditional Proof Components

Later work is recorded separately in
[CPU-bound conditional integration](cpu-bound-conditional-integration-20260926.md).
The snapshot and results below remain historical.

Compiler candidate: `f9ad51b2d0d84a93ca4186899c6ddccc7b9c6fda`.
This follows [shared CPU replay](portable-cpu-replay-20260925.md).
The bounded CPU codec, shared source correspondence checker, V2 formula API
and invocation V2 codec pass their local checks. At this checkpoint, the live
backend still uses formula/invocation V1. V2 production wiring and its outer
descriptor consumers are separate pending changes, not covered by these results.
All [#272](https://github.com/harsh-nod/fe2o3/issues/272) M0-M7 milestones remain
open. No additional tutorial kernel, safe launch or machine proof is qualified.

## Implemented Boundaries

`portable_reference_v1::codec` transports the complete existing kernel/reference
identity tuples, logical signature, acyclic CPU effect IR, observable outputs,
semantic source hash/root and registration/logical names. Its canonical frame is
bounded by 4 MiB, depth 128 and 262,144 aggregate nodes. The full frame, rather
than the older incomplete effect digest, supplies the CPU-input commitment.
Both live output lists must agree. Scoped encode/decode callbacks preserve the
original account, incoming storage floor and denial history, including refusal
and unwind. Owned scratch drops before its reservation is released. The single
fallible boxed-node allocation conversion has an explicit unsafe-policy entry.

`conditional_reference_v1` owns the shared CPU/source conversion, read mapping
and subject checks formerly in the backend. The compiler adapter uses that same
implementation and retains source authentication through its original binding
rederivation. Decoded CPU content is not an authenticated Rust binding, and the
codec does not introduce another semantic interpreter. Conditional loops and
helper summaries remain outside this codec's admitted domain.

The version-specific formula API encodes and checks the same borrowed CPU input,
checks its actual source/root association, then prepares one V2 statement. Its
distinct domain adds exactly the whole-frame CPU commitment to the previous six
ordered commitments. Strict reimport uses an externally accepted key, toolchain
and boundary, not a policy selected by receipt bytes. Retained proofs are
move-only; callback borrows cannot escape. V1 cannot be relabeled as V2 or used
to construct ordinary formal-memory/native authority.

Invocation V2 carries that seventh field and checks the new statement preimage,
while reusing V1's row/shape validation. V1 bytes, identities, errors and public
APIs remain separate. A coherent content hash is still not a signature or source
authentication. The numerical model remains shared IEEE; this is not proof of
LLVM/ISA arithmetic or of an advanced kernel's transcendental error bound.

## Local Validation

Every successful guard below used the unchanged 8,262-file source inventory
SHA256 `2de3b8e73a3d57461672865a21e84cbeeed0df240ce0b85489c63d75dc7e2208`.
This documentation was added afterward. Pinned nightly `2026-04-03`, locked
offline dependencies, one Cargo job/test thread, hidden GPUs, disabled HIP,
a 12-GiB virtual-memory limit and a 1,200-second deadline were used.
Guard implementation SHA256:
`20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.

| Guard | Result | Log SHA256 |
| --- | --- | --- |
| `cpu-bound-v2-formula-tests-r3` | 44 passed, 1 ignored | `7fbaa38bc6bedc197bf3fd9c70cb5b3d08aeaf9af67b0b32d551bb6b221636ba` |
| `cpu-bound-v2-reference-tests-r1` | 59 passed | `ba4a422d525567e87281e6d9fa26e622fb1f4ec6019efe33c9d312c148b2b7f2` |
| `cpu-bound-v2-formula-docs-r1` | 12 compile-fail passed | `25c2510b982527598c95ac6f0dcce775d39237f113c75d70d04671b86a290f2c` |
| `cpu-bound-v2-reference-docs-r1` | 4 compile-fail passed | `053f9c70787914dfea4efde91c252aa2836b4311b1ee9311799a7ec1324b0076` |
| `cpu-bound-v2-backend-join-tests-r2` | 14 passed | `204a14007d7ef4a7ae85c51377ed21fee0b9fb58a42319c851728cdf55d9e112` |
| `cpu-bound-v2-backend-reference-tests-r1` | 87 passed | `0dbaac14f2cb9ba5fa58378d26a2b1c6d8d2b0d2c5b004aa77d27055897eb64c` |
| `cpu-bound-v2-prefix-tests-r1` | 9 passed, 1 ignored | `349bd044056157e6705d7a6a075aab89d7ed84314e5616f6dcd82eead38d2409` |
| `cpu-bound-v2-descriptor-tests-r2` | 95 unit/integration and 8 compile-fail passed | `b70c1035835c96dc68db3348cd6c4cda6566441bb94c64f245abf8f54d8b7008` |
| `cpu-bound-v2-artifact-contract-tests-r1` | 7 passed | `f78fc368b0205d197fa6b45f53761eb4899a439dd548979d9865e2932623e461` |
| `cpu-bound-v2-normal-check-r1` | library check passed | `ed0d6337eaabc16ab54c9c205c66bfbd3e63a743e7c9df2b9a72278d45878db2` |
| `cpu-bound-v2-unsafe-policy-r1` | 5 passed, baseline refresh ignored | `99b16e0b8ecd33cfdf2ead04af41e9e8156d41531b921a701d5b6e6dd31cd91f` |

Coverage includes canonical/hostile CPU frames, complete identity fields,
V1/V2 separation, exact/one-short budgets, original-account cleanup, strict
unsigned/foreign-key refusal, adapter parity and ordinary reference regressions.
Ignored development/protected tests are not passes. The genuine source-bound
composition hook is compiled but has not been executed in a protected runtime.
Warnings remain; this is not a complete workspace or warning-free build claim.
Dependency policy and hygiene checks pass. The unsafe baseline was not refreshed.

Two test defects were fixed before the final formula pass: a nested callback
captured a foreign ledger with an invalid lifetime; another test incorrectly
expected an incoherent CPU occurrence to reach the commitment callback. It now
requires the exact early codec refusal and original-account preservation.
Neither fix relaxes production validation. Failed logs respectively:
`8dec170ff9821d5b0177c77e22884417620b68b5a3ebdec039d35b0bff61aff6`,
`73da3181492d23373196c42aed1f7bc4cfe77e16ade111a935163985451e176d`.

The first backend-join guard was interrupted while its bounded Cargo child
continued. That original job was observed to completion before a fresh stable
guard was run; the interrupted guard is not credited. An earlier broad pipeline
run on a different candidate was deliberately stopped during an expensive
resource matrix after 178 completed passes and 3 ignored tests. Its remainder
was unrun/in progress, so it is not a full-suite pass.

## Protected Retry And Remaining Work

The second frozen-r11 MI350 run tested older candidate
`a66be6440b418871ea39c17c012e50372aeccbe7`, not this candidate or V2. Runtime audit,
real proof execution, false-proof rejection and the original actual-source
Policy6 parent passed. The main matrix completed gfx942/F with one agreement,
replay and installation before the expected finalizer refusal, and gfx942/fixed6
with none. Its zero-work case correctly refused, but the test classifier missed
the nested canonical-encode work-limit variant and failed. The remaining main
cases and all late cases were not run. The corrected classifier accepts that
exact typed work error, not allocation, accounting or string lookalikes.

The archived report remains `tests_passed: false`. Result archive SHA256:
`959bb6226db4db890bcb3e7958d45c676dc0f2396e711f52343823c04e8e4fcc`;
inside-report SHA256:
`7e9b551441dd79c985ef645a4bca0f7207e98d0747b7efbdf2309f9861dc1b1b`.
The service drained. Its run, service/slice, incoming upload and bootstrap
directories were removed after archive verification. Shared runtimes were not
changed. A fresh protected capture and complete main/late matrices are required.

Next work is live V2 execution/projection, genuine mutation/reimport tests and
strict versioned descriptor consumers; then the paired bounded native packet
producer/independent consumer with externally accepted policy and retained
source-through-final-F checks. One V2 proof is required, not additive V1/V2
executions. Native finalization, machine refinement, durable publication,
generated host admission, safe GPU launch and the complete target-matched
tutorial matrix remain unfinished. `ConditionalFinalizerRequired` still stops
the production conditional route before native output.
