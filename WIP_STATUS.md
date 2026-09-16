# Issue 271 Integration Snapshot

Reviewable WIP, not default production activation, tutorial qualification or
milestone completion. Do not merge this historical tree over current main.
Select and qualify changes against current main separately.

## Exact Source

- Host: `XSJHARMENON01`.
- Checkout: `/home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913`.
- Checkout HEAD: `10b190b8267c0c4011b4ef6bbb52680a3c44b391`.
- Source: 5069 files, manifest SHA256
  `26b2f1526ab1d918aae514db0dde341ef6f1b0c33c2cc70883538907cc1cc6ea`.
- Previous WIP: `551648ad418eff780f7d23155c02f12aad21bacc`.

This status file is outside the source manifest. Snapshotting preserves the
checkout HEAD/index and verifies each source Git blob.

## New Work

The admitted generic DisjointSlice getter/Option checkpoint now covers both
gfx942/gfx950 and two Store constants. It checks N/B/checked-O shape, source
occurrences and exact own-Store retention. The semantic factory is not actual
rustc provenance. Unique-predecessor continuations use dominating SSA values;
the test no longer incorrectly requires invented phi/block-argument tuples.

`FE2O3_EXTRACT_COLLECTED_SHAPE_V1=1` now selects a bounded, authority-free
diagnostic in the extraction tool. It uses real rustc collection and the existing
source import, SSA, occurrence-capture, materialization and target scopes. It
prints original references, source/types/ownership, SSA and actual N/B/O graphs.
An exact function-qualified Store query avoids collisions between function-local
coordinates. Output is capped at four MiB and marked incomplete until all scope
postflights succeed. Conflicting output modes and malformed selectors refuse;
dependency/probe passthrough and default production behavior are unchanged.

## Exact-Source Checks

- All 839 backend library tests passed, without failures or ignores.
- All 18 extractor binary tests passed, without failures or ignores.
- All three explicitly enabled actual-collector integrations passed in 557.90s:
  positive gfx942/gfx950 shapes, changed reference effect digest, and preserved
  unsafe/ABI admission refusals. These used six fresh private target builds.
- Source/helper guards passed. Every owned integration scratch directory was
  absent afterward. Pinned rustfmt checks passed for all five observer paths.
- A separate fresh gfx942 diagnostic completed in 1m34s with source, tool and
  shared-library guards passing and its private target removed. Its saved log
  SHA256 is `60c8fc5b5059dbbe5e6d6d8b3b376af277727719c77a07ee4a7db0abfadfddbe`.
  The exact own Store remains retained after six neutral blocks become four
  optimized blocks. The log is diagnostic text, not authenticated evidence.

The initial standalone transcript wrapper failed before collection because its
clean environment lacked the backend library search path. Correcting that DIAG
wrapper required no compiler or test relaxation; its failed run remains recorded.

These are scoped compiler checks, not whole-workspace/all-feature, source-proof,
LLVM, GPU or corpus qualification. The new marker-error unit does not substitute
for a live callback Err/panic test through rustc. Older lowerer/doc/native test
results belong to their previously recorded snapshots, not a fresh run here.

## Current Boundaries

The generic checked control/effect relation for getter, Some and owned Store is
still being implemented separately; it is not in this snapshot. Physical merges,
functional-address composition, original reference/aggregate obligations and
fresh Complete formal-memory requirements remain enforced. Diagnostic completion
explicitly says source proof was not run and artifact/launch authority is false.

Both public main refs were last read at `7f90d187702a3059a40c7cfd21576c6eec6c9684`.
Main includes peer compiler-context and lockfile changes absent from this WIP.
The eleven-file CPU tooling batch is being independently qualified on current
main; older CPU observations do not qualify the rebased compiler tree. The
separate #272 WIP is unchanged by this snapshot.

M5/M7 remain active and all M0-M9 acceptance boxes remain open. Default activation,
complete source/output proof composition, legacy retirement, full tutorial and
simulator coverage, target-matched hardware runs and release/site pins remain
unfinished. Approved Verus runtime qualification is deferred, not passed. No
kernel-name dispatch, verification bypass or runtime qualification exception is
introduced by this snapshot.
