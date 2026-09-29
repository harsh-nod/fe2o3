//! Inert root/issuer framing. Authentication and custody remain with the broker.
use crate::{
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::{error::Error as StdError, fmt, mem::size_of};

/// One exact SEQPACKET payload, including zero padding and identity digest.
pub const COMPILER_EXECUTION_ROOT_CONTROL_BYTES_V3: usize = 4096;
const N: usize = COMPILER_EXECUTION_ROOT_CONTROL_BYTES_V3;
const BODY: usize = 200;
const DIGEST: usize = N - 32;
/// Bounded opaque broker payload. Its nested schema is NOT admitted by this codec.
pub const COMPILER_EXECUTION_ROOT_CONTROL_PAYLOAD_BYTES_V3: usize = DIGEST - BODY;
const MAGIC: &[u8; 8] = b"F2O3CRC3";
const DOMAIN: &[u8] = b"FE2O3/ROOT-ISSUER-CONTROL/V3\0";
const RETAINED: usize = size_of::<(CompilerExecutionRootControlRecordV3, Storage)>();
const BINDING_RETAINED: usize = size_of::<(CompilerExecutionRootControlBindingV3, Storage)>();
/// Fixed logical framing/hash/copy work, not an instruction or wall-time bound.
pub const COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3: usize = resources::ENTRY_WORK + 32 * N;
/// Additional logical scratch above prepaid borrowed inputs, not stack/RSS.
pub const COMPILER_EXECUTION_ROOT_CONTROL_STORAGE_V3: usize =
    4 * RETAINED + 4 * N + 2 * size_of::<Sha256>() + 4096;

/// Closed operation labels, not permission to execute a transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum CompilerExecutionRootControlKindV3 {
    Reconcile = 1,
    Observe = 2,
    Validate = 3,
    Retire = 4,
}
use CompilerExecutionRootControlKindV3 as Kind;

/// Inert association data, never root or issuer admission. The root epoch must
/// come from the actual retained attempt; every replacement connection needs a
/// new independently generated nonzero generation. This codec cannot prove
/// freshness, original trace custody, peer credentials, or runtime measurement.
#[derive(Debug, Eq, PartialEq)]
pub struct CompilerExecutionRootControlBindingV3 {
    policy: [u8; 32],
    manifest: [u8; 32],
    epoch: [u8; 32],
    generation: [u8; 32],
}
use CompilerExecutionRootControlBindingV3 as Binding;

impl Binding {
    pub fn new(
        policy: &Policy,
        manifest: &Manifest,
        epoch: [u8; 32],
        generation: [u8; 32],
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(
            b,
            policy.retained_storage() + manifest.retained_storage(),
            || {
                if manifest.policy_identity() != policy.identity() {
                    return Err(Error::Framing("root control manifest policy"));
                }
                let binding = Self {
                    policy: *policy.identity().as_bytes(),
                    manifest: *manifest.identity().as_bytes(),
                    epoch,
                    generation,
                };
                binding.validate()?;
                Ok((binding, Storage(BINDING_RETAINED)))
            },
        )
    }

    fn validate(&self) -> Result<()> {
        if [self.policy, self.manifest, self.epoch, self.generation].contains(&[0; 32]) {
            return Err(Error::Framing("zero root control association"));
        }
        Ok(())
    }

    pub const fn retained_storage(&self) -> usize {
        BINDING_RETAINED
    }
}

