//! Inert bounded records for an authenticated retained-proof transport, not proof authority.

use crate::{MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2, WorkerV3ApplicationSessionTranscriptV1};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

pub const WORKER_V3_APPLICATION_PROOF_SESSION_BYTES_V1: usize = 192;
pub const WORKER_V3_APPLICATION_PROOF_MAX_PACKET_BYTES_V1: usize = 64 + 2048;
pub const WORKER_V3_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1: usize = 64 * 1024 * 1024;
const SESSION_DOMAIN: &[u8] = b"FE2O3/APPLICATION-PROOF-SESSION/V1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationProofProtocolErrorV1;
impl fmt::Display for WorkerV3ApplicationProofProtocolErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid application proof protocol record")
    }
}
impl Error for WorkerV3ApplicationProofProtocolErrorV1 {}
type Result<T> = std::result::Result<T, WorkerV3ApplicationProofProtocolErrorV1>;
fn require(value: bool) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(WorkerV3ApplicationProofProtocolErrorV1)
    }
}

/// Canonical matching data supplied by the authenticated root Ready transition.
/// Independently installed policy and original pidfd custody must admit it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationProofSessionV1 {
    transcript: WorkerV3ApplicationSessionTranscriptV1,
    deployment: [u8; 32],
    nonce: [u8; 32],
    controller: (u32, u32, u32),
    bytes: [u8; WORKER_V3_APPLICATION_PROOF_SESSION_BYTES_V1],
}
impl WorkerV3ApplicationProofSessionV1 {
    pub fn new(
        transcript: WorkerV3ApplicationSessionTranscriptV1,
        deployment: [u8; 32],
        nonce: [u8; 32],
        controller: (u32, u32, u32),
    ) -> Result<Self> {
        require(
            deployment != [0; 32]
                && nonce != [0; 32]
                && nonce != transcript.app_nonce()
                && nonce != transcript.root_nonce()
                && controller.0 > 0
                && controller.0 <= i32::MAX as u32
                && controller.1 > 0
                && controller.1 != u32::MAX
                && controller.2 > 0
                && controller.2 != u32::MAX,
        )?;
        let mut bytes = [0; WORKER_V3_APPLICATION_PROOF_SESSION_BYTES_V1];
        bytes[..8].copy_from_slice(b"F3APSS1\0");
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[16..48].copy_from_slice(&transcript.app_nonce());
        bytes[48..80].copy_from_slice(&transcript.root_nonce());
        bytes[80..112].copy_from_slice(&transcript.binding());
        bytes[112..144].copy_from_slice(&deployment);
        bytes[144..176].copy_from_slice(&nonce);
        bytes[176..180].copy_from_slice(&controller.0.to_le_bytes());
        bytes[180..184].copy_from_slice(&controller.1.to_le_bytes());
        bytes[184..188].copy_from_slice(&controller.2.to_le_bytes());
        Ok(Self {
            transcript,
            deployment,
            nonce,
            controller,
            bytes,
        })
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require(bytes.len() == WORKER_V3_APPLICATION_PROOF_SESSION_BYTES_V1)?;
        require(
            &bytes[..8] == b"F3APSS1\0"
                && bytes[8..10] == 1u16.to_le_bytes()
                && bytes[10..16] == [0; 6]
                && bytes[188..] == [0; 4],
        )?;
        let transcript = WorkerV3ApplicationSessionTranscriptV1::new(
            bytes[16..48].try_into().unwrap(),
            bytes[48..80].try_into().unwrap(),
            bytes[80..112].try_into().unwrap(),
        )
        .map_err(|_| WorkerV3ApplicationProofProtocolErrorV1)?;
        Self::new(
            transcript,
            bytes[112..144].try_into().unwrap(),
            bytes[144..176].try_into().unwrap(),
            (u32_at(bytes, 176), u32_at(bytes, 180), u32_at(bytes, 184)),
        )
    }
    pub const fn canonical_bytes(&self) -> &[u8; WORKER_V3_APPLICATION_PROOF_SESSION_BYTES_V1] {
        &self.bytes
    }
    pub const fn transcript(&self) -> WorkerV3ApplicationSessionTranscriptV1 {
        self.transcript
    }
    pub const fn deployment(&self) -> [u8; 32] {
        self.deployment
    }
    pub const fn nonce(&self) -> [u8; 32] {
        self.nonce
    }
    pub const fn controller(&self) -> (u32, u32, u32) {
        self.controller
    }
    pub fn identity(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(SESSION_DOMAIN);
        hash.update(self.bytes);
        hash.finalize().into()
    }
}

