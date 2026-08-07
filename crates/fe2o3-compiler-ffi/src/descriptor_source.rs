use std::{error::Error, fmt};

use fe2o3_kernel_descriptor::{
    DecodeError, DeviceDescriptorTableV1, MAX_DESCRIPTOR_TABLE_BYTES,
    decode_device_descriptor_table_v1, encode_device_descriptor_table_v1,
};
use sha2::{Digest, Sha256};

/// ELF section carrying the canonical V1 compiler descriptor source.
pub const COMPILER_DESCRIPTOR_SECTION_NAME_V1: &str = ".fe2o3.kd.v1";

/// SHA-256 and byte length of one exact canonical unfinalized descriptor table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerDescriptorSourceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl CompilerDescriptorSourceIdentityV1 {
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches(self, bytes: &[u8]) -> bool {
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        self.byte_len == bytes.len() as u64 && self.sha256 == actual
    }
}

/// Exact canonical compiler-side descriptor input retained before HSACO finalization.
///
/// The table must carry a zero canonical code-object digest. A later ELF-aware stage may embed
/// these exact bytes and replace that field with the digest of the linked executable. Public
/// construction is deliberately authority-free: this value validates structure and byte identity,
/// but does not authenticate rustc or grant link, load, or launch authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerDescriptorSourceV1 {
    table: DeviceDescriptorTableV1,
    identity: CompilerDescriptorSourceIdentityV1,
    canonical_bytes: Vec<u8>,
}

impl fmt::Debug for CompilerDescriptorSourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerDescriptorSourceV1")
            .field("target", &self.table.device_target())
            .field("code_object_version", &self.table.code_object_version())
            .field("kernel_count", &self.table.kernels().len())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl CompilerDescriptorSourceV1 {
    pub fn new(table: DeviceDescriptorTableV1) -> Result<Self, CompilerDescriptorSourceErrorV1> {
        if table.canonical_code_object_digest().as_bytes() != &[0; 32] {
            return Err(CompilerDescriptorSourceErrorV1::FinalizedDigest);
        }
        let canonical_bytes = encode_device_descriptor_table_v1(&table)
            .map_err(CompilerDescriptorSourceErrorV1::InvalidTable)?;
        debug_assert!(canonical_bytes.len() <= MAX_DESCRIPTOR_TABLE_BYTES);
        let identity = CompilerDescriptorSourceIdentityV1 {
            sha256: Sha256::digest(&canonical_bytes).into(),
            byte_len: canonical_bytes.len() as u64,
        };
        Ok(Self {
            table,
            identity,
            canonical_bytes,
        })
    }

    /// Strictly decodes one complete canonical zero-digest descriptor source.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerDescriptorSourceErrorV1> {
        let table = decode_device_descriptor_table_v1(bytes)
            .map_err(CompilerDescriptorSourceErrorV1::Decode)?;
        let value = Self::new(table)?;
        if value.canonical_bytes != bytes {
            return Err(CompilerDescriptorSourceErrorV1::NonCanonicalEncoding);
        }
        Ok(value)
    }

    pub const fn table(&self) -> &DeviceDescriptorTableV1 {
        &self.table
    }

    pub const fn identity(&self) -> CompilerDescriptorSourceIdentityV1 {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl TryFrom<&[u8]> for CompilerDescriptorSourceV1 {
    type Error = CompilerDescriptorSourceErrorV1;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::decode(bytes)
    }
}

/// Failure to construct or strictly decode an unfinalized compiler descriptor source.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerDescriptorSourceErrorV1 {
    FinalizedDigest,
    NonCanonicalEncoding,
    InvalidTable(fe2o3_kernel_descriptor::ValidationError),
    Decode(DecodeError),
}

impl fmt::Display for CompilerDescriptorSourceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FinalizedDigest => formatter
                .write_str("compiler descriptor source must have a zero code-object digest"),
            Self::NonCanonicalEncoding => {
                formatter.write_str("noncanonical compiler descriptor source encoding")
            }
            Self::InvalidTable(error) => write!(formatter, "invalid descriptor table: {error}"),
            Self::Decode(error) => write!(formatter, "invalid descriptor source encoding: {error}"),
        }
    }
}

impl Error for CompilerDescriptorSourceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidTable(error) => Some(error),
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}
