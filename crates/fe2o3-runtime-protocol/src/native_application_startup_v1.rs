//! Inert native currentness-ready/startup-ACK framing, never a live admission.
use crate::{
    NativeApplicationProofSessionV1 as Session, NativeApplicationSessionTranscriptV1 as Transcript,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

pub const NATIVE_APPLICATION_STARTUP_BYTES_V1: usize = 208;
const BYTES: usize = NATIVE_APPLICATION_STARTUP_BYTES_V1;
const READY_MAGIC: &[u8; 8] = b"F3NASR1\0";
const ACK_MAGIC: &[u8; 8] = b"F3NASA1\0";
const READY_DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION-CURRENTNESS-READY/V1\0";
const ACK_DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION-STARTUP-ACK/V1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeApplicationStartupKindV1 {
    CurrentnessReady,
    Acknowledgment,
}
use NativeApplicationStartupKindV1 as Kind;
impl Kind {
    fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::CurrentnessReady => READY_MAGIC,
            Self::Acknowledgment => ACK_MAGIC,
        }
    }
    fn domain(self) -> &'static [u8] {
        match self {
            Self::CurrentnessReady => READY_DOMAIN,
            Self::Acknowledgment => ACK_DOMAIN,
        }
    }
}

/// Complete native transcript/session/currentness-gate association. These are
/// equality bytes only. A live root owner must establish original peer and child
/// custody, and a live host owner must complete fresh VerifyCurrent before ACK.
/// Neither this codec nor a matching ACK authenticates those transitions.
///
/// ```compile_fail
/// use fe2o3_runtime_protocol::NativeApplicationStartupRecordV1 as Record;
/// fn duplicate(value: Record) { let _ = value.clone(); }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationStartupRecordV1 {
    kind: Kind,
    transcript: Transcript,
    bytes: [u8; BYTES],
}
use NativeApplicationStartupRecordV1 as Record;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationStartupStorageV1(usize);
impl NativeApplicationStartupStorageV1 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
use NativeApplicationStartupStorageV1 as Storage;

#[derive(Debug)]
pub enum NativeApplicationStartupErrorV1 {
    Resource(Resource),
    Length,
    Header,
    Binding,
    Kind,
}
use NativeApplicationStartupErrorV1 as Error;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native startup frame: {self:?}")
    }
}
impl std::error::Error for Error {}

impl Record {
    pub const WORK: usize = 64 * 1024;
    pub const SCRATCH: usize = 16 * 1024;
    pub const STORAGE: usize = size_of::<(Self, Storage)>();

    /// Borrows a native proof-session description, not proof or readiness authority.
    /// All original inputs stay prepaid; the full output charge is unreserved.
    pub fn ready(
        session: &Session,
        currentness_gate: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(session.retained_storage(), budget, |_| {
            Ok((
                Self::encode(
                    Kind::CurrentnessReady,
                    session.transcript(),
                    session.identity(),
                    currentness_gate,
                )?,
                Storage(Self::STORAGE),
            ))
        })
    }
    /// Describes the exact preceding native Ready. This does not run VerifyCurrent
    /// or authorize writing the original ACK pipe; live ownership is mandatory.
    pub fn acknowledge(ready: &Self, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(Self::STORAGE, budget, |_| {
            if ready.kind != Kind::CurrentnessReady {
                return Err(Error::Kind);
            }
            Ok((
                Self::encode(
                    Kind::Acknowledgment,
                    ready.transcript,
                    ready.proof_session_identity(),
                    ready.currentness_gate_identity(),
                )?,
                Storage(Self::STORAGE),
            ))
        })
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(
            if bytes.len() == BYTES { BYTES } else { 0 },
            budget,
            |budget| {
                if bytes.len() != BYTES {
                    return Err(Error::Length);
                }
                let kind = if &bytes[..8] == READY_MAGIC {
                    Kind::CurrentnessReady
                } else if &bytes[..8] == ACK_MAGIC {
                    Kind::Acknowledgment
                } else {
                    return Err(Error::Header);
                };
                if bytes[8..10] != 1_u16.to_le_bytes()
                    || bytes[10..12] != [0; 2]
                    || bytes[12..16] != (BYTES as u32).to_le_bytes()
                {
                    return Err(Error::Header);
                }
                let (transcript, storage) =
                    Transcript::decode(&bytes[16..112], budget).map_err(|_| Error::Binding)?;
                budget.reserve_storage(storage.additional_storage())?;
                let value = Self::encode(
                    kind,
                    transcript,
                    bytes[112..144].try_into().unwrap(),
                    bytes[144..176].try_into().unwrap(),
                )?;
                if value.bytes != bytes {
                    return Err(Error::Binding);
                }
                Ok((value, Storage(Self::STORAGE)))
            },
        )
    }
    pub fn check_acknowledgment(&self, ready: &Self, budget: &mut Budget<'_>) -> Result<()> {
        metered(2 * Self::STORAGE, budget, |_| {
            if self.kind != Kind::Acknowledgment || ready.kind != Kind::CurrentnessReady {
                return Err(Error::Kind);
            }
            if self.bytes[16..176] != ready.bytes[16..176] {
                return Err(Error::Binding);
            }
            Ok(())
        })
    }
    fn encode(
        kind: Kind,
        transcript: Transcript,
        session: [u8; 32],
        gate: [u8; 32],
    ) -> Result<Self> {
        if session == [0; 32] || gate == [0; 32] {
            return Err(Error::Binding);
        }
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(kind.magic());
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
        bytes[16..112].copy_from_slice(&transcript.canonical_bytes());
        bytes[112..144].copy_from_slice(&session);
        bytes[144..176].copy_from_slice(&gate);
        let mut hash = Sha256::new();
        hash.update(kind.domain());
        hash.update(&bytes[..176]);
        bytes[176..].copy_from_slice(&hash.finalize());
        Ok(Self {
            kind,
            transcript,
            bytes,
        })
    }
    pub const fn kind(&self) -> Kind {
        self.kind
    }
    pub const fn transcript(&self) -> Transcript {
        self.transcript
    }
    pub fn proof_session_identity(&self) -> [u8; 32] {
        self.bytes[112..144].try_into().unwrap()
    }
    pub fn currentness_gate_identity(&self) -> [u8; 32] {
        self.bytes[144..176].try_into().unwrap()
    }
    pub fn identity(&self) -> [u8; 32] {
        self.bytes[176..].try_into().unwrap()
    }
    pub const fn canonical_bytes(&self) -> &[u8; BYTES] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        Self::STORAGE
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}
fn metered<T>(
    floor: usize,
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Result<T> {
    budget.with_prepaid_scope(floor, 8, Record::WORK, Record::SCRATCH, run)
}

#[cfg(test)]
mod tests;
