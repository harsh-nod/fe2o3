# fe2o3 AMD target IDs

`fe2o3-amd-target` parses the concrete AMDGPU processor and feature portion of
an AMD target ID, such as `gfx942:sramecc+:xnack-`. It has no runtime or FFI
dependencies and is usable without `std`.

The crate also adapts reviewed AMD production profiles into
`fe2o3-target-spec`'s target-neutral `TargetProfileSpecV1`. Generic compiler,
proof, and host layers can commit to that portable target profile while this
crate continues to own AMD target-ID parsing and AMD capability facts.

The accepted grammar is deliberately narrow:

- the processor must be a known, canonical, lowercase, concrete `gfx` name;
- generic family processors and marketing aliases are rejected;
- the only feature modifiers are `xnack+`, `xnack-`, `sramecc+`, and
  `sramecc-`;
- each modifier must be configurable for the selected concrete processor;
- each feature may occur at most once; and
- formatting emits features in AMD's canonical `sramecc`, then `xnack` order.

## Resolved target identity V2

`resolve_amd_target_v2` creates one bounded, canonical target identity from an
explicit override or injected device observations. An override has absolute
precedence: when present, the resolver never queries detection, and an invalid
override returns its parse or capability error without falling back. Override
feature order is normalized through the existing `AmdTargetId` API. An omitted
supported override feature remains `unspecified`, which preserves the existing
code-object `Any` meaning.

Without an override, the resolver accepts at most 64 observations of at most 64
bytes each. Every detected target is parsed and capability checked. A detected
target must report an explicit enabled or disabled state for every target-ID
feature configurable by that architecture. Multiple identical targets are
accepted, including equivalent target IDs with different input feature order.
Different architectures or feature states are ambiguous and fail closed. Zero
devices, too many devices, malformed observations, incomplete feature states,
and detector errors are distinct failures.

Detection is injected through `AmdTargetDetectionV2`; this crate does not spawn
commands, parse shell output, read environment variables, or choose a runtime.
An adapter is expected to query an in-process driver or runtime API and retain
stable device indices for one resolution call. No HSA adapter or shared
consumer is wired in this V2 lane yet.

`ResolvedAmdTargetIdentityV2` binds the concrete architecture, SRAM ECC and
XNACK states, and selection source (`detected` or `override`). Its fixed-capacity
canonical bytes are versioned and field ordered. `canonical_digest` is SHA-256
over exactly those bytes. Decoding accepts only records that reproduce the same
canonical bytes, and detected records with unspecified configurable features
remain invalid.

The V2 identity is normalized configuration, not attestation. An override does
not prove device presence. Injected observations do not prove that the adapter
queried trustworthy hardware, and identical observations do not bind a
particular device ordinal. Runtime admission must separately authenticate its
query path, selected device, executable target, and compatibility decision.

## Target capabilities

`AmdTargetId::capabilities` derives a canonical V1 capability record from the
concrete processor and its target-ID features. The record includes supported
and default wavefront widths, the maximum addressable LDS bytes for one
workgroup, lowerable atomic scopes, and narrowly named matrix and VMEM/LDS copy
instruction families. Its `Display` implementation and `encode_canonical`
method emit the same deterministic text.

Additional typed queries describe reviewed source-contract prerequisites for
workgroup dimensions, standard Rust atomics, native split barriers, FP8 and MX
formats, MFMA numerical families, LDS transpose loads, device diagnostics, and
launch-bounds metadata. These queries carry a conservative complete `gfx942`
profile and a deliberately partial `gfx950` low-precision profile. Unreviewed
decisions fail closed with an explicit `unreviewed` status plus empty positive
projections. This is distinct from `unsupported`, which is a reviewed negative
fact for a specific target. The older broad target queries retain their
existing behavior.

The advanced queries do not add fields to the V1 canonical text encoding. The
encoding remains byte-for-byte compatible and identifies the exact target from
which these deterministic queries are derived. The separate
`AdvancedCapabilityModelIdentity` binds that target to the explicit advanced
model revision for future proof and admission identities. A future serialized
capability record must use a new explicit version rather than reinterpreting
V1.

The model deliberately distinguishes target facts from runtime facts.
Cooperative launch is always `runtime-evidence`: HIP's observed device
attribute and an occupancy-safe launch must authorize it. System-scope atomic
lowering is a target fact, but a system-scope operation additionally needs
runtime evidence that its allocation and mapping are eligible. An instruction
family is not itself a complete high-level matrix or asynchronous-copy
contract.

