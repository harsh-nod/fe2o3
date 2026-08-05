#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod abi;
mod binding;
mod container;
mod container_decode;
mod container_encode;
mod decode;
mod digest;
mod encode;
mod error;
mod identities;
mod manifest;
mod proof;
mod proof_decode;
mod proof_encode;
mod selection;

pub use abi::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    MAX_ABI_BYTES, MAX_ABI_FIELDS, Mutability, ScalarType, TypeIdentity,
};
pub use binding::{
    MatchedProofEvidenceV1, PROOF_IDENTITY_VERSION, ProofMatchError, ProofMatchPolicy,
    ProofTargetError, V1_REQUIRED_PROPERTIES,
};
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
pub use encode::{MANIFEST_MAGIC, MANIFEST_VERSION, MAX_MANIFEST_BYTES};
pub use error::{DecodeError, ValidationError};
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
pub use selection::{DeclaredTargetMismatch, KernelSelectionError, SelectedNativeKernel};