/// Move-only canonical bytes, without custody, signing, retirement, or launch
/// authority. The digest detects corruption and binds replies; it is NOT a MAC.
/// A correctly hashed payload may still be entirely hostile.
///
/// Before a transition, the broker must authenticate the actual retained sender,
/// match this record against its independently held binding, enforce cumulative
/// sequence/replay limits, and admit the operation-specific payload. In
/// particular a Retire reply does not prove durable completion or lock release.
///
/// Constructors and decode return FULL UNRESERVED storage. Keep borrowed owners
/// prepaid on the original budget; scopes restore entry storage on success,
/// refusal and unwind without refunding work or denial history.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionRootControlRecordV3 as R;
/// fn duplicate(r: R) { let _ = r.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionRootControlRecordV3 as R,
///     CompilerExecutionReceiptPublicationAckV3 as A};
/// fn retirement(r: R) -> A { r.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionRootControlBindingV3 as B,
///     CompilerExecutionIssuerPolicyV2 as P, CompilerExecutionServiceLaunchManifestV3 as M};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(p: &P, m: &M, b: &mut Budget<'_>) { B::new(p, m, [1;32], [2;32], b); }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionRootControlRecordV3 {
    bytes: [u8; N],
}
use CompilerExecutionRootControlRecordV3 as Record;

impl Record {
    pub fn request(
        binding: &Binding,
        sequence: u64,
        kind: Kind,
        payload: &[u8],
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(
            b,
            binding.retained_storage() + payload_floor(payload),
            || {
                binding.validate()?;
                let mut bytes = header(sequence, kind, payload)?;
                bytes[24..56].copy_from_slice(&binding.policy);
                bytes[56..88].copy_from_slice(&binding.manifest);
                bytes[88..120].copy_from_slice(&binding.epoch);
                bytes[120..152].copy_from_slice(&binding.generation);
                Ok(finish(bytes))
            },
        )
    }

