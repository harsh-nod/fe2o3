//! Schema-independent, authority-free application handoff values for a future V3 envelope.
//!
//! This module identifies exact opaque envelope bytes but does not decode an envelope or claim
//! that those bytes are valid, current, published, loadable, or launchable. Application and input
//! occurrences are likewise descriptive values. Authority must remain in non-serializable host
//! capabilities outside this crate.

use core::fmt;

use sha2::{Digest, Sha256};

use crate::static_application::{
    SealedStaticApplicationErrorV1, sealed_static_application_identity_v1,
};

const FRAME_HEADER_BYTES_V1: usize = 8 + 2 + 2 + 4 + 4;
const FRAME_CHECKSUM_BYTES_V1: usize = 32;
const IDENTITY_BYTES_V1: usize = 32;
const EXACT_IDENTITY_PAYLOAD_BYTES_V1: usize = IDENTITY_BYTES_V1 + 8;
const APPLICATION_INPUT_BYTES_V1: usize = 2 + 2 + IDENTITY_BYTES_V1;
const APPLICATION_OCCURRENCE_FIXED_PAYLOAD_BYTES_V1: usize =
    EXACT_IDENTITY_PAYLOAD_BYTES_V1 + IDENTITY_BYTES_V1 + 2 + 2 + IDENTITY_BYTES_V1;

const LOAD_ENVELOPE_IDENTITY_MAGIC_V1: [u8; 8] = *b"F3LEIV1\0";
const APPLICATION_IDENTITY_MAGIC_V1: [u8; 8] = *b"F3AIDV1\0";
const APPLICATION_OCCURRENCE_MAGIC_V1: [u8; 8] = *b"F3AOCV1\0";
const APPLICATION_CHALLENGE_MAGIC_V1: [u8; 8] = *b"F3ACHV1\0";
const APPLICATION_COMMITMENT_MAGIC_V1: [u8; 8] = *b"F3ACMV1\0";
const APPLICATION_EXPECTATION_MAGIC_V1: [u8; 8] = *b"F3AEXV1\0";
const APPLICATION_ACK_MAGIC_V1: [u8; 8] = *b"F3AAKV1\0";

const LOAD_ENVELOPE_EXACT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/OPAQUE-LOAD-ENVELOPE-EXACT-IDENTITY/V1\0";
const APPLICATION_EXACT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/SEALED-APPLICATION-EXACT-IDENTITY/V1\0";
const APPLICATION_OCCURRENCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/APPLICATION-OCCURRENCE-IDENTITY/V1\0";
const LOAD_ENVELOPE_IDENTITY_CHECKSUM_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/OPAQUE-LOAD-ENVELOPE-IDENTITY-CHECKSUM/V1\0";
const APPLICATION_IDENTITY_CHECKSUM_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/APPLICATION-IDENTITY-CHECKSUM/V1\0";
const APPLICATION_OCCURRENCE_CHECKSUM_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/APPLICATION-OCCURRENCE-CHECKSUM/V1\0";
const APPLICATION_CHALLENGE_CHECKSUM_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/APPLICATION-HANDOFF-CHALLENGE-CHECKSUM/V1\0";
const APPLICATION_COMMITMENT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/APPLICATION-HANDOFF-COMMITMENT/V1\0";
const APPLICATION_COMMITMENT_CHECKSUM_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/APPLICATION-HANDOFF-COMMITMENT-CHECKSUM/V1\0";
const APPLICATION_EXPECTATION_CHECKSUM_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/APPLICATION-HANDOFF-EXPECTATION-CHECKSUM/V1\0";
const APPLICATION_ACK_CHECKSUM_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V3/APPLICATION-HANDOFF-ACK-CHECKSUM/V1\0";

/// Wire version used only by the side-by-side application handoff V3 protocol.
pub const WORKER_V3_APPLICATION_HANDOFF_VERSION_V1: u16 = 1;
/// Maximum number of inherited input occurrences bound to one application spawn.
pub const MAX_WORKER_V3_APPLICATION_INPUTS_V1: usize = 64;
/// Maximum allocation admitted by the default V3 application handoff codec budget.
pub const MAX_WORKER_V3_APPLICATION_HANDOFF_ALLOCATION_BYTES_V1: usize = 16 * 1024;
/// Maximum canonical application-occurrence wire size.
pub const MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1: usize = FRAME_HEADER_BYTES_V1
    + APPLICATION_OCCURRENCE_FIXED_PAYLOAD_BYTES_V1
    + MAX_WORKER_V3_APPLICATION_INPUTS_V1 * APPLICATION_INPUT_BYTES_V1
    + FRAME_CHECKSUM_BYTES_V1;

const APPLICATION_CHALLENGE_PAYLOAD_BYTES_V1: usize = IDENTITY_BYTES_V1;
const APPLICATION_COMMITMENT_PAYLOAD_BYTES_V1: usize = IDENTITY_BYTES_V1;
const APPLICATION_EXPECTATION_PAYLOAD_BYTES_V1: usize =
    EXACT_IDENTITY_PAYLOAD_BYTES_V1 * 2 + IDENTITY_BYTES_V1 * 2;
const APPLICATION_ACK_PAYLOAD_BYTES_V1: usize =
    APPLICATION_CHALLENGE_PAYLOAD_BYTES_V1 + APPLICATION_EXPECTATION_PAYLOAD_BYTES_V1;
pub const WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_BYTES_V1: usize =
    FRAME_HEADER_BYTES_V1 + APPLICATION_CHALLENGE_PAYLOAD_BYTES_V1 + FRAME_CHECKSUM_BYTES_V1;
pub const WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_BYTES_V1: usize =
    FRAME_HEADER_BYTES_V1 + APPLICATION_COMMITMENT_PAYLOAD_BYTES_V1 + FRAME_CHECKSUM_BYTES_V1;
pub const WORKER_V3_APPLICATION_HANDOFF_EXPECTATION_BYTES_V1: usize =
    FRAME_HEADER_BYTES_V1 + APPLICATION_EXPECTATION_PAYLOAD_BYTES_V1 + FRAME_CHECKSUM_BYTES_V1;
pub const WORKER_V3_APPLICATION_HANDOFF_ACK_BYTES_V1: usize =
    FRAME_HEADER_BYTES_V1 + APPLICATION_ACK_PAYLOAD_BYTES_V1 + FRAME_CHECKSUM_BYTES_V1;

const EXACT_IDENTITY_WIRE_BYTES_V1: usize =
    FRAME_HEADER_BYTES_V1 + EXACT_IDENTITY_PAYLOAD_BYTES_V1 + FRAME_CHECKSUM_BYTES_V1;

/// Explicit limits for V3 handoff codec allocations and attacker-controlled input counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationHandoffCodecBudgetV1 {
    max_wire_bytes: usize,
    max_allocation_bytes: usize,
    max_inputs: usize,
}

impl WorkerV3ApplicationHandoffCodecBudgetV1 {
    pub const fn new(
        max_wire_bytes: usize,
        max_allocation_bytes: usize,
        max_inputs: usize,
    ) -> Self {
        Self {
            max_wire_bytes,
            max_allocation_bytes,
            max_inputs,
        }
    }

    pub const fn production() -> Self {
        Self::new(
            MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1,
            MAX_WORKER_V3_APPLICATION_HANDOFF_ALLOCATION_BYTES_V1,
            MAX_WORKER_V3_APPLICATION_INPUTS_V1,
        )
    }

    pub const fn max_wire_bytes(self) -> usize {
        self.max_wire_bytes
    }

    pub const fn max_allocation_bytes(self) -> usize {
        self.max_allocation_bytes
    }

    pub const fn max_inputs(self) -> usize {
        self.max_inputs
    }
}

impl Default for WorkerV3ApplicationHandoffCodecBudgetV1 {
    fn default() -> Self {
        Self::production()
    }
}

/// Exact identity of opaque canonical V3 load-envelope bytes.
///
/// The digest is intentionally independent of a load-envelope Rust type. Constructing or
/// decoding this value authenticates no schema and grants no publication, load, or launch
/// authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3LoadEnvelopeIdentityV1 {
    sha256: [u8; IDENTITY_BYTES_V1],
    byte_len: u64,
}

