# Issue 271 WIP Snapshot

Component-tested integration for review and coordination. This is not a complete
production replacement, release qualification, or completion of issue #271.

## Source Identity

- Source host: `XSJHARMENON01`.
- Source checkout: `/home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913`.
- Source HEAD: `10b190b8267c0c4011b4ef6bbb52680a3c44b391`, plus its dirty working tree.
- Git-visible source files: 5,047, excluding this status file.
- Source manifest SHA-256: `76735dce3b5535734403c54f51a3ee0c22c11eeca15def8bc3b1580b1c084414`.
- Branch: `wip/issue271-canonical-mixed-ssa-20260915` on both compiler remotes.
- Previous WIP snapshot: `e9a1b5c9910e68ccd1eb45222b557d4efd4946c2`.

The manifest binds source root, HEAD, paths and file bytes or symlink targets;
it is not a complete toolchain/runtime identity. Snapshot creation preserves
the source checkout HEAD, index and bytes. This fast-forward refresh preserves
the previous WIP history but does not rebase the integration onto current main.
Already-published changes overlap this snapshot, and newer main changes still
require reconciliation. Do not merge the entire old-base WIP blindly.

## Exact Tested Scope

On this exact snapshot, with nightly-2026-04-03, locked/offline dependencies,
HIP/HSA disabled, one build job and unchanged source/helper inputs:

```sh
cargo +nightly-2026-04-03 test --locked --no-fail-fast --lib \
  -p dialect-gpu -p fe2o3-amdgcn-model -p fe2o3-kernel-analysis \
  -p fe2o3-kernel-opt -p fe2o3-lower-mir-kernel -p fe2o3-pliron \
  -p rustc-codegen-fe2o3
```

**2,624 passed, zero failed, zero ignored.** Package counts are 46, 36, 151, 25,
362, 1,260 and 744 respectively. This is a library integration result, not
all-target/all-feature repository CI, tutorial qualification or GPU execution.

## Repairs Since The Previous Snapshot

All fifteen backend failures documented in the preceding snapshot are resolved
in this run, with thirteen additional regression tests:

- Canonical-only projection retains exact source bounds assertions and removes
  only their redundant ordinary access-side guard expansion. Source SSA,
  index/extent/view identity, unique success predecessor, dominance and actual
  output control checks remain mandatory. Legacy projection is unchanged.
- Partial canonical control explicitly represents ordinary-call continuation
  as conditional on Return. It does not claim the callee terminates; defined
  callee, normal destination and no-cleanup restrictions remain enforced.
- The private direct-view analysis wrapper restores storage accounting after
  analysis data drops during unwinding, preserving the original panic payload,
  prior failure history and subsequent reuse of the same view/ledger.
- Hostile index tests now mutate the actual U64 index association instead of
  accidentally selecting an unrelated U32 argument. The helper-first test now
  resolves actual output entry/callee identities rather than assuming source
  function order survives canonical materialization.

The snapshot includes scoped Store/control analysis, exact source-rooted literal
associations, retained ordinary-call checks and a same-output Complete-only
formal callback. It also contains policy-3 execution/transition infrastructure,
dominance CSE, integer identity checks and native V12 lowering. Independent
pieces have landed or are being separately qualified on current main; this WIP
is not their publication authority.

## Remaining Work

M5 exact optimized-graph verification and M7 production integration remain the
critical path. Physical-address correspondence, whole-output formal/native
lowering integration, authenticated functional/reference joins and final
protected consumers are unfinished. None may be inferred from a successful
scoped Store/control or Complete-only formal callback.

M3's broader scalar/interprocedural policy, M4 loops/memory, M6 target optimizer,
M8 tutorial qualification and M9 scale/release gates remain governed by the full
issue. The default production route has not been replaced by this snapshot.
Approved final Verus runtime qualification is deferred, not passed. No complete
compiler verification, all-kernel simulator result or hardware claim is made.

## Coordination

- Roadmap: https://github.com/harsh-nod/fe2o3/issues/271
- Canonical branch: https://github.com/harsh-nod/fe2o3/tree/wip/issue271-canonical-mixed-ssa-20260915
- Mirror branch: https://github.com/powderluv/fe2o3/tree/wip/issue271-canonical-mixed-ssa-20260915
- Separate #272: https://github.com/harsh-nod/fe2o3/tree/wip/issue272-semantic-mir-v15-v16-20260915

The #272 snapshot remains separate at `a9559337915285ceed881bb2bef59437c5112eda`;
its source checkout was not changed. Semantic-MIR V15/V16 are not canonical KIR
versions. Use independent worktrees and coordinate bounded changes in the issue.
