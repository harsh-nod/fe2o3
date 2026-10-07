//! Native-family, metered application descriptions. No serialized value is authority.
use crate::{
    MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5,
    NATIVE_CONDITIONAL_APPLICATION_BINDING_BYTES_V1 as ASSOCIATION_BYTES,
    NativeConditionalApplicationBindingErrorV1 as AssociationError,
    NativeConditionalApplicationBindingV1 as Association,
    WorkerV3ApplicationHandoffChallengeV1 as Challenge,
    WorkerV3ApplicationHandoffCodecBudgetV1 as CodecBudget,
    WorkerV3ApplicationInputOccurrenceV1 as InputOccurrence,
    WorkerV3ApplicationOccurrenceV1 as Occurrence,
    WorkerV3ApplicationRegistrationDescriptorsV1 as Descriptors,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V3 as HANDOFF_BYTES,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_STORAGE_V3 as HANDOFF_SCRATCH,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V3 as HANDOFF_WORK,
    CompilerExecutionAttestationStorageV3 as HandoffStorage,
    CompilerExecutionSupervisorHandoffErrorV3 as HandoffError,
    CompilerExecutionSupervisorHandoffV3 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

mod proof_session;
mod session;
pub use proof_session::{
    NATIVE_APPLICATION_PROOF_SESSION_BYTES_V1, NativeApplicationProofSessionV1,
};
pub use session::{
    NATIVE_APPLICATION_SESSION_MAX_BYTES_V1, NativeApplicationSessionKindV1,
    NativeApplicationSessionMessageV1, NativeApplicationSessionTranscriptV1,
};

const INPUT_MAGIC: &[u8; 8] = b"F3NAIN1\0";
const BINDING_MAGIC: &[u8; 8] = b"F3NARG1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION-REGISTRATION/V1\0";
const HEADER: usize = 24;
const OCCURRENCE_BYTES: usize = 304;
const OCCURRENCE_OFFSET: usize = HEADER + 40;
const DESCRIPTORS_OFFSET: usize = OCCURRENCE_OFFSET + OCCURRENCE_BYTES;
const CHALLENGE_OFFSET: usize = DESCRIPTORS_OFFSET + 16;
const INPUT_CHECKSUM: usize = CHALLENGE_OFFSET + 32;
pub const NATIVE_APPLICATION_REGISTRATION_INPUT_BYTES_V1: usize = INPUT_CHECKSUM + 32;
const INPUT_BYTES: usize = NATIVE_APPLICATION_REGISTRATION_INPUT_BYTES_V1;
const ASSOCIATION_OFFSET: usize = HEADER + HANDOFF_BYTES;
const INPUT_OFFSET: usize = ASSOCIATION_OFFSET + ASSOCIATION_BYTES;
const BINDING_CHECKSUM: usize = INPUT_OFFSET + INPUT_BYTES;
pub const NATIVE_APPLICATION_REGISTRATION_BYTES_V1: usize = BINDING_CHECKSUM + 32;
const BINDING_BYTES: usize = NATIVE_APPLICATION_REGISTRATION_BYTES_V1;
const OCCURRENCE_CODEC: CodecBudget = CodecBudget::new(OCCURRENCE_BYTES, 1024, 4);
const ENTRY: usize = 8;
// Conservative logical scratch for fixed arrays, four input records and codec
// temporaries. It is not a process-RSS or generated instruction/stack bound.
const OUTER_SCRATCH: usize = 64 * 1024;
const OUTER_WORK: usize = 32 * 1024;
const OCCURRENCE_STORAGE: usize = size_of::<Occurrence>() + 4 * size_of::<InputOccurrence>();
const HANDOFF_RETAINED: usize = size_of::<(Handoff, HandoffStorage)>();

/// Extra reservation after a consuming success, or full reservation after decode.
/// Consuming failures drop the inputs but leave their reservations with the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationRegistrationStorageV1(usize);
impl NativeApplicationRegistrationStorageV1 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Logical quota on the original account, including nested native codecs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationRegistrationQuoteV1 {
    input_floor: usize,
    work: usize,
    outer_work: usize,
    scratch: usize,
    retained: usize,
    additional: usize,
}
impl NativeApplicationRegistrationQuoteV1 {
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
    pub const fn additional_storage(self) -> usize {
        self.additional
    }
}
type Quote = NativeApplicationRegistrationQuoteV1;
use self::NativeApplicationRegistrationStorageV1 as Storage;

#[derive(Debug)]
pub enum NativeApplicationRegistrationErrorV1 {
    Resource(Resource),
    Length,
    Header,
    Identity,
    InputProfile,
    Descriptors,
    AssociationMismatch,
    Transcript,
    Kind,
    Application(crate::WorkerV3ApplicationHandoffProtocolErrorV1),
    Handoff(HandoffError),
    Association(AssociationError),
}
type Error = NativeApplicationRegistrationErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<crate::WorkerV3ApplicationHandoffProtocolErrorV1> for Error {
    fn from(value: crate::WorkerV3ApplicationHandoffProtocolErrorV1) -> Self {
        Self::Application(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native application registration: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Move-only description of one four-input occurrence and exact opaque readiness.
/// It authenticates no descriptor, V5 syntax, provenance, currentness or freshness.
///
/// ```compile_fail
/// use fe2o3_runtime_protocol::NativeApplicationRegistrationInputsV1;
/// fn duplicate(value: NativeApplicationRegistrationInputsV1) { let _ = value.clone(); }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationRegistrationInputsV1 {
    occurrence: Occurrence,
    descriptors: Descriptors,
    challenge: Challenge,
    bytes: [u8; INPUT_BYTES],
}
type Inputs = NativeApplicationRegistrationInputsV1;
impl Inputs {
    const RETAINED: usize =
        size_of::<Self>() + 4 * size_of::<InputOccurrence>() + size_of::<Storage>();

    pub fn construction_quote(readiness_length: usize) -> Result<Quote> {
        valid_readiness_length(readiness_length)?;
        let work = readiness_length
            .checked_mul(4)
            .and_then(|v| v.checked_add(OUTER_WORK))
            .ok_or(Resource::Arithmetic)?;
        Ok(Quote {
            input_floor: readiness_length
                .checked_add(OCCURRENCE_STORAGE)
                .ok_or(Resource::Arithmetic)?,
            work,
            outer_work: work,
            scratch: OUTER_SCRATCH,
            retained: Self::RETAINED,
            additional: Self::RETAINED - OCCURRENCE_STORAGE,
        })
    }

    /// The borrowed readiness bytes and consumed four-input occurrence must be prepaid.
    pub fn new(
        readiness: &[u8],
        occurrence: Occurrence,
        descriptors: Descriptors,
        challenge: Challenge,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        budget.charge_work(ENTRY)?;
        if occurrence.inputs().len() != 4 {
            return Err(Error::InputProfile);
        }
        let q = Self::construction_quote(readiness.len())?;
        budget.with_prepaid_scope(
            q.input_floor,
            0,
            q.outer_work - ENTRY,
            OUTER_SCRATCH,
            |_| {
                let value = Self::from_parts(
                    Sha256::digest(readiness).into(),
                    readiness.len() as u64,
                    occurrence,
                    descriptors,
                    challenge,
                )?;
                Ok((value, Storage(q.additional)))
            },
        )
    }

    pub const fn decoding_quote() -> Quote {
        fixed_quote(INPUT_BYTES, Self::RETAINED)
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(
            bytes.len() == INPUT_BYTES,
            Self::decoding_quote(),
            budget,
            |_| {
                header(bytes, INPUT_MAGIC, INPUT_BYTES)?;
                let hash = bytes[HEADER..HEADER + 32].try_into().unwrap();
                let length =
                    u64::from_le_bytes(bytes[HEADER + 32..OCCURRENCE_OFFSET].try_into().unwrap());
                let occurrence = Occurrence::decode_canonical_with_budget(
                    &bytes[OCCURRENCE_OFFSET..DESCRIPTORS_OFFSET],
                    OCCURRENCE_CODEC,
                )?;
                let descriptors = std::array::from_fn::<_, 4, _>(|i| {
                    i32::from_le_bytes(
                        bytes[DESCRIPTORS_OFFSET + i * 4..DESCRIPTORS_OFFSET + (i + 1) * 4]
                            .try_into()
                            .unwrap(),
                    )
                });
                let descriptors = Descriptors::new(
                    descriptors[0],
                    descriptors[1],
                    descriptors[2],
                    descriptors[3],
                )
                .map_err(|_| Error::Descriptors)?;
                let challenge = Challenge::from_bytes(
                    bytes[CHALLENGE_OFFSET..INPUT_CHECKSUM].try_into().unwrap(),
                )?;
                let value = Self::from_parts(hash, length, occurrence, descriptors, challenge)?;
                if value.bytes != bytes {
                    return Err(Error::Identity);
                }
                Ok((value, Storage(Self::RETAINED)))
            },
        )
    }

    fn from_parts(
        readiness: [u8; 32],
        length: u64,
        occurrence: Occurrence,
        descriptors: Descriptors,
        challenge: Challenge,
    ) -> Result<Self> {
        if readiness == [0; 32] {
            return Err(Error::Identity);
        }
        valid_readiness_length(usize::try_from(length).map_err(|_| Error::Length)?)?;
        if occurrence.inputs().len() != 4
            || occurrence
                .inputs()
                .iter()
                .zip(1..=4)
                .any(|(input, slot)| input.slot() != slot)
        {
            return Err(Error::InputProfile);
        }
        let encoded = occurrence.encode_canonical_with_budget(OCCURRENCE_CODEC)?;
        if encoded.len() != OCCURRENCE_BYTES {
            return Err(Error::InputProfile);
        }
        let mut bytes = [0; INPUT_BYTES];
        write_header(&mut bytes, INPUT_MAGIC);
        bytes[HEADER..HEADER + 32].copy_from_slice(&readiness);
        bytes[HEADER + 32..OCCURRENCE_OFFSET].copy_from_slice(&length.to_le_bytes());
        bytes[OCCURRENCE_OFFSET..DESCRIPTORS_OFFSET].copy_from_slice(&encoded);
        for (chunk, descriptor) in bytes[DESCRIPTORS_OFFSET..CHALLENGE_OFFSET]
            .chunks_exact_mut(4)
            .zip(descriptors.as_array())
        {
            chunk.copy_from_slice(&descriptor.to_le_bytes());
        }
        bytes[CHALLENGE_OFFSET..INPUT_CHECKSUM].copy_from_slice(&challenge.as_bytes());
        seal(&mut bytes);
        Ok(Self {
            occurrence,
            descriptors,
            challenge,
            bytes,
        })
    }
    pub const fn occurrence(&self) -> &Occurrence {
        &self.occurrence
    }
    pub const fn descriptors(&self) -> Descriptors {
        self.descriptors
    }
    pub const fn challenge(&self) -> Challenge {
        self.challenge
    }
    pub fn readiness_sha256(&self) -> [u8; 32] {
        self.bytes[HEADER..HEADER + 32].try_into().unwrap()
    }
    pub fn readiness_byte_len(&self) -> u64 {
        u64::from_le_bytes(
            self.bytes[HEADER + 32..OCCURRENCE_OFFSET]
                .try_into()
                .unwrap(),
        )
    }
    pub const fn canonical_bytes(&self) -> &[u8; INPUT_BYTES] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
    pub const fn authenticates_application_occurrence(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationRegistrationIdentityV1([u8; 32]);
impl NativeApplicationRegistrationIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Actual native owners retained together, but still only an inert association.
/// Independent live owners must authenticate policy, readiness and process/FD custody.
/// Consuming failures drop inputs without refunding their original reservations.
///
/// ```compile_fail
/// use fe2o3_runtime_protocol::{NativeApplicationRegistrationBindingV1 as Native,
///     WorkerV3ApplicationRegistrationBindingV1 as Legacy};
/// fn downgrade(value: Native) -> Legacy { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_runtime_protocol::NativeApplicationRegistrationBindingV1;
/// fn duplicate(value: NativeApplicationRegistrationBindingV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_runtime_protocol::{NativeApplicationRegistrationBindingV1 as Binding,
///     NativeConditionalApplicationBindingV1 as Association,
///     NativeApplicationRegistrationInputsV1 as Inputs};
/// use fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorHandoffV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(h: CompilerExecutionSupervisorHandoffV1, a: Association, i: Inputs, b: &mut Budget<'_>) {
///     let _ = Binding::new(h, a, i, b);
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationRegistrationBindingV1 {
    handoff: Handoff,
    association: Association,
    inputs: Inputs,
    bytes: [u8; BINDING_BYTES],
}
type Binding = NativeApplicationRegistrationBindingV1;
impl Binding {
    const INHERITED: usize = HANDOFF_RETAINED + size_of::<Association>() + Inputs::RETAINED;
    const RETAINED: usize = size_of::<Self>() + HANDOFF_RETAINED - size_of::<Handoff>()
        + Inputs::RETAINED
        - size_of::<Inputs>()
        + size_of::<Storage>();
    pub const fn construction_quote() -> Quote {
        Quote {
            input_floor: Self::INHERITED,
            work: OUTER_WORK,
            outer_work: OUTER_WORK,
            scratch: OUTER_SCRATCH,
            retained: Self::RETAINED,
            additional: Self::RETAINED - Self::INHERITED,
        }
    }
    pub fn new(
        handoff: Handoff,
        association: Association,
        inputs: Inputs,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(true, Self::construction_quote(), budget, |_| {
            let value = Self::from_parts(handoff, association, inputs)?;
            Ok((value, Storage(Self::RETAINED - Self::INHERITED)))
        })
    }
    fn from_parts(handoff: Handoff, association: Association, inputs: Inputs) -> Result<Self> {
        if association.compiler_handoff_identity() != *handoff.identity().as_bytes()
            || association.readiness_sha256() != inputs.readiness_sha256()
            || association.readiness_byte_len() != inputs.readiness_byte_len()
        {
            return Err(Error::AssociationMismatch);
        }
        let mut bytes = [0; BINDING_BYTES];
        write_header(&mut bytes, BINDING_MAGIC);
        bytes[HEADER..ASSOCIATION_OFFSET].copy_from_slice(handoff.canonical_bytes());
        bytes[ASSOCIATION_OFFSET..INPUT_OFFSET].copy_from_slice(association.canonical_bytes());
        bytes[INPUT_OFFSET..BINDING_CHECKSUM].copy_from_slice(inputs.canonical_bytes());
        seal(&mut bytes);
        Ok(Self {
            handoff,
            association,
            inputs,
            bytes,
        })
    }
    pub fn decoding_quote() -> Quote {
        let association = Association::decoding_quote();
        let inputs = Inputs::decoding_quote();
        let nested_peak = HANDOFF_SCRATCH
            .max(HANDOFF_RETAINED + association.scratch())
            .max(HANDOFF_RETAINED + size_of::<Association>() + inputs.scratch)
            .max(Self::INHERITED);
        Quote {
            input_floor: BINDING_BYTES,
            work: OUTER_WORK + HANDOFF_WORK + association.work() + inputs.work,
            outer_work: OUTER_WORK,
            scratch: OUTER_SCRATCH + nested_peak,
            retained: Self::RETAINED,
            additional: Self::RETAINED,
        }
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(
            bytes.len() == BINDING_BYTES,
            Self::decoding_quote(),
            budget,
            |budget| {
                header(bytes, BINDING_MAGIC, BINDING_BYTES)?;
                let (handoff, storage) =
                    Handoff::decode(&bytes[HEADER..ASSOCIATION_OFFSET], budget)
                        .map_err(Error::Handoff)?;
                budget.reserve_storage(storage.additional_storage())?;
                let association =
                    Association::decode(&bytes[ASSOCIATION_OFFSET..INPUT_OFFSET], budget)
                        .map_err(Error::Association)?;
                budget.reserve_storage(size_of::<Association>())?;
                let (inputs, storage) =
                    Inputs::decode(&bytes[INPUT_OFFSET..BINDING_CHECKSUM], budget)?;
                budget.reserve_storage(storage.additional_storage())?;
                let value = Self::from_parts(handoff, association, inputs)?;
                if value.bytes != bytes {
                    return Err(Error::Identity);
                }
                Ok((value, Storage(Self::RETAINED)))
            },
        )
    }
    pub const fn compiler_handoff(&self) -> &Handoff {
        &self.handoff
    }
    pub const fn association(&self) -> &Association {
        &self.association
    }
    pub const fn inputs(&self) -> &Inputs {
        &self.inputs
    }
    pub const fn occurrence(&self) -> &Occurrence {
        self.inputs.occurrence()
    }
    pub const fn descriptors(&self) -> Descriptors {
        self.inputs.descriptors()
    }
    pub const fn challenge(&self) -> Challenge {
        self.inputs.challenge()
    }
    pub const fn canonical_bytes(&self) -> &[u8; BINDING_BYTES] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
    pub fn identity(&self) -> NativeApplicationRegistrationIdentityV1 {
        NativeApplicationRegistrationIdentityV1(self.bytes[BINDING_CHECKSUM..].try_into().unwrap())
    }
    pub const fn authenticates_application_occurrence(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

const fn fixed_quote(input_floor: usize, retained: usize) -> Quote {
    Quote {
        input_floor,
        work: OUTER_WORK,
        outer_work: OUTER_WORK,
        scratch: OUTER_SCRATCH,
        retained,
        additional: retained,
    }
}
fn metered<T>(
    full_input: bool,
    q: Quote,
    budget: &mut Budget<'_>,
    operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Result<T> {
    budget.with_prepaid_scope(
        if full_input { q.input_floor } else { 0 },
        ENTRY,
        q.outer_work,
        OUTER_SCRATCH,
        operation,
    )
}
fn valid_readiness_length(length: usize) -> Result<()> {
    if length == 0 || length > MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 {
        Err(Error::Length)
    } else {
        Ok(())
    }
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
fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
fn seal(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let identity = checksum(&bytes[..end]);
    bytes[end..].copy_from_slice(&identity);
}

#[cfg(test)]
mod tests;
