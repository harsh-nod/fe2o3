# Formal Compiler V2

Status: three bounded, production-connected compiler claims are mechanically
proved. fe2o3 is not a formally verified compiler as a whole.

This ledger excludes LLVM and every later machine boundary. It closes one
exact instance of each of the first three V1 proof gaps: front/middle-end
composition, control-flow and direct-call simulation, and soundness of a
path-sensitive affine-bounds analysis. No result in this ledger grants
compilation, publication, load, or launch authority.

## Closed V2 Claims

### Source to semantic MIR to canonical KIR

For one effect-free direct `u32` binary expression over two copied,
unprojected function parameters, fe2o3 proves equal ordered operand values and
equal results across source semantics, admitted semantic MIR, and canonical
KIR. The closed operator set is wrapping add, subtract, multiply, bitwise and,
bitwise or, and bitwise xor.

The Verus model uses explicit source and MIR environments, an exact
source-local-to-MIR-local relation, an exact MIR-local-to-KIR-SSA relation,
and an independent KIR valuation. It proves the corresponding destination
updates with functional map insertion. The production validator independently
replays the live owner and binds the exact rustc function and statement span,
parameter locals and SSA values, operator, ordered operands, result, `u32`
type, and absence of effects.

A syntactically eligible source operation is `Proved` only when the complete
nonempty roster is present and revalidates. Missing coverage is `Incomplete`;
an ineligible operation is `NotApplicable`. Empty evidence is never a proof of
an eligible operation.

### Semantic MIR to canonical KIR control flow

For one direct `u32` call to an internal helper with the exact four-block
diamond `if x == 0 { x } else { C }`, fe2o3 proves a six-transition forward
simulation from semantic MIR to canonical KIR. The model gives MIR and KIR
separate program counters, environments, machine states, helper parameters,
selected-arm values, join transfer, helper return, and caller call-result
cells.

The proved observation boundary is the caller continuation immediately after
the call result is bound. Both executions terminate there with the same
selected arm, result, and abstract call/branch/join/return trace. The root
continuation, its ranked store, memory effects, and all later operations are
outside this theorem. Fuel below the exact transition count proves neither
execution result.

The production validator derives an authority-free `NotEligible` or
`Verified` status from the same rustc compilation owner, checks the exact call
destination and helper/callee relation, replays both MIR and KIR shapes and SSA
transfers, and retains the status across target-neutral pipeline custody. The
production extraction fixture makes the otherwise scalar root rank-eligible
with a separate `DisjointSlice<u32>` store; that store is deliberately not
part of the semantic claim.

### Path-sensitive constrained affine bounds

For every statically shaped affine access dimension whose admission depends
on one unambiguous affine branch fact, fe2o3 proves
`0 <= f(x) < extent` over one exact, nonempty constrained integer box. The
certificate binds the access site, branch site, ordered SSA comparison,
successor polarity, normalized guard inequality, affine map, box, extent,
satisfying witness, and bounded nonnegative multiplier vectors.

The Verus proof establishes the nonnegative linear-combination theorem, a
nonempty-domain witness property, the true-edge inequality normalization, and
the graph lemma that removing the selected edge makes the access unreachable.
The production checker reconstructs the affine expressions and CFG from the
ranked recipe, verifies that exact edge condition and cut property, and
requires exactly one certificate for every eligible site. Missing, extra,
duplicate, ambiguous, or substituted records fail closed. The legacy
`RankedBoundsReportV1::is_clean()` result is not proof authority.

## Composition Boundary

The three theorems are independently production-connected, but they do not
form a whole-program theorem. Their claims may be used only when the exact
live identities and eligibility predicates accepted by the corresponding
validator are present:

```text
same rustc session and exact source/MIR identities
  -> exact source/MIR/KIR scalar-expression correspondence
  -> exact MIR/KIR helper-call CFG correspondence, when eligible
  -> exact ranked access, affine fact, and CFG-edge correspondence, when eligible
```

Each proof status remains private, authority-free compiler custody. Unsupported
input continues to compile only according to the ordinary production gates;
it cannot inherit a theorem from an adjacent proved fragment.

The statuses are revalidated immediately before extraction or publication,
then consumed at the compiler-module handoff boundary. They are not serialized
into the LLVM, compiler-module, artifact, runtime, or worker handoff formats,
all of which are outside this ledger.

## Trusted Computing Base

The V2 claims still trust:

- rustc HIR and raw-MIR extraction, monomorphization, source provenance, and
  eligibility enumeration;
- the Rust validator-to-Verus correspondence, including opcode, local, SSA,
  block, integer, affine-expression, and CFG semantics;
- live-owner replay, canonical KIR construction and verification, proof-model
  and proof-source identity hashing, and private evidence custody;
- the ranked-recipe-to-PLIRON correspondence and the interpretation of its
  true CFG edge; and
- the pinned Verus, vstd, Z3, Rust, and host execution closures.

The proof runners reject `assume`, `admit`, and external-body shortcuts in the
new proof sources. Hostile proof and live-program mutations are required to be
rejected, but tests do not remove the trusted correspondence above.

## Explicitly Unproved

Formal Compiler V2 does not establish:

- whole-Rust, whole-MIR, or whole-KIR semantic preservation;
- general expression trees, projections, integers other than the closed
  `u32` fragments, floats, aggregates, pointers, or side effects;
- general CFGs, loops, recursion, indirect calls, unwind behavior, or caller
  continuation and memory semantics for the bounded helper theorem;
- soundness of every Presburger, sparse-index, layout, initialization,
  convergence, race, alias, or disjointness analysis;
- dynamic extents, block-argument affine replay, multi-origin path facts,
  equalities, congruences, or remainder certificates in the constrained
  affine proof;
- general ABI lowering or runtime behavior; or
- KIR-to-LLVM, LLVM optimization, code generation, linking, HSACO, ISA,
  driver, firmware, or GPU semantic refinement.

Therefore the accurate statement is: fe2o3 has mechanically verified,
production-connected semantic-preservation and analysis-soundness theorems for
three explicitly bounded target-neutral fragments. It is not yet a formally
verified compiler.

## Reproduction

The compiler and runtime proof sets use two pinned Verus closures:

```sh
VERUS_COMPILER=/path/to/verus-0.2026.08.02.b677dd5 \
VERUS_RUNTIME=/path/to/verus-0.2026.08.09.92f466f \
  scripts/ci-local.sh verus
```

The `Formal Compiler V2` workflow authenticates those release archives before
running the complete local proof lane. The V1 ledger remains available as the
historical statement of the earlier noncompositional boundary.
