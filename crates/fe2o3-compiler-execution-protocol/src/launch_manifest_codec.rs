//! Family-neutral identity frame. Contextual policy admission stays with callers.
use crate::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionServiceLaunchManifestErrorV1 as Error,
};
use sha2::{Digest, Sha256};

pub(crate) const PREIMAGE_BYTES: usize = 80;
pub(crate) const BYTES: usize = 112;
const MAGIC: [u8; 8] = *b"F2O3CEL1";
const VERSION: u16 = 1;
const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-LAUNCH-MANIFEST/V1\0";

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub client: Client,
    pub service: Service,
    pub policy: [u8; 32],
    pub identity: [u8; 32],
    pub bytes: [u8; BYTES],
}

pub(crate) fn encode(client: Client, service: Service, policy: [u8; 32]) -> Record {
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&VERSION.to_le_bytes());
    bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
    bytes[24..28].copy_from_slice(&client.pid().to_le_bytes());
    bytes[28..32].copy_from_slice(&client.uid().to_le_bytes());
    bytes[32..36].copy_from_slice(&client.gid().to_le_bytes());
    bytes[40..44].copy_from_slice(&service.uid().to_le_bytes());
    bytes[44..48].copy_from_slice(&service.gid().to_le_bytes());
    bytes[48..80].copy_from_slice(&policy);
    let identity = derive_identity(&bytes[..PREIMAGE_BYTES]);
    bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
    Record {
        client,
        service,
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
    if bytes[10..12].iter().any(|b| *b != 0) || bytes[16..24].iter().any(|b| *b != 0) {
        return Err(Error::Reserved);
    }
    if read_u32(bytes, 12) as usize != BYTES {
        return Err(Error::Length);
    }
    if bytes[36..40].iter().any(|b| *b != 0) {
        return Err(Error::Reserved);
    }
    let client = Client::new(
        read_u32(bytes, 24),
        read_u32(bytes, 28),
        read_u32(bytes, 32),
    )?;
    let service = Service::new(read_u32(bytes, 40), read_u32(bytes, 44))
        .map_err(Error::ExternalAnchorServiceIdentity)?;
    let policy: [u8; 32] = bytes[48..80].try_into().unwrap();
    if policy == [0; 32] {
        return Err(Error::PolicyIdentity);
    }
    let identity = bytes[PREIMAGE_BYTES..].try_into().unwrap();
    if !matches(identity, bytes) {
        return Err(Error::Identity);
    }
    let record = encode(client, service, policy);
    if record.bytes.as_slice() != bytes {
        return Err(Error::Canonical);
    }
    Ok(record)
}

pub(crate) fn matches(identity: [u8; 32], bytes: &[u8]) -> bool {
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
