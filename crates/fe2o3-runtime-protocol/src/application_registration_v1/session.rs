//! Bounded descriptive handshake records; OS-authenticated custody supplies their meaning.

use super::*;

const INPUT_BYTES: usize = IDENTITY_OFFSET - OCCURRENCE_OFFSET;
const DESCRIPTORS: usize = DESCRIPTORS_OFFSET - OCCURRENCE_OFFSET;
const EXPECTATION: usize = EXPECTATION_OFFSET - OCCURRENCE_OFFSET;
const CHALLENGE: usize = CHALLENGE_OFFSET - OCCURRENCE_OFFSET;
const SESSION_MAGIC: &[u8; 8] = b"F3ASES1\0";
const SESSION_HEADER: usize = 112;

/// Largest exact packet: a root Challenge carrying the complete registration binding.
pub const WORKER_V3_APPLICATION_SESSION_MAX_BYTES_V1: usize =
    SESSION_HEADER + WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1;

/// Canonical 600-byte local application input capsule, without a post-spawn compiler handoff.
/// Copied bytes carry neither original descriptor custody nor authenticated observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationRegistrationInputsV1 {
    pub(super) occurrence: WorkerV3ApplicationOccurrenceV1,
    pub(super) descriptors: WorkerV3ApplicationRegistrationDescriptorsV1,
    pub(super) expectation: WorkerV3ApplicationHandoffExpectationV1,
    pub(super) challenge: WorkerV3ApplicationHandoffChallengeV1,
    bytes: [u8; INPUT_BYTES],
}

impl WorkerV3ApplicationRegistrationInputsV1 {
    pub fn new(
        occurrence: WorkerV3ApplicationOccurrenceV1,
        descriptors: WorkerV3ApplicationRegistrationDescriptorsV1,
        expectation: WorkerV3ApplicationHandoffExpectationV1,
        challenge: WorkerV3ApplicationHandoffChallengeV1,
    ) -> Result<Self> {
        if occurrence.inputs().len() != 4
            || occurrence
                .inputs()
                .iter()
                .zip(1..=4)
                .any(|(input, slot)| input.slot() != slot)
        {
            return Err(WorkerV3ApplicationRegistrationErrorV1::InputProfile);
        }
        if WorkerV3ApplicationHandoffExpectationV1::new(expectation.envelope(), &occurrence)
            != expectation
        {
            return Err(WorkerV3ApplicationRegistrationErrorV1::ExpectationMismatch);
        }
        let encoded = occurrence.encode_canonical_with_budget(OCCURRENCE_BUDGET)?;
        if encoded.len() != FOUR_INPUT_OCCURRENCE_BYTES {
            return Err(WorkerV3ApplicationRegistrationErrorV1::InputProfile);
        }
        let mut bytes = [0; INPUT_BYTES];
        bytes[..DESCRIPTORS].copy_from_slice(&encoded);
        for (slot, value) in bytes[DESCRIPTORS..EXPECTATION]
            .chunks_exact_mut(4)
            .zip(descriptors.0)
        {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        bytes[EXPECTATION..CHALLENGE].copy_from_slice(&expectation.encode_canonical()?);
        bytes[CHALLENGE..].copy_from_slice(&challenge.encode_canonical()?);
        Ok(Self {
            occurrence,
            descriptors,
            expectation,
            challenge,
            bytes,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != INPUT_BYTES {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Length);
        }
        let occurrence = WorkerV3ApplicationOccurrenceV1::decode_canonical_with_budget(
            &bytes[..DESCRIPTORS],
            OCCURRENCE_BUDGET,
        )?;
        let values: [i32; 4] = std::array::from_fn(|i| {
            i32::from_le_bytes(
                bytes[DESCRIPTORS + i * 4..DESCRIPTORS + (i + 1) * 4]
                    .try_into()
                    .unwrap(),
            )
        });
        let descriptors = WorkerV3ApplicationRegistrationDescriptorsV1::new(
            values[0], values[1], values[2], values[3],
        )?;
        let expectation = WorkerV3ApplicationHandoffExpectationV1::decode_canonical(
            &bytes[EXPECTATION..CHALLENGE],
        )?;
        let challenge =
            WorkerV3ApplicationHandoffChallengeV1::decode_canonical(&bytes[CHALLENGE..])?;
        let value = Self::new(occurrence, descriptors, expectation, challenge)?;
        if value.bytes != bytes {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Canonical);
        }
        Ok(value)
    }

    pub const fn canonical_bytes(&self) -> &[u8; INPUT_BYTES] {
        &self.bytes
    }

    pub fn matches_binding(&self, binding: &WorkerV3ApplicationRegistrationBindingV1) -> bool {
        self.bytes == binding.canonical_bytes()[OCCURRENCE_OFFSET..IDENTITY_OFFSET]
    }
}

/// Closed phase set for the dedicated application channel, distinct from compiler auditing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WorkerV3ApplicationSessionKindV1 {
    Hello = 1,
    Challenge = 2,
    Accept = 3,
    Ready = 4,
}

