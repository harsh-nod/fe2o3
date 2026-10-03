# Authenticated Fill Artifact Association

Parent: `ea159c955d2c9e4544d7e5481b1d2acd95986bd6`.
This checkpoint advances ordinary multi-GPU application admission. It does not
grant launch authority, prove compiler refinement or claim a new GPU run.

## Implementation

`check_gfx942_fill_analysis_v1` returns a move-only borrowed association between
an authenticated analyzer execution and the exact inspected 68-byte gfx942 fill
model. It derives the model from the retained request payload, not another file
or a caller digest. It checks the singleton selected entry/function, descriptor
bytes, exact effects, three-block control flow, 14 instruction boundaries,
branches, memory widths and relevant flags. Existing strict decoding validates
trace encodings and structural consistency. This is not an independent proof
of LLVM MC decoding or ISA interpretation. LLVM's Barrier flag at END denotes
control-flow termination; it is not rejected as a synchronization barrier.

The genuine Worker emits a 272-byte kernarg allocation: 16 explicit bytes plus
256 implicit AMDHSA bytes. Inspection now closes over exact COV6 16/272 layouts,
8-byte alignment and pointer/length metadata. Both single-wave and full-dispatch
checks use the full descriptor-derived storage extent for overflow and overlap.
The kernel still reads exactly 16 bytes. Hidden contents, actual backing and
initialization are not authenticated by this model. The shared dispatch theorem
has the same full-storage acceptance predicate; wave semantics are unchanged.

## Genuine Worker Qualification

The new ignored finalizer test consumes the unchanged genuine `fill.handoff`,
imports all six conditional receipts, replays target lowering and checks the
complete fill program. It runs the actual protected Worker bootstrap/replay,
checks exact LLVM/object/LLD derivation, finalizes its output, then analyzes those
exact finalized bytes using the same measured private Worker copy. It verifies
the source/compiler associations, exact final LLVM identity and model owner.
It does not create test-signed compiler carriage or assert protected compiler
origin. Worker pinning here uses the freshly measured test executable, not a
deployment-approved compiler-origin policy.

The full handoff SHA256 is
`778098953929152bc6ffe7a1251052a04db63fa12c9a449694bb4078747d2b1a`
(32616 bytes). Actual finalized HSACO SHA256 is
`8b6c2e5b67ba2f76bb57d5f42aedbaf968f8dc12bb121daa5d7853cc55d158c6`
(6160 bytes). `finalizer-4` passes 1/1. A separate `qualified-finalized`
authenticated analyzer test passes 1/1 using this HSACO, verifies the deployed
no-fork profile and evaluates modeled output/guard bytes for N=0/64/65/4097.
These are native worker executions, not GPU kernel executions.

The current C++ Worker was built on MI300X with ROCm 7.2.4 LLVM/LLD, bounded to
two build jobs. Its machine-effect CTest passes 1/1. Exact executable hashes,
build claim, source archive, CMake cache and test logs are retained. All remote
work used one owned scratch directory; no GPU queues or allocations were created.

## Regression And Proof

| Campaign | Result |
| --- | --- |
| Kernel-analysis library | 70 passed, 2 default ignores |
| Ownership doctests | 3 passed |
| Physical machine-effect integration | 25 passed |
| Finalizer integration | 23 passed, 3 default ignores |
| Full dispatch proof | 44 cumulative obligations, 13 mutants, 12 controls |
| Wave regression proof | 30 obligations, 16 mutants, 14 controls |

Both proof campaigns accept, with unchanged before/after source snapshots,
pinned verifier closure, strict expected logical failures and owned-process
cleanup. The 44 obligations include the 30 wave obligations. Fresh development
diagnostics established exact primary/secondary spans after the proof extension;
the classifier was not relaxed. Two new mutants cover unsupported storage extent
and accepting output inside the implicit tail. The 18 dispatch stages all pass.

CPU tests cover nine canonically decodable trace/effect substitutions, changed
payload/challenge/instruction bytes, hidden-tail overlap, adjacency, high-address
overflow, empty output and seven structured metadata mutations. Captured request
and bundle fixtures are inert replay data; they cannot construct authenticated
execution owners. Strict library/native-analysis Clippy, targeted rustfmt and
whitespace checks pass. The separate ignored native tests were explicitly run;
the other default ignores are not claimed as tested.

Development failures remain distinct: an initial standalone diagnostic ELF
lacked required DF_SYMBOLIC; production-policy relinking fixed that input.
Another run exposed END's Barrier flag. Genuine Worker runs then exposed the
272-byte layout. `finalizer-2` never executed because its binary copy was still
in progress; the final runs waited for copy completion. Only `finalizer-4`,
`qualified-finalized`, `qualified-dispatch` and `qualified-wave` qualify the final
genuine-artifact path and current proof extension. Earlier `qualified-1` concerns
the standalone 16-byte profile, not the complete handoff path.

## Remaining Multi-GPU Gate

The host still needs a retained source/compiler/finalizer/machine association
inside its current-publication audit, a distinct protected conditional artifact
and a consuming invocation transition. Coverage must match actual preparation,
selected device, patched storage and current publication. Native entry values,
scheduling, visibility and completion remain required associations. Passing this
test is not protected compiler-origin attestation or a semantic-to-machine proof.

First hardware acceptance is admitted fill on each selected GPU, completed
output -> staging -> settled upload -> PUBLIC XGMI -> guarded readback in both
directions. Peer-input compute consumption needs a later read-input kernel.

## Reproduction

The qualification archive contains the code-only patch, scripts, CPU logs,
both accepted proof campaigns, strict source snapshots, agent review and native
records. Use fresh owned output directories when rerunning. Native tests require
the explicit executable/build identity environment recorded by `run-finalizer.sh`
and `run-native.sh`; they do not select devices.

Code-only patch SHA256:
`6890b8ad3af0c23161843cf72d2424752b1b50c6715b3cb9e795da65ece3c60b`.
The [qualification archive](qualification.tar.xz) has SHA256
`b775d1ca0caef390e7b67b9fc8f3799058fe2e89728144f817aba7a386c4a52c`.
Its complete 777-entry listing and extracted patch digest were checked before
owned scratch cleanup. No other shared-host paths were removed.
