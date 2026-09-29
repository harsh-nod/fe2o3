//! Inert original-root intake transcript, never invocation or process authority.
use crate::{
    CompilerExecutionIssuerPolicyV3 as Policy,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3;
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

pub const COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V3: usize = 224;
const N: usize = COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V3;
const MAGIC: &[u8; 8] = b"F2O3CRI3";
const DOMAIN: &[u8] = b"FE2O3/COMPILER-ROOT-INTAKE/V3\0";
const DIGEST: usize = N - 32;
const RETAINED: usize = size_of::<(CompilerExecutionRootIntakeRecordV3, Storage)>();
pub const COMPILER_EXECUTION_ROOT_INTAKE_WORK_V3: usize = resources::ENTRY_WORK + 32 * N;
pub const COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V3: usize =
    8 * RETAINED + 8 * N + 2 * size_of::<Sha256>() + 4096;

/// Closed transport phases. None grants authority or reports readiness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompilerExecutionRootIntakeKindV3 {
    Hello = 1,
    Challenge = 2,
    Input = 3,
    Ack = 4,
}
use CompilerExecutionRootIntakeKindV3 as Kind;

/// Exact descriptor ordering; input count follows the stdio mask, not a wire count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompilerExecutionRootIntakeRoleV3 {
    Invocation = 1,
    WorkingDirectory = 2,
    Stdin = 3,
    Stdout = 4,
    Stderr = 5,
}
use CompilerExecutionRootIntakeRoleV3 as Role;

/// Fixed canonical transcript bytes. Hashes detect corruption and bind frames;
/// they do NOT authenticate a peer, prove nonce freshness, admit an invocation,
/// establish original-root custody, or authorize an FD195 transition.
///
/// Hello/challenge/ACK carry zero rights; each ordered Input carries exactly one.
/// The receiver must enforce those transport facts, actual packet credentials,
/// the complete role sequence and retained ownership before sending a reply.
/// The sole ACK status is terminal RuntimeEnforcementUnavailable, NOT success.
/// EOF or a partial transcript is refusal, never an implicit ACK.
///
/// `invocation_bytes` always declares the complete sealed invocation file, even
/// on cwd/stdio frames. This codec checks conversion to usize and the existing
/// MAX_DESCRIPTOR_BYTES_V3; the consumer checks exact actual sealed length BEFORE
/// allocation/read. Framing grants no size allowance on the caller's account.
///
/// Constructors/decode return full UNRESERVED storage. Keep input owners prepaid
/// on the original account; work and denial history are never refunded.
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionRootIntakeRecordV3 {
    bytes: [u8; N],
}
use CompilerExecutionRootIntakeRecordV3 as Record;

impl Record {
    pub fn hello(
        policy: &Policy,
        invocation: [u8; 32],
        nonce: [u8; 32],
        stdio_mask: u8,
        invocation_bytes: u64,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(b, policy.retained_storage(), || {
            let mut bytes = [0; N];
            bytes[..8].copy_from_slice(MAGIC);
            bytes[8..12].copy_from_slice(&3u32.to_le_bytes());
            bytes[12..16].copy_from_slice(&(N as u32).to_le_bytes());
            bytes[16] = Kind::Hello as u8;
            bytes[18] = stdio_mask;
            bytes[24..56].copy_from_slice(&nonce);
            bytes[88..120].copy_from_slice(policy.identity().as_bytes());
            bytes[120..152].copy_from_slice(&invocation);
            bytes[184..192].copy_from_slice(&invocation_bytes.to_le_bytes());
            finish(bytes)
        })
    }

