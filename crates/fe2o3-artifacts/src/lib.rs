#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod abi;
mod container;
mod decode;
mod digest;
mod encode;
mod error;
mod identities;
mod manifest;

pub use abi::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, MAX_ABI_BYTES, MAX_ABI_FIELDS, Mutability,
    ScalarType,
};
pub use container::{
    ArtifactContainerV1, CodeObjectPayload, ContainerValidationError, MAX_CODE_OBJECT_BYTES,
    MAX_EMBEDDED_PAYLOAD_BYTES,
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