/// Declared immutable inputs; neither these hashes nor the two received FDs establish origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationProofInputsV1 {
    kernel: [u8; 32],
    envelope: ([u8; 32], u64),
    payload: ([u8; 32], u64),
    bytes: [u8; 112],
}
impl WorkerV3ApplicationProofInputsV1 {
    pub fn new(
        kernel: [u8; 32],
        envelope: ([u8; 32], u64),
        payload: ([u8; 32], u64),
    ) -> Result<Self> {
        require(
            kernel != [0; 32]
                && envelope.0 != [0; 32]
                && payload.0 != [0; 32]
                && envelope.1 > 0
                && envelope.1 <= MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2 as u64
                && payload.1 > 0
                && payload.1 <= WORKER_V3_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1 as u64,
        )?;
        let mut bytes = [0; 112];
        bytes[..32].copy_from_slice(&kernel);
        bytes[32..64].copy_from_slice(&envelope.0);
        bytes[64..72].copy_from_slice(&envelope.1.to_le_bytes());
        bytes[72..104].copy_from_slice(&payload.0);
        bytes[104..112].copy_from_slice(&payload.1.to_le_bytes());
        Ok(Self {
            kernel,
            envelope,
            payload,
            bytes,
        })
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require(bytes.len() == 112)?;
        Self::new(
            bytes[..32].try_into().unwrap(),
            (bytes[32..64].try_into().unwrap(), u64_at(bytes, 64)),
            (bytes[72..104].try_into().unwrap(), u64_at(bytes, 104)),
        )
    }
    pub const fn canonical_bytes(&self) -> &[u8; 112] {
        &self.bytes
    }
    pub const fn kernel(&self) -> [u8; 32] {
        self.kernel
    }
    pub const fn envelope(&self) -> ([u8; 32], u64) {
        self.envelope
    }
    pub const fn payload(&self) -> ([u8; 32], u64) {
        self.payload
    }
}

