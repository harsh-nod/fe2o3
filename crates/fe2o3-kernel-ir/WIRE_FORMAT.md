# Kernel IR Wire Formats V1 through V11

This document freezes the canonical binary representations produced by
`encode_module_v1` through `encode_module_v11`. `decode_module_v1` accepts only
V1; each later decoder accepts canonical bytes up to its own version for
migration safety. Every encoder always emits exactly its named version.

`KERNEL_IR_DOMAIN_V5`, the byte string `FE2O3/KERNEL-IR/V5\0`, is the public
domain separator for identities derived from canonical V5 module bytes. It is
not an additional wire prefix; the versioned header remains part of the bytes.
`KERNEL_IR_DOMAIN_V6` provides the corresponding distinct
`FE2O3/KERNEL-IR/V6\0` namespace for canonical V6 content.
`KERNEL_IR_DOMAIN_V11` provides the distinct `FE2O3/KERNEL-IR/V11\0`
namespace for canonical V11 content. Exact verified V11 ownership uses
`VerifiedCanonicalKernelIrV11` and its separate policy-qualified identity;
neither raw nor verified identities grant proof, publication, or execution
authority.

### Content and Verified-Policy Identities

Canonical V5 bytes and verified V5 ownership have deliberately different
identity namespaces. `KERNEL_IR_DOMAIN_V5` identifies exact raw V5 content. A
content identity says only that two identities cover the same canonical wire
bytes; it does not claim that `verify_module` accepted the decoded module.

`VerifiedCanonicalKernelIrIdentityV5` instead identifies bytes admitted by
`VerifiedCanonicalKernelIrV5` under a specific semantic-verification policy.
Policy version 1 hashes this tuple with SHA-256:

```text
u32(len("FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V5\0")) ||
"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V5\0" ||
u16(VERIFIED_CANONICAL_KERNEL_IR_POLICY_V5) ||
u64(canonical_v5_byte_length) ||
canonical_v5_bytes
```

All integers in that tuple are little-endian. The separate domain and policy
version prevent a raw content identity from being confused with a verified
owner and allow future verification-policy changes to produce distinct
identities even for unchanged bytes. Neither identity is a proof-discharge,
artifact-publication, executable, or runtime-launch authority.

Exact V6 production ownership is established only by
`VerifiedCanonicalKernelIrV6`. The owner admits exact V6 bytes only after the
bounded decoder reproduces them byte-for-byte with the V6 encoder and
`verify_module` accepts the resulting typed module. Its policy V1 identity is
SHA-256 of this exact tuple:

```text
u32(len("FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V6\0")) ||
"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V6\0" ||
u16(VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_POLICY_V1) ||
u64(canonical_v6_byte_length) ||
canonical_v6_bytes
```

All integers are little-endian. The domain is exactly 38 bytes, the policy is
frozen at 1, and the complete 20-byte V6 envelope is part of the retained and
hashed canonical bytes. The identity retains the same framed canonical length
alongside the digest for explicit custody comparisons. Only the non-`Clone`
owner establishes typed canonicality and semantic validity; its copyable
identity remains an inert identifier. It does not establish source-to-KIR
refinement, proof discharge, publication, executable, or launch authority.

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
u16     version = 1 through 11
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
`Bf16=12`, `F32=13`, `F64=14`, `I128=15` (V4), and `U128=16` (V4).

Address-space tags are `Private=1`, `Workgroup=2`, `Global=3`, `Constant=4`,
and `Generic=5`. Access-mode tags are `ReadOnly=1`, `ReadWrite=2`, and
`WriteOnly=3` (V9).

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
| 20 (V2) | `Wave` | `WaveOperation` |
| 21 (V3) | `InlineAssembly` | `InlineAssembly` |
| 22 (V5) | `Matrix` | `MatrixOperation` |
| 23 (V7) | `GuardedLoad` | `ValueId pointer, ValueId predicate, ValueId fallback, MemoryAccess` |
| 24 (V9) | `Gfx950LdsTranspose` | `Gfx950LdsTransposeOperation` |
| 25 (V9) | `GuardedStore` | `ValueId pointer, ValueId predicate, ValueId value, MemoryAccess` |
| 26 (V10) | `MemoryIntrinsic` | `u32 semantic_instance_bytes, byte[semantic_instance_bytes] instance, variant operands` |

