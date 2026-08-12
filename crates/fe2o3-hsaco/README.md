# fe2o3-hsaco

`fe2o3-hsaco` performs bounded, read-only inspection of AMDGPU HSA code
objects. It accepts untrusted bytes, validates the ELF envelope, locates the
AMDGPU metadata note, and exposes the target and physical kernel argument
metadata needed by a later ABI matcher.

Inspection is descriptive metadata evidence only. An `InspectedHsaco` does not
authorize module loading or kernel launch, and it does not establish that a
metadata kernel name or `.symbol` agrees with an ELF symbol, kernel descriptor,
or executable entry point.

`inspect_and_bind_kernel_descriptors` is a separate, fail-closed operation for
callers that need that additional descriptive relationship. For every metadata
kernel, it requires one exact `.symbol` match in bounded `SHT_SYMTAB` data and
one exact `.name` match. The former must be a 64-byte `STT_OBJECT` in a
64-byte-aligned, read-only allocated section and `PT_LOAD`; the latter must be
a nonempty, 256-byte-aligned `STT_FUNC` in a read-only executable section and
`PT_LOAD`. Both symbols must use the same binding, chosen from the pinned LLVM
set `STB_GLOBAL` or `STB_WEAK`; mixed global/weak pairs reject. Section and
segment address-to-file mappings must agree. Exactly one file-backed `PT_LOAD`
may intersect each requested virtual range; additional `p_memsz` intersections,
including zero-fill ranges and ranges with inappropriate permissions, reject.
`SHT_DYNSYM` is deliberately not identity evidence: a name that also occurs
there does not create a second static-symbol binding.

The static symbol scan is capped at 32,768 records. Symbol tables must use the
ELF64 24-byte record layout, have integral bounded counts, link to a bounded
`SHT_STRTAB`, contain a valid null symbol and local partition, and give every
record a bounded NUL-terminated UTF-8 name. Extended symbol section indexes are
not accepted.

The descriptor layout and resource masks are pinned to LLVM revision
`846473237377990d00b9c353f6a2c86116b52ea5`, specifically
`llvm/include/llvm/Support/AMDHSAKernelDescriptor.h`,
`llvm/lib/Target/AMDGPU/AMDGPUAsmPrinter.cpp`, `SIProgramInfo.cpp`, and the
AMDGPU disassembler. Binding rejects nonzero reserved bytes and bits,
unsupported kernarg preload, inappropriate target-specific resource bits, and
unchecked entry arithmetic. A signed entry displacement is accepted only when
`descriptor_address + displacement` can be computed without overflow and is
exactly the `STT_FUNC` address.

The binder exactly cross-checks group/private/kernarg sizes, wave32/wave64,
V5/V6 dynamic-stack use, private-segment enablement, emitted WGP mode, HSA-fixed
resource bits, and whether encoded register capacity can contain the metadata
counts. The SGPR checks use the pinned LLVM emitter's 8-register block encoding
through GFX9, bounded by the documented 112-register maximum, and the documented
128-register allocation limit for GFX10-GFX12. Targets without a pinned SGPR
limit fail closed. Processors
carrying LLVM's `FeatureArchitectedFlatScratch` must not request the legacy
private-segment-buffer or flat-scratch-init kernel properties. GFX90A-family
accumulator offset is also derived exactly from the total VGPR and AGPR counts.
Register block counts can be larger than the reported count because LLVM can
raise allocation for occupancy constraints. FP modes, user-SGPR composition,
workitem/workgroup system-register enables, forward progress, non-GFX90A AGPR
packing, instruction prefetch, and other target resource choices remain raw
descriptive fields because metadata does not uniquely determine them.

`InspectedKernelBindings` still carries no authority. A loader must bind this
description together with the payload digest, compiler descriptor, manifest
ABI, observed device, HIP module, resolved function, and context before launch.

The inspected implicit-argument offset and size preserve the complete,
4-byte-rounded span declared by `.kernarg_segment_size`, including reserved and
trailing bytes not named by metadata records. LLVM permits
`amdgpu-implicitarg-num-bytes` overrides. V4 metadata emits another hidden
record only at each 8-byte threshold, so a nonzero rounded span can validly
contain no record. The inspector validates those thresholds without assuming a
universal 56-byte V4 or 256-byte V5/V6 profile. A later authority must bind the
span and hidden-record profile to a trusted compiler descriptor before using it
to construct a dispatch.

The parser accepts AMDGPU HSA code object versions 4 through 6. It requires
metadata version 1.1 for code object V4 and version 1.2 for V5 or V6. MessagePack
is pre-scanned iteratively with explicit size, depth, node, string, collection,
kernel, and argument limits before the bounded value tree is decoded.
AMDHSA does not require a minimal or otherwise canonical MessagePack encoding.
Systems that need an exact metadata identity must hash the original metadata
descriptor bytes; re-encoding this descriptive model is not an identity
operation.

Physical argument names are observations from the code object. This crate does
not infer logical slices or aggregates from adjacent pieces, and it does not
assume that manifest logical names equal physical metadata names.

Argument maps accept the keys documented by the local LLVM AMDGPU usage schema.
The deprecated `.value_type` compatibility key is normalized into a closed
enum and preserved when present, so exact consumers can reject contradictory
type declarations. The inspector also accepts and preserves the older producer
extension `.align`. Unknown argument keys and value-type spellings are
rejected because they may change physical ABI semantics. Optional qualifiers
preserve whether the producer emitted them.

Register requirements, spill counts, workgroup processor mode, dynamic-stack
use, and temporary GFX1250-family revision data are retained as execution
evidence. Source-language declarations and optimization hints are only
syntax-validated because they do not describe executable resource use or
launch behavior.
