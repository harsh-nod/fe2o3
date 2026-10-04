# Current-source fixed-prefix local-order continuation — 2026-09-23

This earlier qualification is superseded for U3 acceptance by the
[saved-recipe and public-example report](source-local-order-recipes-20260923.md).
The results below retain their original scope and counts.

This is a bounded implementation and qualification increment for issue #282 U3.
The accepted original exits remain **M1/V1/V2/U1/U2 (5/18)**. It is not yet a
saved-recipe author workflow, protected artifact, native object or hardware run.

## What changed

The existing checked local-order service now has a move-only owned tail.
It retains the actual input/output pair and independently replays the existing
permutation and full-module transition relation. A digest or inert receipt
cannot reconstruct this owner.

A source-owned continuation consumes the unchanged fixed Policy6 prefix once.
It retains original semantic/KIR N, bound B, intermediate source-origin rows and
actual prefix output I. A request names three current N coordinates; those roles
are transported through the retained transitions and matched to the contiguous
XOR/OR/AND diamond in I. No raw N SSA identifier is assumed stable in I.

The new compiler-private composition is named
`source-local-order-policy6-v1/gfx942`. It gates the actual authenticated
gfx942:xnack-/wave64 target and RawEmpty helper-source policy, consumes the same
prefix, and emits LLVM/descriptors from actual checked L. Fresh native,
private-memory, division, helper, trap, formal and workgroup observations are
derived from the current owners. Legacy Policy6/Policy8 and the protected
output-owner enum are unchanged. A protected publication request explicitly
refuses; no fallback emits unscheduled I.

The supported source is still one direct nongeneric root with immutable distinct
u32 parameters and the initializer `(a ^ b) & (c | d)`, required/maximum
workgroup 64x1x1. One canonical assertion-trap declaration may accompany the
single semantic/kernel root; arbitrary helper declarations, duplicate traps,
changed signatures or hidden bodies are refused. This is not whole-kernel
assembly, arbitrary pass selection or a machine-order guarantee.

The actual-source qualifier captures current HIR/Instance identity, performs the
ordinary source and ranked checks, joins the original N to current source,
continues through I to L, independently replays the continuation and rechecks
retained source currentness. It observes real L LLVM/descriptors and simulates
the entire resulting kernel. This qualifier remains explicitly test-only; the
underlying typed continuation is release-active but compiler-private.

## Exact observed results

The fresh `phase28-source-local-order-actual-r2/observation.json` is 125,701
bytes, SHA-256
`25bb27ab4899de8282cea92c138e2dcb68ffd77ca5a44442e939879e121bd063`.

There are 13 actual compiler callbacks:

- Five successes: original SourceOrder, original ReverseReady, its fresh
  repeat, and both orders after renaming formals and adding a comment.
- Eight exact refusals: ambiguous selection, local alias, changed operator,
  duplicate input, wrong target, wrong launch, BOM normalization and source
  changed after retention.

Each success has five independent input vectors, lengths 0/1/65 and two
whole-kernel CPU replays: **150 simulations total**. The independent
`(a ^ b) & (c | d)` oracle checks output, initializedness and canaries.
The original and repeated ReverseReady observations match. No finite sample
is presented as a universal proof.

For the original source:

| Stage | Canonical identity |
| --- | --- |
| Original N | `ae8b93a3cc316d4b5649692a174c859e434aa93140230328be87f25ae2ec4324` |
| Fixed prefix I / SourceOrder L | `d8dd002b0e3f9ca35e17c8e665310a06d8dd2a3e467103227a0df287e712efea` |
| ReverseReady L | `504b57dd66948d26fbc8f7309be3f401c527f5e88a07b8399122e301824f77b2` |

SourceOrder's selected result sequence is [14,15,16]; its LLVM is 11,517 bytes,
SHA-256 `45adb50a5077d9fa1295ebdbe15f069f1568194f5a0b0b20953ec4cc7adc561f`.
ReverseReady's sequence is [15,14,16]; its LLVM is 11,515 bytes, SHA-256
`536a81d80566c9a3767bd755725dd816b853ef55cef1e98a5d2d9fa635dcb1c7`.
The renamed source produces freshly joined, different current identities.

The largest observed continuation ledger uses 91,587,662 work units and
67,794,139 bytes of logical peak storage. Each positive oracle records 30 runs,
31,380 steps and 85,866 prepaid host-payload bytes. These are logical budgets,
not process RSS or performance SLOs.

## Gates and retained evidence

All paths below are under the task root on mi350; these are test observations,
not signed release attestations.

| Gate | Result | Receipt SHA-256 |
| --- | --- | --- |
| `compiler-source-local-order-composed-r1` | 8 source-owner tests; 127 lowerer doctests; 10 focused backend passes, 6 ignored | `4b1a2aa5f2341f80f6f29c863988f5cf99fb6a5c31eb6ff03fbcbd515ebd4233` |
| `compiler-source-local-order-actual-r2` | 13 callbacks, 150 simulations, exact LLVM readback | `a3dce3b422006472bcb0107e8935c5f078cb83df3f01a83ae95e0b5e728af6ae` |
| `compiler-source-local-order-regressions-r1` | optimizer 335/3 ignored; lowerer 1,478/0; backend 1,734/115 | `2172146ff67564792bacdf4e5c995eef3325a44fa5fd155d66884a134b1032d6` |

The common pre-documentation source census is 6,722 files / 101,720,018 bytes,
SHA-256 `8dfaaca9cf5967dc1325799ef857c74de68e7784e00058cf719db06edf79940b`.
A later equivalent nested-if simplification fixes a new Clippy warning.
Existing unrelated warnings remain; strict warning-clean workspace status is
not claimed. Publication also requires format/hygiene/DCO and regression checks
on the final peer-preserving tree.

Earlier failed attempts are retained: an opaque-ledger equality assertion needed
a boolean comparison; the direct-root census initially rejected the legitimate
canonical assertion-trap declaration; the first actual ladder lacked the
absolute pinned RUSTC environment variable. None was accepted as qualification.
The successful rerun used a fresh output path and did not weaken the source
fixture or expected refusal classes.

## Reproduce

Use the pinned nightly toolchain/provider closure and the repository's ordinary
offline build settings. Set RUSTC to that toolchain's absolute rustc path, not
only PATH, and choose a fresh absolute output directory:

```sh
cargo test --offline --locked -p fe2o3-kernel-opt --lib
cargo test --offline --locked -p fe2o3-lower-mir-kernel --lib
cargo test --offline --locked -p rustc-codegen-fe2o3 --lib

FE2O3_TEST_SOURCE_LOCAL_ORDER_CONTINUATION_OUTPUT=/absolute/new/continuation-run \
  cargo test --offline --locked -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::local_order::continuation::actual_source_local_order_continuation_ladder \
  -- --exact --ignored --nocapture
```

The actual ladder is ignored by default and separately selected. Keep divergent
worktrees' dependency artifacts distinct and preserve bounded work/storage,
subprocess, source-copy and output limits.

## Remaining U3 work

A versioned inert recipe still needs a normal author-facing Create/Replay path,
two retained legal schedules, exact/advisory canonical constraints, deterministic
replay, real edit/refusal/rebind/regeneration cases and fresh current records.
Historical recipe annotations cannot import owners or substitute for current
source checks. A codec or a test-only callback alone does not complete U3.
See [the earlier feasibility experiment](../source-local-order-feasibility-v1.md)
and [the existing local-order model](../checked-u32-local-order-v1.md) for their
separately dated evidence.
