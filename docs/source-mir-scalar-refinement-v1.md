# Source-to-semantic-MIR scalar refinement V1

This lane certifies individual, directly observed `u32` binary expressions. It
does not certify a whole Rust function or a whole MIR body.

## Accepted slice

The production collector may emit a certificate when all of these checks pass:

- the HIR expression is a direct binary expression over two direct parameter
  bindings;
- the HIR result, raw MIR operands, raw MIR destination, admitted semantic MIR
  operands, and admitted destination are unprojected `u32` locals;
- HIR and raw MIR classify the same operation from `+`, `-`, `*`, `&`, `|`, or
  `^`;
- the exact HIR expression provenance equals the retained raw-MIR statement
  provenance and the admitted semantic-MIR statement provenance; and
- the retained preflight maps bind each raw MIR local to the recorded semantic
  local and canonical semantic-local identity.

Wrapping add, subtract, and multiply are admitted only when rustc overflow
checks are disabled. Bitwise operations are independent of that policy.
Unsupported and non-matching statements are skipped, so a valid production
compilation may retain zero V1 records. A record is a positive claim about one
selected statement; absence is never interpreted as whole-body coverage.

## Claim

The executable source and semantic-MIR models both read the selected left and
right `u32` values, evaluate the checked operation, and write the result to the
selected destination. Under explicit equality of the operation, type width,
source/MIR input values, and source/MIR destination identity, the Verus theorem
`fe2o3_source_mir_u32_element_refines_v1` proves equal wrapping output and equal
ordered read/read/write effects. The theorem contains no `assume`, `admit`, or
external body.

The equality of runtime operand values remains a theorem precondition. The
certificate records HIR binding identities, raw MIR local ordinals, semantic
local IDs, and semantic-local identities, but it does not establish any later
KIR `ValueId` equality. A MIR-to-KIR proof must independently revalidate that
mapping.

## Identity and authority

The model identity binds the model and policy versions, theorem name, exact
positive proof-source SHA-256, and pinned Verus executable SHA-256. Evidence
also binds the HIR owner, HIR expression, monomorphized raw MIR body, exact
source provenance, admitted semantic MIR, operation, and all three local axes.
The rustc-codegen production importer constructs the authenticated wrapper in
the live compiler session; caller-created observation structs produce inert
evidence only.

Neither form grants artifact, LLVM, runtime, publication, or launch authority.
The production transaction retains and revalidates the evidence without using
it as an admission gate for unsupported statements.

## Remaining trusted base

The remaining frontend trusted base is rustc parsing and macro expansion, HIR
resolution and type checking, HIR-to-MIR lowering, source-map coordinates,
monomorphized `instance_mir`, and the same-session rustc identity hash helpers.
The production preflight raw-local-to-semantic-local map, semantic-MIR admission
validator, Rust certificate validator and SHA-256 implementation, pinned Verus
binary, Verus frontend, vstd, and Z3 are also trusted for this bounded claim.
LLVM, AMDGPU lowering, the runtime, and hardware are outside the claim.

Run the proof lane with the pinned `0.2026.08.02.b677dd5` closure:

```text
VERUS=/path/to/verus scripts/test-source-mir-scalar-refinement-verus-v1.sh
```

The lane proves the positive theorem and requires operator, type, effect, and
source-binding mutations to fail verification.