    /// Replies preserve the exact request association, sequence, operation and
    /// digest. This does not classify the broker payload as a successful result.
    pub fn reply(request: &Self, payload: &[u8], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, RETAINED + payload_floor(payload), || {
            if request.is_reply() {
                return Err(Error::Framing("reply requires a root control request"));
            }
            let mut bytes = header(request.sequence(), request.kind(), payload)?;
            bytes[20] = 1;
            bytes[24..152].copy_from_slice(&request.bytes[24..152]);
            bytes[160..192].copy_from_slice(request.identity());
            Ok(finish(bytes))
        })
    }

    pub fn decode(bytes: &[u8], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, resources::fixed_input_floor(bytes, N), || {
            if bytes.len() != N {
                return Err(Error::Framing("root control length"));
            }
            if &bytes[..8] != MAGIC
                || bytes[8..10] != 3u16.to_le_bytes()
                || bytes[12..20] != (N as u64).to_le_bytes()
            {
                return Err(Error::Framing("root control header"));
            }
            kind(bytes)?;
            let length = u32::from_le_bytes(fixed(&bytes[192..196])) as usize;
            if length > COMPILER_EXECUTION_ROOT_CONTROL_PAYLOAD_BYTES_V3 {
                return Err(Error::Framing("root control payload length"));
            }
            if bytes[20] > 1
                || bytes[21..24] != [0; 3]
                || bytes[196..200] != [0; 4]
                || bytes[BODY + length..DIGEST].iter().any(|&v| v != 0)
            {
                return Err(Error::Framing("root control reserved bytes"));
            }
            for start in [24, 56, 88, 120] {
                if bytes[start..start + 32] == [0; 32] {
                    return Err(Error::Framing("zero root control association"));
                }
            }
            if u64::from_le_bytes(fixed(&bytes[152..160])) == 0
                || (bytes[160..192] == [0; 32]) != (bytes[20] == 0)
            {
                return Err(Error::Framing("root control sequence or reply join"));
            }
            if bytes[DIGEST..] != digest(bytes) {
                return Err(Error::Framing("root control identity"));
            }
            Ok((
                Self {
                    bytes: fixed(bytes),
                },
                Storage(RETAINED),
            ))
        })
    }

    /// Pure association comparison, not credential authentication or admission.
    pub fn matches_binding(&self, binding: &Binding, b: &mut Budget<'_>) -> Result<bool> {
        metered(b, RETAINED + BINDING_RETAINED, || {
            Ok(self.bytes[24..56] == binding.policy
                && self.bytes[56..88] == binding.manifest
                && self.bytes[88..120] == binding.epoch
                && self.bytes[120..152] == binding.generation)
        })
    }

    /// Includes the original payload through its digest, not just an operation
    /// or numeric sequence. The caller must still check its own pending request.
    pub fn matches_reply(&self, request: &Self, b: &mut Budget<'_>) -> Result<bool> {
        metered(b, 2 * RETAINED, || {
            Ok(self.is_reply()
                && !request.is_reply()
                && self.kind() == request.kind()
                && self.bytes[24..160] == request.bytes[24..160]
                && self.bytes[160..192] == *request.identity())
        })
    }

    pub fn kind(&self) -> Kind {
        kind(&self.bytes).expect("admitted root control kind")
    }
    pub const fn is_reply(&self) -> bool {
        self.bytes[20] == 1
    }
    pub fn sequence(&self) -> u64 {
        u64::from_le_bytes(fixed(&self.bytes[152..160]))
    }
    pub fn payload(&self) -> &[u8] {
        let length = u32::from_le_bytes(fixed(&self.bytes[192..196])) as usize;
        &self.bytes[BODY..BODY + length]
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

fn header(sequence: u64, kind: Kind, payload: &[u8]) -> Result<[u8; N]> {
    if sequence == 0 || payload.len() > COMPILER_EXECUTION_ROOT_CONTROL_PAYLOAD_BYTES_V3 {
        return Err(Error::Framing("root control sequence or payload length"));
    }
    let mut bytes = [0; N];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8..10].copy_from_slice(&3u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&(kind as u16).to_le_bytes());
    bytes[12..20].copy_from_slice(&(N as u64).to_le_bytes());
    bytes[152..160].copy_from_slice(&sequence.to_le_bytes());
    bytes[192..196].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes[BODY..BODY + payload.len()].copy_from_slice(payload);
    Ok(bytes)
}
fn kind(bytes: &[u8]) -> Result<Kind> {
    match u16::from_le_bytes(fixed(&bytes[10..12])) {
        1 => Ok(Kind::Reconcile),
        2 => Ok(Kind::Observe),
        3 => Ok(Kind::Validate),
        4 => Ok(Kind::Retire),
        _ => Err(Error::Framing("root control kind")),
    }
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update(&bytes[..DIGEST]);
    hash.finalize().into()
}
fn finish(mut bytes: [u8; N]) -> (Record, Storage) {
    let identity = digest(&bytes);
    bytes[DIGEST..].copy_from_slice(&identity);
    (Record { bytes }, Storage(RETAINED))
}
fn fixed<const N: usize>(bytes: &[u8]) -> [u8; N] {
    bytes.try_into().expect("checked fixed slice")
}
fn payload_floor(payload: &[u8]) -> usize {
    if payload.len() <= COMPILER_EXECUTION_ROOT_CONTROL_PAYLOAD_BYTES_V3 {
        payload.len()
    } else {
        0
    }
}
fn metered<T>(
    b: &mut Budget<'_>,
    floor: usize,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    resources::fixed(
        b,
        floor,
        COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3,
        COMPILER_EXECUTION_ROOT_CONTROL_STORAGE_V3,
        operation,
    )
}

#[derive(Debug)]
pub enum CompilerExecutionRootControlErrorV3 {
    Framing(&'static str),
    Resource(Resource),
}
use CompilerExecutionRootControlErrorV3 as Error;
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
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Framing(_) => None,
            Self::Resource(e) => Some(e),
        }
    }
}
impl fmt::Debug for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionRootControlRecordV3")
            .field("kind", &self.kind())
            .field("sequence", &self.sequence())
            .field("reply", &self.is_reply())
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

const _: () = {
    assert!(
        crate::COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V3
            <= COMPILER_EXECUTION_ROOT_CONTROL_PAYLOAD_BYTES_V3
    );
    assert!(8 * size_of::<Error>() + 8 * BINDING_RETAINED + 64 * size_of::<usize>() <= 4096);
};

#[cfg(test)]
#[path = "root_control_v3_tests.rs"]
mod tests;
