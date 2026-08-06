//! Reserved names and registration values shared by fe2o3 macros and the backend.
//!
//! Kernel registration is a compiler contract, not an authenticity boundary. The
//! backend validates that records are structurally correct and internally
//! consistent, but Rust source can reproduce the reserved names and field values.
//! Code compiled with the fe2o3 backend is therefore trusted to emit honest
//! registrations.

use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

pub const RESERVED_ROOT: &str = "fe2o3_";
pub const KERNEL_PREFIX: &str = "fe2o3_kernel_";
pub const DEVICE_PREFIX: &str = "fe2o3_device_";
pub const DEVICE_EXTERN_PREFIX: &str = "fe2o3_device_extern_";

/// Final-path-segment prefix for kernel registration statics.
pub const KERNEL_REGISTRATION_PREFIX: &str = "__fe2o3_kernel_registration_";

/// ASCII `FE2O3KRN`, interpreted as a little-endian `u64`.
pub const KERNEL_REGISTRATION_MAGIC: u64 = 0x4e52_4b33_4f32_4546;
pub const KERNEL_REGISTRATION_VERSION_V1: u16 = 1;
pub const KERNEL_REGISTRATION_VERSION_V2: u16 = 2;
/// An ordinary `#[kernel]` registration without a generated typed profile.
pub const KERNEL_REGISTRATION_KIND_KERNEL: u16 = 1;
/// A `#[kernel(typed)]` registration using the exact typed vecadd V1 profile.
pub const KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1: u16 = 2;
/// A `#[kernel(typed)]` registration whose exact vecadd ABI identities are
/// derived from canonical rustc type/layout evidence.
pub const KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2: u16 = 3;

/// V1 is an immutable `#[used]` static with this exact tuple shape:
///
/// `(u64 magic, u16 version, u16 kind, &str logical_name, &str export_name, fn pointer)`.
///
/// The function pointer is the direct association to the generated kernel item.
pub const KERNEL_REGISTRATION_V1_FIELD_COUNT: usize = 6;

/// V2 extends V1 with canonical crate and kernel binding IDs before the
/// function pointer. Typed registrations must use this version.
pub const KERNEL_REGISTRATION_V2_FIELD_COUNT: usize = 8;

/// Environment variable populated by the fe2o3 rustc wrapper with the exact
/// compilation unit's crate binding ID.
pub const CRATE_BINDING_ID_ENV_V1: &str = "FE2O3_CRATE_BINDING_ID_V1";

/// Stable profile tag included in typed vecadd kernel binding derivation.
pub const TYPED_VECADD_F32_PROFILE_TAG_V1: &str = "typed-vecadd-f32-v1";
/// Stable profile tag for rustc-derived typed vecadd ABI evidence.
pub const TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2: &str = "typed-vecadd-f32-rustc-layout-v2";

const CRATE_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.crate-binding.v1\0";
const KERNEL_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.kernel-binding.v1\0";
const ARTIFACT_ACCESSOR_PREFIX_V1: &str = "__fe2o3_artifact_v1_";
const HOST_KERNEL_PREFIX_V1: &str = "__fe2o3_host_kernel_v1_";
const BINDING_ID_BYTES: usize = 32;
const BINDING_ID_HEX_BYTES: usize = BINDING_ID_BYTES * 2;

/// Full SHA-256 identity of one rustc compilation unit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CrateBindingIdV1([u8; BINDING_ID_BYTES]);

impl CrateBindingIdV1 {
    /// Constructs an identity from exact digest bytes.
    pub const fn from_bytes(bytes: [u8; BINDING_ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    pub const fn as_bytes(self) -> [u8; BINDING_ID_BYTES] {
        self.0
    }

    /// Parses the canonical lowercase hexadecimal representation.
    pub fn from_hex(value: &str) -> Result<Self, BindingIdError> {
        parse_binding_hex(value).map(Self)
    }

    /// Returns the canonical lowercase hexadecimal representation.
    pub fn to_hex(self) -> String {
        encode_binding_hex(self.0)
    }
}

/// Full SHA-256 identity of one kernel in one rustc compilation unit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KernelBindingIdV1([u8; BINDING_ID_BYTES]);

impl KernelBindingIdV1 {
    /// Constructs an identity from exact digest bytes.
    pub const fn from_bytes(bytes: [u8; BINDING_ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    pub const fn as_bytes(self) -> [u8; BINDING_ID_BYTES] {
        self.0
    }

    /// Parses the canonical lowercase hexadecimal representation.
    pub fn from_hex(value: &str) -> Result<Self, BindingIdError> {
        parse_binding_hex(value).map(Self)
    }

    /// Returns the canonical lowercase hexadecimal representation.
    pub fn to_hex(self) -> String {
        encode_binding_hex(self.0)
    }
}

/// Error returned for a noncanonical binding ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingIdError {
    reason: &'static str,
}

impl fmt::Display for BindingIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.reason)
    }
}

impl Error for BindingIdError {}

/// Derives one crate binding from rustc's crate name and ordered `-C metadata`
/// values. Field lengths and the metadata count make the encoding unambiguous.
pub fn derive_crate_binding_id_v1<'a>(
    crate_name: &str,
    metadata: impl IntoIterator<Item = &'a str>,
) -> CrateBindingIdV1 {
    let metadata = metadata.into_iter().collect::<Vec<_>>();
    let mut digest = Sha256::new();
    digest.update(CRATE_BINDING_DOMAIN_V1);
    update_field(&mut digest, crate_name.as_bytes());
    digest.update((metadata.len() as u64).to_le_bytes());
    for value in metadata {
        update_field(&mut digest, value.as_bytes());
    }
    CrateBindingIdV1(digest.finalize().into())
}

