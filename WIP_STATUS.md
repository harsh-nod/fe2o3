# Issue 271 WIP Snapshot

This branch shares the unpublished canonical mixed-SSA integration for review
and coordination. It is not merge-ready, a release, or completion of issue #271.
No public main branch is changed by publishing this snapshot.

## Source Identity

- Source host: `XSJHARMENON01`.
- Source checkout: `/home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913`.
- Source HEAD: `10b190b8267c0c4011b4ef6bbb52680a3c44b391`, plus its dirty working tree.
- Captured Git-visible source: 5,011 files.
- Source manifest SHA-256: `236e16c2d5ff5cfc3f63c04ec658951b96943b965a1365f6a6130f914ecc4a68`.
- Branch: `wip/issue271-canonical-mixed-ssa-20260915` on both compiler remotes.

The source manifest precedes this status file. Its hash binds the recorded
source root, HEAD, sorted paths and file bytes or symlink targets; it is not a
Git tree ID or a complete toolchain/runtime identity. The commit tree is the
fetchable source snapshot. The live checkout, its index and HEAD are preserved.

This branch starts from the integration checkout's own base, not current main.
It contains changes that have already landed independently and other unfinished
work. Main advanced to `2585ce64fa101f21d8b22c626955a5f3c75fa76e` during the latest
validation. Its new integer/launch correspondence changes are not yet reconciled
into this integration. Do not merge or stack the entire snapshot blindly.

## Tested Scope

On this exact 5,011-file source snapshot, the following command passed with
nightly-2026-04-03, locked/offline dependencies, HIP/HSA disabled, one build job
and unchanged source/helper inputs:

```sh
cargo +nightly-2026-04-03 test --locked --offline --no-fail-fast \
  -p fe2o3-mir-model -p fe2o3-amdgcn-model \
  -p fe2o3-lower-mir-kernel -p rustc-codegen-fe2o3
```

Result: 1,836 passed, zero failed, 94 explicitly ignored, across 79 suites.
The observed cross-crate build scratch directory was independently confirmed
absent after the test. Lowerer all-target Clippy with `-D warnings` also passed.
These are compiler/component results, not all-feature repository CI, all-48
tutorial qualification, GPU execution, formal compiler verification or release
approval. Ignored tests remain unexecuted.

## Implemented And Remaining

The snapshot contains canonical graph/correspondence infrastructure, bounded
ordinary helper result transport, checked-output analysis work, and the reviewed
enum downcast and simultaneous SSA edge-fact repairs. The current critical path
is M5 exact optimized-graph verification and M7 production integration.

Retained ordinary helper-call checked-output binding has a reviewed follow-up
proposal but is not included here. Dominance-aware CSE and its versioned fixed
policy integration are in development. SROA, expanded scalar/interprocedural,
loop, memory and target optimization scope remains governed by the full issue;
an additive library API does not complete a production milestone.

All tutorial compilation/reference/simulator/hardware gates, final optimized
program admission and publication/documentation work must still be completed.
Approved final Verus qualification is deferred, not passed. KFD is not a
substitute for that proof runtime. Keep issues #271 and #277 open.

## Coordination

- Roadmap: https://github.com/harsh-nod/fe2o3/issues/271
- Canonical branch: https://github.com/harsh-nod/fe2o3/tree/wip/issue271-canonical-mixed-ssa-20260915
- Mirror branch: https://github.com/powderluv/fe2o3/tree/wip/issue271-canonical-mixed-ssa-20260915
- Separate capability/schema snapshot: https://github.com/harsh-nod/fe2o3/tree/wip/issue272-semantic-mir-v15-v16-20260915

The #272 snapshot has a separate base and owner. Its semantic-MIR V15/V16
changes must not be confused with canonical KIR versions or silently combined
with this branch. Coordinate shared-schema allocation and source custody before
integrating either snapshot. Reviewers should work in their own worktrees and
link findings or bounded follow-up commits in the owning issue.
