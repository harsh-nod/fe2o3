# Issue 271 WIP Snapshot

Component-tested integration for review, not a completed production replacement
or release qualification. Issue #271 remains open.

## Source Identity

- Host: `XSJHARMENON01`.
- Checkout: `/home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913`.
- Source HEAD: `10b190b8267c0c4011b4ef6bbb52680a3c44b391`, plus working-tree changes.
- Git-visible source files: 5,055, excluding this status file.
- Source manifest SHA-256: `a25a768844581e5e56e480778bbe1ded99ed874a2fe088213b37f1cef09a4b6b`.
- Branch: `wip/issue271-canonical-mixed-ssa-20260915`, on both remotes.
- Previous snapshot: `0d11366088f627706b617cd0a9ac774b5aa62ac3`.

The source HEAD, index and bytes were preserved when creating this snapshot.
This advances WIP history, not its main base. Newer main includes independent
changes absent here. Do not merge this entire old-base snapshot into main.

## Tested Scope

Locked/offline tests used nightly-2026-04-03, HIP/HSA disabled, one build job,
and source/helper stability guards. The seven-package library run passed
**2,654 tests, zero failures, zero ignored**: GPU 46, AMD model 36, analysis 151,
optimizer 26, lowerer 374, Pliron 1,260, backend 761.

That run's manifest was
`18566a2c8251860aba412c0527c525f2016e4eb4b40ae62cabef4c3613c09bc3`.
After it, only two expectations in the new AMD allocation integration test were
corrected to the existing F16/Bf16 storage and Workgroup diagnostic contracts.
On the final snapshot, all **10 allocation integration tests** and all **51
documentation tests** for the lowerer, Pliron and optimizer passed. No production
or library-test code changed between these runs. LLVM 18 assembler checks are
syntax checks, not production LLVM 22 object or GPU qualification.

This is not full repository CI, complete tutorial qualification, hardware
execution, or proof of universal compiler correctness.

## Changes Since Previous Snapshot

- Exact source-to-optimized-output physical-address correspondence for the
  supported Global leaf grammar, with independent address-use checks.
- Whole-output formal/native join tests, including a later incomplete root
  which correctly prevents a Complete-only callback.
- Native lowering of positive direct typed constant-count private scalar
  allocations, restoring the real private-array source-to-output tests.
- Additional policy-3 ownership/lifetime and receipt-boundary regressions.

Address correspondence is not a bounds, alias, race, or functional refinement
proof. Counted allocation lowering does not establish scratch resource or
lifetime safety. Unsupported counts and address forms still fail closed.

Both public mains were independently confirmed at
`8af54567c4c143473447d58a27be3d1b384b67e6`, containing checked policy-3 execution
and its boundary tests. The allocation repair is undergoing separate main-base
qualification. Policy 3 is still not the default production policy.

## Remaining Work

M5 optimized-graph verification and M7 production integration remain critical.
Prepared but not included here: exact U64-literal affine extraction, borrowed
source/ranked authentication, and untrusted policy-3 semantic replay. The
functional/full-projection address join is still under design review.

Genuine functional/reference custody, final protected consumers and default
pipeline activation remain unfinished. Broader scalar/interprocedural, loop,
memory and GPU optimizer work and M8/M9 tutorial/scale/release qualification
remain governed by the complete issue. No milestone is closed by this snapshot.
Approved Verus runtime qualification remains deferred, not passed.

## Coordination

- Roadmap: https://github.com/harsh-nod/fe2o3/issues/271
- Canonical WIP: https://github.com/harsh-nod/fe2o3/tree/wip/issue271-canonical-mixed-ssa-20260915
- Mirror: https://github.com/powderluv/fe2o3/tree/wip/issue271-canonical-mixed-ssa-20260915
- Separate #272 WIP: https://github.com/harsh-nod/fe2o3/tree/wip/issue272-semantic-mir-v15-v16-20260915

#272 remains separate at `a9559337915285ceed881bb2bef59437c5112eda`.
Its source checkout was not changed or qualified by this work.
