//! Exact descriptive binding for the dedicated four-input application registration profile.

use std::{error::Error, fmt};

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1,
    CompilerExecutionSupervisorHandoffErrorV1, CompilerExecutionSupervisorHandoffV1,
};
use sha2::{Digest, Sha256};

use crate::{
    WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_BYTES_V1,
    WORKER_V3_APPLICATION_HANDOFF_EXPECTATION_BYTES_V1, WorkerV3ApplicationHandoffChallengeV1,
    WorkerV3ApplicationHandoffCodecBudgetV1, WorkerV3ApplicationHandoffExpectationV1,
    WorkerV3ApplicationHandoffProtocolErrorV1, WorkerV3ApplicationOccurrenceV1,
};

const MAGIC: &[u8; 8] = b"F3AREG1\0";
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/APPLICATION-REGISTRATION/V1\0";
const HEADER_BYTES: usize = 24;
const FOUR_INPUT_OCCURRENCE_BYTES: usize = 304;
const OCCURRENCE_OFFSET: usize = HEADER_BYTES + COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1;
const DESCRIPTORS_OFFSET: usize = OCCURRENCE_OFFSET + FOUR_INPUT_OCCURRENCE_BYTES;
const EXPECTATION_OFFSET: usize = DESCRIPTORS_OFFSET + 4 * 4;
const CHALLENGE_OFFSET: usize =
    EXPECTATION_OFFSET + WORKER_V3_APPLICATION_HANDOFF_EXPECTATION_BYTES_V1;
const IDENTITY_OFFSET: usize = CHALLENGE_OFFSET + WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_BYTES_V1;
const OCCURRENCE_BUDGET: WorkerV3ApplicationHandoffCodecBudgetV1 =
    WorkerV3ApplicationHandoffCodecBudgetV1::new(FOUR_INPUT_OCCURRENCE_BYTES, 1024, 4);

mod ready;
pub use ready::{
    WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1, WorkerV3ApplicationSupervisorReadyErrorV1,
    WorkerV3ApplicationSupervisorReadyV1,
};
mod session;
pub use session::{
    WORKER_V3_APPLICATION_SESSION_MAX_BYTES_V1, WorkerV3ApplicationRegistrationInputsV1,
    WorkerV3ApplicationSessionKindV1, WorkerV3ApplicationSessionMessageV1,
    WorkerV3ApplicationSessionTranscriptV1,
};

/// Exact byte length of the dedicated application binding, distinct from compiler-only handoff.
pub const WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1: usize = IDENTITY_OFFSET + 32;

/// Four historical child coordinates in envelope/directory/ACK/proof-endpoint order.
///
/// Numbers are not descriptor custody or evidence of OS object association.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationRegistrationDescriptorsV1([i32; 4]);

impl WorkerV3ApplicationRegistrationDescriptorsV1 {
    pub fn new(envelope: i32, directory: i32, acknowledgment: i32, proof: i32) -> Result<Self> {
        let values = [envelope, directory, acknowledgment, proof];
        for (index, value) in values.iter().enumerate() {
            if *value <= 2
                || *value == COMPILER_EXECUTION_SERVICE_CHILD_FD_V1
                || values[..index].contains(value)
            {
                return Err(WorkerV3ApplicationRegistrationErrorV1::Descriptors);
            }
        }
        Ok(Self(values))
    }

    pub const fn as_array(self) -> [i32; 4] {
        self.0
    }
}

/// Domain-separated consistency identity, not authentication or process custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationRegistrationIdentityV1([u8; 32]);

impl WorkerV3ApplicationRegistrationIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Canonical inert binding of original compiler handoff and exact application inputs.
///
/// Every value is serializable and reproducible. Admission must separately authenticate Cargo,
/// original pidfds, parentage, endpoint creator/addresses and per-message credentials. Neither
/// this record nor its checksum establishes freshness, proof custody or invocation authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationRegistrationBindingV1 {
    handoff: CompilerExecutionSupervisorHandoffV1,
    occurrence: WorkerV3ApplicationOccurrenceV1,
    descriptors: WorkerV3ApplicationRegistrationDescriptorsV1,
    expectation: WorkerV3ApplicationHandoffExpectationV1,
    challenge: WorkerV3ApplicationHandoffChallengeV1,
    identity: WorkerV3ApplicationRegistrationIdentityV1,
    bytes: [u8; WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1],
}

