//! Canonical public configuration only; installation and process custody are separate.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

pub const NATIVE_PROOF_CUSTODIAN_CONFIGURATION_BYTES_V1: usize = 360;
pub const NATIVE_PROOF_CUSTODIAN_MAX_CONTROLLER_BYTES_V1: u64 = 128 * 1024 * 1024;
pub const NATIVE_PROOF_CUSTODIAN_MAX_ANALYZER_BYTES_V1: u64 = 512 * 1024 * 1024;
const BYTES: usize = NATIVE_PROOF_CUSTODIAN_CONFIGURATION_BYTES_V1;
const IDENTITY: usize = BYTES - 32;
const MAGIC: &[u8; 8] = b"F3NPCD1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION/PROOF-CUSTODIAN-DEPLOYMENT/V1\0";
const WORK: usize = 4096;
const SCRATCH: usize = 4 * Config::RETAINED;

/// Caller-supplied descriptions, never accepted policy or execution evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeProofCustodianConfigurationPartsV1 {
    pub credentials: (u32, u32),
    pub controller: ([u8; 32], u64),
    pub analyzer_executable: ([u8; 32], u64),
    pub analyzer_runtime_closure: ([u8; 32], u64),
    pub analyzer_identity: [u8; 32],
    pub toolchain_identity: [u8; 32],
    pub verus_identity: [u8; 32],
    pub compiler_policy_identity: [u8; 32],
    pub semantic_policy: ([u8; 32], u64),
}
type Parts = NativeProofCustodianConfigurationPartsV1;

#[derive(Debug)]
pub enum NativeProofCustodianConfigurationErrorV1 {
    Resource(Resource),
    Length,
    Header,
    Identity,
    Credentials,
    Measurement,
}
type Error = NativeProofCustodianConfigurationErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native proof configuration: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Full unreserved output charge; failure retains the operation's scratch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeProofCustodianConfigurationStorageV1(usize);
impl NativeProofCustodianConfigurationStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
use self::NativeProofCustodianConfigurationStorageV1 as Storage;

/// Exact native-only public matching data, with independently pinned semantic policy.
/// No installed path, producer, process, currentness or proof is authenticated here.
/// The unlanded earlier 320-byte shape is rejected, never silently upgraded.
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationProofCustodianConfigurationV1 {
    bytes: [u8; BYTES],
}
type Config = NativeApplicationProofCustodianConfigurationV1;
impl Config {
    pub const RETAINED: usize = size_of::<Self>() + size_of::<Storage>();

