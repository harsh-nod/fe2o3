//! Exact extraction of ordered rustc codegen metadata values.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::error::Error;
use std::ffi::OsStr;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::RustcCompileInvocationV2;

/// Environment carrying the exact ordered Cargo metadata observation digest.
pub const CARGO_METADATA_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2";

/// Domain separator for the exact ordered Cargo metadata observation digest.
pub const CARGO_METADATA_BUILD_OBSERVATION_DOMAIN_V2: &[u8] =
    b"FE2O3/CARGO-METADATA-BUILD-OBSERVATION/V2\0";

/// Digest of the exact ordered rustc metadata values for one compile invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CargoMetadataBuildObservationV2([u8; 32]);

impl CargoMetadataBuildObservationV2 {
    /// Returns the exact digest bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Returns the canonical lowercase hexadecimal encoding.
    #[must_use]
    pub fn to_hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = String::with_capacity(self.0.len() * 2);
        for byte in self.0 {
            encoded.push(HEX[usize::from(byte >> 4)] as char);
            encoded.push(HEX[usize::from(byte & 0x0f)] as char);
        }
        encoded
    }
}

/// Derives the canonical observation of ordered rustc metadata values.
///
/// The count, every value length, duplicates, and argument order are included.
#[must_use]
pub fn derive_cargo_metadata_build_observation_v2<T: AsRef<str>>(
    metadata: &[T],
) -> CargoMetadataBuildObservationV2 {
    let mut digest = Sha256::new();
    digest.update(CARGO_METADATA_BUILD_OBSERVATION_DOMAIN_V2);
    digest.update((metadata.len() as u64).to_le_bytes());
    for value in metadata {
        let value = value.as_ref();
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    CargoMetadataBuildObservationV2(digest.finalize().into())
}

/// An error found while reading metadata from a validated rustc compile invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RustcCodegenMetadataErrorV1 {
    /// A `-C` or `--codegen` value could not be decoded without loss.
    NonUtf8CodegenOption {
        /// Index of the codegen value in the original argument vector.
        argument_index: usize,
    },
    /// A codegen metadata option had no metadata bytes after `metadata=`.
    EmptyMetadata {
        /// Index of the empty metadata value in the original argument vector.
        argument_index: usize,
    },
}

impl fmt::Display for RustcCodegenMetadataErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUtf8CodegenOption { argument_index } => write!(
                formatter,
                "rustc codegen option at argv[{argument_index}] is not valid UTF-8"
            ),
            Self::EmptyMetadata { argument_index } => write!(
                formatter,
                "rustc metadata value at argv[{argument_index}] is empty"
            ),
        }
    }
}

impl Error for RustcCodegenMetadataErrorV1 {}

/// Returns every explicit rustc `-C metadata` value in argument order.
///
/// This recognizes all four accepted split and joined spellings:
/// `-C metadata=...`, `-Cmetadata=...`, `--codegen metadata=...`, and
/// `--codegen=metadata=...`. Duplicates are retained. Other codegen options
/// are inspected but omitted from the result.
///
/// # Errors
///
/// Returns [`RustcCodegenMetadataErrorV1`] when a codegen value is not valid
/// UTF-8 or an explicit metadata value is empty. The validated compile
/// invocation has already established that every split option has a value.
pub fn ordered_rustc_codegen_metadata_v1(
    invocation: RustcCompileInvocationV2<'_>,
) -> Result<Vec<String>, RustcCodegenMetadataErrorV1> {
    let argv = invocation.argv();
    let mut metadata = Vec::new();
    let mut index = 1;
    while index < argv.len() {
        let argument = &argv[index];
        if argument == OsStr::new("--") {
            break;
        }
        if argument == OsStr::new("-C") || argument == OsStr::new("--codegen") {
            let value_index = index + 1;
            let value = argv
                .get(value_index)
                .expect("validated split rustc option has a value");
            inspect_codegen_value_v1(value, value_index, &mut metadata)?;
            index += 2;
            continue;
        }

        let bytes = argument.as_encoded_bytes();
        if bytes.starts_with(b"-C") {
            let value =
                argument
                    .to_str()
                    .ok_or(RustcCodegenMetadataErrorV1::NonUtf8CodegenOption {
                        argument_index: index,
                    })?;
            inspect_codegen_text_v1(&value[2..], index, &mut metadata)?;
        } else if bytes.starts_with(b"--codegen=") {
            let value =
                argument
                    .to_str()
                    .ok_or(RustcCodegenMetadataErrorV1::NonUtf8CodegenOption {
                        argument_index: index,
                    })?;
            inspect_codegen_text_v1(&value["--codegen=".len()..], index, &mut metadata)?;
        }
        index += 1;
    }
    Ok(metadata)
}

fn inspect_codegen_value_v1(
    value: &OsStr,
    argument_index: usize,
    metadata: &mut Vec<String>,
) -> Result<(), RustcCodegenMetadataErrorV1> {
    let value = value
        .to_str()
        .ok_or(RustcCodegenMetadataErrorV1::NonUtf8CodegenOption { argument_index })?;
    inspect_codegen_text_v1(value, argument_index, metadata)
}

fn inspect_codegen_text_v1(
    value: &str,
    argument_index: usize,
    metadata: &mut Vec<String>,
) -> Result<(), RustcCodegenMetadataErrorV1> {
    let Some(value) = value.strip_prefix("metadata=") else {
        return Ok(());
    };
    if value.is_empty() {
        return Err(RustcCodegenMetadataErrorV1::EmptyMetadata { argument_index });
    }
    metadata.push(value.to_owned());
    Ok(())
}
