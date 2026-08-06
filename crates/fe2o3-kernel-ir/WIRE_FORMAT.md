# Kernel IR Wire Formats V1 and V2

This document freezes the canonical binary representations produced by
`encode_module_v1` and `encode_module_v2`. `decode_module_v1` accepts only V1;
`decode_module_v2` accepts canonical V1 and V2 bytes for migration safety.

## Trust Boundary

The decoder accepts untrusted bytes. It checks the total byte bound, every
length and count before allocation, UTF-8 validity, enum and option tags,
reserved fields, type nesting, set ordering, complete input consumption, and
exact decode/re-encode equality. It contains no `unsafe` code.

A successful decode means only that the bytes canonically represent the Rust
`Module` data model. It does **not** establish SSA dominance, type correctness,
valid control flow, legal memory operations, valid synchronization, declared
capability completeness, or target support. Consumers must call
`verify_module` or `verify_module_with_capabilities` before trusting those
properties.

The format serializes all data stored below `Module`. `IntrinsicMetadata`,
`MemoryEffect`, and `MemoryEffectSummary` are deterministic query results and
are recomputed from stored operations, so they are not duplicated on the wire.

## Primitive Encoding

- Integers use fixed-width little-endian encoding.
- A tag or boolean is one `u8`. Boolean and option tags are exactly `0` or `1`.
- Text is `u32(byte_length) || UTF-8 bytes`; no normalization is performed.
- A vector is `u32(item_count) || items` in model order.
- A set is `u32(item_count) || items` in strictly increasing Rust `Ord` order.
- IDs are encoded as their underlying `u32` or text without reinterpretation.
- Signed integers use their two's-complement little-endian bytes.
- Floating constants preserve their explicit IEEE bit payloads.

No padding is implicit. Every field below appears immediately after its
predecessor.

## Header and Module

The 20-byte header is:

```text
byte[8] magic = "FE2O3KI\0"
u16     version = 1 or 2
u16     flags = 0
u32     total_length_including_header
u32     reserved = 0
```

The body is:

```text
text                  module_id
u32                   function_count
u32                   kernel_count
set<TargetCapability> required_capabilities
Function[function_count]
Kernel[kernel_count]
```

The declared length must equal the input length. Trailing bytes are rejected.

## Functions and Blocks

```text
Function =
    text                  function_id
    vec<Type>             signature_parameters
    vec<Type>             signature_results
    option<FunctionBody>  body
    set<TargetCapability> required_capabilities

FunctionBody =
    vec<ValueId>   parameters
    vec<BasicBlock> blocks

BasicBlock =
    u32                block_id
    vec<ValueDef>      parameters
    vec<Operation>     operations
    option<Terminator> terminator

ValueDef = u32(value_id) || Type
Operation = vec<ValueDef>(results) || OperationKind
```

An absent body represents a declaration. An absent terminator remains
representable so the semantic verifier can diagnose malformed frontend output.

## Kernels and Launch Domains

```text
Kernel =
    text                  kernel_id
    text                  entry_function_id
    LaunchDomain          domain
    option<WorkgroupSize> workgroup_size
    set<TargetCapability> required_capabilities

WorkgroupSize = u32(x) || u32(y) || u32(z)
```

| Tag | Launch domain | Payload |
|---:|---|---|
| 1 | `D1` | `LaunchExtent(x)` |
| 2 | `D2` | `LaunchExtent(x), LaunchExtent(y)` |
| 3 | `D3` | `LaunchExtent(x), LaunchExtent(y), LaunchExtent(z)` |

`LaunchExtent` is tag `1` for `Dynamic`, or tag `2` followed by `u32` for
`Static`.

## Types

| Tag | Variant | Payload |
|---:|---|---|
| 1 | `Unit` | none |
| 2 | `Scalar` | `u8 ScalarType` |
| 3 | `Pointer` | `u8 AddressSpace, u8 AccessMode, Type pointee` |
| 4 | `Slice` | `u8 AddressSpace, u8 AccessMode, Type element` |

