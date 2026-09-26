# Strict original-meter induction analysis — 2026-09-26

This is a model qualification checkpoint on top of
`210914b8d509dec5446262d1b29959de0ed30811`. It does not complete root recipe
construction, normal helper admission, ranked attachment or LLVM continuation.
Broad accepted exits remain **M1/V1/V2/U1/U2/U3 (6/18)**.

## Implementation

`analyze_semantic_u32_induction_no_overflow_with_meter_v1` exposes the
existing complete-CFG V1 induction algorithm through the caller's original
resource meter. It never substitutes a reachable-only SSA scope. The legacy
semantic predicates, certificate ceiling, local work counter and refusal
ordering are preserved when resources suffice.

The strict adapter prepays generic error/report frames, graph and inventory
storage, vector growth/copying, initialization, traversal and report construction.
Allocations use fallible reservation; unexpected capacity refuses instead of
charging only after allocation. Resource denial is terminal. Accepted storage
remains charged until the caller drops all partial values and results; this
model API does not refund, reset, replace the ledger or substitute empty facts.
Existing unmetered and bound-snapshot entry points retain their prior behavior.

Thirteen new control groups cover legacy report/counter parity, semantic
refusals, exact and one-short work/storage, every meter-denial position,
sticky denial/unwind, 8,192-byte generic error frames, arithmetic/allocation
failures and report shrinking. A narrowly documented enum-local Clippy
allowance preserves the existing inline semantic call payload: boxing it would
introduce a new allocation and require separate ownership/accounting review.
No enum field or representation was changed by that annotation.

## Qualification

The final offline/locked mi350 gate passed:

- MIR lowerer: **1,861 tests**.
- MIR model: **331 tests**.
- Backend: **2,446 tests**, with **189 ignored**.
- Strict model Clippy (`-D warnings`) and diff checks.
- Selected source, inputs and tools unchanged across the gate.

Receipt: 45,122 bytes,
`ada450258f9bc9fda739842772cf44c8520a18faebd6cbe1ea9fda891c05b14e`.
Qualified source: 8,379 files, 119,765,189 bytes,
`90e1fdc5f5c3b94e4682eccc2fe34375af6853c6af6299d67c1a7147376f0e2a`.

Failed predecessors remain retained. R1 exposed an invalid new test fixture:
an argument was relabeled temporary without changing the ABI. The correction
uses a valid zero-step fixture and still checks the legacy no-certificate result.
R2 passed all three test suites but strict Clippy stopped on the existing large
inline terminator variant. The enum-local annotation resolves that lint without
boxing or disabling strict linting globally.

## Remaining boundary

The actual root still needs to retain this report with rich source tables,
then construct checked assertion/CFG facts, memory effects, ranked coordinates
and a complete source-corresponding recipe on the original ledger. Those
integration and genuine-source qualifications are separate from this model
checkpoint. No artifact, native execution, GPU dispatch, physical capture or
milestone completion is claimed.

