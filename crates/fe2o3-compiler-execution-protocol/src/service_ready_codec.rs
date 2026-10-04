//! Family-neutral inert readiness frame. Contextual admission stays with callers.
use crate::CompilerExecutionServiceReadyErrorV1 as Error;
use sha2::{Digest, Sha256};

pub(crate) const PREIMAGE_BYTES: usize = 88;
pub(crate) const BYTES: usize = 120;
const MAGIC: [u8; 8] = *b"F2O3CER1";
const VERSION: u16 = 1;
const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-READY/V1\0";

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub issuer_pid: u32,
    pub launch_manifest: [u8; 32],
    pub policy: [u8; 32],
    pub identity: [u8; 32],
    pub bytes: [u8; BYTES],
}

pub(crate) fn encode(issuer_pid: u32, launch_manifest: [u8; 32], policy: [u8; 32]) -> Record {
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&VERSION.to_le_bytes());
    bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
    bytes[16..20].copy_from_slice(&issuer_pid.to_le_bytes());
    bytes[24..56].copy_from_slice(&launch_manifest);
    bytes[56..88].copy_from_slice(&policy);
    let identity = derive_identity(&bytes[..PREIMAGE_BYTES]);
    bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
    Record {
        issuer_pid,
        launch_manifest,
        policy,
        identity,
        bytes,
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Record, Error> {
    if bytes.len() != BYTES {
        return Err(Error::Length);
    }
    if bytes[..8] != MAGIC {
        return Err(Error::Magic);
    }
    if u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != VERSION {
        return Err(Error::Version);
    }
    if bytes[10..12].iter().any(|byte| *byte != 0) || bytes[20..24].iter().any(|byte| *byte != 0) {
        return Err(Error::Reserved);
    }
    if read_u32(bytes, 12) as usize != BYTES {
        return Err(Error::Length);
    }
    let issuer_pid = read_u32(bytes, 16);
    if issuer_pid == 0 {
        return Err(Error::IssuerPid);
    }
    let launch_manifest: [u8; 32] = bytes[24..56].try_into().unwrap();
    if launch_manifest == [0; 32] {
        return Err(Error::LaunchManifestIdentity);
    }
    let policy: [u8; 32] = bytes[56..88].try_into().unwrap();
    if policy == [0; 32] {
        return Err(Error::PolicyIdentity);
    }
    let identity = bytes[PREIMAGE_BYTES..].try_into().unwrap();
    if !matches(identity, bytes) {
        return Err(Error::Identity);
    }
    let record = encode(issuer_pid, launch_manifest, policy);
    if record.bytes.as_slice() != bytes {
        return Err(Error::Canonical);
    }
    Ok(record)
}

fn matches(identity: [u8; 32], bytes: &[u8]) -> bool {
    bytes.len() == BYTES
        && bytes[PREIMAGE_BYTES..] == identity
        && derive_identity(&bytes[..PREIMAGE_BYTES]) == identity
}

pub(crate) fn derive_identity(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