`MemoryAccess` is `u8 address_space || u32 alignment || u8 volatile_boolean`.

### V10 Memory Intrinsics

V10 is the first module wire version that admits `MemoryIntrinsic`. The
embedded byte string is the exact bounded
`encode_semantic_operation_instance_id` result for the operation contract.
Its family and opcode must identify one of `PointerDistance`, `VolatileLoad`,
`VolatileStore`, or `CopyNonOverlapping`; launch semantic instances are
rejected in this operation slot. The following operand identities then appear
in the variant's semantic order:

| Variant | Operands |
|---|---|
| `PointerDistance` | `ValueId pointer, ValueId origin` |
| `VolatileLoad` | `ValueId pointer` |
| `VolatileStore` | `ValueId pointer, ValueId value` |
| `CopyNonOverlapping` | `ValueId source, ValueId destination, ValueId count` |

The instance length is capped by the semantic-instance header plus its frozen
maximum payload. Exact instance decoding rejects malformed headers, contracts,
tags, lengths, reserved bytes, truncation, and trailing bytes before any
operand is admitted. V1 through V9 encoders continue to reject every memory
intrinsic, and their operation tags and bytes remain unchanged.

### V6 Checked Integer Arithmetic

V6 extends the binary-operator tag space without changing the operation
record. Tags `1` through `10` retain their frozen V1 meanings. The new tags
are:

| Binary tag | Operator |
|---:|---|
| 11 | `Checked(Add)` |
| 12 | `Checked(Subtract)` |
| 13 | `Checked(Multiply)` |

A checked binary operation must have two same-typed `I8`/`I16`/`I32`/`I64`/
`I128`, `U8`/`U16`/`U32`/`U64`/`U128`, or `Index` operands. Its operation
result vector is exactly `[operand_type, Bool]`, representing the wrapped
value followed by the overflow flag. Signedness comes only from the operand
type: `I*` uses signed overflow and `U*`/`Index` uses unsigned overflow. The
wire record does not duplicate signedness.

V1 through V5 encoders reject checked operators, and their bytes and operator
tags remain frozen. Decoding establishes only canonical wire structure;
`verify_module` establishes the operand and result invariants above.

### V5 Matrix Operations

V5 adds the following fixed-width matrix record. All four-value fragments are
stored as four consecutive `ValueId` values in array order; they have no count
field.

```text
MatrixOperation =
    u32                 active_lanes
    Convergence         convergence
    MatrixOperationKind kind

MatrixMultiplyProfile =
    u16 m || u16 n || u16 k ||
    u8 input_element || u8 accumulator_element || u8 wave_width

MatrixLdsProfile =
    u16 rows || u16 columns || u8 element || u8 layout ||
    u8 fragment_elements || u8 wave_width
```

| Kind tag | Variant | Payload |
|---:|---|---|
| 1 | `MultiplyAccumulate` | `ValueId[4] lhs, ValueId[4] rhs, ValueId[4] accumulator, MatrixMultiplyProfile` |
| 2 | `LdsLoad` | `ValueId base, MatrixLdsProfile` |
| 3 | `LdsStore` | `ValueId base, ValueId[4] values, MatrixLdsProfile` |

Matrix-element tags are `Bf16=1` and `F32=2`. The only matrix-layout tag is
`RowMajorXor4=1`. Wave-width and convergence tags reuse the frozen V2 codecs.
Numeric profile fields are wire data, not semantic validation: decoding a
profile does not establish that a target supports it. `verify_module` remains
responsible for that check.

`MatrixOperation::frontend_binding` is intentionally not representable in V5.
The encoder rejects any matrix operation carrying one instead of omitting the
field silently. No source, compiler, lowering, worker, artifact, load, runtime,
or hardware authority is introduced by this record.

### Semantic Memory Instance Identities

Memory intrinsics are not representable in the kernel module wire versions V1
through V6. Those encoders reject them. They do have an independent
V1 semantic-instance identity used to bind a closed target-neutral obligation
payload before a future module wire version can admit them.

The semantic-instance header is:

```text
byte[8] magic = "FE2O3SI\0"
u16     version = 1
u8      family = 1 (MemoryIntrinsic)
u8      flags = 0
u16     opcode
u16     payload_length
u32     reserved = 0
```

