# fe2o3-amdhsa-loader

`fe2o3-amdhsa-loader` is the bounded pure-Rust foundation for issue #137's R3
loader. It accepts an untrusted byte slice and either returns a canonical,
inert load plan or rejects the object. A lifetime-bound validated envelope can
deterministically materialize that plan into an exact caller-provided,
exclusively borrowed byte slice.
The crate performs no file access, GPU mapping, permission transition, or
dispatch. Envelope planning and materialization allocate nothing; the optional
semantic closure composes the repository's bounded, allocating
`fe2o3-hsaco` inspector instead of maintaining a second metadata parser.

## Admitted foundation profile

The first profile is intentionally the envelope currently emitted by fe2o3's
pinned LLVM/LLD finalizer:

- ELF64, little-endian, AMDGPU HSA OSABI, ABI byte 4 (COV6), `ET_DYN`, and
  `EM_AMDGPU`;
- zero ELF entry point and exact `e_flags = 0x64c`, meaning `gfx942`, XNACK
  disabled, and SRAM-ECC unspecified;
- exactly one each of the reviewed `PT_PHDR`, `PT_DYNAMIC`, `PT_NOTE`,
  `PT_GNU_STACK`, and `PT_GNU_RELRO` records, plus exactly three `PT_LOAD`
  records with `R`, `R|X`, and `R|W` permissions;
- 4 KiB-congruent load segments, checked file and virtual ranges, exact
  descriptor file-to-virtual translation deltas, checked page-rounded mapping
  ranges, no overlap, and no writable-executable load;
- one bounded `AMDGPU` / `NT_AMDGPU_METADATA` note;
- the finalizer's non-relocating dynamic-tag profile. Relocation sections,
  relocation tags, dependencies, constructors, and unknown dynamic features
  reject explicitly.

The returned segments are sorted by virtual address independent of program
header order. Every range and page rounding is checked before it is exposed.
The plan is validated data only; it grants no load or launch authority.

## Content-bound envelope and safe materialization

`validate(&bytes, profile)` returns a `ValidatedEnvelope<'_>` whose private
fields permanently associate the canonical plan with that exact input borrow.
There is no public constructor from a `LoadPlan` and a byte slice. Segment
sources are exact borrowed subslices selected with `SegmentOrdinal`, and the
metadata descriptor is exposed as an exact borrowed subslice without decoding
it. The original `plan()` API remains an inert, copyable description and does
not itself prove any byte association.

The envelope also provides checked materialization instructions in two explicit
phases. The first phase covers every complete page-rounded segment mapping and
every gap between adjacent mappings; those ranges form an exact, gap-free cover
of the full image span. Per-segment descriptions identify the page prefix,
in-memory suffix (including BSS), and page-rounding tail that stay zero. The
second phase associates each exact borrowed file range with its checked
image-relative, prefix-adjusted destination.

`ValidatedEnvelope::materialize_into` requires a caller-provided destination
that is exclusively borrowed for the call and whose length exactly equals the
checked image span. It rechecks all three structured copy ranges before
mutation, zeros the complete destination, and then copies the exact borrowed
`PT_LOAD` sources in canonical virtual-address order. Wrong lengths reject
without changing the destination. The method allocates nothing and cannot
substitute unrelated source bytes because the sources remain tied to the
validated envelope's input lifetime.

The resulting bytes are a CPU-side image only. The crate owns no syscall or
device handle and grants no allocation identity, raw-address authority, GPU
mapping, permission, W^X, relocation, symbol, kernel, loaded-code, launch, or
execution authority. A later adapter must bind the image to one allocation
without substitution and enforce the remaining lifecycle.

## Exact semantic and selected-kernel closure

`ValidatedEnvelope::bind_kernel` consumes the content-bound envelope and runs
the repository's existing bounded `fe2o3-hsaco` MessagePack, symbol, descriptor,
and resource inspector over the same retained byte slice. The composition
requires COV6 metadata 1.2 for exact `gfx942:xnack-`; rejects unknown metadata
fields and malformed or over-limit documents through the inspector; and
requires both parsers to identify the same physical metadata descriptor offset
and length. `printf` roots, init/fini kernels, dynamic stacks, and device
enqueue reject because their runtime lifecycle is absent from this slice.

Every metadata kernel must bind to one bounded static ELF descriptor and entry
symbol before one exact metadata kernel name can be selected. The selected
64-byte descriptor must translate into exactly one canonical read-only load;
the complete nonempty entry-symbol range must independently translate into
exactly one canonical read-execute load. Descriptor fields and encoded
register capacities are cross-checked against metadata by `fe2o3-hsaco`.
`SelectedKernelResourceBindingV1` retains the selected kernarg, group/private
memory, wavefront, register, spill, workgroup, cluster, and raw descriptor
evidence without turning it into launch authority.

Successful closure returns `ClosedRelocationEvidenceV1`. Its private
construction records that envelope validation admitted no `SHT_REL` or
`SHT_RELA` section, no dynamic relocation tag, and no unknown section or
dynamic-tag extension. The policy applies zero relocations; it does not provide
a relocation engine.

`KernelIdentityInputsV1` hashes the exact input object, physical metadata
descriptor, selected descriptor, and selected entry-symbol bytes. A
domain-separated, length-delimited closure digest additionally binds the loader
and relocation profiles, physical ranges and addresses, metadata kernel index,
name, and symbol. These deterministic values are identity inputs for a later
loaded-kernel authority; they are neither an authenticated compiler identity
nor proof that the entry implements its source semantics.

## Proof and implementation gaps

- The authenticated runtime-model Verus lane proves a narrow abstract relation
  for segment rounding, span, non-overlap, canonical permission shape, and
  descriptor equal-delta binding. It does not prove refinement from this
  executable byte parser or the materialization instructions to that relation;
  these checks remain covered by hostile unit tests rather than an executable
  Verus refinement proof.
- The executable parsers now bind the exact metadata note, schema and target,
  every metadata kernel's static descriptor/entry symbols, descriptor resources,
  and one deterministic selected-kernel identity. Verus does not yet prove
  refinement from those executable checks to a semantic loader model.
- Static symbols and the complete selected entry-symbol byte range are bound,
  but no disassembler, control-flow closure, machine-code verifier, source-to-
  ISA refinement, undefined-symbol policy beyond the no-relocation envelope,
  or code/data origin map is claimed.
- Relocations are rejected rather than executed. No relocated byte-image
  refinement is claimed.
- Borrow identity prevents construction from a plan and unrelated bytes, and
  closure hashes detect content substitution when a later authority rechecks
  them. A SHA-256 value is not a signature, trusted producer identity, or proof
  about bytes after an unchecked external copy.
- The executable zero/copy method is not proved to refine the runtime-model
  Verus materialization relation. Allocation identity, mapping permissions, W^X
  lifecycle enforcement, immutable loaded-image transition, compiler/manifest
  ABI binding, observed-device compatibility, KFD/HSA comparison, dispatch
  packet construction, and hardware behavior remain outside this crate.
