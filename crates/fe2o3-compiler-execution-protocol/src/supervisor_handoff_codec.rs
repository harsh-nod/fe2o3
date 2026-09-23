//! Shared identity-only framing; policy and process admission remain external.
use crate::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionSupervisorHandoffErrorV1 as Error, launch_manifest_codec as launch,
};
use sha2::{Digest, Sha256};

const MANIFEST_OFFSET: usize = 40;
pub(crate) const PREIMAGE_BYTES: usize = MANIFEST_OFFSET + launch::BYTES;
pub(crate) const BYTES: usize = PREIMAGE_BYTES + 32;
const MAGIC: [u8; 8] = *b"F2O3CEH1";
const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-HANDOFF/V1\0";

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Frame {
    pub submitter: Client,
    pub identity: [u8; 32],
    pub bytes: [u8; BYTES],
}

pub(crate) fn encode(
    submitter: Client,
    client: Client,
    manifest: &[u8; launch::BYTES],
) -> Result<Frame, Error> {
    validate_relationship(submitter, client)?;
    Ok(encode_validated(submitter, manifest))
}

fn encode_validated(submitter: Client, manifest: &[u8; launch::BYTES]) -> Frame {
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
    bytes[24..28].copy_from_slice(&submitter.pid().to_le_bytes());
    bytes[28..32].copy_from_slice(&submitter.uid().to_le_bytes());
    bytes[32..36].copy_from_slice(&submitter.gid().to_le_bytes());
    bytes[MANIFEST_OFFSET..PREIMAGE_BYTES].copy_from_slice(manifest);
    let identity = derive_identity(&bytes[..PREIMAGE_BYTES]);
    bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
    Frame {
        submitter,
        identity,
        bytes,
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Result<(Frame, launch::Record), Error> {
    if bytes.len() != BYTES {
        return Err(Error::Length);
    }
    if bytes[..8] != MAGIC {
        return Err(Error::Magic);
    }
    if u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != 1 {
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
    let submitter = Client::new(
        read_u32(bytes, 24),
        read_u32(bytes, 28),
        read_u32(bytes, 32),
    )
    .map_err(|_| Error::SubmitterPid)?;
    let manifest =
        launch::decode(&bytes[MANIFEST_OFFSET..PREIMAGE_BYTES]).map_err(Error::LaunchManifest)?;
    validate_relationship(submitter, manifest.client)?;
    let identity = bytes[PREIMAGE_BYTES..].try_into().unwrap();
    if !matches(identity, bytes) {
        return Err(Error::Identity);
    }
    let frame = encode_validated(submitter, &manifest.bytes);
    if frame.bytes.as_slice() != bytes {
        return Err(Error::Canonical);
    }
    Ok((frame, manifest))
}

fn validate_relationship(submitter: Client, client: Client) -> Result<(), Error> {
    if submitter.pid() == client.pid() {
        return Err(Error::SubmitterIsClient);
    }
    if submitter.uid() != client.uid() || submitter.gid() != client.gid() {
        return Err(Error::CredentialMismatch);
    }
    Ok(())
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