impl WorkerV3LoadEnvelopeIdentityV1 {
    pub fn from_exact_bytes(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        if bytes.is_empty() {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::EmptyValue {
                field: "V3 load envelope",
            });
        }
        let byte_len = u64::try_from(bytes.len()).map_err(|_| {
            WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow {
                field: "V3 load envelope",
            }
        })?;
        Ok(Self {
            sha256: hash_exact_bytes(LOAD_ENVELOPE_EXACT_IDENTITY_DOMAIN_V1, bytes),
            byte_len,
        })
    }

    pub const fn sha256(self) -> [u8; IDENTITY_BYTES_V1] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn encode_canonical(self) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
        encode_exact_identity(
            self.sha256,
            self.byte_len,
            LOAD_ENVELOPE_IDENTITY_MAGIC_V1,
            LOAD_ENVELOPE_IDENTITY_CHECKSUM_DOMAIN_V1,
            WorkerV3ApplicationHandoffCodecBudgetV1::production(),
        )
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        let (sha256, byte_len) = decode_exact_identity(
            bytes,
            "V3 load envelope",
            LOAD_ENVELOPE_IDENTITY_MAGIC_V1,
            LOAD_ENVELOPE_IDENTITY_CHECKSUM_DOMAIN_V1,
            WorkerV3ApplicationHandoffCodecBudgetV1::production(),
        )?;
        Ok(Self { sha256, byte_len })
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Exact identity of one validated loader-independent application image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationIdentityV1 {
    sha256: [u8; IDENTITY_BYTES_V1],
    byte_len: u64,
}

impl WorkerV3ApplicationIdentityV1 {
    /// Validates the sealed-static ELF policy before deriving the V3 exact-byte identity.
    pub fn from_sealed_static_elf_v1(
        executable: &[u8],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        sealed_static_application_identity_v1(executable)
            .map_err(WorkerV3ApplicationHandoffProtocolErrorV1::ApplicationImage)?;
        let byte_len = u64::try_from(executable.len()).map_err(|_| {
            WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow {
                field: "application image",
            }
        })?;
        Ok(Self {
            sha256: hash_exact_bytes(APPLICATION_EXACT_IDENTITY_DOMAIN_V1, executable),
            byte_len,
        })
    }

    pub const fn sha256(self) -> [u8; IDENTITY_BYTES_V1] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn encode_canonical(self) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
        encode_exact_identity(
            self.sha256,
            self.byte_len,
            APPLICATION_IDENTITY_MAGIC_V1,
            APPLICATION_IDENTITY_CHECKSUM_DOMAIN_V1,
            WorkerV3ApplicationHandoffCodecBudgetV1::production(),
        )
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        let (sha256, byte_len) = decode_exact_identity(
            bytes,
            "application image",
            APPLICATION_IDENTITY_MAGIC_V1,
            APPLICATION_IDENTITY_CHECKSUM_DOMAIN_V1,
            WorkerV3ApplicationHandoffCodecBudgetV1::production(),
        )?;
        Ok(Self { sha256, byte_len })
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }

    #[cfg(test)]
    const fn from_test_parts(sha256: [u8; IDENTITY_BYTES_V1], byte_len: u64) -> Self {
        Self { sha256, byte_len }
    }
}

/// One inherited application input and the externally observed occurrence that identifies it.
///
/// `slot` is protocol-local and must be nonzero. The occurrence identity is opaque; equality is
/// used only to reject two slots that alias the same observed input.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3ApplicationInputOccurrenceV1 {
    slot: u16,
    identity: [u8; IDENTITY_BYTES_V1],
}

impl WorkerV3ApplicationInputOccurrenceV1 {
    pub fn new(
        slot: u16,
        identity: [u8; IDENTITY_BYTES_V1],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        if slot == 0 {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroInputSlot);
        }
        if identity == [0; IDENTITY_BYTES_V1] {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity {
                field: "application input occurrence",
            });
        }
        Ok(Self { slot, identity })
    }

    pub const fn slot(self) -> u16 {
        self.slot
    }

    pub const fn identity(self) -> [u8; IDENTITY_BYTES_V1] {
        self.identity
    }
}

/// Digest of one application image, spawn identity, and canonical non-aliasing input set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationOccurrenceIdentityV1([u8; IDENTITY_BYTES_V1]);

impl WorkerV3ApplicationOccurrenceIdentityV1 {
    pub const fn as_bytes(self) -> [u8; IDENTITY_BYTES_V1] {
        self.0
    }
}

/// Canonical descriptive record for one application spawn and its inherited inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationOccurrenceV1 {
    application: WorkerV3ApplicationIdentityV1,
    spawn_identity: [u8; IDENTITY_BYTES_V1],
    inputs: Box<[WorkerV3ApplicationInputOccurrenceV1]>,
    identity: WorkerV3ApplicationOccurrenceIdentityV1,
}

impl WorkerV3ApplicationOccurrenceV1 {
    pub fn new(
        application: WorkerV3ApplicationIdentityV1,
        spawn_identity: [u8; IDENTITY_BYTES_V1],
        inputs: &[WorkerV3ApplicationInputOccurrenceV1],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        Self::new_with_budget(
            application,
            spawn_identity,
            inputs,
            WorkerV3ApplicationHandoffCodecBudgetV1::production(),
        )
    }