impl WorkerV3ApplicationRegistrationBindingV1 {
    pub fn new(
        handoff: CompilerExecutionSupervisorHandoffV1,
        occurrence: WorkerV3ApplicationOccurrenceV1,
        descriptors: WorkerV3ApplicationRegistrationDescriptorsV1,
        expectation: WorkerV3ApplicationHandoffExpectationV1,
        challenge: WorkerV3ApplicationHandoffChallengeV1,
    ) -> Result<Self> {
        let inputs = WorkerV3ApplicationRegistrationInputsV1::new(
            occurrence,
            descriptors,
            expectation,
            challenge,
        )?;
        let mut bytes = [0; WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[12..16]
            .copy_from_slice(&(WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1 as u32).to_le_bytes());
        bytes[HEADER_BYTES..OCCURRENCE_OFFSET].copy_from_slice(handoff.canonical_bytes());
        bytes[OCCURRENCE_OFFSET..IDENTITY_OFFSET].copy_from_slice(inputs.canonical_bytes());
        let identity =
            WorkerV3ApplicationRegistrationIdentityV1(derive_identity(&bytes[..IDENTITY_OFFSET]));
        bytes[IDENTITY_OFFSET..].copy_from_slice(identity.as_bytes());
        Ok(Self {
            handoff,
            occurrence: inputs.occurrence,
            descriptors,
            expectation,
            challenge,
            identity,
            bytes,
        })
    }

    /// Validates fixed framing and identity before any nested allocation, then re-encodes exactly.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1 {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Length);
        }
        if &bytes[..8] != MAGIC || bytes[8..10] != 1u16.to_le_bytes() {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Header);
        }
        if bytes[10..12] != [0; 2] || bytes[16..24] != [0; 8] {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Reserved);
        }
        if bytes[12..16] != (bytes.len() as u32).to_le_bytes() {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Length);
        }
        if bytes[IDENTITY_OFFSET..] != derive_identity(&bytes[..IDENTITY_OFFSET]) {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Identity);
        }
        let handoff =
            CompilerExecutionSupervisorHandoffV1::decode(&bytes[HEADER_BYTES..OCCURRENCE_OFFSET])
                .map_err(WorkerV3ApplicationRegistrationErrorV1::CompilerHandoff)?;
        let inputs = WorkerV3ApplicationRegistrationInputsV1::decode(
            &bytes[OCCURRENCE_OFFSET..IDENTITY_OFFSET],
        )?;
        let value = Self::new(
            handoff,
            inputs.occurrence,
            inputs.descriptors,
            inputs.expectation,
            inputs.challenge,
        )?;
        if value.bytes != bytes {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Canonical);
        }
        Ok(value)
    }

    pub const fn compiler_handoff(&self) -> &CompilerExecutionSupervisorHandoffV1 {
        &self.handoff
    }
    pub const fn occurrence(&self) -> &WorkerV3ApplicationOccurrenceV1 {
        &self.occurrence
    }
    pub const fn descriptors(&self) -> WorkerV3ApplicationRegistrationDescriptorsV1 {
        self.descriptors
    }
    pub const fn expectation(&self) -> WorkerV3ApplicationHandoffExpectationV1 {
        self.expectation
    }
    pub const fn challenge(&self) -> WorkerV3ApplicationHandoffChallengeV1 {
        self.challenge
    }
    pub const fn identity(&self) -> WorkerV3ApplicationRegistrationIdentityV1 {
        self.identity
    }
    pub const fn canonical_bytes(&self) -> &[u8; WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1] {
        &self.bytes
    }
}

fn derive_identity(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(IDENTITY_DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

type Result<T> = std::result::Result<T, WorkerV3ApplicationRegistrationErrorV1>;

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3ApplicationRegistrationErrorV1 {
    Length,
    Header,
    Reserved,
    Identity,
    Descriptors,
    InputProfile,
    ExpectationMismatch,
    Canonical,
    SessionTranscript,
    CompilerHandoff(CompilerExecutionSupervisorHandoffErrorV1),
    ApplicationHandoff(WorkerV3ApplicationHandoffProtocolErrorV1),
}

impl From<WorkerV3ApplicationHandoffProtocolErrorV1> for WorkerV3ApplicationRegistrationErrorV1 {
    fn from(error: WorkerV3ApplicationHandoffProtocolErrorV1) -> Self {
        Self::ApplicationHandoff(error)
    }
}

impl fmt::Display for WorkerV3ApplicationRegistrationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompilerHandoff(error) => {
                write!(f, "application registration compiler binding: {error}")
            }
            Self::ApplicationHandoff(error) => {
                write!(f, "application registration inputs: {error}")
            }
            _ => write!(f, "invalid Worker V3 application registration: {self:?}"),
        }
    }
}

impl Error for WorkerV3ApplicationRegistrationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CompilerHandoff(error) => Some(error),
            Self::ApplicationHandoff(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
