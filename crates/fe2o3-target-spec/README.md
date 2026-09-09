# fe2o3 target spec

`fe2o3-target-spec` defines target-neutral profile metadata and semantic
capability queries without importing a vendor runtime, linker, or lowering
model. It is `no_std`; production target crates implement the query contract
and return immutable, replayable decisions.

The crate owns the reusable shape of a target profile:

- vendor and architecture family;
- exact architecture spelling;
- optional rustc and LLVM target spellings;
- expected artifact format;
- execution model;
- optional data-layout text; and
- a sorted list of feature states.

`TargetCapabilityQueryV1` separately asks whether one complete requirement is
supported. Requirements cover scalar types, subgroup size, logical address
spaces and access, atomic operations with success/failure ordering and memory
scope, barriers and their participation contract, fences, collectives and
their participation and numerical contracts, matrix/tensor instruction tiles,
asynchronous transfer and completion, numerical behavior, resource limits,
kernel ABI, and profile-relative object format. A decision is bound to the
exact profile and capability-model revision and reports `Supported`,
`Unsupported`, `Incomplete`, `Unreviewed`, or required dynamic launch evidence.
The query boundary validates both the model identity and target-independent
semantics before consulting the provider. Invalid load/store/fence or
compare-exchange ordering therefore cannot become a target decision.

`TargetCapabilityClosureSpecV1` records canonical roots and a complete,
strictly ordered dependency graph. The bounded, allocation-free closure query
rejects duplicate or unsorted records, missing roots or dependencies, cycles,
unreachable nodes, omitted provider answers, nondeterministic answers,
unsupported operations, and incomplete or unreviewed facts. Launch-dependent
answers can be retained for a later exact evidence join, but
`admit_static_target_capability_closure_v1` rejects them. Closure values are
inert evidence inputs and grant no lowering, artifact, publication, load, or
launch authority.

Neutral canonical capability records contain only logical concepts plus opaque
deterministic target-profile and model-revision fingerprints. They contain no
backend names, instruction-set names, runtime names, vendor-specific subgroup
terminology, or numeric address-space IDs. The fingerprints are identity keys,
not attestations or cryptographic digests. Canonical `Display` forms provide
deterministic material for later evidence records; the production trust
boundary must bind them with its owned digest and authenticated target-selection
evidence.

Vendor crates still own vendor semantics. For example, `fe2o3-amd-target`
continues to parse AMD target IDs, derive AMD capability records, and gate
`gfx942` or `gfx950` production facts. This crate only gives those facts a
portable envelope so generic compiler, proof, and host APIs can refer to target
profiles without depending on AMD-specific types.

`SyntheticConformanceTargetV1` is a reusable portability fixture. Its unusual
subgroup, ABI, matrix, transfer, and resource choices expose assumptions copied
from a production target. It has no compiler target, lowering, artifact writer,
runtime, or launch implementation; a positive query result never claims a
backend exists.

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