    pub fn new(parts: Parts, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(budget, 0, || {
            let mut bytes = [0; BYTES];
            bytes[..8].copy_from_slice(MAGIC);
            bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
            bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
            bytes[24..28].copy_from_slice(&parts.credentials.0.to_le_bytes());
            bytes[28..32].copy_from_slice(&parts.credentials.1.to_le_bytes());
            put_measurement(&mut bytes, 32, parts.controller);
            put_measurement(&mut bytes, 72, parts.analyzer_executable);
            put_measurement(&mut bytes, 112, parts.analyzer_runtime_closure);
            for (offset, identity) in [
                (152, parts.analyzer_identity),
                (184, parts.toolchain_identity),
                (216, parts.verus_identity),
                (248, parts.compiler_policy_identity),
            ] {
                bytes[offset..offset + 32].copy_from_slice(&identity);
            }
            bytes[280] = 6;
            put_measurement(&mut bytes, 288, parts.semantic_policy);
            let identity = hash(&bytes[..IDENTITY]);
            bytes[IDENTITY..].copy_from_slice(&identity);
            Self::decode_inner(&bytes).map(|value| (value, Storage(Self::RETAINED)))
        })
    }

    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(budget, if bytes.len() == BYTES { BYTES } else { 0 }, || {
            Self::decode_inner(bytes).map(|value| (value, Storage(Self::RETAINED)))
        })
    }

    fn decode_inner(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != BYTES {
            return Err(Error::Length);
        }
        if &bytes[..8] != MAGIC
            || bytes[8..10] != 1u16.to_le_bytes()
            || bytes[10..12] != [0; 2]
            || bytes[12..16] != (BYTES as u32).to_le_bytes()
            || bytes[16..24] != [0; 8]
            || bytes[280] != 6
            || bytes[281..288] != [0; 7]
        {
            return Err(Error::Header);
        }
        if bytes[IDENTITY..] != hash(&bytes[..IDENTITY]) {
            return Err(Error::Identity);
        }
        let value = Self {
            bytes: bytes.try_into().unwrap(),
        };
        let parts = value.parts();
        if [parts.credentials.0, parts.credentials.1]
            .iter()
            .any(|id| *id == 0 || *id == u32::MAX)
        {
            return Err(Error::Credentials);
        }
        if !measurement(
            parts.controller,
            NATIVE_PROOF_CUSTODIAN_MAX_CONTROLLER_BYTES_V1,
        ) || !measurement(
            parts.analyzer_executable,
            NATIVE_PROOF_CUSTODIAN_MAX_ANALYZER_BYTES_V1,
        ) || !measurement(parts.analyzer_runtime_closure, u64::MAX)
            || !measurement(parts.semantic_policy, u64::MAX)
            || [
                parts.analyzer_identity,
                parts.toolchain_identity,
                parts.verus_identity,
                parts.compiler_policy_identity,
            ]
            .contains(&[0; 32])
        {
            return Err(Error::Measurement);
        }
        Ok(value)
    }

    pub fn parts(&self) -> Parts {
        Parts {
            credentials: (
                u32::from_le_bytes(self.bytes[24..28].try_into().unwrap()),
                u32::from_le_bytes(self.bytes[28..32].try_into().unwrap()),
            ),
            controller: get_measurement(&self.bytes, 32),
            analyzer_executable: get_measurement(&self.bytes, 72),
            analyzer_runtime_closure: get_measurement(&self.bytes, 112),
            analyzer_identity: self.bytes[152..184].try_into().unwrap(),
            toolchain_identity: self.bytes[184..216].try_into().unwrap(),
            verus_identity: self.bytes[216..248].try_into().unwrap(),
            compiler_policy_identity: self.bytes[248..280].try_into().unwrap(),
            semantic_policy: get_measurement(&self.bytes, 288),
        }
    }
    pub const fn canonical_bytes(&self) -> &[u8; BYTES] {
        &self.bytes
    }
    pub fn identity(&self) -> [u8; 32] {
        self.bytes[IDENTITY..].try_into().unwrap()
    }
    pub const fn boundary(&self) -> u8 {
        6
    }
    pub const fn authenticates_deployment(&self) -> bool {
        false
    }
}

fn measurement(value: ([u8; 32], u64), max: u64) -> bool {
    value.0 != [0; 32] && value.1 > 0 && value.1 <= max && usize::try_from(value.1).is_ok()
}
fn get_measurement(bytes: &[u8], offset: usize) -> ([u8; 32], u64) {
    (
        bytes[offset..offset + 32].try_into().unwrap(),
        u64::from_le_bytes(bytes[offset + 32..offset + 40].try_into().unwrap()),
    )
}
fn put_measurement(bytes: &mut [u8], offset: usize, value: ([u8; 32], u64)) {
    bytes[offset..offset + 32].copy_from_slice(&value.0);
    bytes[offset + 32..offset + 40].copy_from_slice(&value.1.to_le_bytes());
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}
fn metered<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    action: impl FnOnce() -> Result<T>,
) -> Result<T> {
    budget.charge_work(WORK)?;
    if budget.storage() < floor {
        return Err(Resource::Accounting.into());
    }
    budget.reserve_storage(SCRATCH)?;
    let output = action()?;
    budget.release_storage(SCRATCH)?;
    Ok(output)
}

#[cfg(test)]
mod tests;
