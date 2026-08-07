#![no_std]
#![forbid(unsafe_code)]

//! Target-neutral vocabulary for describing a kernel's launch, memory indexing,
//! and verification artifacts.
//!
//! This is intentionally a small contract layer, not a SIMT execution model.

mod artifact;
mod control_flow_v1;
mod index;
mod kernel_frontend_v1;
mod launch;
mod memory_v1;

pub use artifact::{
    ArtifactDigest, ArtifactIdentity, KernelIdentity, ProofArtifact, ProofIdentity, ProofStatus,
    ToolIdentity,
};
pub use control_flow_v1::{
    ControlFlowContractErrorV1, IntegerSwitchCaseV1, IntegerSwitchTypeV1, LoopBoundV1,
    MAX_INTEGER_SWITCH_CASES_V1, MAX_SOURCE_INTEGER_SWITCHES_V1, MAX_SOURCE_LOOPS_V1,
    StructuredTransferKindV1, UnsupportedControlFlowV1,
};
pub use index::{BoundedIndex, IdentityWriteIndex};
pub use kernel_frontend_v1::{
    AssemblyEffectSetV1, AssemblyOperandSetV1, AssemblyOptionSetV1, KernelFrontendContractErrorV1,
    KernelFrontendContractV1, LaunchBoundsV1, MAX_RESIDENT_WORKGROUPS_PER_COMPUTE_UNIT_V1,
    MAX_WORKGROUP_THREADS_V1, UnsafeAssemblyDeclarationV1, UnsafeAssemblyTargetV1,
    WorkgroupDimensionsV1,
};
pub use launch::{LaunchDomain1d, LaunchGeometry1d, ThreadId1d, ThreadInDomain1d};
pub use memory_v1::{
    AccessKindV1, AddressSpaceIdV1, AffineWriteMappingV1, AllocationProvenanceIdV1,
    AllocationSpecV1, BrandedLaunchDomain1dV1, BrandedThreadId1dV1, ByteRegionV1,
    IndependentThreadContractV1, IndependentThreadFactsV1, InitializationStateV1, LaunchIdentityV1,
    MAX_ALLOCATION_BYTES_V1, MAX_LAUNCH_THREADS_V1, MAX_READ_BINDINGS_V1, ObligationFailureV1,
    ObligationKindV1, ObligationResultV1, PermissionKindV1, ProofObligationV1, RegionBindingV1,
    RegionCapabilityV1, RegionPermissionV1, SpecificationFactV1,
};
