# fe2o3 AMD target IDs

`fe2o3-amd-target` parses the concrete AMDGPU processor and feature portion of
an AMD target ID, such as `gfx942:sramecc+:xnack-`. It has no runtime or FFI
dependencies and is usable without `std`.

The accepted grammar is deliberately narrow:

- the processor must be a known, canonical, lowercase, concrete `gfx` name;
- generic family processors and marketing aliases are rejected;
- the only feature modifiers are `xnack+`, `xnack-`, `sramecc+`, and
  `sramecc-`;
- each modifier must be configurable for the selected concrete processor;
- each feature may occur at most once; and
- formatting emits features in AMD's canonical `sramecc`, then `xnack` order.

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
to `llvm/include/llvm/BinaryFormat/ELF.h` at the same revision. The crate does
not accept generic processors because their family-provision semantics are
outside this exact-processor compatibility model.
