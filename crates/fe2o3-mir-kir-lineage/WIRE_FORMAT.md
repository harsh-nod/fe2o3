# Canonical MIR-to-KIR Lineage V4 Wire Format

All integers after the magic are unsigned LEB128 in their unique shortest form.
Identity values are exactly 32 raw bytes. Lists have no padding. Record order is
exact Kernel IR order; no decoder normalization or sorting is permitted.

```text
magic                                  [8]byte = "FE2O3L4\0"
version                                varuint = 4
flags                                  varuint = 0

semantic_mir_identity_scheme           raw_canonical_sha256(0)
semantic_mir_canonical_wire_version     semantic_mir_v2(2) | semantic_mir_v3(3)
semantic_mir_raw_canonical_sha256       [32]byte, nonzero
semantic_mir_canonical_length           varuint, 1..=128 MiB

kernel_ir_identity_scheme               verified_canonical_v5_sha256_policy_v1(0)
                                      | verified_canonical_v6_sha256_policy_v1(1)
kernel_ir_canonical_wire_version        kernel_ir_v5(5) | kernel_ir_v6(6)
kernel_ir_domain_policy_identity        [32]byte, nonzero
kernel_ir_canonical_length              varuint, 1..=16 MiB

lowering_policy_version                varuint = 2
lineage_policy_mode                    production_v3_to_v6(0) | legacy_inert_v2_to_v5(1)
target                                 gfx942(0)
ranked_bounds_policy                   retain(0) | validated_discharge(1)
f32_declaration_policy                 referenced_only(0)
diagnostic_declaration_policy          referenced_only(0)
correspondence_policy                  exhaustive_typed_traversal(0)
checked_arithmetic_policy              semantic_v3_to_kir_v6_checked_v1(0)
                                      | legacy_inert_no_refinement_authority(1)
max_semantic_functions                 varuint, nonzero
max_kir_functions                      varuint, nonzero
max_kernels                            varuint, nonzero
max_blocks                             varuint, nonzero
max_statements                         varuint, nonzero
max_operations                         varuint, nonzero
max_work                               varuint, nonzero

total_semantic_functions               varuint
total_kir_functions                    varuint
total_kernels                          varuint
total_semantic_blocks                  varuint
total_synthetic_blocks                 varuint
total_statements                       varuint
total_terminators                      varuint
total_operations                       varuint

function[total_kir_functions]
kernel[total_kernels]
EOF
```

The only production-eligible pair is semantic MIR V3 with its raw canonical
SHA-256 identity to Kernel IR V6 with exact verified-canonical SHA-256 policy V1
scheme tag 1 and checked-arithmetic policy tag 0. Semantic MIR V2 to Kernel IR
V5 may decode only under the explicit legacy-inert mode, exact V5 scheme tag 0,
and no-refinement-authority tag 1. Mixed pairs are invalid. Neither mode grants
authority.

## Exact Kernel IR V6 Identity Scheme

The V6 scheme is not an arbitrary 32-byte SHA-256 claim. The complete canonical
KIR V6 artifact is bounded to 16 MiB including its 20-byte envelope:

```text
magic             [8]byte = "FE2O3KI\0"
version           u16-le = 6
flags             u16-le = 0
total_length      u32-le = exact complete byte length
reserved          u32-le = 0
canonical_payload remaining bytes in authoritative KIR V6 field order
```

The authoritative KIR V6 owner must typed-decode the complete bytes, reject
trailing input, semantically verify the module, and reproduce those exact bytes
by canonical V6 re-encoding. This std-only crate checks only the fixed envelope
when recomputing; its result remains inert.

The identity is SHA-256 of this exact preimage, in order and with no padding
between fields beyond SHA-256's own finalization:

```text
u32-le(38)
b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V6\0"
u16-le(1)                    # identity policy version
u64-le(canonical_v6_length)
canonical_v6_bytes
```