    pub fn new_with_budget(
        application: WorkerV3ApplicationIdentityV1,
        spawn_identity: [u8; IDENTITY_BYTES_V1],
        inputs: &[WorkerV3ApplicationInputOccurrenceV1],
        budget: WorkerV3ApplicationHandoffCodecBudgetV1,
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        validate_application_identity(application)?;
        if spawn_identity == [0; IDENTITY_BYTES_V1] {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity {
                field: "application spawn",
            });
        }
        validate_input_count(inputs.len(), budget)?;
        let allocation_bytes = inputs
            .len()
            .checked_mul(core::mem::size_of::<WorkerV3ApplicationInputOccurrenceV1>())
            .ok_or(WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow {
                field: "application inputs allocation",
            })?;
        require_allocation_budget(allocation_bytes, budget)?;
        let mut canonical = Vec::new();
        canonical.try_reserve_exact(inputs.len()).map_err(|_| {
            WorkerV3ApplicationHandoffProtocolErrorV1::AllocationFailed {
                field: "application inputs",
            }
        })?;
        canonical.extend_from_slice(inputs);
        canonical.sort_unstable_by_key(|input| input.slot);
        validate_canonical_inputs(&canonical)?;
        let identity = derive_application_occurrence_identity(
            application,
            spawn_identity,
            canonical.as_slice(),
        );
        Ok(Self {
            application,
            spawn_identity,
            inputs: canonical.into_boxed_slice(),
            identity,
        })
    }

    pub const fn application(&self) -> WorkerV3ApplicationIdentityV1 {
        self.application
    }

    pub const fn spawn_identity(&self) -> [u8; IDENTITY_BYTES_V1] {
        self.spawn_identity
    }

    pub const fn inputs(&self) -> &[WorkerV3ApplicationInputOccurrenceV1] {
        &self.inputs
    }

    pub const fn identity(&self) -> WorkerV3ApplicationOccurrenceIdentityV1 {
        self.identity
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
        self.encode_canonical_with_budget(WorkerV3ApplicationHandoffCodecBudgetV1::production())
    }

    pub fn encode_canonical_with_budget(
        &self,
        budget: WorkerV3ApplicationHandoffCodecBudgetV1,
    ) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
        validate_application_identity(self.application)?;
        validate_input_count(self.inputs.len(), budget)?;
        validate_canonical_inputs(&self.inputs)?;
        if self.identity
            != derive_application_occurrence_identity(
                self.application,
                self.spawn_identity,
                &self.inputs,
            )
        {
            return Err(
                WorkerV3ApplicationHandoffProtocolErrorV1::IdentityMismatch {
                    field: "application occurrence",
                },
            );
        }
        let payload_len = APPLICATION_OCCURRENCE_FIXED_PAYLOAD_BYTES_V1
            .checked_add(
                self.inputs
                    .len()
                    .checked_mul(APPLICATION_INPUT_BYTES_V1)
                    .ok_or(WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow {
                        field: "application occurrence",
                    })?,
            )
            .ok_or(WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow {
                field: "application occurrence",
            })?;
        let mut payload = allocate_bytes(payload_len, budget, "application occurrence payload")?;
        push_exact_identity(
            &mut payload,
            self.application.sha256,
            self.application.byte_len,
        );
        payload.extend_from_slice(&self.spawn_identity);
        payload.extend_from_slice(&(self.inputs.len() as u16).to_le_bytes());
        payload.extend_from_slice(&0_u16.to_le_bytes());
        for input in &self.inputs {
            payload.extend_from_slice(&input.slot.to_le_bytes());
            payload.extend_from_slice(&0_u16.to_le_bytes());
            payload.extend_from_slice(&input.identity);
        }
        payload.extend_from_slice(&self.identity.0);
        debug_assert_eq!(payload.len(), payload_len);
        encode_frame(
            APPLICATION_OCCURRENCE_MAGIC_V1,
            APPLICATION_OCCURRENCE_CHECKSUM_DOMAIN_V1,
            &payload,
            MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1,
            budget,
            "application occurrence",
        )
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        Self::decode_canonical_with_budget(
            bytes,
            WorkerV3ApplicationHandoffCodecBudgetV1::production(),
        )
    }

    pub fn decode_canonical_with_budget(
        bytes: &[u8],
        budget: WorkerV3ApplicationHandoffCodecBudgetV1,
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        let payload = decode_frame(
            bytes,
            APPLICATION_OCCURRENCE_MAGIC_V1,
            APPLICATION_OCCURRENCE_CHECKSUM_DOMAIN_V1,
            APPLICATION_OCCURRENCE_FIXED_PAYLOAD_BYTES_V1,
            MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1,
            budget,
            "application occurrence",
        )?;
        let mut reader = ReaderV1::new(payload);
        let application = WorkerV3ApplicationIdentityV1 {
            sha256: reader.array()?,
            byte_len: reader.u64()?,
        };
        validate_application_identity(application)?;
        let spawn_identity = reader.array()?;
        if spawn_identity == [0; IDENTITY_BYTES_V1] {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity {
                field: "application spawn",
            });
        }
        let input_count = usize::from(reader.u16()?);
        if reader.u16()? != 0 {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::NonZeroReserved);
        }
        validate_input_count(input_count, budget)?;
        let input_bytes = input_count.checked_mul(APPLICATION_INPUT_BYTES_V1).ok_or(
            WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow {
                field: "application occurrence inputs",
            },
        )?;
        let expected_payload_len = APPLICATION_OCCURRENCE_FIXED_PAYLOAD_BYTES_V1
            .checked_add(input_bytes)
            .ok_or(WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow {
                field: "application occurrence",
            })?;
        if payload.len() != expected_payload_len {
            return Err(
                WorkerV3ApplicationHandoffProtocolErrorV1::InvalidPayloadLength {
                    field: "application occurrence",
                    actual: payload.len(),
                    expected: expected_payload_len,
                },
            );
        }
        let allocation_bytes = input_count
            .checked_mul(core::mem::size_of::<WorkerV3ApplicationInputOccurrenceV1>())
            .ok_or(WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow {
                field: "application inputs allocation",
            })?;
        require_allocation_budget(allocation_bytes, budget)?;
        let mut inputs = Vec::new();
        inputs.try_reserve_exact(input_count).map_err(|_| {
            WorkerV3ApplicationHandoffProtocolErrorV1::AllocationFailed {
                field: "application inputs",
            }
        })?;
        for _ in 0..input_count {
            let slot = reader.u16()?;
            if reader.u16()? != 0 {
                return Err(WorkerV3ApplicationHandoffProtocolErrorV1::NonZeroReserved);
            }
            inputs.push(WorkerV3ApplicationInputOccurrenceV1::new(
                slot,
                reader.array()?,
            )?);
        }
        validate_canonical_inputs(&inputs)?;
        let identity = WorkerV3ApplicationOccurrenceIdentityV1(reader.array()?);
        if !reader.is_empty() {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::TrailingBytes {
                field: "application occurrence payload",
                actual: reader.remaining_len(),
            });
        }
        let expected_identity =
            derive_application_occurrence_identity(application, spawn_identity, &inputs);
        if identity != expected_identity {
            return Err(
                WorkerV3ApplicationHandoffProtocolErrorV1::IdentityMismatch {
                    field: "application occurrence",
                },
            );
        }
        let value = Self {
            application,
            spawn_identity,
            inputs: inputs.into_boxed_slice(),
            identity,
        };
        if value.encode_canonical()?.as_slice() != bytes {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::NonCanonical {
                field: "application occurrence",
            });
        }
        Ok(value)
    }

    pub const fn authenticates_application_occurrence(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Fresh runner value that scopes one acknowledgment to one application spawn.
///
/// Freshness must be supplied and tracked by the runner. This serializable value does not itself
/// prove freshness and carries no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationHandoffChallengeV1([u8; IDENTITY_BYTES_V1]);

impl WorkerV3ApplicationHandoffChallengeV1 {
    pub fn from_bytes(
        bytes: [u8; IDENTITY_BYTES_V1],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        if bytes == [0; IDENTITY_BYTES_V1] {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroChallenge);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(self) -> [u8; IDENTITY_BYTES_V1] {
        self.0
    }

    pub fn encode_canonical(self) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
        encode_digest_frame(
            self.0,
            APPLICATION_CHALLENGE_MAGIC_V1,
            APPLICATION_CHALLENGE_CHECKSUM_DOMAIN_V1,
            WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_BYTES_V1,
            "Worker V3 application challenge",
        )
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        Self::from_bytes(decode_digest_frame(
            bytes,
            APPLICATION_CHALLENGE_MAGIC_V1,
            APPLICATION_CHALLENGE_CHECKSUM_DOMAIN_V1,
            WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_BYTES_V1,
            "Worker V3 application challenge",
        )?)
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Canonical content binding for an opaque V3 envelope and one application occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationHandoffCommitmentV1([u8; IDENTITY_BYTES_V1]);

impl WorkerV3ApplicationHandoffCommitmentV1 {
    pub const fn as_bytes(self) -> [u8; IDENTITY_BYTES_V1] {
        self.0
    }

    pub fn encode_canonical(self) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
        encode_digest_frame(
            self.0,
            APPLICATION_COMMITMENT_MAGIC_V1,
            APPLICATION_COMMITMENT_CHECKSUM_DOMAIN_V1,
            WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_BYTES_V1,
            "Worker V3 application commitment",
        )
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        let value = decode_digest_frame(
            bytes,
            APPLICATION_COMMITMENT_MAGIC_V1,
            APPLICATION_COMMITMENT_CHECKSUM_DOMAIN_V1,
            WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_BYTES_V1,
            "Worker V3 application commitment",
        )?;
        if value == [0; IDENTITY_BYTES_V1] {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity {
                field: "Worker V3 application commitment",
            });
        }
        Ok(Self(value))
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Expected immutable state for one Worker V3 application handoff.
///
/// The expectation binds exact opaque envelope bytes to the application image and complete
/// application occurrence, including its spawn identity and non-aliasing inherited inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationHandoffExpectationV1 {
    envelope: WorkerV3LoadEnvelopeIdentityV1,
    application: WorkerV3ApplicationIdentityV1,
    occurrence: WorkerV3ApplicationOccurrenceIdentityV1,
    commitment: WorkerV3ApplicationHandoffCommitmentV1,
}

impl WorkerV3ApplicationHandoffExpectationV1 {
    pub fn new(
        envelope: WorkerV3LoadEnvelopeIdentityV1,
        occurrence: &WorkerV3ApplicationOccurrenceV1,
    ) -> Self {
        let application = occurrence.application();
        let occurrence = occurrence.identity();
        let commitment = derive_application_handoff_commitment(envelope, application, occurrence);
        Self {
            envelope,
            application,
            occurrence,
            commitment,
        }
    }

    pub const fn envelope(self) -> WorkerV3LoadEnvelopeIdentityV1 {
        self.envelope
    }

    pub const fn application(self) -> WorkerV3ApplicationIdentityV1 {
        self.application
    }

    pub const fn occurrence(self) -> WorkerV3ApplicationOccurrenceIdentityV1 {
        self.occurrence
    }

    pub const fn commitment(self) -> WorkerV3ApplicationHandoffCommitmentV1 {
        self.commitment
    }

    pub const fn acknowledgment(
        self,
        challenge: WorkerV3ApplicationHandoffChallengeV1,
    ) -> WorkerV3ApplicationHandoffAckV1 {
        WorkerV3ApplicationHandoffAckV1 {
            challenge,
            envelope: self.envelope,
            application: self.application,
            occurrence: self.occurrence,
            commitment: self.commitment,
        }
    }

    pub fn encode_canonical(self) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
        validate_handoff_state(
            self.envelope,
            self.application,
            self.occurrence,
            self.commitment,
        )?;
        let budget = WorkerV3ApplicationHandoffCodecBudgetV1::production();
        let mut payload = allocate_bytes(
            APPLICATION_EXPECTATION_PAYLOAD_BYTES_V1,
            budget,
            "Worker V3 application expectation payload",
        )?;
        push_handoff_state(
            &mut payload,
            self.envelope,
            self.application,
            self.occurrence,
            self.commitment,
        );
        encode_frame(
            APPLICATION_EXPECTATION_MAGIC_V1,
            APPLICATION_EXPECTATION_CHECKSUM_DOMAIN_V1,
            &payload,
            WORKER_V3_APPLICATION_HANDOFF_EXPECTATION_BYTES_V1,
            budget,
            "Worker V3 application expectation",
        )
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        let payload = decode_frame(
            bytes,
            APPLICATION_EXPECTATION_MAGIC_V1,
            APPLICATION_EXPECTATION_CHECKSUM_DOMAIN_V1,
            APPLICATION_EXPECTATION_PAYLOAD_BYTES_V1,
            WORKER_V3_APPLICATION_HANDOFF_EXPECTATION_BYTES_V1,
            WorkerV3ApplicationHandoffCodecBudgetV1::production(),
            "Worker V3 application expectation",
        )?;
        if payload.len() != APPLICATION_EXPECTATION_PAYLOAD_BYTES_V1 {
            return Err(
                WorkerV3ApplicationHandoffProtocolErrorV1::InvalidPayloadLength {
                    field: "Worker V3 application expectation",
                    actual: payload.len(),
                    expected: APPLICATION_EXPECTATION_PAYLOAD_BYTES_V1,
                },
            );
        }
        let mut reader = ReaderV1::new(payload);
        let (envelope, application, occurrence, commitment) = decode_handoff_state(&mut reader)?;
        debug_assert!(reader.is_empty());
        let value = Self {
            envelope,
            application,
            occurrence,
            commitment,
        };
        if value.encode_canonical()?.as_slice() != bytes {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::NonCanonical {
                field: "Worker V3 application expectation",
            });
        }
        Ok(value)
    }

    pub const fn authenticates_application_occurrence(self) -> bool {
        false
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Canonical liveness acknowledgment for one challenge and expected V3 application state.
///
/// Every field is available to both participants. The ACK proves only reproducible possession of
/// those values; it grants no publication, currentness, load, launch, or process authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationHandoffAckV1 {
    challenge: WorkerV3ApplicationHandoffChallengeV1,
    envelope: WorkerV3LoadEnvelopeIdentityV1,
    application: WorkerV3ApplicationIdentityV1,
    occurrence: WorkerV3ApplicationOccurrenceIdentityV1,
    commitment: WorkerV3ApplicationHandoffCommitmentV1,
}

impl WorkerV3ApplicationHandoffAckV1 {
    pub const fn challenge(self) -> WorkerV3ApplicationHandoffChallengeV1 {
        self.challenge
    }

    pub const fn envelope(self) -> WorkerV3LoadEnvelopeIdentityV1 {
        self.envelope
    }

    pub const fn application(self) -> WorkerV3ApplicationIdentityV1 {
        self.application
    }

    pub const fn occurrence(self) -> WorkerV3ApplicationOccurrenceIdentityV1 {
        self.occurrence
    }

    pub const fn commitment(self) -> WorkerV3ApplicationHandoffCommitmentV1 {
        self.commitment
    }

    pub fn encode_canonical(self) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
        validate_handoff_state(
            self.envelope,
            self.application,
            self.occurrence,
            self.commitment,
        )?;
        let budget = WorkerV3ApplicationHandoffCodecBudgetV1::production();
        let mut payload = allocate_bytes(
            APPLICATION_ACK_PAYLOAD_BYTES_V1,
            budget,
            "Worker V3 application acknowledgment payload",
        )?;
        payload.extend_from_slice(&self.challenge.0);
        push_handoff_state(
            &mut payload,
            self.envelope,
            self.application,
            self.occurrence,
            self.commitment,
        );
        encode_frame(
            APPLICATION_ACK_MAGIC_V1,
            APPLICATION_ACK_CHECKSUM_DOMAIN_V1,
            &payload,
            WORKER_V3_APPLICATION_HANDOFF_ACK_BYTES_V1,
            budget,
            "Worker V3 application acknowledgment",
        )
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3ApplicationHandoffProtocolErrorV1> {
        let payload = decode_frame(
            bytes,
            APPLICATION_ACK_MAGIC_V1,
            APPLICATION_ACK_CHECKSUM_DOMAIN_V1,
            APPLICATION_ACK_PAYLOAD_BYTES_V1,
            WORKER_V3_APPLICATION_HANDOFF_ACK_BYTES_V1,
            WorkerV3ApplicationHandoffCodecBudgetV1::production(),
            "Worker V3 application acknowledgment",
        )?;
        if payload.len() != APPLICATION_ACK_PAYLOAD_BYTES_V1 {
            return Err(
                WorkerV3ApplicationHandoffProtocolErrorV1::InvalidPayloadLength {
                    field: "Worker V3 application acknowledgment",
                    actual: payload.len(),
                    expected: APPLICATION_ACK_PAYLOAD_BYTES_V1,
                },
            );
        }
        let mut reader = ReaderV1::new(payload);
        let challenge = WorkerV3ApplicationHandoffChallengeV1::from_bytes(reader.array()?)?;
        let (envelope, application, occurrence, commitment) = decode_handoff_state(&mut reader)?;
        debug_assert!(reader.is_empty());
        let value = Self {
            challenge,
            envelope,
            application,
            occurrence,
            commitment,
        };
        if value.encode_canonical()?.as_slice() != bytes {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::NonCanonical {
                field: "Worker V3 application acknowledgment",
            });
        }
        Ok(value)
    }

    pub fn validate(
        self,
        expected: WorkerV3ApplicationHandoffExpectationV1,
        challenge: WorkerV3ApplicationHandoffChallengeV1,
    ) -> Result<(), WorkerV3ApplicationHandoffProtocolErrorV1> {
        if self.challenge != challenge {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ChallengeMismatch);
        }
        if self.application != expected.application {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ApplicationMismatch);
        }
        if self.occurrence != expected.occurrence {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ApplicationOccurrenceMismatch);
        }
        if self.envelope != expected.envelope {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::EnvelopeMismatch);
        }
        if self.commitment != expected.commitment {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::CommitmentMismatch);
        }
        Ok(())
    }

    pub const fn authenticates_application_occurrence(self) -> bool {
        false
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Strict V3 application handoff construction or decoding failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3ApplicationHandoffProtocolErrorV1 {
    ApplicationImage(SealedStaticApplicationErrorV1),
    ZeroChallenge,
    EmptyValue {
        field: &'static str,
    },
    ZeroIdentity {
        field: &'static str,
    },
    ZeroInputSlot,
    EmptyInputs,
    TooManyInputs {
        actual: usize,
        max: usize,
    },
    DuplicateInputSlot {
        slot: u16,
    },
    AliasedInputs,
    LengthOverflow {
        field: &'static str,
    },
    WireTooLarge {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    AllocationBudgetExceeded {
        field: &'static str,
        required: usize,
        max: usize,
    },
    AllocationFailed {
        field: &'static str,
    },
    Truncated {
        field: &'static str,
        actual: usize,
        minimum: usize,
    },
    TrailingBytes {
        field: &'static str,
        actual: usize,
    },
    BadMagic {
        field: &'static str,
    },
    UnsupportedVersion {
        field: &'static str,
        actual: u16,
    },
    NonZeroReserved,
    InvalidTotalLength {
        field: &'static str,
        declared: usize,
        actual: usize,
    },
    InvalidPayloadLength {
        field: &'static str,
        actual: usize,
        expected: usize,
    },
    ChecksumMismatch {
        field: &'static str,
    },
    IdentityMismatch {
        field: &'static str,
    },
    NonCanonicalInputs,
    NonCanonical {
        field: &'static str,
    },
    ChallengeMismatch,
    CommitmentMismatch,
    EnvelopeMismatch,
    ApplicationMismatch,
    ApplicationOccurrenceMismatch,
}

impl fmt::Display for WorkerV3ApplicationHandoffProtocolErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApplicationImage(error) => {
                write!(formatter, "invalid application image: {error}")
            }
            Self::ZeroChallenge => {
                formatter.write_str("Worker V3 application handoff challenge is zero")
            }
            Self::EmptyValue { field } => write!(formatter, "{field} is empty"),
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity is zero"),
            Self::ZeroInputSlot => formatter.write_str("application input slot is zero"),
            Self::EmptyInputs => formatter.write_str("application occurrence has no inputs"),
            Self::TooManyInputs { actual, max } => {
                write!(
                    formatter,
                    "application occurrence has {actual} inputs; maximum is {max}"
                )
            }
            Self::DuplicateInputSlot { slot } => {
                write!(formatter, "application input slot {slot} is duplicated")
            }
            Self::AliasedInputs => {
                formatter.write_str("application input occurrences alias each other")
            }
            Self::LengthOverflow { field } => write!(formatter, "{field} length overflows"),
            Self::WireTooLarge { field, actual, max } => {
                write!(
                    formatter,
                    "{field} wire is {actual} bytes; maximum is {max}"
                )
            }
            Self::AllocationBudgetExceeded {
                field,
                required,
                max,
            } => write!(
                formatter,
                "{field} requires {required} allocation bytes; budget is {max}"
            ),
            Self::AllocationFailed { field } => write!(formatter, "failed to allocate {field}"),
            Self::Truncated {
                field,
                actual,
                minimum,
            } => write!(
                formatter,
                "{field} is truncated ({actual} bytes; minimum is {minimum})"
            ),
            Self::TrailingBytes { field, actual } => {
                write!(formatter, "{field} has {actual} trailing bytes")
            }
            Self::BadMagic { field } => write!(formatter, "invalid {field} magic"),
            Self::UnsupportedVersion { field, actual } => {
                write!(formatter, "unsupported {field} version {actual}")
            }
            Self::NonZeroReserved => formatter.write_str("reserved V3 handoff field is nonzero"),
            Self::InvalidTotalLength {
                field,
                declared,
                actual,
            } => write!(
                formatter,
                "{field} declares {declared} bytes but received {actual}"
            ),
            Self::InvalidPayloadLength {
                field,
                actual,
                expected,
            } => write!(
                formatter,
                "{field} payload is {actual} bytes; expected {expected}"
            ),
            Self::ChecksumMismatch { field } => write!(formatter, "{field} checksum mismatch"),
            Self::IdentityMismatch { field } => write!(formatter, "{field} identity mismatch"),
            Self::NonCanonicalInputs => {
                formatter.write_str("application inputs are not in canonical slot order")
            }
            Self::NonCanonical { field } => write!(formatter, "{field} is not canonical"),
            Self::ChallengeMismatch => {
                formatter.write_str("Worker V3 application handoff challenge mismatch")
            }
            Self::CommitmentMismatch => {
                formatter.write_str("Worker V3 application handoff commitment mismatch")
            }
            Self::EnvelopeMismatch => {
                formatter.write_str("Worker V3 application handoff envelope mismatch")
            }
            Self::ApplicationMismatch => {
                formatter.write_str("Worker V3 application handoff application mismatch")
            }
            Self::ApplicationOccurrenceMismatch => {
                formatter.write_str("Worker V3 application handoff application occurrence mismatch")
            }
        }
    }
}