/// Inert equality data. Freshness and sender/process association are checked by the live owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationSessionTranscriptV1 {
    app_nonce: [u8; 32],
    root_nonce: [u8; 32],
    binding: [u8; 32],
}

impl WorkerV3ApplicationSessionTranscriptV1 {
    pub fn new(app_nonce: [u8; 32], root_nonce: [u8; 32], binding: [u8; 32]) -> Result<Self> {
        if app_nonce == [0; 32]
            || root_nonce == [0; 32]
            || binding == [0; 32]
            || app_nonce == root_nonce
        {
            return Err(WorkerV3ApplicationRegistrationErrorV1::SessionTranscript);
        }
        Ok(Self {
            app_nonce,
            root_nonce,
            binding,
        })
    }
    pub const fn app_nonce(self) -> [u8; 32] {
        self.app_nonce
    }
    pub const fn root_nonce(self) -> [u8; 32] {
        self.root_nonce
    }
    pub const fn binding(self) -> [u8; 32] {
        self.binding
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Body {
    Hello(Box<WorkerV3ApplicationRegistrationInputsV1>),
    Challenge(Box<WorkerV3ApplicationRegistrationBindingV1>),
    Empty,
}

/// Canonical handshake message. A decoded Ready is not a registered session or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationSessionMessageV1 {
    kind: WorkerV3ApplicationSessionKindV1,
    transcript: WorkerV3ApplicationSessionTranscriptV1,
    body: Body,
    bytes: Vec<u8>,
}

impl WorkerV3ApplicationSessionMessageV1 {
    pub fn hello(
        inputs: WorkerV3ApplicationRegistrationInputsV1,
        app_nonce: [u8; 32],
    ) -> Result<Self> {
        if app_nonce == [0; 32] {
            return Err(WorkerV3ApplicationRegistrationErrorV1::SessionTranscript);
        }
        Ok(Self::encode(
            WorkerV3ApplicationSessionKindV1::Hello,
            WorkerV3ApplicationSessionTranscriptV1 {
                app_nonce,
                root_nonce: [0; 32],
                binding: [0; 32],
            },
            Body::Hello(Box::new(inputs)),
        ))
    }

    pub fn challenge(
        binding: WorkerV3ApplicationRegistrationBindingV1,
        app_nonce: [u8; 32],
        root_nonce: [u8; 32],
    ) -> Result<Self> {
        let transcript = WorkerV3ApplicationSessionTranscriptV1::new(
            app_nonce,
            root_nonce,
            *binding.identity().as_bytes(),
        )?;
        Ok(Self::encode(
            WorkerV3ApplicationSessionKindV1::Challenge,
            transcript,
            Body::Challenge(Box::new(binding)),
        ))
    }

    pub fn accept(transcript: WorkerV3ApplicationSessionTranscriptV1) -> Self {
        Self::encode(
            WorkerV3ApplicationSessionKindV1::Accept,
            transcript,
            Body::Empty,
        )
    }

    pub fn ready(transcript: WorkerV3ApplicationSessionTranscriptV1) -> Self {
        Self::encode(
            WorkerV3ApplicationSessionKindV1::Ready,
            transcript,
            Body::Empty,
        )
    }

