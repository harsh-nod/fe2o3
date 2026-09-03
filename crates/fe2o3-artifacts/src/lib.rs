#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod abi;
mod binding;
mod bundle;
mod bundle_decode;
mod bundle_encode;
mod container;
mod container_decode;
mod container_encode;
mod decode;
mod digest;
mod direct_link;
mod direct_link_bridge;
mod direct_link_decode;
mod direct_link_encode;
mod encode;
mod error;
mod generated_kernel_identity;
mod gfx942_bundle;
mod host_launch_abi;
mod identities;
mod manifest;
mod proof;
mod proof_decode;
mod proof_encode;
mod proof_executable_binding;
mod rust_layout;
mod selection;

pub use abi::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, MAX_ABI_BYTES, MAX_ABI_FIELDS,
    Mutability, ScalarType, TypeIdentity,
};
pub use binding::{
    MatchedProofEvidenceV1, PROOF_IDENTITY_VERSION, ProofMatchError, ProofMatchPolicy,
    ProofTargetError, V1_REQUIRED_PROPERTIES,
};
pub use bundle::{
    BUNDLE_INDEX_DIGEST_ALGORITHM, BundleIndexV1, BundleKernelIndexEntryV1,
    BundlePayloadReferenceV1, BundleTargetAssociationV1, BundleValidationError, MAX_BUNDLE_KERNELS,
    MAX_BUNDLE_PAYLOAD_REFERENCES, MAX_BUNDLE_TARGET_ASSOCIATIONS, MAX_KERNEL_PAYLOAD_REFERENCES,
};
pub use bundle_decode::BundleDecodeError;
pub use bundle_encode::{BUNDLE_INDEX_MAGIC, BUNDLE_INDEX_VERSION, MAX_BUNDLE_INDEX_BYTES};
pub use container::{
    ArtifactContainerV1, CodeObjectPayload, ContainerValidationError, MAX_CODE_OBJECT_BYTES,
    MAX_EMBEDDED_PAYLOAD_BYTES,
};
pub use container_decode::ContainerDecodeError;
pub use container_encode::{
    CONTAINER_HEADER_BYTES, CONTAINER_MAGIC, CONTAINER_VERSION, MAX_CONTAINER_BYTES,
    PAYLOAD_DESCRIPTOR_BYTES,
};
pub use digest::{DigestAlgorithm, DigestMismatch, PayloadDigest};
pub use direct_link::{
    DIRECT_LINK_EVIDENCE_DIGEST_ALGORITHM, DirectLinkBindingExpectationV1,
    DirectLinkBindingSourceV1, DirectLinkBindingV1, DirectLinkBundleEvidenceV1,
    DirectLinkBundleIndexIdentityV1, DirectLinkContainerIdentityV1, DirectLinkEvidenceError,
    DirectLinkFfiClosureIdentityV1, DirectLinkFinalizationIdentityV1,
    DirectLinkFinalizedPayloadIdentityV1, DirectLinkLinkedOutputIdentityV1,
    DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1,
    DirectLinkToolchainConfigurationIdentityV1, DirectLinkToolchainExecutableIdentityV1,
    DirectLinkToolchainIdentityV1, DirectLinkTransformationIdentityV1,
    DirectLinkWorkerConfigurationIdentityV1, DirectLinkWorkerExecutableIdentityV1,
    DirectLinkWorkerIdentityV1, MAX_DIRECT_LINK_BINDINGS, ValidatedDirectLinkBundleEvidenceV1,
};
pub use direct_link_bridge::{
    CallerClaimedPackageIdentityV1, DirectLinkBridgeError, DirectLinkBridgeIdentityKindV1,
    DirectLinkManifestClaimScopeFieldV1, DirectLinkPublicationBridgeV1,
    DirectLinkPublicationOccurrenceIdentityV1, DirectLinkPublicationScopeProvenanceV1,
    ManifestClaimDerivedLinkPublicationScopeV1, ManifestClaimDerivedTargetIdentityV1,
    ManifestClaimDirectLinkCurrentPublicationLeaseV1,
    ManifestClaimDirectLinkCurrentPublicationTokenV1, ManifestClaimDirectLinkDurablePlanHandoffV1,
    ManifestClaimDirectLinkDurablePublicationResultV1, ManifestClaimDirectLinkPublicationBridgeV1,
    NonAuthoritativeDirectLinkPublicationDiagnosticsV1, derive_manifest_claim_target_identity_v1,
    publish_manifest_claim_direct_link_durable_v1, recover_manifest_claim_direct_link_durable_v1,
};
pub use direct_link_decode::DirectLinkDecodeError;
pub use direct_link_encode::{
    DIRECT_LINK_EVIDENCE_HEADER_BYTES, DIRECT_LINK_EVIDENCE_MAGIC, DIRECT_LINK_EVIDENCE_VERSION,
    MAX_DIRECT_LINK_EVIDENCE_BYTES,
};
pub use encode::{MANIFEST_MAGIC, MANIFEST_VERSION, MAX_MANIFEST_BYTES};
pub use error::{DecodeError, ValidationError};
pub use generated_kernel_identity::{
    COMPILER_LAYOUT_REGISTRATION_IDENTITY_DOMAIN_V1, GENERATED_HOST_CONTRACT_IDENTITY_DOMAIN_V1,
    GENERATED_KERNEL_IDENTITY_DOMAIN_V2, derive_compiler_layout_registration_identity_v1,
    derive_generated_host_contract_identity_v1, derive_generated_kernel_identity_v2,
};
pub use gfx942_bundle::{
    GFX942_TWO_KERNEL_BUNDLE_VERSION_V1, GFX942_TWO_KERNEL_COUNT, Gfx942BundleError,
    Gfx942KernelProofBindingV1, Gfx942TwoKernelBundleV1,
};
pub use host_launch_abi::{HostLaunchAbi, HostLaunchAbiError};
pub use identities::{
    BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity, CompilerIdentity, DigestBytes,
    Dimensions, Endianness, IdentityText, LaunchContract, MAX_IDENTITY_TEXT_BYTES, MAX_NAME_BYTES,
    Name, PointerWidth, TargetIdentity, ToolIdentity,
};
pub use manifest::{KernelEntry, MAX_CODE_OBJECTS, MAX_KERNELS, ManifestV1};
pub use proof::{
    ConfigurationEntry, MAX_CONFIGURATION_ENTRIES, MAX_PROOF_PROPERTIES, MAX_TRUSTED_ITEMS,
    MeasuredToolIdentity, ProofArtifactIdentity, ProofExecutionIdentity, ProofOutcome,
    ProofProperty, ProofRecordV1, ProofTargetIdentity, SourceContractIdentity, TrustedItem,
    VerificationModelIdentity,
};
pub use proof_decode::ProofDecodeError;
pub use proof_encode::{MAX_PROOF_RECORD_BYTES, PROOF_RECORD_MAGIC, PROOF_RECORD_VERSION};
pub use proof_executable_binding::{
    ExecutableCodeObjectVersionV1, PROOF_EXECUTABLE_BINDING_DOMAIN_V1,
    PROOF_EXECUTABLE_BINDING_VERSION_V1, ProofExecutableBindingError, ProofExecutableBindingV1,
    ProofExecutableSemanticIdentityV1, ProofToolPolicyIdentityV1,
};
pub use rust_layout::{
    MAX_RUST_LAYOUT_ALIGNMENT, MAX_RUST_LAYOUT_BYTES, MAX_RUST_LAYOUT_COMPONENTS,
    RUST_LAYOUT_EVIDENCE_DOMAIN_V1, RUST_LAYOUT_EVIDENCE_VERSION_V1, RUST_TYPE_EVIDENCE_DOMAIN_V1,
    RustDisjointIndexSpaceV1, RustLayoutEvidenceError, RustLayoutEvidenceV1,
    RustPhysicalComponentKindV1, RustPhysicalComponentV1, RustPointerMutabilityV1,
    RustScalarElementTypeV1, RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
};
pub use selection::{DeclaredTargetMismatch, KernelSelectionError, SelectedNativeKernel};