impl std::error::Error for WorkerV3ApplicationHandoffProtocolErrorV1 {}

fn derive_application_handoff_commitment(
    envelope: WorkerV3LoadEnvelopeIdentityV1,
    application: WorkerV3ApplicationIdentityV1,
    occurrence: WorkerV3ApplicationOccurrenceIdentityV1,
) -> WorkerV3ApplicationHandoffCommitmentV1 {
    let mut digest = Sha256::new();
    digest.update(APPLICATION_COMMITMENT_IDENTITY_DOMAIN_V1);
    digest.update(envelope.sha256);
    digest.update(envelope.byte_len.to_le_bytes());
    digest.update(application.sha256);
    digest.update(application.byte_len.to_le_bytes());
    digest.update(occurrence.0);
    WorkerV3ApplicationHandoffCommitmentV1(digest.finalize().into())
}

fn validate_handoff_state(
    envelope: WorkerV3LoadEnvelopeIdentityV1,
    application: WorkerV3ApplicationIdentityV1,
    occurrence: WorkerV3ApplicationOccurrenceIdentityV1,
    commitment: WorkerV3ApplicationHandoffCommitmentV1,
) -> Result<(), WorkerV3ApplicationHandoffProtocolErrorV1> {
    validate_exact_identity(envelope.sha256, envelope.byte_len, "V3 load envelope")?;
    validate_application_identity(application)?;
    if occurrence.0 == [0; IDENTITY_BYTES_V1] {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity {
            field: "application occurrence",
        });
    }
    if commitment.0 == [0; IDENTITY_BYTES_V1] {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity {
            field: "Worker V3 application commitment",
        });
    }
    let expected = derive_application_handoff_commitment(envelope, application, occurrence);
    if commitment != expected {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::CommitmentMismatch);
    }
    Ok(())
}

