# Formal Compiler V1

Status: bounded, compositional verification milestone. fe2o3 is not a
formally verified compiler as a whole.

This document is the claim boundary for the first production-connected
compiler proofs. It deliberately excludes LLVM and everything after the
canonical target-neutral/target-binding boundary. A local theorem may be used
only for the exact input relation checked by its production validator. Proof
records and evidence objects grant no compilation, publication, load, or
launch authority.

## Implemented Proof Ledger

| Boundary | Closed V1 fragment | Mechanically checked result | Production connection |
|:--|:--|:--|:--|
| Rust source/HIR to semantic MIR | Direct `u32` add, subtract, multiply, and/or/xor expressions over direct parameter bindings; arithmetic requires rustc overflow checks to be disabled | Equal selected-element result and ordered abstract effects under exact operator, type, operand-value, and destination relations | Same-session rustc HIR, raw MIR, source provenance, local identities, and admitted semantic MIR are independently compared; unsupported statements receive no certificate and zero records is valid |
| Semantic MIR to canonical KIR | Single-block, straight-line, constant-rooted chains of `u32` wrapping add, subtract, multiply, and/or/xor | Equal selected-element result and ordered abstract effects under exact ordered-operand, operator, and destination-to-result relations | The live production owner is fully replayed; constants and previously certified locals must map to the exact effect-free KIR SSA values, and each span contains only its prescribed constants and binary operation |
| Ranked affine bounds | One affine output over an unconstrained, nonempty integer box of rank at most 16 | Checker acceptance implies `0 <= f(x) < extent` for every point in the exact box | The ranked bounds pass emits the checked endpoint certificate and the independent verifier reruns the canonical checker |
| Target capability binding | Processor capability plus Wave64 requirement added to otherwise identical canonical KIR | Body, ABI, effect set, and admitted trace are unchanged | Production KIR-to-target replay invokes an independent exact-module validator and retains the checked result |
| Generated ABI to runtime preparation | Exact COV6 `vecadd(&[f32], &[f32], DisjointSlice<f32>)` preparation profile | Kernel and generated-contract identities, six ABI components, geometry limits, allocation-relative effects, and ownership are preserved into prepared state | Worker V3 projects authenticated descriptor, generated-host, layout, effect, physical-kernel, geometry, and alias-admission state into the executable checker |

The first three rows describe target-neutral compiler reasoning. Target
binding proves only that adding the selected AMD capability facts does not
change the already admitted program semantics. Runtime preparation ends before
publication or execution.

## Composition Rule

These rows do not compose merely because their names are adjacent. An eventual
composition requires exact shared identities and the checked relation at each
handoff:

```text
same rustc session and source identity
  -> exact admitted semantic-MIR identity
  -> exact canonical KIR identity and correspondence
  -> exact target-binding relation
  -> exact generated ABI/effect/geometry identities
  -> prepared, still-unlaunched runtime state
```

Evidence is authority-free and is retained with the production owner so a
later verifier can recheck those joins. An unsupported operation, absent
certificate, identity mismatch, arithmetic overflow, malformed certificate,
or hostile substitution cannot inherit a theorem from a neighboring row.

V1 does not yet implement that end-to-end composition. In particular, the
source-to-MIR slice accepts direct parameter operands while the MIR-to-KIR
slice accepts constant-rooted chains, so those two accepted languages do not
currently overlap. The target-binding and vecadd runtime-preparation proofs
also remain separately keyed claims rather than one source-to-preparation
theorem.

## Explicitly Unproved

Formal Compiler V1 does not establish:

- whole-Rust or whole-MIR semantic preservation;
- general control-flow, calls, pointers, aggregates, floats, atomics,
  barriers, tensor operations, or MFMA refinement;
- parameter-rooted MIR-to-KIR values or a general proof that semantic-MIR
  locals and KIR SSA values denote equal dynamic values outside the closed
  constant-rooted scalar relation;
- soundness of every Presburger, sparse-index, layout, initialization,
  convergence, or race analysis;
- general ABI lowering or runtime behavior outside the exact COV6 vecadd
  preparation profile;
- KIR-to-LLVM, LLVM optimization, code generation, linking, object/HSACO, ISA,
  driver, firmware, or GPU semantic refinement; or
- correctness of rustc, the Rust implementation relative to each Verus model,
  Verus, vstd, Z3, or the host execution environment.

The LLVM and machine boundary is intentionally out of scope for this
milestone. Therefore neither documentation nor diagnostics may shorten this
ledger to "fe2o3 is formally verified" or "this GPU binary is formally
verified."

## Reproduction

The compiler and runtime proof sets use two different pinned Verus closures.
Run the complete checked set with:

```sh
VERUS_COMPILER=/path/to/verus-0.2026.08.02.b677dd5 \
VERUS_RUNTIME=/path/to/verus-0.2026.08.09.92f466f \
  scripts/ci-local.sh verus
```

Every dedicated runner pins its Verus executable and proof sources, rejects
`assume`, `admit`, and external-body shortcuts in the new proof files, and
requires hostile mutations to fail verification. The superseding
`formal-compiler-v3.yml` workflow downloads both releases and runs this lane on
changes to the relevant proof, compiler, analysis, verifier, host, and runtime
paths. This V1 ledger remains the historical statement of the earlier proof
boundary.

## Next Closure Work

The next useful proofs are semantic rather than administrative:
parameter-rooted MIR-to-KIR correspondence so the first two rows overlap,
general CFG simulation, byte-range memory and race-analysis soundness, and
composition of the per-boundary evidence into one independently checked
source-to-preparation claim. LLVM and machine refinement remain a separate
project.