All obligation fields are one-byte closed tags. Layout is
`u64(size_bytes) || u32(alignment_bytes)`. A memory instance is canonical only
when that layout exactly equals the closed element tag's expected layout;
decoding rejects mismatched size or alignment before returning an identity.
The memory payloads are:

```text
PointerDistance (opcode 1, 21 bytes) =
    kind || unit || element || address_space ||
    equal_addresses_or_same_allocation ||
    both_in_bounds_or_one_past_when_addresses_differ ||
    exact_multiple_of_declared_unit || difference_fits_isize ||
    ordering || layout

VolatileLoad/Store (opcodes 2/3, 19 bytes) =
    element || address_space || origin || range ||
    aligned_for_element || trap_or_zst_no_access ||
    external_side_effect_isolation || layout

CopyNonOverlapping (opcode 4, 22 bytes) =
    element || source_address_space || destination_address_space ||
    aligned_for_element || source_readable_when_bytes_positive ||
    destination_writable_when_bytes_positive ||
    nonoverlapping_when_bytes_positive ||
    count_times_element_size_fits_usize ||
    signed_offsets_fit_isize_and_stay_in_allocations_when_bytes_positive ||
    alignment_required_ranges_and_overlap_conditional_on_positive_bytes ||
    layout
```

Pointer-distance kind tags are `Signed=1`, `Unsigned=2`; unit tags are
`Elements=1`, `Bytes=2`. Its four singleton obligation tags are `1`; ordering
is `SignedMayBeNegative=1` or `UnsignedPointerAtOrAfterOrigin=2` and must agree
with kind. Equal pointer addresses satisfy the first pointer-distance branch;
when addresses differ, same-allocation provenance and the in-bounds-or-one-past
range are both required.

Positive-sized volatile origin is `RustAllocation=1` or
`ExternalMmioNotRustAllocation=2`; range is
`ReadableInitializedElement=1` or `WritableElement=2` and must agree with the
operation. Alignment is `AlignedForElement=1` and positive-sized access trap is
`NonTrapping=1`. External-effect tags are `NotExternal=1` and
`SideEffectsDoNotModifyRustAllocatedMemory=2`. Every external load and store
must carry tag `2` independently of its non-Rust-allocation origin tag. External
MMIO is admitted only with the explicit `Global` address space; `Generic`,
local, and constant address spaces fail closed.

For the currently representable ZST, `Unit` (size 0, alignment 1), origin and
range are `ZeroSizedNoAccess=3`, trap is `ZeroSizedNoAccess=2`, and external
effect is `NotExternal=1`. This profile still requires
`AlignedForElement=1`, emits no volatile memory effect, and does not claim
Rust-allocation or external-MMIO provenance. Positive-sized accesses reject the
ZST profile, and the `Unit` ZST rejects positive-sized profiles. Other ZST
layouts remain unrepresentable and fail closed.
Unknown tags and inconsistent cross-field combinations are rejected.

The copy contract requires alignment even when `count * size_of::<T>()` is
zero, including ZST copies. Readable/writable ranges, allocation bounds, and
non-overlap are conditional on a positive byte count. The multiplication must
fit `usize`; positive-byte pointer ranges must also satisfy Rust's signed
pointer-offset and allocation bounds.

Frozen independent vectors, shown as hexadecimal bytes, are:

```text
signed element u32 global pointer distance:
46 45 32 4f 33 53 49 00 01 00 01 00 01 00 15 00 00 00 00 00
01 01 08 03 01 01 01 01 01 04 00 00 00 00 00 00 00 04 00 00 00

external-MMIO u32 global volatile load:
46 45 32 4f 33 53 49 00 01 00 01 00 02 00 13 00 00 00 00 00
08 03 02 01 01 01 02 04 00 00 00 00 00 00 00 04 00 00 00

external-MMIO u32 global volatile store:
46 45 32 4f 33 53 49 00 01 00 01 00 03 00 13 00 00 00 00 00
08 03 02 02 01 01 02 04 00 00 00 00 00 00 00 04 00 00 00

aligned ZST global volatile no-access load:
46 45 32 4f 33 53 49 00 01 00 01 00 02 00 13 00 00 00 00 00
00 03 03 03 01 02 01 00 00 00 00 00 00 00 00 01 00 00 00

u32 constant-to-global copy_nonoverlapping:
46 45 32 4f 33 53 49 00 01 00 01 00 04 00 16 00 00 00 00 00
08 04 03 01 01 01 01 01 01 01 04 00 00 00 00 00 00 00 04 00 00 00
```