fn push_handoff_state(
    bytes: &mut Vec<u8>,
    envelope: WorkerV3LoadEnvelopeIdentityV1,
    application: WorkerV3ApplicationIdentityV1,
    occurrence: WorkerV3ApplicationOccurrenceIdentityV1,
    commitment: WorkerV3ApplicationHandoffCommitmentV1,
) {
    push_exact_identity(bytes, envelope.sha256, envelope.byte_len);
    push_exact_identity(bytes, application.sha256, application.byte_len);
    bytes.extend_from_slice(&occurrence.0);
    bytes.extend_from_slice(&commitment.0);
}

fn decode_handoff_state(
    reader: &mut ReaderV1<'_>,
) -> Result<
    (
        WorkerV3LoadEnvelopeIdentityV1,
        WorkerV3ApplicationIdentityV1,
        WorkerV3ApplicationOccurrenceIdentityV1,
        WorkerV3ApplicationHandoffCommitmentV1,
    ),
    WorkerV3ApplicationHandoffProtocolErrorV1,
> {
    let envelope = WorkerV3LoadEnvelopeIdentityV1 {
        sha256: reader.array()?,
        byte_len: reader.u64()?,
    };
    let application = WorkerV3ApplicationIdentityV1 {
        sha256: reader.array()?,
        byte_len: reader.u64()?,
    };
    let occurrence = WorkerV3ApplicationOccurrenceIdentityV1(reader.array()?);
    let commitment = WorkerV3ApplicationHandoffCommitmentV1(reader.array()?);
    validate_handoff_state(envelope, application, occurrence, commitment)?;
    Ok((envelope, application, occurrence, commitment))
}

fn encode_digest_frame(
    value: [u8; IDENTITY_BYTES_V1],
    magic: [u8; 8],
    checksum_domain: &[u8],
    wire_bytes: usize,
    field: &'static str,
) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
    if value == [0; IDENTITY_BYTES_V1] {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity { field });
    }
    encode_frame(
        magic,
        checksum_domain,
        &value,
        wire_bytes,
        WorkerV3ApplicationHandoffCodecBudgetV1::production(),
        field,
    )
}

fn decode_digest_frame(
    bytes: &[u8],
    magic: [u8; 8],
    checksum_domain: &[u8],
    wire_bytes: usize,
    field: &'static str,
) -> Result<[u8; IDENTITY_BYTES_V1], WorkerV3ApplicationHandoffProtocolErrorV1> {
    let payload = decode_frame(
        bytes,
        magic,
        checksum_domain,
        IDENTITY_BYTES_V1,
        wire_bytes,
        WorkerV3ApplicationHandoffCodecBudgetV1::production(),
        field,
    )?;
    if payload.len() != IDENTITY_BYTES_V1 {
        return Err(
            WorkerV3ApplicationHandoffProtocolErrorV1::InvalidPayloadLength {
                field,
                actual: payload.len(),
                expected: IDENTITY_BYTES_V1,
            },
        );
    }
    payload.try_into().map_err(|_| {
        WorkerV3ApplicationHandoffProtocolErrorV1::InvalidPayloadLength {
            field,
            actual: payload.len(),
            expected: IDENTITY_BYTES_V1,
        }
    })
}

fn validate_application_identity(
    application: WorkerV3ApplicationIdentityV1,
) -> Result<(), WorkerV3ApplicationHandoffProtocolErrorV1> {
    validate_exact_identity(
        application.sha256,
        application.byte_len,
        "application image",
    )
}

fn validate_exact_identity(
    sha256: [u8; IDENTITY_BYTES_V1],
    byte_len: u64,
    field: &'static str,
) -> Result<(), WorkerV3ApplicationHandoffProtocolErrorV1> {
    if sha256 == [0; IDENTITY_BYTES_V1] {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity { field });
    }
    if byte_len == 0 {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::EmptyValue { field });
    }
    Ok(())
}

fn validate_input_count(
    actual: usize,
    budget: WorkerV3ApplicationHandoffCodecBudgetV1,
) -> Result<(), WorkerV3ApplicationHandoffProtocolErrorV1> {
    if actual == 0 {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::EmptyInputs);
    }
    let max = budget.max_inputs.min(MAX_WORKER_V3_APPLICATION_INPUTS_V1);
    if actual > max {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::TooManyInputs { actual, max });
    }
    Ok(())
}

fn validate_canonical_inputs(
    inputs: &[WorkerV3ApplicationInputOccurrenceV1],
) -> Result<(), WorkerV3ApplicationHandoffProtocolErrorV1> {
    for input in inputs {
        if input.slot == 0 {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroInputSlot);
        }
        if input.identity == [0; IDENTITY_BYTES_V1] {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity {
                field: "application input occurrence",
            });
        }
    }
    for pair in inputs.windows(2) {
        if pair[0].slot == pair[1].slot {
            return Err(
                WorkerV3ApplicationHandoffProtocolErrorV1::DuplicateInputSlot {
                    slot: pair[0].slot,
                },
            );
        }
        if pair[0].slot > pair[1].slot {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::NonCanonicalInputs);
        }
    }
    for (index, input) in inputs.iter().enumerate() {
        if inputs[..index]
            .iter()
            .any(|other| other.identity == input.identity)
        {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::AliasedInputs);
        }
    }
    Ok(())
}

fn derive_application_occurrence_identity(
    application: WorkerV3ApplicationIdentityV1,
    spawn_identity: [u8; IDENTITY_BYTES_V1],
    inputs: &[WorkerV3ApplicationInputOccurrenceV1],
) -> WorkerV3ApplicationOccurrenceIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(APPLICATION_OCCURRENCE_IDENTITY_DOMAIN_V1);
    digest.update(application.sha256);
    digest.update(application.byte_len.to_le_bytes());
    digest.update(spawn_identity);
    digest.update((inputs.len() as u16).to_le_bytes());
    for input in inputs {
        digest.update(input.slot.to_le_bytes());
        digest.update(input.identity);
    }
    WorkerV3ApplicationOccurrenceIdentityV1(digest.finalize().into())
}

