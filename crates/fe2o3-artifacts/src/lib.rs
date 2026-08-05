#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod abi;
mod error;
mod identities;

pub use abi::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, MAX_ABI_BYTES, MAX_ABI_FIELDS, Mutability,
    ScalarType,
};
pub use error::ValidationError;
pub use identities::{
    BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity, CompilerIdentity, DigestBytes,
    Dimensions, Endianness, IdentityText, LaunchContract, MAX_IDENTITY_TEXT_BYTES, MAX_NAME_BYTES,
    Name, PointerWidth, TargetIdentity, ToolIdentity,
};
