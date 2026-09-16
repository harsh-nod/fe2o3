# Issue 271 WIP Snapshot

Review and coordination snapshot, not merge-ready, release-qualified, or completion
of issue #271. This fast-forward refresh changes neither public main nor #272.

## Source Identity

- Source host: `XSJHARMENON01`.
- Source checkout: `/home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913`.
- Source HEAD: `10b190b8267c0c4011b4ef6bbb52680a3c44b391`, plus its dirty working tree.
- Git-visible source files: 5,047, excluding this status file.
- Source manifest SHA-256: `d469d73a6b873243f7f47dcf5a4cf669e8765c8ddfd702b507d898acb7df49d6`.
- Branch: `wip/issue271-canonical-mixed-ssa-20260915` on both compiler remotes.
- Previous snapshot: `1cd6971aceecd8cd00e35e9a802ac6eb9ddc7fb7`.

The manifest binds source root, HEAD, paths and file bytes or symlink targets;
it is not a complete toolchain/runtime identity. This branch preserves the old
WIP history. It does not rebase the integration onto current main. Both mains
were independently read at `179340e30877b5691c1e7643bc64230f8a617429` before
refresh. Some changes here already landed independently; newer main changes
still require reconciliation. Do not merge the full snapshot blindly.

## Tested Scope

On this exact source snapshot, nightly-2026-04-03 with locked/offline dependencies,
HIP/HSA disabled, one build job, and source/helper before/after guards passed:

```sh
cargo +nightly-2026-04-03 test --locked --lib -p fe2o3-lower-mir-kernel
cargo +nightly-2026-04-03 test --locked --doc \
  -p fe2o3-lower-mir-kernel -p fe2o3-kernel-opt -p fe2o3-pliron
```

- Lowerer library: 362 passed, zero failed or ignored.
- Documentation: 42 passed, zero failed or ignored, including compile-fail
  lifetime and invalid-admission cases.

The immediately preceding seven-package library run had 2,595 passed, 16 failed,
zero ignored. One failure was a resource-test assertion corrected in this
snapshot and covered by the complete lowerer rerun. **Fifteen backend failures
remain unresolved here.** No fully green integrated suite is claimed.

## New Integration And Open Failures

The snapshot now includes scoped canonical Store-value/control analysis, exact
source-rooted literal guard associations, retained ordinary-call checks, and a
Complete-only formal-memory callback borrowing the same actual optimized output.
Dominance-aware CSE, exact integer identity checks and native V12 lowering are
also present; those independent primitives already landed on both mains.

The fifteen failures fall into three shared integration gaps:

1. Ordinary guarded slice projection splits source blocks into generated guard,
   access and continuation blocks; the new exact control checker intentionally
   refuses this unsupported partition. The proposed repair retains the source
   assertion and normalizes only exactly authorized ordinary accesses.
2. Literal capture expects the predicate at the source segment tail, while the
   projector currently relocates it to a generated access segment. The same
   source-assertion retention repair is expected to address this without relaxing
   actual N/O Compare or source SSA checks.
3. The older partial-control frame refuses ordinary calls before the new exact
   retained-Call analysis runs. A recorder-only conditional-return state is being
   implemented; legacy grammar and no-return/cleanup refusals remain explicit.

Those repairs are **not included**. Physical-address correspondence and a scoped
whole-output formal/native-lowering continuation are also unfinished. This branch
does not replace the default production pipeline or discharge missing physical
address, functional, reference, convergence or final-publication obligations.

## Remaining Milestones

The critical path remains M5 exact optimized-graph verification and M7 production
integration. M3's broader scalar/interprocedural policy, M4 loops/memory, M6 target
optimization, M8 tutorial qualification and M9 scale/release gates remain open.
Library APIs and independently tested batches do not complete those milestones.

Approved final Verus runtime qualification is deferred, not passed. This snapshot
claims no all-feature repository CI, complete tutorial compilation/simulation,
GPU execution, universal compiler verification, or release approval. KFD is not
a substitute for proof-runtime admission.

## Coordination

- Roadmap: https://github.com/harsh-nod/fe2o3/issues/271
- Canonical branch: https://github.com/harsh-nod/fe2o3/tree/wip/issue271-canonical-mixed-ssa-20260915
- Mirror branch: https://github.com/powderluv/fe2o3/tree/wip/issue271-canonical-mixed-ssa-20260915
- Separate #272: https://github.com/harsh-nod/fe2o3/tree/wip/issue272-semantic-mir-v15-v16-20260915

The #272 snapshot remains separate at `a9559337915285ceed881bb2bef59437c5112eda`;
its foreign source checkout is untouched. Semantic-MIR V15/V16 versions are not
canonical KIR versions. Use independent worktrees and coordinate shared schema,
source custody and bounded follow-up commits in the owning issue.
