# Checked-Arithmetic Refinement Policy V1

This document is the normative checked-arithmetic component of
`LOWERING_POLICY_VERSION_V4 = 2`. The V4 lineage wire records this obligation;
it does not prove that a lowering satisfied it. Only the external move-only
owner gate named by
`CHECKED_ARITHMETIC_EXTERNAL_OWNER_GATE_V4` may establish the relation.

## Admitted Semantic MIR

The production rule applies only to an exact Semantic MIR V3
`CheckedBinary(Add | Sub | Mul)` rvalue. Both operands and the wrapped result
must have the same plain scalar integer type `T`. The admitted concrete `T`
set is:

- unsigned `u8`, `u16`, `u32`, `u64`, and `u128`;
- signed `i8`, `i16`, `i32`, `i64`, and `i128`; and
- `usize` or `isize` only after target resolution for gfx942, where the pointer
  width is exactly 64 and the KIR carrier is respectively `u64` or `i64`.

Signedness and width must be preserved exactly. Boolean, character, floating,
pointer, reference, vector, aggregate, enum, newtype, and unresolved abstract
index shapes are not admitted by this rule. No implicit widening, narrowing,
signedness conversion, or target-independent `usize`/`isize` carrier is
permitted.

## Required KIR V6 Operation

One admitted Semantic MIR checked rvalue refines to exactly one native Kernel IR
V6 `Checked` operation whose kind is the corresponding `Add`, `Sub`, or `Mul`.
The operation has exactly two operands of the resolved KIR type `T` and exactly
two SSA results, in this order:

1. the wrapped result of type `T`;
2. the overflow result of KIR `bool` type.

For an `N`-bit unsigned `T`, the wrapped result is the mathematical result
modulo `2^N`, and overflow is true exactly when the mathematical result is
outside `[0, 2^N - 1]`. For an `N`-bit two's-complement signed `T`, the wrapped
result is the low `N` bits interpreted as `T`, and overflow is true exactly when
the mathematical result is outside `[-2^(N-1), 2^(N-1) - 1]`.

Checked arithmetic is a native KIR operation. It introduces no intrinsic,
helper, diagnostic, or trap declaration and therefore needs no additional
function classification in lineage.

## Operand And Projection Order

The lowerer must process the left operand before the right operand and each
operand exactly once. `Copy`, `Move`, `Constant`, and already-materialized value
forms retain their Semantic MIR meaning. A place projection is traversed from
its base through every projection element in source order. Any KIR address,
index, load, cast, or materialization operation required for an operand must be
emitted before the `Checked` operation, remain in that operand's order, and may
not be duplicated, fused with the other operand, or moved across an effectful
operation.

The Semantic MIR destination has tuple shape `(T, bool)`. KIR result zero maps
only to tuple field zero and result one only to tuple field one. A destination
place projection is preserved in source order before those final field
mappings. If the destination is addressable, required writes occur in wrapped
then overflow order. If later MIR statements project fields from the assigned
tuple, each later projection belongs to its own statement and must use the same
field mapping. No projection may exchange or silently discard the two results;
ordinary dead-value handling may make an unused result have no later use.

## Assertion Independence

The checked-rvalue rule ends after producing `(wrapped T, bool overflow)`. It
must not inspect, synthesize, merge, discharge, invert, or remove an assertion.
A later Semantic MIR assertion that consumes the overflow flag is lowered
independently under the assertion and diagnostic-trap policy, even when it is
the immediately following terminator. Conversely, the checked operation remains
required when the overflow result is ignored.

## Lineage Attribution

The source statement's operation span is contiguous and includes, in execution
order, every operation emitted specifically to evaluate its left operand, every
operation emitted specifically to evaluate its right operand, the one required
`Checked` operation, and any destination projection or materialization
operations emitted for that assignment. The span must contain exactly one
`Checked` operation. It contains no operation from a preceding or following
statement and no terminator operation. A later assertion is attributed to its
own terminator span. Zero-operation statements remain explicit and do not move
the first-operation ordinal of the next record.

## Required External Gate

The policy tag and frozen vector are inert identifiers of this contract, not
evidence that it was followed. Before accepting the relation, the named external
gate must own and validate all of the following in one custody-bound operation:

1. exact raw-canonical Semantic MIR V3 bytes and their recomputed SHA-256;
2. exact canonical Kernel IR V6 bytes accepted by the authoritative typed KIR
   decoder, canonical re-encoder, and semantic verifier;
3. the exact V6 SHA-256 policy V1 KIR identity construction and gfx942 target;
4. exhaustive typed statement, operand, projection, result, type, block,
   terminator, CFG, function, and kernel correspondence; and
5. every lineage statement and terminator operation span, including the rules
   above.

A real cross-crate fixture must lower Semantic MIR V3 Add, Sub, and Mul cases
through the production lowerer, validate exact KIR V6 owners, and exercise this
gate. That fixture is a required release gate after the Semantic MIR V3 and KIR
V6 owner patches land. It intentionally does not exist in this std-only design
crate, and V4 lineage must not be treated as production authority until it does.

## Frozen Policy Vector

`CHECKED_ARITHMETIC_REFINEMENT_POLICY_VECTOR_V4` freezes the compact policy
identifier. Multibyte integers are little-endian. Its fields, in order, are:

```text
magic "FE2O3CA1"
policy_version u16 = 1
semantic_version u16 = 3
kir_version u16 = 6
target u8 = gfx942(0)
operation_count u8 = 3; operations = Add(0), Sub(1), Mul(2)
width_count u8 = 5; widths u16 = 8, 16, 32, 64, 128
signedness_count u8 = 2; unsigned(0), signed(1)
index_mapping u8 = target_pointer_width(0); width u16 = 64
result_count u8 = 2; wrapped(0), overflow_bool(1)
operand_order u8 = left_then_right_once(0)
projection_policy u8 = preserve_order_no_fusion(0)
assertion_policy u8 = independent(0)
span_policy u8 = complete_contiguous_source_statement(0)
```

The frozen bytes are:

```text
4645324f334341310100030006000003000102050800100020004000800002000100400002000100000000
```