These identities record obligations only. Decoding does not prove them, grant
compiler-import authority, or authorize an executable artifact.

Intrinsic tag `1` is `InvocationIndex` followed by `u8 IndexKind`, `u8 Axis`,
and the explicit result `Type`. Intrinsic tag `2` is `LaunchExtent` followed by
`u8 Axis` and the explicit result `Type`. Index-kind tags are `Global=1`,
`Workgroup=2`, `Local=3`, `WorkgroupSize=4`, and `WorkgroupCount=5`. Axis tags
are `X=1`, `Y=2`, and `Z=3`.

Unary tags are `Negate=1` and `Not=2`. Frozen binary tags follow declaration
order from `Add=1` through `ShiftRight=10`; V6 checked tags are listed above.
Compare tags follow declaration order from `Equal=1` through
`GreaterThanOrEqual=6`. Frozen scalar cast tags follow declaration order from
`Truncate=1` through `Bitcast=8`. V11 adds cast tag `9`,
`RestrictPointerAccess`. V1 through V10 encoders reject that tag, and decoders
reject a forged tag `9` under an older header.

`RestrictPointerAccess` is an exact pointer-identity operation, not a scalar
conversion or address-space cast. `verify_module` accepts only a pointer whose
pointee type and address space are unchanged while access is narrowed from
`ReadWrite` to `ReadOnly`. Identity casts, `ReadOnly` to `ReadWrite` widening,
write-only substitutions, pointee changes, and address-space changes are
invalid. The operation does not itself prove aliasing, lifetime, or Rust
reference validity.

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

Capability sets are encoded in strict `TargetCapability` order and reject
duplicates. Their semantic closure is verified after decoding: in particular,
`DynamicWorkgroupMemory` without `WorkgroupMemory` is malformed, and
synchronization operations over workgroup memory derive `WorkgroupMemory` even
when a producer omitted that declaration. Wire decoding alone grants no target
or lowering authority.

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
| Embedded semantic-operation instance | semantic header plus 256 payload bytes |
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

Existing tags and field meanings must never be changed. Every version is
additive: older encoders reject newer-only model nodes, older decoders reject
newer headers, and later decoders accept earlier versions while enforcing the
tags legal for the actual header version. V5 therefore cannot make operation
tag 22 legal under a forged V1-V4 header. The frozen V1 golden fixture is
`tests/fixtures/full_v1.hex`; the independent V2 fixtures are
`tests/fixtures/g4_sync_v2.hex` and `tests/fixtures/integer_switch_v2.hex`.

## Separate Semantic Operation Schemas and Instances

These formats are not module encodings and do not change V1-V5 module bytes.

encode_semantic_operation_schema produces a fixed-width payload-blind dispatch
key:

```text
byte[8] magic = "FE2O3SO\0"
u16     version = 1
u8      family
u8      reserved = 0
u16     family-local opcode
u16     reserved = 0
```

A schema intentionally aliases payload variants of one opcode. It must not be
used as a proof, artifact, cache, equivalence, or executable identity.

encode_semantic_operation_instance_id produces the canonical full semantic
instance:

```text
byte[8] magic = "FE2O3SI\0"
u16     version = 1
u8      family
u8      flags = 0
u16     family-local opcode
u16     payload_length
u32     reserved = 0
byte[payload_length] canonical semantic payload
```

V1 launch-invocation payloads contain an index-kind tag followed by an axis
tag. Launch-extent payloads contain an axis tag. Thus every admitted launch
axis and index level has a distinct full instance identity despite sharing a
schema.

Both decoders reject unknown versions, families, opcodes, flags, payload tags,
payload sizes, reserved fields, truncation, and trailing bytes. Neither format
grants lowering authority. A semantic operation still needs closed
OperationKind admission, independently extracted operands, verification, and
backend support. Serializing an operation payload inside a Module requires an
explicitly new module wire version.