The public recomputation helper implements this construction without external
dependencies. For the frozen 32-byte V6 envelope fixture with twelve `a5`
payload bytes, the digest is
`5cabd337ffb422441a25bcd9ca4e4f03d6f32816b29a06d29adb4e4715dfc437`.
Decoded lineage exposes the digest only as an inert claimed scheme digest, not
as a recomputation token.

## Function Record

```text
kir_function_ordinal                   varuint, exact record ordinal
classification                        tag
```

Classification payloads:

```text
SemanticBody(0):
  semantic_function_ordinal            varuint, contiguous source ordinal
  semantic_block_count                 varuint
  kir_block_record_count               varuint
  block[kir_block_record_count]

F32IntrinsicDeclaration(1):
  intrinsic                            closed tag 0..12

DiagnosticTrapDeclaration(2):
  diagnostic                           runtime_assert_failure(0)
```

Declarations contain no blocks. Declaration identities must be unique. A
runtime-assert synthetic block and its one shared diagnostic declaration must
either both be present or both be absent. F32 declaration references cannot be
proved from count-only lineage; the required exhaustive typed traversal checks
them against exact KIR operations before authority is granted. KIR checked
arithmetic is a native operation and does not introduce another declaration
classification.

## Block Record

```text
kir_block_ordinal                      varuint, unique and in range
block_operation_count                  varuint
classification                        tag
```

Classification payloads:

```text
SemanticBlock(0):
  semantic_block_ordinal               varuint, unique and in range
  statement_count                      varuint
  statement_operation_count[statement_count]
  terminator_operation_count           varuint

SyntheticBlock(1):
  rule                                 runtime_assert_failure_trap(0)
```

Statement ordinals are implicit record ordinals. Each statement's first
operation is the cumulative count of preceding statements; the terminator's
first operation is the cumulative count of all statements. Zero-operation
statements therefore occupy one canonical byte and remain explicit. The sum of
all statement counts and the terminator count must equal
`block_operation_count`. Gaps and overlaps are unrepresentable on this wire. A
runtime-assert synthetic block has exactly one operation.

## Kernel Record

```text
kernel_ordinal                         varuint, exact record ordinal
semantic_function_ordinal              varuint, unique semantic body
kir_function_ordinal                   varuint, matching semantic body
```

## Checked Arithmetic

Lowering policy version 2 normatively requires the production policy described
in [CHECKED_ARITHMETIC_REFINEMENT.md](CHECKED_ARITHMETIC_REFINEMENT.md). Its wire
tag and frozen vector record a validation obligation only. A named external
move-only owner gate and a real Semantic MIR V3 to KIR V6 cross-crate fixture
remain mandatory before production authority can exist. Legacy-inert records
carry an explicit no-refinement-authority tag. KIR checked arithmetic is a
native operation and creates no declaration class.

## Admission

The exported hard lineage cap `MAX_LINEAGE_BYTES_V4` is exactly 4,194,304 bytes;
it is independent of the 16 MiB referenced KIR identity-length cap. A caller's
`max_input_bytes` may tighten but cannot widen this bound. Decode rejects an
oversized slice before creating a reader, charging work, or allocating record
storage. Canonical construction checks the same hard limit before growing its
output buffer. Every supplied count limit is enforced by both paths.

One checked `max_work` budget covers input bytes, all declared
function/kernel/block/statement/terminator records,
structural validation traversals, every initialized bitmap slot, and every byte
of canonical re-encoding. Declared record work is checked before record
allocation. Parser vectors grow with fallible one-record reservations. Wire
assembly uses crate-private constructors without repeating coverage validation;
the one authoritative structural pass is budgeted. The accepted model is
re-encoded under the same input and work bounds and must reproduce every input
byte.

A downstream move-only validator must first compare the embedded artifact
header versions and exact identity schemes to these typed lineage identities.
Only then may it traverse MIR and KIR operations, operands, results, types,
metadata, block parameters, terminators, CFG edges, function metadata, and
kernel metadata. This ordering prevents a digest-shaped value under a different
scheme or wire version from being treated as the claimed artifact.

The resulting value is inert data. It grants no compiler, verifier, proof,
artifact, publication, load, or launch authority.