/// No Release or settlement message exists in this proof-only transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WorkerV3ApplicationProofKindV1 {
    Active = 1,
    Request = 2,
    Proved = 3,
    Probe = 4,
    Retained = 5,
    Rejected = 6,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationProofMessageV1 {
    kind: WorkerV3ApplicationProofKindV1,
    session: [u8; 32],
    sequence: u64,
    bytes: Vec<u8>,
}
impl WorkerV3ApplicationProofMessageV1 {
    /// Validates kind-specific bounds before allocation. Live owners enforce direction and phase.
    pub fn new(
        kind: WorkerV3ApplicationProofKindV1,
        session: [u8; 32],
        sequence: u64,
        body: &[u8],
    ) -> Result<Self> {
        use WorkerV3ApplicationProofKindV1 as K;
        require(session != [0; 32] && sequence > 0 && sequence < u64::MAX && body.len() <= 2048)?;
        match kind {
            K::Active => require(sequence == 1 && body.is_empty())?,
            K::Request => {
                require(sequence == 1)?;
                WorkerV3ApplicationProofInputsV1::decode(body)?;
            }
            K::Proved => require(sequence == 1 && !body.is_empty())?,
            K::Probe => require(sequence >= 2 && body.is_empty())?,
            K::Retained => require(sequence >= 2 && !body.is_empty())?,
            K::Rejected => {
                require(!body.is_empty() && body.len() <= 512 && std::str::from_utf8(body).is_ok())?
            }
        }
        let mut bytes = vec![0; 64 + body.len()];
        bytes[..8].copy_from_slice(b"F3APMS1\0");
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[10] = kind as u8;
        bytes[11] = if kind == K::Request { 2 } else { 0 };
        bytes[12..16].copy_from_slice(&((64 + body.len()) as u32).to_le_bytes());
        bytes[16..48].copy_from_slice(&session);
        bytes[48..56].copy_from_slice(&sequence.to_le_bytes());
        bytes[64..].copy_from_slice(body);
        Ok(Self {
            kind,
            session,
            sequence,
            bytes,
        })
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require((64..=WORKER_V3_APPLICATION_PROOF_MAX_PACKET_BYTES_V1).contains(&bytes.len()))?;
        require(
            &bytes[..8] == b"F3APMS1\0"
                && bytes[8..10] == 1u16.to_le_bytes()
                && u32_at(bytes, 12) as usize == bytes.len()
                && bytes[56..64] == [0; 8],
        )?;
        use WorkerV3ApplicationProofKindV1 as K;
        let kind = match bytes[10] {
            1 => K::Active,
            2 => K::Request,
            3 => K::Proved,
            4 => K::Probe,
            5 => K::Retained,
            6 => K::Rejected,
            _ => return Err(WorkerV3ApplicationProofProtocolErrorV1),
        };
        require(bytes[11] == if kind == K::Request { 2 } else { 0 })?;
        Self::new(
            kind,
            bytes[16..48].try_into().unwrap(),
            u64_at(bytes, 48),
            &bytes[64..],
        )
    }
    pub const fn kind(&self) -> WorkerV3ApplicationProofKindV1 {
        self.kind
    }
    pub const fn session(&self) -> [u8; 32] {
        self.session
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn required_rights(&self) -> usize {
        usize::from(self.bytes[11])
    }
    pub fn body(&self) -> &[u8] {
        &self.bytes[64..]
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
fn u32_at(bytes: &[u8], start: usize) -> u32 {
    u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap())
}
fn u64_at(bytes: &[u8], start: usize) -> u64 {
    u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sessions_bind_transcript_deployment_nonce_and_controller() {
        let transcript =
            WorkerV3ApplicationSessionTranscriptV1::new([1; 32], [2; 32], [3; 32]).unwrap();
        let value = WorkerV3ApplicationProofSessionV1::new(
            transcript,
            [4; 32],
            [5; 32],
            (42, 61000, 61000),
        )
        .unwrap();
        assert_eq!(
            WorkerV3ApplicationProofSessionV1::decode(value.canonical_bytes()).unwrap(),
            value
        );
        for index in 0..value.bytes.len() {
            let mut changed = value.bytes;
            changed[index] ^= 1;
            if let Ok(other) = WorkerV3ApplicationProofSessionV1::decode(&changed) {
                assert_ne!(other.identity(), value.identity());
            }
        }
        assert!(
            WorkerV3ApplicationProofSessionV1::new(
                transcript,
                [4; 32],
                [1; 32],
                (42, 61000, 61000)
            )
            .is_err()
        );
        assert!(
            WorkerV3ApplicationProofSessionV1::new(transcript, [4; 32], [5; 32], (42, 0, 0))
                .is_err()
        );
    }
    #[test]
    fn packet_phase_shapes_rights_and_bounds_are_exact() {
        use WorkerV3ApplicationProofKindV1 as K;
        let input =
            WorkerV3ApplicationProofInputsV1::new([1; 32], ([2; 32], 128), ([3; 32], 256)).unwrap();
        for (kind, sequence, body, rights) in [
            (K::Active, 1, &[][..], 0),
            (K::Request, 1, &input.bytes[..], 2),
            (K::Proved, 1, b"subject", 0),
            (K::Probe, 2, &[], 0),
            (K::Retained, 2, b"subject", 0),
            (K::Rejected, 1, b"rejected", 0),
        ] {
            let message =
                WorkerV3ApplicationProofMessageV1::new(kind, [4; 32], sequence, body).unwrap();
            assert_eq!(message.required_rights(), rights);
            assert_eq!(
                WorkerV3ApplicationProofMessageV1::decode(&message.bytes).unwrap(),
                message
            );
            for index in [0, 8, 11, 12, 56, 63] {
                let mut changed = message.bytes.clone();
                changed[index] ^= 1;
                assert!(WorkerV3ApplicationProofMessageV1::decode(&changed).is_err());
            }
            assert!(WorkerV3ApplicationProofMessageV1::decode(&message.bytes[..63]).is_err());
            assert!(WorkerV3ApplicationProofMessageV1::new(kind, [0; 32], sequence, body).is_err());
        }
        assert!(
            WorkerV3ApplicationProofMessageV1::new(K::Request, [4; 32], 2, &input.bytes).is_err()
        );
        assert!(WorkerV3ApplicationProofMessageV1::new(K::Probe, [4; 32], 1, &[]).is_err());
        assert!(
            WorkerV3ApplicationProofMessageV1::new(K::Retained, [4; 32], u64::MAX, b"subject")
                .is_err()
        );
        assert!(WorkerV3ApplicationProofMessageV1::new(K::Proved, [4; 32], 1, &[0; 2049]).is_err());
        assert!(
            WorkerV3ApplicationProofInputsV1::new([1; 32], ([2; 32], 0), ([3; 32], 256)).is_err()
        );
        assert!(
            WorkerV3ApplicationProofInputsV1::new(
                [1; 32],
                ([2; 32], 128),
                (
                    [3; 32],
                    WORKER_V3_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1 as u64 + 1
                )
            )
            .is_err()
        );
    }
}