Scalar tags follow declaration order: `Bool=1`, `I8=2`, `I16=3`, `I32=4`,
`I64=5`, `U8=6`, `U16=7`, `U32=8`, `U64=9`, `Index=10`, `F16=11`,
`Bf16=12`, `F32=13`, and `F64=14`.

Address-space tags are `Private=1`, `Workgroup=2`, `Global=3`, `Constant=4`,
and `Generic=5`. Access-mode tags are `ReadOnly=1` and `ReadWrite=2`.

## Operations

| Tag | Variant | Payload |
|---:|---|---|
| 1 | `Constant` | `Constant` |
| 2 | `Intrinsic` | `IntrinsicOperation` |
| 3 | `Unary` | `u8 op, ValueId operand` |
| 4 | `Binary` | `u8 op, ValueId lhs, ValueId rhs` |
| 5 | `Compare` | `u8 predicate, ValueId lhs, ValueId rhs` |
| 6 | `Cast` | `u8 kind, ValueId value, Type to` |
| 7 | `Select` | `ValueId condition, ValueId true, ValueId false` |
| 8 | `Call` | `text callee, vec<ValueId> arguments` |
| 9 | `Alloca` | `Type element, option<ValueId> count, u8 address_space, u32 alignment` |
| 10 | `SliceLength` | `ValueId slice` |
| 11 | `SliceData` | `ValueId slice` |
| 12 | `GetElementPointer` | `ValueId base, ValueId offset` |
| 13 | `Load` | `ValueId pointer, MemoryAccess` |
| 14 | `Store` | `ValueId pointer, ValueId value, MemoryAccess` |
| 15 | `Barrier` | `Barrier` |
| 16 | `Atomic` | `Atomic` |
| 17 (V2) | `Fence` | `Fence` |
| 18 (V2) | `WorkgroupBarrier` | `WorkgroupBarrier` |
| 19 (V2) | `WorkgroupMemory` | `WorkgroupMemory` |

`MemoryAccess` is `u8 address_space || u32 alignment || u8 volatile_boolean`.

Intrinsic tag `1` is `InvocationIndex` followed by `u8 IndexKind`, `u8 Axis`,
and the explicit result `Type`. Intrinsic tag `2` is `LaunchExtent` followed by
`u8 Axis` and the explicit result `Type`. Index-kind tags are `Global=1`,
`Workgroup=2`, `Local=3`, `WorkgroupSize=4`, and `WorkgroupCount=5`. Axis tags
are `X=1`, `Y=2`, and `Z=3`.

Unary tags are `Negate=1` and `Not=2`. Binary tags follow declaration order
from `Add=1` through `ShiftRight=10`. Compare tags follow declaration order
from `Equal=1` through `GreaterThanOrEqual=6`. Cast tags follow declaration
order from `Truncate=1` through `Bitcast=8`.

Constant tags follow declaration order from `Bool=1` through `F64Bits=14`.
Each payload has the width named by the variant; `Index` always carries a
`u64` wire value.

`Barrier` is:

```text
u8                execution_scope
u8                memory_scope
u8                memory_ordering
set<AddressSpace> address_spaces
```

`Atomic` is:

```text
u8              atomic_kind
ValueId         pointer
option<ValueId> value
option<ValueId> compare
MemoryAccess    access
u8              scope
u8              ordering
option<u8>      failure_ordering
```

The V2-only records are:

```text
Fence =
    u8                memory_scope
    u8                memory_ordering
    set<AddressSpace> address_spaces

WorkgroupBarrier =
    u8                memory_scope
    u8                memory_ordering
    set<AddressSpace> address_spaces
    Convergence       convergence

Convergence =
    u8(tag = 1, Uniform)
    u8(scope)

WorkgroupMemory =
    Type element
    WorkgroupMemoryExtent extent
    u32 alignment

WorkgroupMemoryExtent =
    u8(tag = 1, Static) || u32(elements)
  | u8(tag = 2, Dynamic)
```

