# fe2o3-amdhsa-loader

`fe2o3-amdhsa-loader` is the data-only foundation for issue #137's R3 loader.
It accepts an untrusted byte slice and either returns a canonical, inert load
plan or rejects the object. It performs no file access, allocation, mapping,
copy, relocation, symbol lookup, or dispatch.

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
- 4 KiB-congruent load segments, checked file and virtual ranges, checked
  page-rounded mapping ranges, no overlap, and no writable-executable load;
- one bounded `AMDGPU` / `NT_AMDGPU_METADATA` note;
- the finalizer's non-relocating dynamic-tag profile. Relocation sections,
  relocation tags, dependencies, constructors, and unknown dynamic features
  reject explicitly.

The returned segments are sorted by virtual address independent of program
header order. Every range and page rounding is checked before it is exposed.
The plan is validated data only; it grants no load or launch authority.

`fe2o3-hsaco` remains the repository's existing descriptive MessagePack,
kernel metadata, symbol, and descriptor inspector. This crate does not copy
that parser. It owns the stricter load-planning boundary and records only the
metadata note's file range. A later composition must require both surfaces (or
refactor a shared validated envelope) before granting loader authority.

## Proof and implementation gaps

- These parser checks have hostile unit tests but no Verus refinement proof.
- The metadata descriptor is not decoded here, so its `amdhsa.version` and
  `amdhsa.target` fields are not yet bound to the ELF ABI and flags by this
  crate. The exact ELF-side profile and note identity are checked.
- Static and dynamic symbol contents, undefined symbols, kernel descriptors,
  resource metadata, origin maps, and selected-kernel identity are not loader
  authority in this foundation.
- Relocations are rejected rather than executed. No relocated byte-image
  refinement is claimed.
- Authentication, content identity, allocation, copying, mapping permissions,
  W^X lifecycle enforcement, immutable loaded-image transition, KFD/HSA
  comparison, and hardware behavior remain outside this crate.
