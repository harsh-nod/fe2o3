# Production total-output refinement V2

`require_total_output_refinement_v2` is the non-vacuous composition gate after
the mandatory eight PLIRON passes. It is not another graph analysis pass. It
accepts only a live `ProductionRankedKernelLoweringInputV1` and its matching
move-only `ProductionMiddleEndEvidenceV5`.

Admission requires all of the following:

- the V5 ranked-kernel identity and canonical evidence identity match;
- every mandatory PLIRON report is clean;
- at least one `TotalView` contract is declared and proved;
- every observable global write participates in the effect-refinement
  bijection and every effect contract is proved against the safe CPU reference;
- at least one authenticated functional-refinement receipt remains retained;
- typed semantic roots are nonempty and every arithmetic domain is discharged;
- retained typed recipes reconcile exactly with the live PLIRON roots; and
- collective contribution coverage has an independent proved finite
  collective-value contract.

The resulting theorem is deliberately narrow: every coordinate in each proved
finite output view has one modeled final write, and that write's typed value is
equal to the bound safe-reference MIR effect under the admitted contract.
Boolean and fixed-width integer values use interpreted bitvector semantics.
Floating-point roots provide typed IEEE operator congruence only.

The report grants no source-to-MIR proof, target IEEE value proof,
MIR-to-PLIRON or PLIRON-to-LLVM refinement, machine-code proof, artifact
authority, load/launch authority, hardware-execution authority, or universal
kernel-correctness claim. Those boundaries are explicit false accessors.
