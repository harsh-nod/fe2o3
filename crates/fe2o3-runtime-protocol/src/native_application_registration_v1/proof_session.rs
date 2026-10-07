//! Native proof-controller matching data, never deployment or pidfd custody.
use super::*;
type Transcript = NativeApplicationSessionTranscriptV1;
const MAGIC: &[u8; 8] = b"F3NAPF1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION-PROOF-SESSION/V1\0";
const IDENTITY_OFFSET: usize = 200;
pub const NATIVE_APPLICATION_PROOF_SESSION_BYTES_V1: usize = IDENTITY_OFFSET + 32;
const BYTES: usize = NATIVE_APPLICATION_PROOF_SESSION_BYTES_V1;

/// Move-only inert description of a native controller and original transcript.
/// The caller must independently admit the pinned native deployment and retain
/// the actual controller pidfd. PID numbers, nonces and bytes do not do so.
///
/// ```compile_fail
/// use fe2o3_runtime_protocol::{NativeApplicationProofSessionV1 as Native,
///     WorkerV3ApplicationProofSessionV1 as Legacy};
/// fn downgrade(value: Native) -> Legacy { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_runtime_protocol::NativeApplicationProofSessionV1;
/// fn duplicate(value: NativeApplicationProofSessionV1) { let _ = value.clone(); }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationProofSessionV1 {
    transcript: Transcript,
    bytes: [u8; BYTES],
}
type Session = NativeApplicationProofSessionV1;
impl Session {
    pub(super) const RETAINED: usize = size_of::<Self>() + size_of::<Storage>();
    pub const fn construction_quote() -> Quote {
        fixed_quote(0, Self::RETAINED)
    }
    pub const fn decoding_quote() -> Quote {
        fixed_quote(BYTES, Self::RETAINED)
    }
    pub fn new(
        transcript: Transcript,
        deployment: [u8; 32],
        nonce: [u8; 32],
        controller: (u32, u32, u32),
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(true, Self::construction_quote(), budget, |_| {
            Ok((
                Self::from_parts(transcript, deployment, nonce, controller)?,
                Storage(Self::RETAINED),
            ))
        })
    }
    fn from_parts(
        transcript: Transcript,
        deployment: [u8; 32],
        nonce: [u8; 32],
        controller: (u32, u32, u32),
    ) -> Result<Self> {
        if deployment == [0; 32]
            || nonce == [0; 32]
            || nonce == transcript.app_nonce()
            || nonce == transcript.root_nonce()
            || controller.0 == 0
            || controller.0 > i32::MAX as u32
            || controller.1 == 0
            || controller.1 == u32::MAX
            || controller.2 == 0
            || controller.2 == u32::MAX
        {
            return Err(Error::Transcript);
        }
        let mut bytes = [0; BYTES];
        write_header(&mut bytes, MAGIC);
        bytes[24..56].copy_from_slice(&transcript.app_nonce());
        bytes[56..88].copy_from_slice(&transcript.root_nonce());
        bytes[88..120].copy_from_slice(&transcript.binding());
        bytes[120..152].copy_from_slice(&deployment);
        bytes[152..184].copy_from_slice(&nonce);
        bytes[184..188].copy_from_slice(&controller.0.to_le_bytes());
        bytes[188..192].copy_from_slice(&controller.1.to_le_bytes());
        bytes[192..196].copy_from_slice(&controller.2.to_le_bytes());
        seal_proof(&mut bytes);
        Ok(Self { transcript, bytes })
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(bytes.len() == BYTES, Self::decoding_quote(), budget, |_| {
            if bytes.len() != BYTES {
                return Err(Error::Length);
            }
            if &bytes[..8] != MAGIC
                || bytes[8..10] != 1u16.to_le_bytes()
                || bytes[10..12] != [0; 2]
                || bytes[12..16] != (BYTES as u32).to_le_bytes()
                || bytes[16..24] != [0; 8]
                || bytes[196..200] != [0; 4]
            {
                return Err(Error::Header);
            }
            if bytes[IDENTITY_OFFSET..] != proof_checksum(&bytes[..IDENTITY_OFFSET]) {
                return Err(Error::Identity);
            }
            let transcript = Transcript::new(
                bytes[24..56].try_into().unwrap(),
                bytes[56..88].try_into().unwrap(),
                bytes[88..120].try_into().unwrap(),
            )?;
            let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let value = Self::from_parts(
                transcript,
                bytes[120..152].try_into().unwrap(),
                bytes[152..184].try_into().unwrap(),
                (word(184), word(188), word(192)),
            )?;
            if value.bytes != bytes {
                return Err(Error::Identity);
            }
            Ok((value, Storage(Self::RETAINED)))
        })
    }
    pub const fn transcript(&self) -> Transcript {
        self.transcript
    }
    pub fn deployment(&self) -> [u8; 32] {
        self.bytes[120..152].try_into().unwrap()
    }
    pub fn nonce(&self) -> [u8; 32] {
        self.bytes[152..184].try_into().unwrap()
    }
    pub fn controller(&self) -> (u32, u32, u32) {
        let word = |offset| u32::from_le_bytes(self.bytes[offset..offset + 4].try_into().unwrap());
        (word(184), word(188), word(192))
    }
    pub fn identity(&self) -> [u8; 32] {
        self.bytes[IDENTITY_OFFSET..].try_into().unwrap()
    }
    pub const fn canonical_bytes(&self) -> &[u8; BYTES] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
    pub const fn authenticates_controller_custody(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}
fn proof_checksum(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
pub(super) fn seal_proof(bytes: &mut [u8; BYTES]) {
    let identity = proof_checksum(&bytes[..IDENTITY_OFFSET]);
    bytes[IDENTITY_OFFSET..].copy_from_slice(&identity);
}