    fn encode(
        kind: WorkerV3ApplicationSessionKindV1,
        transcript: WorkerV3ApplicationSessionTranscriptV1,
        body: Body,
    ) -> Self {
        let payload: &[u8] = match &body {
            Body::Hello(value) => value.canonical_bytes(),
            Body::Challenge(value) => value.canonical_bytes(),
            Body::Empty => &[],
        };
        let mut bytes = vec![0; SESSION_HEADER + payload.len()];
        bytes[..8].copy_from_slice(SESSION_MAGIC);
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[10] = kind as u8;
        bytes[12..16].copy_from_slice(&((SESSION_HEADER + payload.len()) as u32).to_le_bytes());
        bytes[16..48].copy_from_slice(&transcript.app_nonce);
        bytes[48..80].copy_from_slice(&transcript.root_nonce);
        bytes[80..112].copy_from_slice(&transcript.binding);
        bytes[112..].copy_from_slice(payload);
        Self {
            kind,
            transcript,
            body,
            bytes,
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < SESSION_HEADER || bytes.len() > WORKER_V3_APPLICATION_SESSION_MAX_BYTES_V1
        {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Length);
        }
        if &bytes[..8] != SESSION_MAGIC || bytes[8..10] != 1u16.to_le_bytes() {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Header);
        }
        if bytes[11] != 0 {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Reserved);
        }
        let payload = match bytes[10] {
            1 => INPUT_BYTES,
            2 => WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1,
            3 | 4 => 0,
            _ => return Err(WorkerV3ApplicationRegistrationErrorV1::Header),
        };
        if bytes.len() != SESSION_HEADER + payload
            || bytes[12..16] != (bytes.len() as u32).to_le_bytes()
        {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Length);
        }
        let app_nonce = bytes[16..48].try_into().unwrap();
        let root_nonce = bytes[48..80].try_into().unwrap();
        let binding = bytes[80..112].try_into().unwrap();
        let value = match bytes[10] {
            1 => {
                if root_nonce != [0; 32] || binding != [0; 32] {
                    return Err(WorkerV3ApplicationRegistrationErrorV1::SessionTranscript);
                }
                Self::hello(
                    WorkerV3ApplicationRegistrationInputsV1::decode(&bytes[SESSION_HEADER..])?,
                    app_nonce,
                )?
            }
            2 => {
                let registration =
                    WorkerV3ApplicationRegistrationBindingV1::decode(&bytes[SESSION_HEADER..])?;
                if registration.identity().as_bytes() != &binding {
                    return Err(WorkerV3ApplicationRegistrationErrorV1::SessionTranscript);
                }
                Self::challenge(registration, app_nonce, root_nonce)?
            }
            kind => {
                let transcript =
                    WorkerV3ApplicationSessionTranscriptV1::new(app_nonce, root_nonce, binding)?;
                if kind == 3 {
                    Self::accept(transcript)
                } else {
                    Self::ready(transcript)
                }
            }
        };
        if value.bytes != bytes {
            return Err(WorkerV3ApplicationRegistrationErrorV1::Canonical);
        }
        Ok(value)
    }

    pub const fn kind(&self) -> WorkerV3ApplicationSessionKindV1 {
        self.kind
    }
    pub const fn app_nonce(&self) -> [u8; 32] {
        self.transcript.app_nonce
    }
    pub fn transcript(&self) -> Option<WorkerV3ApplicationSessionTranscriptV1> {
        (self.kind != WorkerV3ApplicationSessionKindV1::Hello).then_some(self.transcript)
    }
    pub fn inputs(&self) -> Option<&WorkerV3ApplicationRegistrationInputsV1> {
        match &self.body {
            Body::Hello(value) => Some(value),
            _ => None,
        }
    }
    pub fn registration(&self) -> Option<&WorkerV3ApplicationRegistrationBindingV1> {
        match &self.body {
            Body::Challenge(value) => Some(value),
            _ => None,
        }
    }
    pub const fn canonical_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
    pub const fn rights(&self) -> usize {
        if matches!(self.kind, WorkerV3ApplicationSessionKindV1::Challenge) {
            1
        } else {
            0
        }
    }
}
