# fe2o3-kernel-descriptor

This crate defines the bounded, canonical `DeviceDescriptorTableV1` byte
schema. A table describes every fe2o3 kernel in one AMDHSA code object. It is
portable build evidence, not launch authority.

`DeviceDescriptorTableV2` is a versioned extension that embeds the complete
canonical V1 bytes unchanged and appends one target-requirement record for
every V1 kernel. V1 encoding, decoding, and digests remain byte-for-byte
stable.

Decoding proves only that bytes are a canonical, internally consistent V1
table. The decoder treats every field as attacker-controlled. In particular,
the compiler identity, source and executable-IR evidence, Rust type identity,
device layout identity, canonical AMD device target, and code-object digest are
declarations until a later trusted layer matches the complete table
byte-for-byte against a host-linked copy selected by the generated Rust kernel
marker.

## V1 boundary

V1 supports scalar values, shared scalar slices, and unique scalar
`DisjointSlice` arguments. It deliberately has no raw pointer, generic address
space, record, enum, union, or recursive type representation. Physical ABI
components are restricted to scalar by-value values or a global pointer
immediately followed by a by-value `u64` slice length.

`KernelId` is an opaque selector owned by the manifest/macro pipeline. Its 32
bytes define logical identity directly; V1 specifies no hash preimage or
derivation. Only an exact trusted binding between the generated host marker,
manifest, table, and executable entry can grant that selector authority.

The table contains device information only. Host targets and host layouts will
belong to a separate host-binding schema.

## Canonical tiled GEMM V1 structural descriptor profile

`admit_tiled_gemm_v1_structural_descriptor_v1` is a sealed, fail-closed policy
for the declared ABI and metadata of direct-global `tiled_gemm_v1`. It requires
COV6 on `gfx942:xnack-`, wave64 execution, one `[64, 1, 1]` workgroup with
maximum flat size 64, zero static and dynamic LDS, and the exact subgroup,
matrix-multiply, AMD-wave, and AMD-MFMA capability declarations. Its four
logical slices are `A: &[u16]`, `B: &[u16]`, `C: &[f32]`, and
`D: DisjointSlice<f32>` with exact pointer/length physical offsets
`0, 8, ..., 56`. The kernarg span is 64 explicit bytes followed by the
256-byte COV6 implicit suffix, for 320 bytes total.

The `u16` scalar and layout records establish only the storage-level type of A
and B. The capability and build-evidence records are declarations. Descriptor
admission does not inspect a kernel body, authenticate code origin, or prove
BF16 or MFMA instruction semantics.

The 288-byte fragment-level frontend evidence probe is a different profile. It
describes eight BF16 fragments and four F32 fragments as by-value arguments and
has only 32 explicit bytes. Its constants remain available to describe that
frontend boundary, but the structural tiled GEMM admission API deliberately
rejects it. The 288-byte and 320-byte profiles are never interchangeable.

The expected source and executable-IR evidence supplied to admission must
match exactly and capability omission or substitution fails closed. These V1
evidence records are still caller-provided declarations; successful admission
does not authenticate their compiler origin or grant publication, load, or
launch authority.

## Row-softmax V1 structural descriptor profile

`admit_row_softmax_v1_structural_descriptor_v1` is an independent sealed
profile for the intended fixed one-row, 64-element F32 row-softmax boundary.
It requires COV6 on `gfx942:xnack-`, one wave64 `[64, 1, 1]` workgroup, a
declared maximum grid of `[1, 1, 1]`, zero static and dynamic LDS, and exactly
the subgroup and AMD-wave capability declarations. Its ABI is
`input: &[f32]` followed by `output: DisjointSlice<f32>`, with pointer/length
pairs at offsets `0/8` and `16/24`. The explicit span is 32 bytes and the COV6
implicit suffix is 256 bytes, producing a 288-byte kernarg segment.

V1 slice descriptors encode the presence and physical location of each `u64`
length, not its runtime value. The constant row length of 64 is therefore a
host-profile requirement that this structural admission cannot validate.
`ROW_SOFTMAX_V1_INTENDED_HOST_ROW_ELEMENTS` names that external requirement;
admitted descriptor evidence deliberately has no row-length accessor.
Likewise, unique output ownership is an unauthenticated descriptor declaration;
it does not prove runtime non-aliasing or race freedom.

Admission checks no instruction body. It does not establish that the entry
computes a maximum, calls or implements `exp`, reduces a denominator,
normalizes outputs, handles NaN or infinity in any specified way, meets an
error budget, or computes functional softmax at all. It also does not
authenticate source/compiler origin, prove Verus verification, or grant
publication, load, or launch authority.