fn hash_exact_bytes(domain: &[u8], bytes: &[u8]) -> [u8; IDENTITY_BYTES_V1] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn encode_exact_identity(
    sha256: [u8; IDENTITY_BYTES_V1],
    byte_len: u64,
    magic: [u8; 8],
    checksum_domain: &[u8],
    budget: WorkerV3ApplicationHandoffCodecBudgetV1,
) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
    validate_exact_identity(sha256, byte_len, "exact identity")?;
    let mut payload = allocate_bytes(EXACT_IDENTITY_PAYLOAD_BYTES_V1, budget, "exact identity")?;
    push_exact_identity(&mut payload, sha256, byte_len);
    encode_frame(
        magic,
        checksum_domain,
        &payload,
        EXACT_IDENTITY_WIRE_BYTES_V1,
        budget,
        "exact identity",
    )
}

fn decode_exact_identity(
    bytes: &[u8],
    field: &'static str,
    magic: [u8; 8],
    checksum_domain: &[u8],
    budget: WorkerV3ApplicationHandoffCodecBudgetV1,
) -> Result<([u8; IDENTITY_BYTES_V1], u64), WorkerV3ApplicationHandoffProtocolErrorV1> {
    let payload = decode_frame(
        bytes,
        magic,
        checksum_domain,
        EXACT_IDENTITY_PAYLOAD_BYTES_V1,
        EXACT_IDENTITY_WIRE_BYTES_V1,
        budget,
        field,
    )?;
    if payload.len() != EXACT_IDENTITY_PAYLOAD_BYTES_V1 {
        return Err(
            WorkerV3ApplicationHandoffProtocolErrorV1::InvalidPayloadLength {
                field,
                actual: payload.len(),
                expected: EXACT_IDENTITY_PAYLOAD_BYTES_V1,
            },
        );
    }
    let mut reader = ReaderV1::new(payload);
    let sha256 = reader.array()?;
    let byte_len = reader.u64()?;
    validate_exact_identity(sha256, byte_len, field)?;
    Ok((sha256, byte_len))
}

fn push_exact_identity(bytes: &mut Vec<u8>, sha256: [u8; IDENTITY_BYTES_V1], byte_len: u64) {
    bytes.extend_from_slice(&sha256);
    bytes.extend_from_slice(&byte_len.to_le_bytes());
}

fn encode_frame(
    magic: [u8; 8],
    checksum_domain: &[u8],
    payload: &[u8],
    protocol_max: usize,
    budget: WorkerV3ApplicationHandoffCodecBudgetV1,
    field: &'static str,
) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
    let total_len = FRAME_HEADER_BYTES_V1
        .checked_add(payload.len())
        .and_then(|length| length.checked_add(FRAME_CHECKSUM_BYTES_V1))
        .ok_or(WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow { field })?;
    let max = protocol_max.min(budget.max_wire_bytes);
    if total_len > max {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::WireTooLarge {
            field,
            actual: total_len,
            max,
        });
    }
    let total_u32 = u32::try_from(total_len)
        .map_err(|_| WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow { field })?;
    let payload_u32 = u32::try_from(payload.len())
        .map_err(|_| WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow { field })?;
    let mut bytes = allocate_bytes(total_len, budget, field)?;
    bytes.extend_from_slice(&magic);
    bytes.extend_from_slice(&WORKER_V3_APPLICATION_HANDOFF_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&total_u32.to_le_bytes());
    bytes.extend_from_slice(&payload_u32.to_le_bytes());
    bytes.extend_from_slice(payload);
    let checksum = checksum(checksum_domain, &bytes);
    bytes.extend_from_slice(&checksum);
    debug_assert_eq!(bytes.len(), total_len);
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
fn decode_frame<'a>(
    bytes: &'a [u8],
    expected_magic: [u8; 8],
    checksum_domain: &[u8],
    minimum_payload_bytes: usize,
    protocol_max: usize,
    budget: WorkerV3ApplicationHandoffCodecBudgetV1,
    field: &'static str,
) -> Result<&'a [u8], WorkerV3ApplicationHandoffProtocolErrorV1> {
    let max = protocol_max.min(budget.max_wire_bytes);
    if bytes.len() > max {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::WireTooLarge {
            field,
            actual: bytes.len(),
            max,
        });
    }
    let minimum = FRAME_HEADER_BYTES_V1
        .checked_add(minimum_payload_bytes)
        .and_then(|length| length.checked_add(FRAME_CHECKSUM_BYTES_V1))
        .ok_or(WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow { field })?;
    if bytes.len() < FRAME_HEADER_BYTES_V1 {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::Truncated {
            field,
            actual: bytes.len(),
            minimum,
        });
    }
    let mut header = ReaderV1::new(bytes);
    if header.array::<8>()? != expected_magic {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::BadMagic { field });
    }
    let version = header.u16()?;
    if version != WORKER_V3_APPLICATION_HANDOFF_VERSION_V1 {
        return Err(
            WorkerV3ApplicationHandoffProtocolErrorV1::UnsupportedVersion {
                field,
                actual: version,
            },
        );
    }
    if header.u16()? != 0 {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::NonZeroReserved);
    }
    let declared_total = usize::try_from(header.u32()?)
        .map_err(|_| WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow { field })?;
    let payload_len = usize::try_from(header.u32()?)
        .map_err(|_| WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow { field })?;
    if declared_total < bytes.len() {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::TrailingBytes {
            field,
            actual: bytes.len() - declared_total,
        });
    }
    if declared_total > bytes.len() {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::Truncated {
            field,
            actual: bytes.len(),
            minimum: declared_total,
        });
    }
    if declared_total != bytes.len() {
        return Err(
            WorkerV3ApplicationHandoffProtocolErrorV1::InvalidTotalLength {
                field,
                declared: declared_total,
                actual: bytes.len(),
            },
        );
    }
    let expected_total = FRAME_HEADER_BYTES_V1
        .checked_add(payload_len)
        .and_then(|length| length.checked_add(FRAME_CHECKSUM_BYTES_V1))
        .ok_or(WorkerV3ApplicationHandoffProtocolErrorV1::LengthOverflow { field })?;
    if expected_total != declared_total {
        return Err(
            WorkerV3ApplicationHandoffProtocolErrorV1::InvalidPayloadLength {
                field,
                actual: payload_len,
                expected: declared_total
                    .saturating_sub(FRAME_HEADER_BYTES_V1 + FRAME_CHECKSUM_BYTES_V1),
            },
        );
    }
    if bytes.len() < minimum {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::Truncated {
            field,
            actual: bytes.len(),
            minimum,
        });
    }
    let body_len = bytes.len() - FRAME_CHECKSUM_BYTES_V1;
    let (body, actual_checksum) = bytes.split_at(body_len);
    if checksum(checksum_domain, body) != actual_checksum {
        return Err(WorkerV3ApplicationHandoffProtocolErrorV1::ChecksumMismatch { field });
    }
    Ok(&bytes[FRAME_HEADER_BYTES_V1..body_len])
}

fn allocate_bytes(
    capacity: usize,
    budget: WorkerV3ApplicationHandoffCodecBudgetV1,
    field: &'static str,
) -> Result<Vec<u8>, WorkerV3ApplicationHandoffProtocolErrorV1> {
    require_allocation_budget(capacity, budget)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| WorkerV3ApplicationHandoffProtocolErrorV1::AllocationFailed { field })?;
    Ok(bytes)
}

fn require_allocation_budget(
    required: usize,
    budget: WorkerV3ApplicationHandoffCodecBudgetV1,
) -> Result<(), WorkerV3ApplicationHandoffProtocolErrorV1> {
    if required > budget.max_allocation_bytes {
        return Err(
            WorkerV3ApplicationHandoffProtocolErrorV1::AllocationBudgetExceeded {
                field: "V3 application handoff",
                required,
                max: budget.max_allocation_bytes,
            },
        );
    }
    Ok(())
}

fn checksum(domain: &[u8], bytes: &[u8]) -> [u8; IDENTITY_BYTES_V1] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

struct ReaderV1<'a> {
    remaining: &'a [u8],
}

