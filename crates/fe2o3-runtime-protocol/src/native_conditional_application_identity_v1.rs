//! Inert, metered V5-readiness/V3-issuer association for application registration.
use crate::{
    InertConditionalWorkerReadinessWireV5 as Wire, MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionReceiptCarriageV3 as Carriage, CompilerExecutionSupervisorHandoffV3 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

const MAGIC: &[u8; 8] = b"F3NCAI1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-CONDITIONAL-APPLICATION/V1\0";
const HEADER: usize = 24;
const CHECKSUM: usize = HEADER + 5 * 32;
pub const NATIVE_CONDITIONAL_APPLICATION_BINDING_BYTES_V1: usize = CHECKSUM + 32;
const BYTES: usize = NATIVE_CONDITIONAL_APPLICATION_BINDING_BYTES_V1;
const SCRATCH: usize = 4 * size_of::<NativeConditionalApplicationBindingV1>()
    + 2 * size_of::<Sha256>()
    + 2 * size_of::<Wire<'static>>()
    + 4096;
const ENTRY: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalApplicationIdentityV1([u8; 32]);
impl NativeConditionalApplicationIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Fixed descriptive association, never process, proof, currentness or GPU authority.
///
/// The readiness view authenticates no nested source or finalizer evidence. A consumer
/// must retain/recover the original V5 publication, authenticate the V3 policy and live
/// issuer independently, and check actual descriptor/pidfd custody before registration.
/// This type has no conversion to the legacy Worker V3 application binding.
///
/// ```compile_fail
/// use fe2o3_runtime_protocol::{NativeConditionalApplicationBindingV1 as Native,
///     WorkerV3ApplicationRegistrationBindingV1 as Legacy};
/// fn downgrade(value: Native) -> Legacy { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_runtime_protocol::NativeConditionalApplicationBindingV1;
/// fn duplicate(value: NativeConditionalApplicationBindingV1) { let _ = value.clone(); }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct NativeConditionalApplicationBindingV1 {
    bytes: [u8; BYTES],
}

/// Logical units on the caller's existing account, not instruction/RSS bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalApplicationBindingQuoteV1 {
    input_floor: usize,
    work: usize,
}
impl NativeConditionalApplicationBindingQuoteV1 {
    pub const fn input_floor(self) -> usize {
        self.input_floor
    }
    pub const fn work(self) -> usize {
        self.work
    }
    pub const fn scratch(self) -> usize {
        SCRATCH
    }
    /// Returned unreserved; borrowed inputs keep their original charges.
    pub const fn retained_storage(self) -> usize {
        size_of::<NativeConditionalApplicationBindingV1>()
    }
}

#[derive(Debug)]
pub enum NativeConditionalApplicationBindingErrorV1 {
    Resource(Resource),
    Readiness(crate::ConditionalWorkerReadinessCodecErrorV5),
    Length,
    Header,
    Identity,
    PolicyMismatch,
    CarriageMismatch,
}
type Error = NativeConditionalApplicationBindingErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native conditional application binding: {self:?}")
    }
}
impl std::error::Error for Error {}

fn quote(length: usize, owners: usize) -> Result<NativeConditionalApplicationBindingQuoteV1> {
    if length == 0 || length > MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 {
        return Err(Error::Length);
    }
    let input_floor = length.checked_add(owners).ok_or(Resource::Arithmetic)?;
    let work = input_floor
        .checked_add(BYTES)
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_add(ENTRY))
        .ok_or(Resource::Arithmetic)?;
    Ok(NativeConditionalApplicationBindingQuoteV1 { input_floor, work })
}

