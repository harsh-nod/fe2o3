# fe2o3 target spec

`fe2o3-target-spec` defines target-neutral profile metadata used to describe
the compilation target without importing a vendor runtime, linker, or lowering
model. It is `no_std` and stores borrowed static strings so target crates can
publish reviewed profiles as ordinary constants.

The crate owns the reusable shape of a target profile:

- vendor and architecture family;
- exact architecture spelling;
- optional rustc and LLVM target spellings;
- expected artifact format;
- execution model;
- optional data-layout text; and
- a sorted list of feature states.

Vendor crates still own vendor semantics. For example, `fe2o3-amd-target`
continues to parse AMD target IDs, derive AMD capability records, and gate
`gfx942` or `gfx950` production facts. This crate only gives those facts a
portable envelope so generic compiler, proof, and host APIs can refer to target
profiles without depending on AMD-specific types.

```rust
use fe2o3_target_spec::{
    TargetArchitectureFamilyV1, TargetArtifactFormatV1, TargetExecutionModelV1,
    TargetFeatureSpecV1, TargetFeatureStateV1, TargetProfileSpecV1, TargetVendorV1,
};

const FEATURES: &[TargetFeatureSpecV1] = &[TargetFeatureSpecV1::new_unchecked(
    "fast-math",
    TargetFeatureStateV1::Disabled,
)];

let profile = TargetProfileSpecV1::from_static_parts(
    TargetVendorV1::Nvidia,
    TargetArchitectureFamilyV1::Nvptx,
    "sm_90",
    Some("nvptx64-nvidia-cuda"),
    Some("nvptx64-nvidia-cuda"),
    TargetArtifactFormatV1::PtxText,
    TargetExecutionModelV1::GpuGrid,
    None,
    FEATURES,
);

assert!(profile.validate().is_ok());
assert_eq!(profile.vendor().as_str(), "nvidia");
```