Atomic-kind tags follow declaration order from `Load=1` through `BitXor=11`.
Scope tags follow declaration order from `Invocation=1` through `System=5`.
Ordering tags follow declaration order from `Relaxed=1` through
`SequentiallyConsistent=5`.

## Terminators

| Tag | Variant | Payload |
|---:|---|---|
| 1 | `Branch` | `BlockId target, vec<ValueId> arguments` |
| 2 | `ConditionalBranch` | `ValueId condition, BlockId then_target, vec<ValueId> then_arguments, BlockId else_target, vec<ValueId> else_arguments` |
| 3 | `Switch` | `ValueId selector, vec<SwitchCase> cases, BlockId default_target, vec<ValueId> default_arguments` |
| 4 | `Return` | `vec<ValueId> values` |
| 5 | `Unreachable` | none |
| 6 (V2) | `IntegerSwitch` | `ValueId selector, vec<IntegerSwitchCase> cases, BlockId default_target, vec<ValueId> default_arguments` |

`SwitchCase` is `u64 value || BlockId target || vec<ValueId> arguments`.
This legacy V1 record remains unchanged. Its case order is preserved;
uniqueness and selector compatibility are semantic verification concerns.

`IntegerSwitchCase` is
`Constant value || BlockId target || vec<ValueId> arguments`. Cases must be
strictly increasing under `Constant` ordering. The V2 encoder rejects duplicate
or out-of-order cases, and the V2 decoder rejects such bytes as noncanonical.
Semantic verification additionally requires an integer or index selector,
integer or index constants whose exact type matches the selector, valid case
destinations, a valid mandatory default destination, and type-correct edge
arguments.

## Target Capabilities

| Tag | Variant | Payload |
|---:|---|---|
| 1 | `Float16` | none |
| 2 | `BFloat16` | none |
| 3 | `Float64` | none |
| 4 | `Int64` | none |
| 5 | `Subgroups` | none |
| 6 | `SubgroupSize` | `u32 size` |
| 7 | `WorkgroupMemory` | none |
| 8 | `WorkgroupBarrier` | none |
| 9 | `Atomic` | `u16 width_bits, u8 address_space, u8 max_scope` |
| 10 | `DynamicWorkgroupMemory` | none |
| 11 | `Extension` | `text namespace, text name` |
| 12 (V2) | `WaveWidth` | `u8 width`, where `Wave32=1`, `Wave64=2` |

## Resource Bounds

| Resource | Maximum |
|---|---:|
| Encoded module | 16 MiB |
| Any text field | 4096 bytes |
| Functions or kernels | 16384 each |
| Capabilities in one set | 1024 |
| Signature parameters or results | 65536 each |
| Function parameter identities | 65536 |
| Blocks per function | 65536 |
| Parameters or operations per block | 65536 each |
| Results per operation | 65536 |
| Any value-argument list | 65536 |
| Legacy or typed integer switch cases | 65536 |
| Nested pointer/slice depth | 64 |
| Barrier address spaces | 5 |

The total-byte bound remains authoritative even when individual count bounds
would permit a larger in-memory model.

## Canonicality and Evolution

All reserved fields are zero. Unknown versions, flags, tags, duplicate set
members, and out-of-order set members are rejected. Decoding must consume the
entire declared input, and re-encoding the decoded model must reproduce every
byte exactly.

Existing tags and field meanings must never be changed. V2 is additive: V1
encoders reject V2-only model nodes, the V1 decoder rejects V2 headers, and the
V2 decoder accepts both versions while enforcing the tags legal for the actual
header version. The frozen V1 golden fixture is `tests/fixtures/full_v1.hex`;
the independent V2 fixtures are `tests/fixtures/g4_sync_v2.hex` and
`tests/fixtures/integer_switch_v2.hex`.