The `gfx942` profile admits wave64 workgroups of at most 1024 work-items and
reviewed 8-, 16-, 32-, and 64-bit standard atomic legalizability for selected
complete operation/address-space/scope/ordering tuples. Subword and generic
address-space legalizability may require CAS loops or flat operations;
legalizability never claims a machine-native atomic instruction. Signed and
unsigned min/max are distinct operations. The profile also admits FNUZ
E4M3/E5M2 encodings and a bounded set of MFMA numerical families. It rejects
native split barriers, OCP FP8, MX formats, and profiling markers. AMD
flat-workgroup-size and waves-per-EU metadata are target facts, but
minimum-workgroups-per-compute-unit remains unsupported until there is a
reviewed occupancy translation. Device printf and debug-trap observation
require runtime evidence. These facts do not authorize a source operation:
exact matrix shapes/layouts, linked device libraries, and launch admission
remain separate checks.

The partial `gfx950` profile admits OCP E4M3/E5M2 FP8, MXFP8/MXBF8/MXFP4,
scaled `f8f6f4` MFMA, and the format-specific `ds_read_b64_tr_b4`,
`ds_read_b64_tr_b8`, and `ds_read_b64_tr_b16` transpose loads. Other advanced
gfx950 areas remain `unreviewed`; this profile does not inherit unrelated
gfx942 atomic, diagnostic, launch-bound, or workgroup-limit decisions.

```rust
use fe2o3_amd_target::{AmdTargetId, CapabilitySupport, WavefrontWidth};

let target: AmdTargetId = "gfx942".parse()?;
let capabilities = target.capabilities()?;
assert_eq!(capabilities.default_wavefront_width(), WavefrontWidth::Wave64);
assert_eq!(capabilities.max_lds_bytes_per_workgroup(), 64 * 1024);
assert_eq!(
    capabilities.cooperative_launch(),
    CapabilitySupport::RequiresRuntimeEvidence,
);
# Ok::<(), Box<dyn core::error::Error>>(())
```

```rust
use fe2o3_amd_target::{AmdTargetId, FeatureState};

let target: AmdTargetId = "gfx942:xnack-:sramecc+".parse()?;
assert_eq!(target.processor(), "gfx942");
assert_eq!(target.sramecc(), Some(FeatureState::Enabled));
assert_eq!(target.to_string(), "gfx942:sramecc+:xnack-");
# Ok::<(), fe2o3_amd_target::ParseAmdTargetIdError>(())
```

## Compatibility direction

`AmdTargetId::is_compatible_with_observed` treats `self` as an artifact target
declaration and its argument as an observed device target. An omitted artifact
feature means `Any`; an explicit artifact state requires the same explicit
observed state. An omitted observed state therefore cannot satisfy an explicit
artifact requirement.

```rust
use fe2o3_amd_target::AmdTargetId;

let artifact: AmdTargetId = "gfx942".parse()?;
let observed: AmdTargetId = "gfx942:sramecc+:xnack-".parse()?;
assert!(artifact.is_compatible_with_observed(&observed));
assert!(!observed.is_compatible_with_observed(&artifact));
# Ok::<(), fe2o3_amd_target::ParseAmdTargetIdError>(())
```

## Trust boundary

An `AmdTargetId` is parsed text, not hardware evidence. Parsing does not prove
that a processor is present, that a feature is implemented by that processor,
or that a code object was compiled for the declaration. Compatibility compares
two declarations only. A runtime loader must obtain its observed target from a
trusted device query, inspect or load the actual code object, and keep any
resulting loading authority in a separate unforgeable type.

The processor and feature-support tables follow
`llvm/include/llvm/TargetParser/AMDGPUTargetParser.def` at LLVM revision
`846473237377990d00b9c353f6a2c86116b52ea5` and must be reviewed when that
source changes. `AmdTargetId::amdhsa_elf_flags_v4_plus` is additionally pinned
to `llvm/include/llvm/BinaryFormat/ELF.h` at the same revision. Capability
profiles are pinned to `llvm/lib/Target/AMDGPU/AMDGPU.td`, `GCNProcessors.td`,
`GCNSubtarget.h`, and `Utils/AMDGPUBaseInfo.cpp` at that revision. The crate
does not accept generic processors because their family-provision semantics
are outside this exact-processor compatibility model.
