# Issue 272 Semantic-MIR V15/V16 WIP Snapshot

This branch shares the named unpublished capability/schema integration at the
user's explicit request. It is not a release, an approved schema dependency or
completion of issue #272. No public main branch is changed by this snapshot.

## Source Identity

- Source host: `XSJHARMENON01`.
- Source checkout: `/home/harsh/work/fe2o3-issue272-capabilities-20260905`.
- Source branch: `codex/issue272-gpu-capabilities`.
- Source HEAD: `f292cbb4b4591b731f9ddc9931b53977eae54e1c`, plus its dirty working tree.
- Captured Git-visible source: 6,508 files.
- Source manifest SHA-256: `8454c72a8b44e1964f6298fbf5cb62068b9af55fdccd93f59cbe2bd97df504e7`.
- Shared branch: `wip/issue272-semantic-mir-v15-v16-20260915` on both compiler remotes.

The source manifest precedes this status file. Its hash binds the recorded
source root, HEAD, sorted paths and file bytes or symlink targets; it is not a
Git tree ID or the full compiler/runtime input closure. The commit tree is the
fetchable source snapshot. The live checkout, its index and HEAD are preserved.

This exact checkout contains `INERT_SEMANTIC_MIR_VERSION_V15` and
`INERT_SEMANTIC_MIR_VERSION_V16` in
`crates/fe2o3-mir-model/src/semantic_mir_v1.rs`. These are semantic-MIR schema
versions, not KIR V13/V14 or optimizer-policy versions. Their presence in a WIP
branch does not establish shared-owner approval, compatibility or production
qualification. Other #272 worktrees and later runtime work are not implicitly
included in this named snapshot.

## Qualification Boundary

The publishing #271 thread did not run compiler, simulator, hardware or proof
qualification on this exact #272 snapshot. Historical reports from its owner
are not inherited test results for this commit. This snapshot may contain
unfinished or failing work. It is shared so agents can inspect and coordinate,
not so they can bypass the existing owner, proof, artifact or launch gates.

No source files, build outputs or processes in the live integration were
modified to publish it. Changes were captured through an isolated Git index
and checked against the source manifest. Build caches, ignored scratch and
external diagnostics are not part of the snapshot.

## Coordination

- Capability roadmap: https://github.com/harsh-nod/fe2o3/issues/272
- Shared middle-end roadmap: https://github.com/harsh-nod/fe2o3/issues/271
- Canonical branch: https://github.com/harsh-nod/fe2o3/tree/wip/issue272-semantic-mir-v15-v16-20260915
- Mirror branch: https://github.com/powderluv/fe2o3/tree/wip/issue272-semantic-mir-v15-v16-20260915
- Separate #271 snapshot: https://github.com/harsh-nod/fe2o3/tree/wip/issue271-canonical-mixed-ssa-20260915

This branch starts from its own historical integration base and is not rebased
on current public main. Do not merge it wholesale or treat the separate #271
snapshot as a compatible parent. Shared schema/version allocation, capability
contracts, actual optimized-graph verification, numerical and memory-history
proofs, runtime admission, final artifact/host integration and tutorial coverage
still require explicit coordination and qualification by their owning stages.

Reviewers should create independent worktrees, preserve the named source
identity in reports, and post findings or bounded follow-up commits in #272.
Keep #272 open; the WIP branch grants no release or launch authority.