    pub fn challenge(hello: &Self, nonce: [u8; 32], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, RETAINED, || {
            if hello.kind() != Kind::Hello {
                return Err(Error::Framing("challenge requires hello"));
            }
            let mut bytes = hello.bytes;
            bytes[16] = Kind::Challenge as u8;
            bytes[56..88].copy_from_slice(&nonce);
            bytes[152..184].copy_from_slice(hello.identity());
            finish(bytes)
        })
    }

    pub fn input(challenge: &Self, role: Role, b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, RETAINED, || {
            if challenge.kind() != Kind::Challenge || !challenge.roles().any(|r| r == role) {
                return Err(Error::Framing("input requires challenge and selected role"));
            }
            let mut bytes = challenge.bytes;
            bytes[16] = Kind::Input as u8;
            bytes[17] = role as u8;
            bytes[152..184].copy_from_slice(challenge.identity());
            finish(bytes)
        })
    }

    /// Inert terminal refusal only. A decoder/caller can fabricate this record;
    /// only actual authenticated receiver custody may send it as a decision.
    /// A last-role record does not prove receipt/retention of earlier descriptors.
    pub fn enforcement_unavailable(last: &Self, b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, RETAINED, || {
            if last.kind() != Kind::Input || last.role() != last.roles().last() {
                return Err(Error::Framing("refusal requires final selected input"));
            }
            let mut bytes = last.bytes;
            bytes[16] = Kind::Ack as u8;
            bytes[17] = 0;
            bytes[19] = 1;
            bytes[152..184].copy_from_slice(last.identity());
            finish(bytes)
        })
    }

    pub fn decode(bytes: &[u8], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, resources::fixed_input_floor(bytes, N), || {
            if bytes.len() != N {
                return Err(Error::Framing("root intake length"));
            }
            validate(bytes)?;
            if bytes[DIGEST..] != digest(bytes) {
                return Err(Error::Framing("root intake digest"));
            }
            Ok((
                Self {
                    bytes: fixed(bytes),
                },
                Storage(RETAINED),
            ))
        })
    }

    /// Exact inert predecessor join. The caller separately authenticates sender,
    /// enforces no extra/missing rights and retains the complete input sequence.
    pub fn matches_predecessor(&self, previous: &Self, b: &mut Budget<'_>) -> Result<bool> {
        metered(b, 2 * RETAINED, || {
            let phase = matches!(
                (previous.kind(), self.kind()),
                (Kind::Hello, Kind::Challenge) | (Kind::Challenge, Kind::Input)
            ) || (previous.kind() == Kind::Input
                && self.kind() == Kind::Ack
                && previous.role() == previous.roles().last());
            Ok(phase
                && self.bytes[18] == previous.bytes[18]
                && self.bytes[24..56] == previous.bytes[24..56]
                && self.bytes[88..152] == previous.bytes[88..152]
                && self.bytes[184..192] == previous.bytes[184..192]
                && (previous.kind() == Kind::Hello || self.bytes[56..88] == previous.bytes[56..88])
                && self.bytes[152..184] == *previous.identity())
        })
    }

    pub fn kind(&self) -> Kind {
        match self.bytes[16] {
            1 => Kind::Hello,
            2 => Kind::Challenge,
            3 => Kind::Input,
            4 => Kind::Ack,
            _ => unreachable!("validated intake kind"),
        }
    }
    pub fn role(&self) -> Option<Role> {
        role(self.bytes[17])
    }
    pub fn roles(&self) -> impl Iterator<Item = Role> {
        [
            Some(Role::Invocation),
            Some(Role::WorkingDirectory),
            (self.bytes[18] & 1 != 0).then_some(Role::Stdin),
            (self.bytes[18] & 2 != 0).then_some(Role::Stdout),
            (self.bytes[18] & 4 != 0).then_some(Role::Stderr),
        ]
        .into_iter()
        .flatten()
    }
    pub const fn stdio_mask(&self) -> u8 {
        self.bytes[18]
    }
    pub fn policy_identity(&self) -> &[u8; 32] {
        self.bytes[88..120].try_into().expect("fixed policy")
    }
    pub fn invocation_identity(&self) -> &[u8; 32] {
        self.bytes[120..152].try_into().expect("fixed invocation")
    }
    pub fn invocation_bytes(&self) -> u64 {
        u64::from_le_bytes(fixed(&self.bytes[184..192]))
    }
    pub fn identity(&self) -> &[u8; 32] {
        self.bytes[DIGEST..].try_into().expect("fixed digest")
    }
    pub const fn canonical_bytes(&self) -> &[u8; N] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}

fn role(byte: u8) -> Option<Role> {
    match byte {
        1 => Some(Role::Invocation),
        2 => Some(Role::WorkingDirectory),
        3 => Some(Role::Stdin),
        4 => Some(Role::Stdout),
        5 => Some(Role::Stderr),
        _ => None,
    }
}
fn validate(bytes: &[u8]) -> Result<()> {
    let length = usize::try_from(u64::from_le_bytes(fixed(&bytes[184..192])))
        .map_err(|_| Error::Framing("root intake length overflows usize"))?;
    if &bytes[..8] != MAGIC
        || bytes[8..12] != 3u32.to_le_bytes()
        || bytes[12..16] != (N as u32).to_le_bytes()
        || bytes[20..24] != [0; 4]
        || bytes[18] & !7 != 0
        || !(1..=MAX_DESCRIPTOR_BYTES_V3).contains(&length)
    {
        return Err(Error::Framing("root intake header or bound"));
    }
    for start in [24, 88, 120] {
        if bytes[start..start + 32] == [0; 32] {
            return Err(Error::Framing("zero intake association"));
        }
    }
    let hello = bytes[16] == Kind::Hello as u8;
    if (bytes[56..88] == [0; 32]) != hello || (bytes[152..184] == [0; 32]) != hello {
        return Err(Error::Framing("intake challenge or predecessor"));
    }
    match bytes[16] {
        1 | 2 if bytes[17] == 0 && bytes[19] == 0 => Ok(()),
        3 if bytes[19] == 0 => {
            let selected = match role(bytes[17]) {
                Some(Role::Invocation | Role::WorkingDirectory) => true,
                Some(Role::Stdin) => bytes[18] & 1 != 0,
                Some(Role::Stdout) => bytes[18] & 2 != 0,
                Some(Role::Stderr) => bytes[18] & 4 != 0,
                None => false,
            };
            if selected {
                Ok(())
            } else {
                Err(Error::Framing("unselected intake role"))
            }
        }
        4 if bytes[17] == 0 && bytes[19] == 1 => Ok(()),
        _ => Err(Error::Framing("intake kind, role or refusal status")),
    }
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update(&bytes[..DIGEST]);
    hash.finalize().into()
}
fn finish(mut bytes: [u8; N]) -> Result<(Record, Storage)> {
    validate(&bytes)?;
    let hash = digest(&bytes);
    bytes[DIGEST..].copy_from_slice(&hash);
    Ok((Record { bytes }, Storage(RETAINED)))
}
fn fixed<const M: usize>(bytes: &[u8]) -> [u8; M] {
    bytes.try_into().expect("fixed intake field")
}
fn metered<T>(
    b: &mut Budget<'_>,
    floor: usize,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    b.with_prepaid_scope(
        floor,
        resources::ENTRY_WORK,
        COMPILER_EXECUTION_ROOT_INTAKE_WORK_V3,
        COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V3,
        |_| operation(),
    )
}

#[derive(Debug)]
pub enum CompilerExecutionRootIntakeErrorV3 {
    Framing(&'static str),
    Resource(Resource),
}
use CompilerExecutionRootIntakeErrorV3 as Error;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(reason) => f.write_str(reason),
            Self::Resource(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            _ => None,
        }
    }
}
impl fmt::Debug for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionRootIntakeRecordV3")
            .field("kind", &self.kind())
            .field("role", &self.role())
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "root_intake_v3_tests.rs"]
mod tests;