## V2 target requirements

V2 binds each kernel ID to declared static and maximum dynamic LDS bytes, one
exact wavefront width, a cooperative-launch flag, closed synchronization bits,
and closed atomic-scope bits. The synchronization bits cover wave and
workgroup barriers plus workgroup, device, and system fences. Atomic bits cover
workgroup, device, and system scopes. Unknown bits and tags are rejected.

Construction and decoding derive `AmdTargetCapabilities` from the V1 target
and reject an unsupported wave width, excessive maximum LDS allocation, or an
unsupported atomic/fence scope. The duplicated LDS values must exactly match
the V1 launch constraints. Requirements must also agree with the broad V1
capability declarations: exact waves require AMD-wave, barriers require their
subgroup or workgroup-memory capability, and atomics/fences require atomics.
There must be exactly one canonically ordered requirements record per kernel;
duplicates, missing records, and dangling kernel IDs fail closed.

These checks compare declarations with a processor model. They do not observe
hardware or prove executable behavior. Cooperative launch always requires a
HIP device-attribute and occupancy check. System-scope atomics and fences also
require evidence that the actual allocation and mapping are eligible. The host
must still authenticate the descriptor against the loaded code object and the
observed device before launch.

The V2 fixed header is:

```text
offset  size  field
0       8     magic = "FE2O3KD\0"
8       2     version = 2
10      2     flags = 0
12      4     total byte length
16      4     embedded canonical V1 byte length
20      2     target-requirement count
22      2     reserved = 0
24      n     complete canonical V1 table
```

Each following 48-byte record contains `kernel_id[32]`, static and maximum
dynamic LDS as `u32`, exact wave tag (`1` wave32, `2` wave64), cooperative tag
(`0` or `1`), synchronization bits `u16`, atomic bits `u16`, and a zero
`reserved:u16`. The embedded canonical code-object digest begins at the outer
offset `CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V2` (40). The complete V2 table
shares the 256 KiB table bound.

## Canonical code-object digest

The 32-byte `CanonicalCodeObjectDigest` is the only self-referential field and
has a fixed offset, `CANONICAL_CODE_OBJECT_DIGEST_OFFSET`, in the table header.
A later ELF-aware linker step will:

1. place exactly one canonical table at a trusted, independently validated
   location in the final HSACO;
2. zero only that fixed 32-byte field in the complete final HSACO;
3. calculate
   `SHA256("FE2O3/AMDHSA-CODE-OBJECT/V1\0" || u64_le(file_len) || bytes)`;
4. patch the result into the field and independently revalidate the output.

This crate defines the digest domain and operation over already-canonicalized
bytes. It does not parse ELF, locate a section, choose a zeroing range, or grant
trust. The canonical digest is intentionally distinct from the raw artifact
payload digest. A container payload digest hashes the literal final bytes for
transport integrity; the canonical digest binds a trusted table to those bytes
without a circular hash.

## Canonical encoding

Integers are little-endian. Names and text use a `u16` byte length followed by
nonempty printable ASCII without NUL. Records use explicit stable tags; Rust
enum discriminants, `Debug` output, `TypeId`, and pretty-printed Rust paths are
never canonical data. Type records, layout records, kernels, and capability
sets are sorted and unique. Arguments remain in source order and their physical
components remain in increasing offset order. Padding gaps, including leading
and inter-argument gaps, are allowed; overlap, reversal, and gaps between a
slice pointer and its length are rejected.

All identity domains hash exactly
`domain || u64_le(payload_len) || payload` with SHA-256. The public domain
constants are normative and include their terminating NUL byte.

The V1 domains are:

```text
FE2O3/RUST-TYPE/V1\0
FE2O3/DEVICE-LAYOUT/V1\0
FE2O3/KERNEL-DESCRIPTOR/V1\0
FE2O3/DEVICE-DESCRIPTOR-TABLE/V1\0
FE2O3/AMDHSA-CODE-OBJECT/V1\0
```

`RustTypeIdentity` hashes the four encoded source-type descriptor bytes.
`DeviceLayoutIdentity` hashes the twelve encoded device-layout descriptor
bytes. `KernelDescriptorDigest` hashes one complete encoded kernel record.
`DeviceDescriptorTableDigest` hashes the complete table, including its
canonical code-object digest field. These digest types and domains are not
interchangeable.

## Wire layout

The fixed table header is:

```text
offset  size  field
0       8     magic = "FE2O3KD\0"
8       2     version = 1
10      2     flags = 0
12      4     total byte length
16      32    CanonicalCodeObjectDigest
48      1     code-object version tag: 4, 5, or 6
49      1     pointer width in bytes = 8
50      1     endianness tag = 1 (little-endian)
51      1     reserved = 0
```

