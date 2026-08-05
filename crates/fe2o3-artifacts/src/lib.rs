#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod abi;
mod encode;
mod error;
mod identities;
mod manifest;

pub use abi::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, MAX_ABI_BYTES, MAX_ABI_FIELDS, Mutability,
    ScalarType,
};
pub use encode::{MANIFEST_MAGIC, MANIFEST_VERSION, MAX_MANIFEST_BYTES};
pub use error::ValidationError;
pub use identities::{
    BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity, CompilerIdentity, DigestBytes,
    Dimensions, Endianness, IdentityText, LaunchContract, MAX_IDENTITY_TEXT_BYTES, MAX_NAME_BYTES,
    Name, PointerWidth, TargetIdentity, ToolIdentity,
};
pub use manifest::{KernelEntry, MAX_CODE_OBJECTS, MAX_KERNELS, ManifestV1};
