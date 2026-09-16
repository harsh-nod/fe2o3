# RustCall argument correspondence

Status: experimental compiler integration for
[#272](https://github.com/harsh-nod/fe2o3/issues/272), not a claim of protected
artifact support or CPU/GPU equivalence. The complete serialized proof/debug
relation is coordinated with
[#271](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5690419624).

## One source signature, several representations

Rust closure calls use the `RustCall` ABI. A source signature containing a
receiver and one outer argument tuple can have either a packed tuple local or
separate tuple-field locals in rustc MIR. Only the outer tuple is expanded;
nested aggregates retain their own fields and layout.

For example, consider the logical inputs `(receiver, (u32, (), (u32, u32)))`.
The final source argument is still argument 1 in both representations:

| Identity | Packed MIR | Expanded MIR |
| --- | --- | --- |
| First tuple field | argument-1 local, projection `Field(0)` | field-0 local, no projection |
| Ignored unit field | argument-1 local, projection `Field(1)` | field-1 local, no projection |
| Nested tuple's second field | argument-1 local, `Field(2), Field(1)` | field-2 local, `Field(1)` |

These local identities are not physical GPU parameter indices. An ignored
zero-sized argument emits no parameter; an admitted by-value aggregate can
emit multiple scalar parameters. Whole-source ownership is retained separately
and does not grant ownership of a pointer's pointee.

`AdmittedInertSemanticMirV1::logical_arguments_v1` derives the mapping from the
admitted source/FnAbi/local tables in linear time. It does not add executable
IR or proof authority. Entry-local order does not determine argument order.

## Canonical encoding

Semantic-MIR V28 adds local-role tag 3 with two little-endian `u32` fields:
`source_argument`, then `tuple_field`. Tags 0, 1 and 2 retain their existing
return, ordinary argument and temporary meanings. V28 inherits the published
V15 intrinsic grammar; it assigns no intrinsic tags and does not activate
historical intermediate schemas.

Admission requires exact types, unique entry bindings, complete outer-tuple
coverage, and exclusivity between packed and expanded representations.
Expanded empty tuples also require V28 despite having no field locals. The
`Unit` spelling of an empty RustCall source tuple requires V28 even when packed;
the older packed empty `Tuple` spelling retains its earlier encoding.
Requesting an older wire version for V28-only content rejects it.

## Production lowering

The production importer reconstructs the monomorphized closure signature and
checks it against rustc's body argument count, receiver, spread argument,
return type and local types. Closure capture layout preserves nominal identity
and capture order. Collection/import observations bind the exact function,
body and target. A closure body's own authenticated receiver is not mistaken
for another host-supplied callable.

The existing prepared-SSA and occurrence records retain source argument and
tuple-field identity. Helper lowering uses the existing checked aggregate ABI
component lowering. Call lowering evaluates each source operand once, then
selects its outer tuple field and scalar components in ABI order. Destination
preparation still precedes moves. Ignored arguments remain part of logical
correspondence even though they contribute no physical operands.

An ordinary zero-sized value can cross an SSA join only when its admitted
layout is zero bytes and reconstructing its binding needs no values. Capability
transport retains its separate checks. Argument-row accounting covers adjusted
ABI rows and physical parameters, including repeated call sites and shared
helpers across roots, before materializing bodies.

Ranked checks borrow complete empty-helper-effect facts from that same
immutable materialization. Shared helpers retain every root/source/physical
function association. This can establish that a helper has no memory or
compiler-ordering effects, but it does not establish a deterministic scalar
return relation. The existing scalar-value checks remain separate.

## Exact live parameter checks

Materialization replays its parameter trace against the admitted logical
argument map and the actual KIR signature. Entry and helper emission share
their ABI representation selection with this check. It covers ordinary
arguments too, not just RustCall:

- Each physical parameter has exactly one function-qualified binding, at the
  correct ABI slot and with the expected KIR type.
- Aggregate bindings have the exact local projection and semantic leaf type.
  Packed RustCall prepends the outer field; expanded locals do not.
- Direct bindings are indexed by their value identity, not by trace order.
  A shared slice is one KIR parameter even though LLVM uses two carrier words.
- Each wholly ignored entry local has exactly one correctly typed ignored
  binding, disjoint from physical bindings. An expanded empty source tuple
  has no invented local or physical parameter.
- Every root association is checked when roots share one physical helper.

For `(receiver, ((u32, (), u32), (), u32))`, the packed tuple's first two
physical fields have local paths `Field(0), Field(0)` and
`Field(0), Field(2)`. Substituting the ignored middle field, a same-typed
neighbor's parameter value, or the enclosing aggregate type is rejected.

`ProductionSemanticKirLimitsV1::with_argument_correspondence_limits` configures
independent cumulative work and peak logical scratch-byte limits. They do not
replace the existing argument-row or emitted-operation limits. Both defaults
are 16 Mi units/bytes. The checker prepays logical-map construction, a bounded
32-bit radix index, field comparisons, and conservative scratch/work allowances
for the existing 256-node ABI-shape derivation. Shared-root checks accumulate
work in one validation ledger. An allocation-free capped sizing pass keeps
small aggregates from consuming a maximum-size argument's allowance.
Function-local scratch is released on success
and ordinary error returns; no unwind or allocator/RSS guarantee is implied.
The pre-ranked canonical ledger remains a separate accounting scope.

This validates the **existing sparse emission trace**. It does not yet expose
a complete typed record for every nested zero-sized field, nor a serialized
entry/call/debug relation. For example, a packed partly ignored tuple has
physical field bindings but no separate ignored-local binding for its unit
field. Completing that relation remains required before proof replay can use
it. No new executable graph or correspondence wire version is introduced.

## Evidence boundary

Legacy serialized V4/V5 proof correspondence does not encode the complete
component/ignored-argument relation. Its consumers explicitly reject RustCall;
rejection is not implementation of that relation. Completing it requires exact
source-field/local-projection/physical-parameter mappings, function-qualified
rosters and synthetic spans, replay in verifier and finalizer, and hostile
mutation coverage. No argument identity may be inferred from iteration order
or reused as kernel-launch authority.

Codec, SSA, lowering and ordinary-source simulator tests exercise different
stages. Neither a model test nor a successful simulator comparison establishes
protected proof execution, artifact publication, hardware execution, or full
tutorial-kernel support. See the [verification model](verification-model.md).

The `rocm-compile` CI lane includes
`ordinary_source_rust_call_closures_match_rust_in_simulation`: a single ordinary
Rust body with owned nested captures, multiple/empty/mixed-unit arguments,
wrapping arithmetic, complete output checks and canaries for gfx942 and gfx950
simulation targets. It needs the pinned compiler and rust-src; it does not
require GPU execution or grant GPU evidence.
