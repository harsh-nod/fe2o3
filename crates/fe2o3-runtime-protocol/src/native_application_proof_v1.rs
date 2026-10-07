//! Metered native proof transport descriptions, never proof or currentness authority.
use crate::{
    MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5, NativeApplicationProofSessionV1 as Session,
    NativeApplicationRegistrationBindingV1 as Registration,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

mod message;
pub use message::{
    NATIVE_APPLICATION_PROOF_MAX_PACKET_BYTES_V1, NativeApplicationProofKindV1,
    NativeApplicationProofMessageV1,
};

pub const NATIVE_APPLICATION_PROOF_INPUT_BYTES_V1: usize = 232;
pub const NATIVE_APPLICATION_PROOF_EVIDENCE_BYTES_V1: usize = 808;
pub const NATIVE_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1: usize = 64 * 1024 * 1024;
/// A bound on described components, not permission to enlarge their own decoders.
pub const NATIVE_APPLICATION_PROOF_MAX_COMPONENT_BYTES_V1: usize = 256 * 1024 * 1024;
const INPUT_BYTES: usize = NATIVE_APPLICATION_PROOF_INPUT_BYTES_V1;
const EVIDENCE_BYTES: usize = NATIVE_APPLICATION_PROOF_EVIDENCE_BYTES_V1;
const INPUT_MAGIC: &[u8; 8] = b"F3NPIN1\0";
const EVIDENCE_MAGIC: &[u8; 8] = b"F3NPEV1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION-PROOF-TRANSPORT/V1\0";
const HEADER: usize = 24;
const ENTRY: usize = 8;
// Fixed logical codec limits; these do not measure process RSS or machine instructions.
const WORK: usize = 64 * 1024;
const SCRATCH: usize = 64 * 1024;
const BOUNDARY: u16 = 6;

#[derive(Debug)]
pub enum NativeApplicationProofErrorV1 {
    Resource(Resource),
    Length,
    Header,
    Identity,
    Association,
    Boundary,
    Kind,
    Sequence,
}
type Error = NativeApplicationProofErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native application proof transport: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Full unreserved output charge. Borrowed inputs keep their original charges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationProofStorageV1(usize);
impl NativeApplicationProofStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
use self::NativeApplicationProofStorageV1 as Storage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationProofQuoteV1 {
    input_floor: usize,
    work: usize,
    scratch: usize,
    retained: usize,
}
impl NativeApplicationProofQuoteV1 {
    pub const fn input_floor(self) -> usize {
        self.input_floor
    }
    pub const fn work(self) -> usize {
        self.work
    }
    pub const fn scratch(self) -> usize {
        self.scratch
    }
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
type Quote = NativeApplicationProofQuoteV1;
const fn quote(input_floor: usize, retained: usize) -> Quote {
    Quote {
        input_floor,
        work: WORK,
        scratch: SCRATCH,
        retained,
    }
}
fn metered<T>(
    exact_length: bool,
    q: Quote,
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Result<T> {
    budget.with_prepaid_scope(
        if exact_length { q.input_floor } else { 0 },
        ENTRY,
        q.work,
        q.scratch,
        run,
    )
}

/// Exact two-file request description bound to an inert native session/registration.
/// No file, source replay, nonce freshness, process or currentness is authenticated.
///
/// ```compile_fail
/// use fe2o3_runtime_protocol::{NativeApplicationProofInputsV1 as Native,
///     WorkerV3ApplicationProofInputsV1 as Legacy};
/// fn downgrade(value: Native) -> Legacy { value.into() }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationProofInputsV1 {
    bytes: [u8; INPUT_BYTES],
}
type Inputs = NativeApplicationProofInputsV1;
impl Inputs {
    const RETAINED: usize = size_of::<Self>() + size_of::<Storage>();
    pub fn construction_quote(session: &Session, registration: &Registration) -> Quote {
        quote(
            session.retained_storage() + registration.retained_storage(),
            Self::RETAINED,
        )
    }
    /// Payload identity is caller-declared and must be recomputed from the actual
    /// second sealed file. Readiness identity is copied from the original binding.
    pub fn new(
        session: &Session,
        registration: &Registration,
        payload: ([u8; 32], u64),
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(
            true,
            Self::construction_quote(session, registration),
            budget,
            |_| {
                if session.transcript().binding() != *registration.identity().as_bytes() {
                    return Err(Error::Association);
                }
                let value = Self::encode(
                    session.identity(),
                    *registration.identity().as_bytes(),
                    session.deployment(),
                    (
                        registration.inputs().readiness_sha256(),
                        registration.inputs().readiness_byte_len(),
                    ),
                    payload,
                )?;
                Ok((value, Storage(Self::RETAINED)))
            },
        )
    }
    pub const fn decoding_quote() -> Quote {
        quote(INPUT_BYTES, Self::RETAINED)
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(
            bytes.len() == INPUT_BYTES,
            Self::decoding_quote(),
            budget,
            |_| {
                header(bytes, INPUT_MAGIC, INPUT_BYTES)?;
                let value = Self::encode(
                    identity(bytes, 24),
                    identity(bytes, 56),
                    identity(bytes, 88),
                    blob(bytes, 120),
                    blob(bytes, 160),
                )?;
                if value.bytes != bytes {
                    return Err(Error::Identity);
                }
                Ok((value, Storage(Self::RETAINED)))
            },
        )
    }
    fn encode(
        session: [u8; 32],
        registration: [u8; 32],
        deployment: [u8; 32],
        readiness: ([u8; 32], u64),
        payload: ([u8; 32], u64),
    ) -> Result<Self> {
        for id in [session, registration, deployment] {
            nonzero(id)?;
        }
        valid_blob(readiness, MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5)?;
        valid_blob(payload, NATIVE_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1)?;
        let mut bytes = [0; INPUT_BYTES];
        write_header(&mut bytes, INPUT_MAGIC);
        bytes[24..56].copy_from_slice(&session);
        bytes[56..88].copy_from_slice(&registration);
        bytes[88..120].copy_from_slice(&deployment);
        put_blob(&mut bytes, 120, readiness);
        put_blob(&mut bytes, 160, payload);
        seal(&mut bytes);
        Ok(Self { bytes })
    }
    pub fn session_identity(&self) -> [u8; 32] {
        identity(&self.bytes, 24)
    }
    pub fn registration_identity(&self) -> [u8; 32] {
        identity(&self.bytes, 56)
    }
    pub fn deployment_identity(&self) -> [u8; 32] {
        identity(&self.bytes, 88)
    }
    pub fn readiness(&self) -> ([u8; 32], u64) {
        blob(&self.bytes, 120)
    }
    pub fn payload(&self) -> ([u8; 32], u64) {
        blob(&self.bytes, 160)
    }
    pub fn identity(&self) -> [u8; 32] {
        identity(&self.bytes, INPUT_BYTES - 32)
    }
    pub const fn canonical_bytes(&self) -> &[u8; INPUT_BYTES] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
    pub const fn authenticates_input_custody(&self) -> bool {
        false
    }
    pub fn registration_check_quote(registration: &Registration) -> Quote {
        quote(Self::RETAINED + registration.retained_storage(), 0)
    }
    /// Rechecks descriptive equality against the retained original registration;
    /// this does not authenticate either owner or the two sealed input files.
    pub fn check_registration(
        &self,
        registration: &Registration,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        metered(
            true,
            Self::registration_check_quote(registration),
            budget,
            |_| {
                if self.registration_identity() != *registration.identity().as_bytes()
                    || self.readiness()
                        != (
                            registration.inputs().readiness_sha256(),
                            registration.inputs().readiness_byte_len(),
                        )
                {
                    return Err(Error::Association);
                }
                Ok(())
            },
        )
    }
    fn matches_session(&self, session: &Session) -> Result<()> {
        if self.session_identity() != session.identity()
            || self.registration_identity() != session.transcript().binding()
            || self.deployment_identity() != session.deployment()
        {
            return Err(Error::Association);
        }
        Ok(())
    }
}

/// Caller-supplied descriptive hashes and exact byte lengths. Each digest of bytes
/// is plain SHA-256, not a substituted typed identity. The three V3 identities and
/// analyzer execution/challenge use their original typed owners' identity bytes.
/// Current-attestation identity is deliberately absent: currentness is a separate
/// host prerequisite, not a statement made by the content-proof custodian.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationProofEvidencePartsV1 {
    pub native_handoff: ([u8; 32], u64),
    pub final_kernel_ir: ([u8; 32], u64),
    pub analysis_request: ([u8; 32], u64),
    pub analysis_bundle: ([u8; 32], u64),
    pub analysis_receipt: ([u8; 32], u64),
    pub generated_source: ([u8; 32], u64),
    pub obligation: ([u8; 32], u64),
    pub signed_receipt: ([u8; 32], u64),
    pub analysis_execution_identity: [u8; 32],
    pub analysis_challenge: [u8; 32],
    pub receipt_verifying_key: [u8; 32],
    pub carriage_identity: [u8; 32],
    pub subject_identity: [u8; 32],
    pub policy_identity: [u8; 32],
}
type Parts = NativeApplicationProofEvidencePartsV1;
impl Parts {
    fn blobs(self) -> [([u8; 32], u64); 8] {
        [
            self.native_handoff,
            self.final_kernel_ir,
            self.analysis_request,
            self.analysis_bundle,
            self.analysis_receipt,
            self.generated_source,
            self.obligation,
            self.signed_receipt,
        ]
    }
    fn identities(self) -> [[u8; 32]; 6] {
        [
            self.analysis_execution_identity,
            self.analysis_challenge,
            self.receipt_verifying_key,
            self.carriage_identity,
            self.subject_identity,
            self.policy_identity,
        ]
    }
}

/// Inert boundary-6 evidence. A consumer must independently recompute every field
/// from original V5/carriage/analyzer/proof owners, retain their protected custody
/// and separately establish currentness. Neither decoding nor a self-signed receipt
/// grants authority. No receipt, source string or success flag is executed here.
///
/// ```compile_fail
/// use fe2o3_runtime_protocol::NativeApplicationProofEvidenceV1;
/// fn duplicate(value: NativeApplicationProofEvidenceV1) { let _ = value.clone(); }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationProofEvidenceV1 {
    bytes: [u8; EVIDENCE_BYTES],
}
type Evidence = NativeApplicationProofEvidenceV1;
impl Evidence {
    const RETAINED: usize = size_of::<Self>() + size_of::<Storage>();
    const PARTS: usize = HEADER + INPUT_BYTES + 8;
    const IDS: usize = Self::PARTS + 8 * 40;
    pub const fn construction_quote() -> Quote {
        quote(Inputs::RETAINED, Self::RETAINED)
    }
    pub fn new(inputs: &Inputs, parts: Parts, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(true, Self::construction_quote(), budget, |_| {
            Ok((Self::encode(inputs, parts)?, Storage(Self::RETAINED)))
        })
    }
    pub const fn decoding_quote() -> Quote {
        let inner = Inputs::decoding_quote();
        Quote {
            input_floor: EVIDENCE_BYTES,
            work: WORK + inner.work,
            scratch: SCRATCH + inner.scratch,
            retained: Self::RETAINED,
        }
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        let q = Self::decoding_quote();
        // Nested calls run on the same account; only this layer's work is prepaid here.
        metered(
            bytes.len() == EVIDENCE_BYTES,
            Quote {
                work: WORK,
                scratch: SCRATCH,
                ..q
            },
            budget,
            |b| {
                header(bytes, EVIDENCE_MAGIC, EVIDENCE_BYTES)?;
                if bytes[HEADER + INPUT_BYTES..Self::PARTS] != [6, 0, 0, 0, 0, 0, 0, 0] {
                    return Err(Error::Boundary);
                }
                let (inputs, storage) = Inputs::decode(&bytes[HEADER..HEADER + INPUT_BYTES], b)?;
                b.reserve_storage(storage.retained_storage())?;
                let parts = Self::read_parts(bytes);
                let value = Self::encode(&inputs, parts)?;
                if value.bytes != bytes {
                    return Err(Error::Identity);
                }
                Ok((value, Storage(Self::RETAINED)))
            },
        )
    }
    fn encode(inputs: &Inputs, parts: Parts) -> Result<Self> {
        for value in parts.blobs() {
            valid_blob(value, NATIVE_APPLICATION_PROOF_MAX_COMPONENT_BYTES_V1)?;
        }
        valid_blob(
            parts.native_handoff,
            fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_BYTES_V5,
        )?;
        valid_blob(parts.final_kernel_ir, fe2o3_kernel_ir::MAX_MODULE_BYTES_V1)?;
        for id in parts.identities() {
            nonzero(id)?;
        }
        let mut bytes = [0; EVIDENCE_BYTES];
        write_header(&mut bytes, EVIDENCE_MAGIC);
        bytes[HEADER..HEADER + INPUT_BYTES].copy_from_slice(inputs.canonical_bytes());
        bytes[HEADER + INPUT_BYTES..HEADER + INPUT_BYTES + 2]
            .copy_from_slice(&BOUNDARY.to_le_bytes());
        for (index, value) in parts.blobs().into_iter().enumerate() {
            put_blob(&mut bytes, Self::PARTS + index * 40, value);
        }
        for (index, id) in parts.identities().into_iter().enumerate() {
            bytes[Self::IDS + index * 32..Self::IDS + (index + 1) * 32].copy_from_slice(&id);
        }
        seal(&mut bytes);
        Ok(Self { bytes })
    }
    fn read_parts(bytes: &[u8]) -> Parts {
        Parts {
            native_handoff: blob(bytes, Self::PARTS),
            final_kernel_ir: blob(bytes, Self::PARTS + 40),
            analysis_request: blob(bytes, Self::PARTS + 80),
            analysis_bundle: blob(bytes, Self::PARTS + 120),
            analysis_receipt: blob(bytes, Self::PARTS + 160),
            generated_source: blob(bytes, Self::PARTS + 200),
            obligation: blob(bytes, Self::PARTS + 240),
            signed_receipt: blob(bytes, Self::PARTS + 280),
            analysis_execution_identity: identity(bytes, Self::IDS),
            analysis_challenge: identity(bytes, Self::IDS + 32),
            receipt_verifying_key: identity(bytes, Self::IDS + 64),
            carriage_identity: identity(bytes, Self::IDS + 96),
            subject_identity: identity(bytes, Self::IDS + 128),
            policy_identity: identity(bytes, Self::IDS + 160),
        }
    }
    pub fn parts(&self) -> Parts {
        Self::read_parts(&self.bytes)
    }
    pub const fn boundary(&self) -> u16 {
        BOUNDARY
    }
    pub fn session_identity(&self) -> [u8; 32] {
        identity(&self.bytes, HEADER + 24)
    }
    pub fn registration_identity(&self) -> [u8; 32] {
        identity(&self.bytes, HEADER + 56)
    }
    pub fn deployment_identity(&self) -> [u8; 32] {
        identity(&self.bytes, HEADER + 88)
    }
    pub fn inputs_identity(&self) -> [u8; 32] {
        identity(&self.bytes, HEADER + INPUT_BYTES - 32)
    }
    pub fn identity(&self) -> [u8; 32] {
        identity(&self.bytes, EVIDENCE_BYTES - 32)
    }
    pub fn decode_inputs(&self, budget: &mut Budget<'_>) -> Result<(Inputs, Storage)> {
        Inputs::decode(&self.bytes[HEADER..HEADER + INPUT_BYTES], budget)
    }
    pub const fn canonical_bytes(&self) -> &[u8; EVIDENCE_BYTES] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
    pub const fn authenticates_proof_execution(&self) -> bool {
        false
    }
    pub const fn authenticates_currentness(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
    pub const fn input_check_quote() -> Quote {
        quote(Self::RETAINED + Inputs::RETAINED, 0)
    }
    /// Exact native request equality only. The caller must still recompute the
    /// evidence parts from actual source/analyzer/proof owners and admit custody.
    pub fn check_inputs(&self, inputs: &Inputs, budget: &mut Budget<'_>) -> Result<()> {
        metered(true, Self::input_check_quote(), budget, |_| {
            if &self.bytes[HEADER..HEADER + INPUT_BYTES] != inputs.canonical_bytes() {
                return Err(Error::Association);
            }
            Ok(())
        })
    }
    fn matches_session(&self, session: &Session) -> Result<()> {
        if self.session_identity() != session.identity()
            || self.registration_identity() != session.transcript().binding()
            || self.deployment_identity() != session.deployment()
        {
            return Err(Error::Association);
        }
        Ok(())
    }
}

fn nonzero(value: [u8; 32]) -> Result<()> {
    if value == [0; 32] {
        Err(Error::Identity)
    } else {
        Ok(())
    }
}
fn valid_blob(value: ([u8; 32], u64), maximum: usize) -> Result<()> {
    nonzero(value.0)?;
    if value.1 == 0 || value.1 > maximum as u64 {
        return Err(Error::Length);
    }
    Ok(())
}
fn identity(bytes: &[u8], offset: usize) -> [u8; 32] {
    bytes[offset..offset + 32].try_into().unwrap()
}
fn blob(bytes: &[u8], offset: usize) -> ([u8; 32], u64) {
    (
        identity(bytes, offset),
        u64::from_le_bytes(bytes[offset + 32..offset + 40].try_into().unwrap()),
    )
}
fn put_blob(bytes: &mut [u8], offset: usize, value: ([u8; 32], u64)) {
    bytes[offset..offset + 32].copy_from_slice(&value.0);
    bytes[offset + 32..offset + 40].copy_from_slice(&value.1.to_le_bytes());
}
fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
fn seal(bytes: &mut [u8]) {
    let start = bytes.len() - 32;
    let digest = checksum(&bytes[..start]);
    bytes[start..].copy_from_slice(&digest);
}
fn write_header(bytes: &mut [u8], magic: &[u8; 8]) {
    let length = bytes.len() as u32;
    bytes[..8].copy_from_slice(magic);
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
}
fn header(bytes: &[u8], magic: &[u8; 8], length: usize) -> Result<()> {
    if bytes.len() != length {
        return Err(Error::Length);
    }
    if &bytes[..8] != magic
        || bytes[8..10] != 1u16.to_le_bytes()
        || bytes[10..12] != [0; 2]
        || bytes[12..16] != (length as u32).to_le_bytes()
        || bytes[16..HEADER] != [0; 8]
    {
        return Err(Error::Header);
    }
    if bytes[length - 32..] != checksum(&bytes[..length - 32]) {
        return Err(Error::Identity);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
