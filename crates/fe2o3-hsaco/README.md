# fe2o3-hsaco

`fe2o3-hsaco` performs bounded, read-only inspection of AMDGPU HSA code
objects. It accepts untrusted bytes, validates the ELF envelope, locates the
AMDGPU metadata note, and exposes the target and physical kernel argument
metadata needed by a later ABI matcher.

Inspection is descriptive metadata evidence only. An `InspectedHsaco` does not
authorize module loading or kernel launch, and it does not establish that a
metadata kernel name or `.symbol` agrees with an ELF symbol, kernel descriptor,
or executable entry point. A loader must still bind those structures together
with the payload digest, compiler descriptor, manifest ABI, observed device,
HIP module, resolved function, and context.

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

Argument maps accept the keys documented by the local LLVM AMDGPU usage schema,
including the deprecated and unused `.value_type` compatibility key. The
inspector also accepts and preserves the older producer extension `.align`.
Unknown argument keys are rejected because a future key may change the physical
ABI. Optional qualifiers preserve whether the producer emitted them.

Register requirements, spill counts, workgroup processor mode, dynamic-stack
use, and temporary GFX1250-family revision data are retained as execution
evidence. Source-language declarations and optimization hints are only
syntax-validated because they do not describe executable resource use or
launch behavior.