impl NativeConditionalApplicationBindingV1 {
    pub fn binding_quote(
        length: usize,
        handoff: &Handoff,
        carriage: &Carriage,
    ) -> Result<NativeConditionalApplicationBindingQuoteV1> {
        let owners = handoff
            .retained_storage()
            .checked_add(carriage.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        quote(length, owners)
    }

    /// Borrows fully prepaid V3 owners and readiness bytes. All work, scratch and
    /// failures use the original account; reserve the quoted result charge before
    /// retaining it. This checks association, not policy provenance or replay.
    pub fn bind(
        readiness: &[u8],
        handoff: &Handoff,
        carriage: &Carriage,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(ENTRY)?;
        let q = Self::binding_quote(readiness.len(), handoff, carriage)?;
        budget.with_prepaid_scope(q.input_floor, 0, q.work - ENTRY, SCRATCH, |_| {
            if handoff.launch_manifest().policy_identity() != carriage.policy().identity() {
                return Err(Error::PolicyMismatch);
            }
            let wire = Wire::decode(readiness, MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5)
                .map_err(Error::Readiness)?;
            if wire.compiler_execution_bytes() != carriage.canonical_bytes() {
                return Err(Error::CarriageMismatch);
            }
            let mut bytes = [0; BYTES];
            bytes[..8].copy_from_slice(MAGIC);
            bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
            bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
            bytes[16..HEADER].copy_from_slice(&(readiness.len() as u64).to_le_bytes());
            let identities = [
                <[u8; 32]>::from(Sha256::digest(readiness)),
                *handoff.identity().as_bytes(),
                *carriage.identity().as_bytes(),
                *carriage.policy().identity().as_bytes(),
                *carriage.request().subject().identity().sha256(),
            ];
            for (field, identity) in bytes[HEADER..CHECKSUM].chunks_exact_mut(32).zip(identities) {
                if identity == [0; 32] {
                    return Err(Error::Identity);
                }
                field.copy_from_slice(&identity);
            }
            let identity = checksum(&bytes[..CHECKSUM]);
            bytes[CHECKSUM..].copy_from_slice(&identity);
            Ok(Self { bytes })
        })
    }

    pub const fn decoding_quote() -> NativeConditionalApplicationBindingQuoteV1 {
        NativeConditionalApplicationBindingQuoteV1 {
            input_floor: BYTES,
            work: ENTRY + 4 * BYTES,
        }
    }

    /// Fixed, inert framing only. Decoding never reconstructs a V3 owner or admits V5 replay.
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<Self> {
        let q = Self::decoding_quote();
        let floor = if bytes.len() == BYTES {
            q.input_floor
        } else {
            0
        };
        budget.with_prepaid_scope(floor, ENTRY, q.work, SCRATCH, |_| {
            let bytes: &[u8; BYTES] = bytes.try_into().map_err(|_| Error::Length)?;
            if &bytes[..8] != MAGIC
                || bytes[8..10] != 1u16.to_le_bytes()
                || bytes[10..12] != [0; 2]
                || bytes[12..16] != (BYTES as u32).to_le_bytes()
            {
                return Err(Error::Header);
            }
            let length = u64::from_le_bytes(bytes[16..HEADER].try_into().unwrap());
            if length == 0 || length > MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 as u64 {
                return Err(Error::Length);
            }
            if bytes[HEADER..CHECKSUM]
                .chunks_exact(32)
                .any(|field| field == [0; 32])
                || bytes[CHECKSUM..] != checksum(&bytes[..CHECKSUM])
            {
                return Err(Error::Identity);
            }
            Ok(Self { bytes: *bytes })
        })
    }

    pub const fn canonical_bytes(&self) -> &[u8; BYTES] {
        &self.bytes
    }
    pub fn identity(&self) -> NativeConditionalApplicationIdentityV1 {
        NativeConditionalApplicationIdentityV1(self.bytes[CHECKSUM..].try_into().unwrap())
    }
    pub fn readiness_byte_len(&self) -> u64 {
        u64::from_le_bytes(self.bytes[16..HEADER].try_into().unwrap())
    }
    /// Descriptive hash of the exact V5 readiness bytes, not source authentication.
    pub fn readiness_sha256(&self) -> [u8; 32] {
        self.bytes[HEADER..HEADER + 32].try_into().unwrap()
    }
    /// Opaque association to a handoff; consumers still authenticate its actual owner.
    pub fn compiler_handoff_identity(&self) -> [u8; 32] {
        self.bytes[HEADER + 32..HEADER + 64].try_into().unwrap()
    }
    /// Exact descriptive carriage association, not authentication of its issuer.
    pub fn carriage_identity(&self) -> [u8; 32] {
        self.bytes[HEADER + 64..HEADER + 96].try_into().unwrap()
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

#[cfg(test)]
mod tests;