/// Derives one kernel binding from its compilation unit, typed profile, and
/// source/export names.
pub fn derive_kernel_binding_id_v1(
    crate_binding: CrateBindingIdV1,
    profile_tag: &str,
    logical_name: &str,
    export_name: &str,
) -> KernelBindingIdV1 {
    let mut digest = Sha256::new();
    digest.update(KERNEL_BINDING_DOMAIN_V1);
    update_field(&mut digest, &crate_binding.as_bytes());
    update_field(&mut digest, profile_tag.as_bytes());
    update_field(&mut digest, logical_name.as_bytes());
    update_field(&mut digest, export_name.as_bytes());
    KernelBindingIdV1(digest.finalize().into())
}

/// Returns the private artifact pointer accessor for one exact kernel binding.
pub fn artifact_pointer_symbol_v1(binding: KernelBindingIdV1) -> String {
    format!("{ARTIFACT_ACCESSOR_PREFIX_V1}{}_ptr", binding.to_hex())
}

/// Returns the private artifact length accessor for one exact kernel binding.
pub fn artifact_length_symbol_v1(binding: KernelBindingIdV1) -> String {
    format!("{ARTIFACT_ACCESSOR_PREFIX_V1}{}_len", binding.to_hex())
}

/// Returns the reserved host symbol for one exact registered kernel function.
pub fn host_kernel_symbol_v1(binding: KernelBindingIdV1) -> String {
    format!("{HOST_KERNEL_PREFIX_V1}{}", binding.to_hex())
}

fn update_field(digest: &mut Sha256, field: &[u8]) {
    digest.update((field.len() as u64).to_le_bytes());
    digest.update(field);
}

fn parse_binding_hex(value: &str) -> Result<[u8; BINDING_ID_BYTES], BindingIdError> {
    if value.len() != BINDING_ID_HEX_BYTES {
        return Err(BindingIdError {
            reason: "binding ID must contain exactly 64 lowercase hexadecimal bytes",
        });
    }
    let mut decoded = [0_u8; BINDING_ID_BYTES];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_hex_nibble(pair[0]).ok_or(BindingIdError {
            reason: "binding ID must contain exactly 64 lowercase hexadecimal bytes",
        })?;
        let low = decode_hex_nibble(pair[1]).ok_or(BindingIdError {
            reason: "binding ID must contain exactly 64 lowercase hexadecimal bytes",
        })?;
        decoded[index] = (high << 4) | low;
    }
    Ok(decoded)
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn encode_binding_hex(bytes: [u8; BINDING_ID_BYTES]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(BINDING_ID_HEX_BYTES);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_registration_v1_values_are_stable() {
        assert_eq!(KERNEL_REGISTRATION_MAGIC.to_le_bytes(), *b"FE2O3KRN");
        assert_eq!(KERNEL_REGISTRATION_VERSION_V1, 1);
        assert_eq!(KERNEL_REGISTRATION_VERSION_V2, 2);
        assert_eq!(KERNEL_REGISTRATION_KIND_KERNEL, 1);
        assert_eq!(KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1, 2);
        assert_eq!(KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2, 3);
        assert_ne!(
            KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1,
            KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2
        );
        assert_eq!(KERNEL_REGISTRATION_V1_FIELD_COUNT, 6);
        assert_eq!(KERNEL_REGISTRATION_V2_FIELD_COUNT, 8);
    }

    #[test]
    fn binding_derivation_is_ordered_domain_separated_and_round_trips() {
        let first = derive_crate_binding_id_v1("same", ["alpha", "beta"]);
        let reordered = derive_crate_binding_id_v1("same", ["beta", "alpha"]);
        let different_crate = derive_crate_binding_id_v1("other", ["alpha", "beta"]);
        assert_ne!(first, reordered);
        assert_ne!(first, different_crate);
        assert_eq!(CrateBindingIdV1::from_hex(&first.to_hex()).unwrap(), first);

        let opaque_kernel =
            derive_kernel_binding_id_v1(first, TYPED_VECADD_F32_PROFILE_TAG_V1, "vecadd", "vecadd");
        let kernel = derive_kernel_binding_id_v1(
            first,
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            "vecadd",
            "vecadd",
        );
        assert_ne!(opaque_kernel, kernel);
        assert_ne!(kernel.as_bytes(), first.as_bytes());
        assert_eq!(
            KernelBindingIdV1::from_hex(&kernel.to_hex()).unwrap(),
            kernel
        );
    }

    #[test]
    fn generated_symbols_include_the_full_kernel_binding() {
        let crate_id = derive_crate_binding_id_v1("crate", ["metadata"]);
        let kernel = derive_kernel_binding_id_v1(
            crate_id,
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            "vecadd",
            "vecadd",
        );
        let hex = kernel.to_hex();

        assert_eq!(
            artifact_pointer_symbol_v1(kernel),
            format!("__fe2o3_artifact_v1_{hex}_ptr")
        );
        assert_eq!(
            artifact_length_symbol_v1(kernel),
            format!("__fe2o3_artifact_v1_{hex}_len")
        );
        assert_eq!(
            host_kernel_symbol_v1(kernel),
            format!("__fe2o3_host_kernel_v1_{hex}")
        );
    }

    #[test]
    fn parser_rejects_noncanonical_binding_ids() {
        for invalid in [
            "",
            "00",
            "000000000000000000000000000000000000000000000000000000000000000",
            "00000000000000000000000000000000000000000000000000000000000000000",
            "000000000000000000000000000000000000000000000000000000000000000G",
            "000000000000000000000000000000000000000000000000000000000000000A",
        ] {
            assert!(CrateBindingIdV1::from_hex(invalid).is_err(), "{invalid}");
            assert!(KernelBindingIdV1::from_hex(invalid).is_err(), "{invalid}");
        }
    }
}