impl<'a> ReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], WorkerV3ApplicationHandoffProtocolErrorV1> {
        if self.remaining.len() < count {
            return Err(WorkerV3ApplicationHandoffProtocolErrorV1::Truncated {
                field: "V3 application handoff field",
                actual: self.remaining.len(),
                minimum: count,
            });
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], WorkerV3ApplicationHandoffProtocolErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerV3ApplicationHandoffProtocolErrorV1::Truncated {
                field: "V3 application handoff array",
                actual: self.remaining.len(),
                minimum: N,
            })
    }

    fn u16(&mut self) -> Result<u16, WorkerV3ApplicationHandoffProtocolErrorV1> {
        self.array().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> Result<u32, WorkerV3ApplicationHandoffProtocolErrorV1> {
        self.array().map(u32::from_le_bytes)
    }

    fn u64(&mut self) -> Result<u64, WorkerV3ApplicationHandoffProtocolErrorV1> {
        self.array().map(u64::from_le_bytes)
    }

    const fn remaining_len(&self) -> usize {
        self.remaining.len()
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn application(seed: u8) -> WorkerV3ApplicationIdentityV1 {
        WorkerV3ApplicationIdentityV1::from_test_parts([seed; 32], 1024 + u64::from(seed))
    }

    fn input(slot: u16, seed: u8) -> WorkerV3ApplicationInputOccurrenceV1 {
        WorkerV3ApplicationInputOccurrenceV1::new(slot, [seed; 32]).unwrap()
    }

    fn occurrence(seed: u8) -> WorkerV3ApplicationOccurrenceV1 {
        WorkerV3ApplicationOccurrenceV1::new(
            application(seed),
            [seed.wrapping_add(1); 32],
            &[
                input(3, seed.wrapping_add(4)),
                input(1, seed.wrapping_add(2)),
                input(2, seed.wrapping_add(3)),
            ],
        )
        .unwrap()
    }

    fn replace_checksum(bytes: &mut [u8], domain: &[u8]) {
        let body_len = bytes.len() - FRAME_CHECKSUM_BYTES_V1;
        let checksum = checksum(domain, &bytes[..body_len]);
        bytes[body_len..].copy_from_slice(&checksum);
    }

    #[test]
    fn opaque_envelope_identity_is_exact_schema_independent_and_inert() {
        let first =
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"opaque-v3-envelope").unwrap();
        let second =
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"opaque-v3-envelope!").unwrap();
        assert_ne!(first, second);
        assert_eq!(first.byte_len(), 18);
        assert!(!first.grants_publication_authority());
        assert!(!first.grants_load_authority());
        assert!(!first.grants_launch_authority());
        let bytes = first.encode_canonical().unwrap();
        assert_eq!(
            WorkerV3LoadEnvelopeIdentityV1::decode_canonical(&bytes),
            Ok(first)
        );
        assert!(matches!(
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(&[]),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::EmptyValue { .. })
        ));
    }

    #[test]
    fn exact_identity_codecs_have_distinct_magic_and_reject_cross_use() {
        let envelope = WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"envelope").unwrap();
        let application = application(7);
        let envelope_bytes = envelope.encode_canonical().unwrap();
        let application_bytes = application.encode_canonical().unwrap();
        assert_ne!(&envelope_bytes[..8], &application_bytes[..8]);
        assert!(matches!(
            WorkerV3ApplicationIdentityV1::decode_canonical(&envelope_bytes),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::BadMagic { .. })
        ));
        assert!(matches!(
            WorkerV3LoadEnvelopeIdentityV1::decode_canonical(&application_bytes),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::BadMagic { .. })
        ));
    }

    #[test]
    fn challenge_expectation_commitment_and_ack_round_trip_and_bind_state() {
        let envelope =
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"opaque envelope").unwrap();
        let occurrence = occurrence(9);
        let expectation = WorkerV3ApplicationHandoffExpectationV1::new(envelope, &occurrence);
        let challenge = WorkerV3ApplicationHandoffChallengeV1::from_bytes([77; 32]).unwrap();
        let ack = expectation.acknowledgment(challenge);

        let challenge_bytes = challenge.encode_canonical().unwrap();
        assert_eq!(
            WorkerV3ApplicationHandoffChallengeV1::decode_canonical(&challenge_bytes),
            Ok(challenge)
        );
        let commitment_bytes = expectation.commitment().encode_canonical().unwrap();
        assert_eq!(
            WorkerV3ApplicationHandoffCommitmentV1::decode_canonical(&commitment_bytes),
            Ok(expectation.commitment())
        );
        let expectation_bytes = expectation.encode_canonical().unwrap();
        assert_eq!(
            WorkerV3ApplicationHandoffExpectationV1::decode_canonical(&expectation_bytes),
            Ok(expectation)
        );
        let ack_bytes = ack.encode_canonical().unwrap();
        let decoded = WorkerV3ApplicationHandoffAckV1::decode_canonical(&ack_bytes).unwrap();
        assert_eq!(decoded, ack);
        assert_eq!(decoded.validate(expectation, challenge), Ok(()));
        assert_eq!(decoded.application(), occurrence.application());
        assert_eq!(decoded.occurrence(), occurrence.identity());

        assert!(!expectation.authenticates_application_occurrence());
        assert!(!expectation.grants_publication_authority());
        assert!(!expectation.grants_load_authority());
        assert!(!expectation.grants_launch_authority());
        assert!(!decoded.authenticates_application_occurrence());
        assert!(!decoded.grants_publication_authority());
        assert!(!decoded.grants_load_authority());
        assert!(!decoded.grants_launch_authority());
    }

    #[test]
    fn ack_rejects_stale_challenge_application_envelope_and_inputs() {
        let envelope = WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"first envelope").unwrap();
        let original_occurrence = occurrence(41);
        let expected = WorkerV3ApplicationHandoffExpectationV1::new(envelope, &original_occurrence);
        let challenge = WorkerV3ApplicationHandoffChallengeV1::from_bytes([42; 32]).unwrap();
        let ack = expected.acknowledgment(challenge);

        let stale_challenge = WorkerV3ApplicationHandoffChallengeV1::from_bytes([43; 32]).unwrap();
        assert_eq!(
            ack.validate(expected, stale_challenge),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::ChallengeMismatch)
        );

        let stale_application = occurrence(44);
        let stale_application_expectation =
            WorkerV3ApplicationHandoffExpectationV1::new(envelope, &stale_application);
        assert_eq!(
            ack.validate(stale_application_expectation, challenge),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::ApplicationMismatch)
        );

        let changed_inputs = WorkerV3ApplicationOccurrenceV1::new(
            original_occurrence.application(),
            original_occurrence.spawn_identity(),
            &[input(1, 43), input(2, 44), input(3, 99)],
        )
        .unwrap();
        let changed_input_expectation =
            WorkerV3ApplicationHandoffExpectationV1::new(envelope, &changed_inputs);
        assert_eq!(
            ack.validate(changed_input_expectation, challenge),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::ApplicationOccurrenceMismatch)
        );

        let stale_envelope =
            WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"second envelope").unwrap();
        let stale_envelope_expectation =
            WorkerV3ApplicationHandoffExpectationV1::new(stale_envelope, &original_occurrence);
        assert_eq!(
            ack.validate(stale_envelope_expectation, challenge),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::EnvelopeMismatch)
        );
    }

    #[test]
    fn every_worker_v3_state_wire_has_distinct_magic_and_rejects_cross_use() {
        let envelope = WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"envelope state").unwrap();
        let occurrence = occurrence(51);
        let expectation = WorkerV3ApplicationHandoffExpectationV1::new(envelope, &occurrence);
        let challenge = WorkerV3ApplicationHandoffChallengeV1::from_bytes([52; 32]).unwrap();
        let wires = [
            challenge.encode_canonical().unwrap(),
            expectation.commitment().encode_canonical().unwrap(),
            expectation.encode_canonical().unwrap(),
            expectation
                .acknowledgment(challenge)
                .encode_canonical()
                .unwrap(),
        ];
        for (index, wire) in wires.iter().enumerate() {
            for other in wires.iter().skip(index + 1) {
                assert_ne!(&wire[..8], &other[..8]);
            }
        }
        assert!(matches!(
            WorkerV3ApplicationHandoffCommitmentV1::decode_canonical(&wires[0]),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::BadMagic { .. })
        ));
        assert!(matches!(
            WorkerV3ApplicationHandoffAckV1::decode_canonical(&wires[2]),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::BadMagic { .. })
        ));

        let legacy_v1_prefix = b"FE2O3-WORKER-V2-APPLICATION-ACK\0";
        assert!(matches!(
            WorkerV3ApplicationHandoffAckV1::decode_canonical(legacy_v1_prefix),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::BadMagic { .. })
        ));
        assert!(crate::WorkerV2ApplicationHandoffAckV1::decode_canonical(&wires[3]).is_err());

        let mut wrong_version = wires[3].clone();
        wrong_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            WorkerV3ApplicationHandoffAckV1::decode_canonical(&wrong_version),
            Err(
                WorkerV3ApplicationHandoffProtocolErrorV1::UnsupportedVersion {
                    field: "Worker V3 application acknowledgment",
                    actual: 2,
                }
            )
        );
    }

    #[test]
    fn ack_codec_rejects_truncation_trailing_bytes_and_state_substitution() {
        let envelope = WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"ack envelope").unwrap();
        let occurrence = occurrence(61);
        let expectation = WorkerV3ApplicationHandoffExpectationV1::new(envelope, &occurrence);
        let challenge = WorkerV3ApplicationHandoffChallengeV1::from_bytes([62; 32]).unwrap();
        let bytes = expectation
            .acknowledgment(challenge)
            .encode_canonical()
            .unwrap();
        for length in 0..bytes.len() {
            assert!(
                WorkerV3ApplicationHandoffAckV1::decode_canonical(&bytes[..length]).is_err(),
                "accepted ACK truncation at {length}"
            );
        }

        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            WorkerV3ApplicationHandoffAckV1::decode_canonical(&trailing),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::TrailingBytes { .. })
                | Err(WorkerV3ApplicationHandoffProtocolErrorV1::WireTooLarge { .. })
        ));

        let application_offset = FRAME_HEADER_BYTES_V1
            + APPLICATION_CHALLENGE_PAYLOAD_BYTES_V1
            + EXACT_IDENTITY_PAYLOAD_BYTES_V1;
        let mut substituted = bytes;
        substituted[application_offset] ^= 1;
        replace_checksum(&mut substituted, APPLICATION_ACK_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            WorkerV3ApplicationHandoffAckV1::decode_canonical(&substituted),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::CommitmentMismatch)
        );
    }

    #[test]
    fn zero_challenge_and_zero_state_identities_fail_closed() {
        assert_eq!(
            WorkerV3ApplicationHandoffChallengeV1::from_bytes([0; 32]),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroChallenge)
        );

        let challenge = WorkerV3ApplicationHandoffChallengeV1::from_bytes([71; 32]).unwrap();
        let mut bytes = challenge.encode_canonical().unwrap();
        bytes[FRAME_HEADER_BYTES_V1..FRAME_HEADER_BYTES_V1 + 32].fill(0);
        replace_checksum(&mut bytes, APPLICATION_CHALLENGE_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            WorkerV3ApplicationHandoffChallengeV1::decode_canonical(&bytes),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroChallenge)
        );
    }

    #[test]
    fn application_occurrence_sorts_inputs_and_round_trips_canonically() {
        let value = occurrence(11);
        assert_eq!(
            value
                .inputs()
                .iter()
                .map(|input| input.slot())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        let bytes = value.encode_canonical().unwrap();
        assert_eq!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical(&bytes),
            Ok(value.clone())
        );
        assert!(!value.authenticates_application_occurrence());
        assert!(!value.grants_publication_authority());
        assert!(!value.grants_load_authority());
        assert!(!value.grants_launch_authority());
    }

    #[test]
    fn application_occurrence_rejects_aliasing_duplicate_slots_and_zero_values() {
        let app = application(3);
        let aliased = [input(1, 4), input(2, 4)];
        assert_eq!(
            WorkerV3ApplicationOccurrenceV1::new(app, [5; 32], &aliased),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::AliasedInputs)
        );
        let duplicated = [input(2, 4), input(2, 5)];
        assert_eq!(
            WorkerV3ApplicationOccurrenceV1::new(app, [5; 32], &duplicated),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::DuplicateInputSlot { slot: 2 })
        );
        assert!(matches!(
            WorkerV3ApplicationOccurrenceV1::new(app, [0; 32], &[input(1, 4)]),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity { .. })
        ));
        assert_eq!(
            WorkerV3ApplicationOccurrenceV1::new(app, [5; 32], &[]),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::EmptyInputs)
        );
        assert_eq!(
            WorkerV3ApplicationInputOccurrenceV1::new(0, [1; 32]),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroInputSlot)
        );
        assert!(matches!(
            WorkerV3ApplicationInputOccurrenceV1::new(1, [0; 32]),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::ZeroIdentity { .. })
        ));
    }

    #[test]
    fn decoder_rejects_every_truncation_and_trailing_bytes() {
        let bytes = occurrence(17).encode_canonical().unwrap();
        for length in 0..bytes.len() {
            assert!(
                WorkerV3ApplicationOccurrenceV1::decode_canonical(&bytes[..length]).is_err(),
                "accepted truncation at {length}"
            );
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical(&trailing),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::TrailingBytes { .. })
                | Err(WorkerV3ApplicationHandoffProtocolErrorV1::WireTooLarge { .. })
        ));
    }

    #[test]
    fn decoder_rejects_hostile_lengths_counts_reserved_and_checksum() {
        let value = occurrence(23);
        let original = value.encode_canonical().unwrap();

        let mut total = original.clone();
        total[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical(&total),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::Truncated { .. })
        ));

        let mut payload = original.clone();
        payload[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical(&payload),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::InvalidPayloadLength { .. })
        ));

        let count_offset = FRAME_HEADER_BYTES_V1 + EXACT_IDENTITY_PAYLOAD_BYTES_V1 + 32;
        let mut count = original.clone();
        count[count_offset..count_offset + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        replace_checksum(&mut count, APPLICATION_OCCURRENCE_CHECKSUM_DOMAIN_V1);
        assert!(matches!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical(&count),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::TooManyInputs { .. })
        ));

        let mut reserved = original.clone();
        reserved[count_offset + 2] = 1;
        replace_checksum(&mut reserved, APPLICATION_OCCURRENCE_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical(&reserved),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::NonZeroReserved)
        );

        let mut corrupted = original;
        corrupted[FRAME_HEADER_BYTES_V1] ^= 1;
        assert!(matches!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical(&corrupted),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn decoder_rejects_noncanonical_and_aliased_wire_inputs() {
        let original = occurrence(29).encode_canonical().unwrap();
        let inputs_offset = FRAME_HEADER_BYTES_V1 + EXACT_IDENTITY_PAYLOAD_BYTES_V1 + 32 + 2 + 2;

        let mut unordered = original.clone();
        let first = unordered[inputs_offset..inputs_offset + APPLICATION_INPUT_BYTES_V1].to_vec();
        let second = unordered[inputs_offset + APPLICATION_INPUT_BYTES_V1
            ..inputs_offset + 2 * APPLICATION_INPUT_BYTES_V1]
            .to_vec();
        unordered[inputs_offset..inputs_offset + APPLICATION_INPUT_BYTES_V1]
            .copy_from_slice(&second);
        unordered[inputs_offset + APPLICATION_INPUT_BYTES_V1
            ..inputs_offset + 2 * APPLICATION_INPUT_BYTES_V1]
            .copy_from_slice(&first);
        replace_checksum(&mut unordered, APPLICATION_OCCURRENCE_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical(&unordered),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::NonCanonicalInputs)
        );

        let mut aliased = original;
        let first_identity = inputs_offset + 4;
        let second_identity = inputs_offset + APPLICATION_INPUT_BYTES_V1 + 4;
        let identity = aliased[first_identity..first_identity + 32].to_vec();
        aliased[second_identity..second_identity + 32].copy_from_slice(&identity);
        replace_checksum(&mut aliased, APPLICATION_OCCURRENCE_CHECKSUM_DOMAIN_V1);
        assert_eq!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical(&aliased),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::AliasedInputs)
        );
    }

    #[test]
    fn count_and_allocation_budgets_fail_before_allocation() {
        let value = occurrence(31);
        let bytes = value.encode_canonical().unwrap();
        let count_budget = WorkerV3ApplicationHandoffCodecBudgetV1::new(
            MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1,
            MAX_WORKER_V3_APPLICATION_HANDOFF_ALLOCATION_BYTES_V1,
            2,
        );
        assert_eq!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical_with_budget(&bytes, count_budget),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::TooManyInputs { actual: 3, max: 2 })
        );

        let allocation_budget = WorkerV3ApplicationHandoffCodecBudgetV1::new(
            MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1,
            core::mem::size_of::<WorkerV3ApplicationInputOccurrenceV1>() * 3 - 1,
            MAX_WORKER_V3_APPLICATION_INPUTS_V1,
        );
        assert!(matches!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical_with_budget(
                &bytes,
                allocation_budget
            ),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::AllocationBudgetExceeded { .. })
        ));
    }

    #[test]
    fn maximum_input_count_is_bounded_and_canonical() {
        let inputs = (1..=MAX_WORKER_V3_APPLICATION_INPUTS_V1)
            .map(|index| input(index as u16, index as u8))
            .collect::<Vec<_>>();
        let value =
            WorkerV3ApplicationOccurrenceV1::new(application(37), [38; 32], &inputs).unwrap();
        let bytes = value.encode_canonical().unwrap();
        assert_eq!(bytes.len(), MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1);
        assert_eq!(
            WorkerV3ApplicationOccurrenceV1::decode_canonical(&bytes),
            Ok(value)
        );

        let mut too_many = inputs;
        too_many.push(input(65, 65));
        assert!(matches!(
            WorkerV3ApplicationOccurrenceV1::new(application(37), [38; 32], &too_many),
            Err(WorkerV3ApplicationHandoffProtocolErrorV1::TooManyInputs { .. })
        ));
    }
}
