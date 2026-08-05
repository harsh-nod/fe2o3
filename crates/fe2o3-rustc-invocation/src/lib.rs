#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod decode;
mod digest;
mod encode;
mod error;
mod model;

pub use decode::decode_descriptor_v1;
pub use digest::{INVOCATION_DIGEST_DOMAIN_V1, InvocationDigest};
pub use encode::{
    INVOCATION_DESCRIPTOR_MAGIC, INVOCATION_DESCRIPTOR_VERSION, encode_descriptor_v1,
};
pub use error::{DecodeError, DigestError, ValidationError};
pub use model::{
    AmdTargetIdTextV1, BackendToolsV1, CargoIdentityV1, CargoPackageV1, CargoTargetKindV1,
    CargoTargetV1, CompileEnvironmentEntryV1, CrateTypeV1, DeviceConfigurationV1, EditionV1,
    MAX_ARGUMENT_BYTES, MAX_COMPILE_ENVIRONMENT_ENTRIES, MAX_DESCRIPTOR_BYTES,
    MAX_ENVIRONMENT_VALUE_BYTES, MAX_FEATURES, MAX_NAME_BYTES, MAX_PATH_BYTES, MAX_RUSTC_ARGUMENTS,
    MAX_TEXT_BYTES, OutputDomainV1, RustcIdentityV1, RustcInvocationDescriptorV1, RustcUnitV1,
    TestStateV1, ToolIdentityV1, VerificationModeV1,
};

#[cfg(test)]
mod tests;