It is followed by length-prefixed compiler name and release, a 20-byte
compiler commit identity, length-prefixed producer name and version, the
length-prefixed canonical `DeviceTargetV1` spelling, `u16` type/layout/kernel
counts, and a zero `u16` reserved field. `DeviceTargetV1` is backed by
`fe2o3_amd_target::AmdTargetId`; decode parses it and requires its input bytes
to equal the canonical `Display` spelling, including canonical feature order.
It remains an untrusted declaration rather than observed-device evidence.
Type records, layout records, and kernels follow in that order.

A source-type record is its 32-byte identity followed by `kind:u8`,
`scalar:u8`, and zero `flags:u16`. Kinds are scalar `1`, shared slice `2`, and
`DisjointSlice` `3`. Scalar tags are `i8=1`, `u8=2`, `i16=3`, `u16=4`, `i32=5`,
`u32=6`, `i64=7`, `u64=8`, `f16=9`, `f32=10`, and `f64=11`.

A device-layout record is its 32-byte identity followed by kind and scalar
tags, `size:u16`, `alignment:u16`, `pointer_width:u8`, `length_width:u8`, zero
`flags:u16`, and zero `reserved:u16`. Scalar size and alignment equal the
scalar width and both width fields are zero. Slice size/alignment are 16/8 and
both width fields are 8.

Each kernel contains, in order:

```text
kernel_id[32]
logical_name:name, entry_name:name, descriptor_symbol:name
source evidence, executable-IR evidence
capability_count:u16, sorted capability tags:u16[]
launch constraints
argument_count:u16, physical_component_count:u16
explicit_argument_size:u32, kernarg_segment_size:u32
kernarg_segment_alignment:u32
ordered arguments[]
```

Each evidence record has its required evidence-kind tag (`1` source, `2`
executable IR), identity-scheme tag `1`, SHA-256 algorithm tag `1`, a zero
reserved byte, opaque identity bytes `[u8; 32]`, and opaque digest bytes
`[u8; 32]`. Identity scheme 1 is an opaque, producer-namespaced declaration.
V1 defines no verifiable preimage for it or for the declared SHA-256 digest;
those bytes are not recomputed by this crate and cannot grant Verus authority.

Capability tags `1..=11` are, respectively, subgroup, ballot, shuffle,
workgroup memory, matrix multiply, async copy, atomics, AMD wave, AMD MFMA,
AMD WMMA, and AMD DS permute. Launch constraints encode rank, block policy
(`0` any, `1` exact, `2` at most), block dimensions, maximum grid dimensions,
maximum flat workgroup size, and static/dynamic shared-memory limits. All
launch integers other than rank and block policy are `u32`; flags are zero.
Every capability tag is an independent requirement. V1 does not imply
dependency closure between tags. A later trusted compiler integration must
derive the requirements from executable IR and match them against the observed
target rather than trusting these declarations.

The kernel ABI layout bounds all explicit physical components. Its explicit
size is canonically the exact end of the final physical component, or zero when
there are no components. It must not exceed the complete kernarg segment size,
which is capped at 1 MiB. Segment alignment is a nonzero power of two no larger
than that ceiling. Every component has alignment no greater than the segment
alignment. Padding after the explicit component span and implicit argument
bytes are represented only by the larger complete segment size. The complete
segment size is deliberately not required to be a multiple of alignment
because valid LLVM V4 metadata may report a rounded or overridden size with
that property.

An argument contains `source_index:u16`, zero flags, its name, source-type and
device-layout identities, ownership/access/alias tags, one zero reserved byte,
`component_count:u16`, one zero `reserved:u16`, and its components. Ownership
tags are by-value `1`, shared `2`, unique `3`; access tags are by-value `1`,
read-only `2`, write-only `3`, read-write `4`; alias tags are value `1`, shared
read-only `2`, exclusive `3`.

A 16-byte physical component contains `kind:u8`, `scalar:u8`, access and alias
tags, `offset:u32`, `size:u16`, `alignment:u16`, zero `flags:u16`, and zero
`reserved:u16`. Kinds are scalar by-value `1`, global pointer `2`, and `u64`
slice length `3`. A global pointer uses scalar tag zero. A slice length uses the
`u64` scalar tag.

The complete table is at most 256 KiB, and each complete kernarg segment is at
most 1 MiB. V1 permits at most 128 kernels, 64
arguments and 128 physical components per kernel, 256 type records, 256 layout
records, 64 capabilities, 128 bytes per name, and 256 bytes per text field.
Count and length bounds are checked before allocation.
