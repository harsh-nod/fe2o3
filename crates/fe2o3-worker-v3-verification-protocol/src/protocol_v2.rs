use core::fmt;
use std::error::Error;

use fe2o3_runtime_protocol::{
    COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3,
    COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3,
    CompilerExecutionCurrentRecordAttestationV3, CompilerExecutionCurrentRecordVerificationErrorV3,
    CompilerExecutionCurrentRecordVerificationV3,
};
use sha2::{Digest, Sha256};

use crate::WorkerV3VerificationRequestV1;

const SHA256_BYTES: usize = 32;
const HEADER_BYTES: usize = 24;
const CHALLENGE_PREIMAGE_BYTES: usize = HEADER_BYTES + 8 + 3 * SHA256_BYTES;
const CURRENT_RECORD_PREIMAGE_BYTES: usize = HEADER_BYTES
    + 3 * SHA256_BYTES
    + COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3
    + COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3;
const TERMINAL_FIXED_PREIMAGE_BYTES: usize = HEADER_BYTES + 8 + 4 * SHA256_BYTES;
const CHALLENGE_MAGIC: [u8; 8] = *b"F3WV2CH\0";
const CURRENT_RECORD_MAGIC: [u8; 8] = *b"F3WV2CR\0";
const TERMINAL_MAGIC: [u8; 8] = *b"F3WV2TR\0";
const VERSION: u16 = 2;
const CHALLENGE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/VERIFICATION-CHALLENGE/V2\0";
const CURRENT_RECORD_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/VERIFICATION-CURRENT-RECORD/V2\0";
const TERMINAL_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/VERIFICATION-TERMINAL/V2\0";

/// Exact byte length of one V2 service-challenge frame.
pub const WORKER_V3_VERIFICATION_CHALLENGE_BYTES_V2: usize =
    CHALLENGE_PREIMAGE_BYTES + SHA256_BYTES;
/// Exact byte length of one V2 compiler-current-record submission frame.
pub const WORKER_V3_VERIFICATION_CURRENT_RECORD_BYTES_V2: usize =
    CURRENT_RECORD_PREIMAGE_BYTES + SHA256_BYTES;
/// Maximum opaque application response length in one V2 terminal frame.
pub const MAX_WORKER_V3_VERIFICATION_APPLICATION_RESPONSE_BYTES_V2: usize = 64 * 1024;
/// Minimum V2 terminal frame length, used by a generic rejection.
pub const MIN_WORKER_V3_VERIFICATION_TERMINAL_BYTES_V2: usize =
    TERMINAL_FIXED_PREIMAGE_BYTES + SHA256_BYTES;
/// Maximum V2 terminal frame length.
pub const MAX_WORKER_V3_VERIFICATION_TERMINAL_BYTES_V2: usize =
    MIN_WORKER_V3_VERIFICATION_TERMINAL_BYTES_V2
        + MAX_WORKER_V3_VERIFICATION_APPLICATION_RESPONSE_BYTES_V2;

/// Move-only service reservation for one compiler-current-record challenge.
///
/// Both fields are nonzero. Construction establishes structure only. The provider that creates a
/// reservation remains responsible for entropy, uniqueness, atomic reservation, persistence, and
/// release policy across every service process and restart in its deployment.
///
/// ```compile_fail
/// use fe2o3_worker_v3_verification_protocol::WorkerV3VerificationChallengeReservationV2;
/// fn duplicate(value: WorkerV3VerificationChallengeReservationV2) {
///     let _again = value.clone();
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct WorkerV3VerificationChallengeReservationV2 {
    challenge: [u8; SHA256_BYTES],
    reservation_identity: [u8; SHA256_BYTES],
}

impl WorkerV3VerificationChallengeReservationV2 {
    /// Constructs one authority-free reservation coordinate.
    pub fn new(
        challenge: [u8; SHA256_BYTES],
        reservation_identity: [u8; SHA256_BYTES],
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV2> {
        if challenge == [0; SHA256_BYTES] {
            return Err(WorkerV3VerificationProtocolErrorV2::ZeroChallenge);
        }
        if reservation_identity == [0; SHA256_BYTES] {
            return Err(WorkerV3VerificationProtocolErrorV2::ZeroReservationIdentity);
        }
        Ok(Self {
            challenge,
            reservation_identity,
        })
    }

    /// Borrows the exact compiler-current-record challenge bytes.
    pub const fn challenge_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.challenge
    }

    /// Borrows the opaque service reservation identity.
    pub const fn reservation_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.reservation_identity
    }

    /// Consumes the move-only coordinate into inert byte arrays.
    pub fn into_bytes(self) -> ([u8; SHA256_BYTES], [u8; SHA256_BYTES]) {
        (self.challenge, self.reservation_identity)
    }

    /// Reports that the coordinate alone grants no authority or durability guarantee.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for WorkerV3VerificationChallengeReservationV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3VerificationChallengeReservationV2")
            .field("reservation_identity", &self.reservation_identity)
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

/// Result represented by the first fixed-size V2 service response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3VerificationChallengeDispositionV2 {
    /// The service retained the Begin inputs and reserved the returned challenge coordinate.
    Reserved,
    /// The Begin transaction was rejected and carries no challenge coordinate.
    Rejected,
}

impl WorkerV3VerificationChallengeDispositionV2 {
    const fn wire_tag(self) -> u16 {
        match self {
            Self::Reserved => 1,
            Self::Rejected => 2,
        }
    }

    const fn from_wire_tag(tag: u16) -> Option<Self> {
        match tag {
            1 => Some(Self::Reserved),
            2 => Some(Self::Rejected),
            _ => None,
        }
    }
}

/// Fixed-size response that binds one service-owned challenge reservation to a Begin request.
pub struct WorkerV3VerificationChallengeFrameV2 {
    disposition: WorkerV3VerificationChallengeDispositionV2,
    request_identity: [u8; SHA256_BYTES],
    reservation: Option<WorkerV3VerificationChallengeReservationV2>,
    canonical_bytes: [u8; WORKER_V3_VERIFICATION_CHALLENGE_BYTES_V2],
}

impl WorkerV3VerificationChallengeFrameV2 {
    /// Constructs the success frame for one exact Begin request and reservation.
    pub fn reserved(
        request: &WorkerV3VerificationRequestV1,
        reservation: &WorkerV3VerificationChallengeReservationV2,
    ) -> Self {
        Self::encode(
            WorkerV3VerificationChallengeDispositionV2::Reserved,
            *request.identity().as_bytes(),
            *reservation.challenge_bytes(),
            *reservation.reservation_identity(),
        )
    }

    /// Constructs the generic rejection frame for one exact decoded Begin request.
    pub fn rejected(request: &WorkerV3VerificationRequestV1) -> Self {
        Self::encode(
            WorkerV3VerificationChallengeDispositionV2::Rejected,
            *request.identity().as_bytes(),
            [0; SHA256_BYTES],
            [0; SHA256_BYTES],
        )
    }

    /// Strictly decodes one complete canonical challenge frame.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3VerificationProtocolErrorV2> {
        if bytes.len() != WORKER_V3_VERIFICATION_CHALLENGE_BYTES_V2 {
            return Err(WorkerV3VerificationProtocolErrorV2::InvalidLength {
                frame: "challenge",
                actual: bytes.len(),
            });
        }
        let mut reader = Reader::new(bytes);
        require_header(&mut reader, CHALLENGE_MAGIC, bytes.len())?;
        let disposition_tag = reader.u16()?;
        let disposition = WorkerV3VerificationChallengeDispositionV2::from_wire_tag(
            disposition_tag,
        )
        .ok_or(WorkerV3VerificationProtocolErrorV2::UnknownDisposition {
            frame: "challenge",
            actual: disposition_tag,
        })?;
        if reader.fixed::<6>()? != [0; 6] {
            return Err(WorkerV3VerificationProtocolErrorV2::NoncanonicalReservedBytes);
        }
        let request_identity = reader.fixed()?;
        let challenge = reader.fixed()?;
        let reservation_identity = reader.fixed()?;
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(WorkerV3VerificationProtocolErrorV2::TrailingBytes);
        }
        match disposition {
            WorkerV3VerificationChallengeDispositionV2::Reserved
                if challenge == [0; SHA256_BYTES] || reservation_identity == [0; SHA256_BYTES] =>
            {
                return Err(if challenge == [0; SHA256_BYTES] {
                    WorkerV3VerificationProtocolErrorV2::ZeroChallenge
                } else {
                    WorkerV3VerificationProtocolErrorV2::ZeroReservationIdentity
                });
            }
            WorkerV3VerificationChallengeDispositionV2::Rejected
                if challenge != [0; SHA256_BYTES] || reservation_identity != [0; SHA256_BYTES] =>
            {
                return Err(WorkerV3VerificationProtocolErrorV2::RejectedChallengeCoordinates);
            }
            _ => {}
        }
        let decoded = Self::encode(
            disposition,
            request_identity,
            challenge,
            reservation_identity,
        );
        if decoded.canonical_bytes.as_slice() != bytes
            || derive_identity(
                CHALLENGE_IDENTITY_DOMAIN,
                &bytes[..CHALLENGE_PREIMAGE_BYTES],
            ) != declared_identity
        {
            return Err(WorkerV3VerificationProtocolErrorV2::IdentityMismatch(
                "challenge",
            ));
        }
        Ok(decoded)
    }

    fn encode(
        disposition: WorkerV3VerificationChallengeDispositionV2,
        request_identity: [u8; SHA256_BYTES],
        challenge: [u8; SHA256_BYTES],
        reservation_identity: [u8; SHA256_BYTES],
    ) -> Self {
        let reservation = match disposition {
            WorkerV3VerificationChallengeDispositionV2::Reserved => Some(
                WorkerV3VerificationChallengeReservationV2::new(challenge, reservation_identity)
                    .expect("reserved frame requires nonzero coordinates"),
            ),
            WorkerV3VerificationChallengeDispositionV2::Rejected => {
                assert_eq!(challenge, [0; SHA256_BYTES]);
                assert_eq!(reservation_identity, [0; SHA256_BYTES]);
                None
            }
        };
        let mut canonical_bytes = [0_u8; WORKER_V3_VERIFICATION_CHALLENGE_BYTES_V2];
        let mut offset = encode_header(
            &mut canonical_bytes,
            CHALLENGE_MAGIC,
            WORKER_V3_VERIFICATION_CHALLENGE_BYTES_V2,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &disposition.wire_tag().to_le_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &[0; 6]);
        put(&mut canonical_bytes, &mut offset, &request_identity);
        put(&mut canonical_bytes, &mut offset, &challenge);
        put(&mut canonical_bytes, &mut offset, &reservation_identity);
        debug_assert_eq!(offset, CHALLENGE_PREIMAGE_BYTES);
        let identity = derive_identity(CHALLENGE_IDENTITY_DOMAIN, &canonical_bytes[..offset]);
        put(&mut canonical_bytes, &mut offset, &identity);
        debug_assert_eq!(offset, canonical_bytes.len());
        Self {
            disposition,
            request_identity,
            reservation,
            canonical_bytes,
        }
    }

    /// Returns the complete canonical frame.
    pub const fn encode_canonical(&self) -> &[u8; WORKER_V3_VERIFICATION_CHALLENGE_BYTES_V2] {
        &self.canonical_bytes
    }

    /// Returns the first-phase disposition.
    pub const fn disposition(&self) -> WorkerV3VerificationChallengeDispositionV2 {
        self.disposition
    }

    /// Checks that the response names one exact Begin request.
    pub fn matches_request(&self, request: &WorkerV3VerificationRequestV1) -> bool {
        self.request_identity == *request.identity().as_bytes()
    }

    /// Borrows the reserved coordinate when the disposition is `Reserved`.
    pub const fn reservation(&self) -> Option<&WorkerV3VerificationChallengeReservationV2> {
        self.reservation.as_ref()
    }

    /// Consumes this frame into its move-only reservation.
    pub fn into_reservation(self) -> Option<WorkerV3VerificationChallengeReservationV2> {
        self.reservation
    }

    /// Reports that framing and reservation coordinates grant no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for WorkerV3VerificationChallengeFrameV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3VerificationChallengeFrameV2")
            .field("disposition", &self.disposition)
            .field("request_identity", &self.request_identity)
            .field("reservation", &self.reservation)
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

/// Fixed-size canonical submission of compiler-current-record verification and attestation bytes.
pub struct WorkerV3VerificationCurrentRecordFrameV2 {
    request_identity: [u8; SHA256_BYTES],
    challenge: [u8; SHA256_BYTES],
    reservation_identity: [u8; SHA256_BYTES],
    verification: CompilerExecutionCurrentRecordVerificationV3,
    attestation: CompilerExecutionCurrentRecordAttestationV3,
    canonical_bytes: [u8; WORKER_V3_VERIFICATION_CURRENT_RECORD_BYTES_V2],
}

impl WorkerV3VerificationCurrentRecordFrameV2 {
    /// Constructs one submission and validates exact nested-record and challenge association.
    pub fn new(
        request: &WorkerV3VerificationRequestV1,
        reservation: &WorkerV3VerificationChallengeReservationV2,
        verification_bytes: &[u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3],
        attestation_bytes: &[u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3],
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV2> {
        Self::from_parts(
            *request.identity().as_bytes(),
            *reservation.challenge_bytes(),
            *reservation.reservation_identity(),
            verification_bytes,
            attestation_bytes,
        )
    }

    fn from_parts(
        request_identity: [u8; SHA256_BYTES],
        challenge: [u8; SHA256_BYTES],
        reservation_identity: [u8; SHA256_BYTES],
        verification_bytes: &[u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3],
        attestation_bytes: &[u8; COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3],
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV2> {
        if challenge == [0; SHA256_BYTES] {
            return Err(WorkerV3VerificationProtocolErrorV2::ZeroChallenge);
        }
        if reservation_identity == [0; SHA256_BYTES] {
            return Err(WorkerV3VerificationProtocolErrorV2::ZeroReservationIdentity);
        }
        let verification = CompilerExecutionCurrentRecordVerificationV3::decode(verification_bytes)
            .map_err(WorkerV3VerificationProtocolErrorV2::CurrentRecord)?;
        let attestation = CompilerExecutionCurrentRecordAttestationV3::decode(attestation_bytes)
            .map_err(WorkerV3VerificationProtocolErrorV2::CurrentRecord)?;
        if attestation.challenge() != challenge {
            return Err(WorkerV3VerificationProtocolErrorV2::ChallengeMismatch);
        }
        if attestation.verification() != &verification
            || attestation.verification().canonical_bytes() != verification_bytes
        {
            return Err(WorkerV3VerificationProtocolErrorV2::VerificationAttestationMismatch);
        }
        let mut canonical_bytes = [0_u8; WORKER_V3_VERIFICATION_CURRENT_RECORD_BYTES_V2];
        let mut offset = encode_header(
            &mut canonical_bytes,
            CURRENT_RECORD_MAGIC,
            WORKER_V3_VERIFICATION_CURRENT_RECORD_BYTES_V2,
        );
        put(&mut canonical_bytes, &mut offset, &request_identity);
        put(&mut canonical_bytes, &mut offset, &challenge);
        put(&mut canonical_bytes, &mut offset, &reservation_identity);
        put(&mut canonical_bytes, &mut offset, verification_bytes);
        put(&mut canonical_bytes, &mut offset, attestation_bytes);
        debug_assert_eq!(offset, CURRENT_RECORD_PREIMAGE_BYTES);
        let identity = derive_identity(CURRENT_RECORD_IDENTITY_DOMAIN, &canonical_bytes[..offset]);
        put(&mut canonical_bytes, &mut offset, &identity);
        debug_assert_eq!(offset, canonical_bytes.len());
        Ok(Self {
            request_identity,
            challenge,
            reservation_identity,
            verification,
            attestation,
            canonical_bytes,
        })
    }

    /// Strictly decodes one exact fixed-size submission.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3VerificationProtocolErrorV2> {
        if bytes.len() != WORKER_V3_VERIFICATION_CURRENT_RECORD_BYTES_V2 {
            return Err(WorkerV3VerificationProtocolErrorV2::InvalidLength {
                frame: "current record",
                actual: bytes.len(),
            });
        }
        let mut reader = Reader::new(bytes);
        require_header(&mut reader, CURRENT_RECORD_MAGIC, bytes.len())?;
        let request_identity = reader.fixed()?;
        let challenge = reader.fixed()?;
        let reservation_identity = reader.fixed()?;
        let verification_bytes = reader
            .take(COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3)?
            .try_into()
            .expect("exact fixed-size current-record verification");
        let attestation_bytes = reader
            .take(COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3)?
            .try_into()
            .expect("exact fixed-size current-record attestation");
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(WorkerV3VerificationProtocolErrorV2::TrailingBytes);
        }
        let decoded = Self::from_parts(
            request_identity,
            challenge,
            reservation_identity,
            verification_bytes,
            attestation_bytes,
        )?;
        if decoded.canonical_bytes.as_slice() != bytes
            || derive_identity(
                CURRENT_RECORD_IDENTITY_DOMAIN,
                &bytes[..CURRENT_RECORD_PREIMAGE_BYTES],
            ) != declared_identity
        {
            return Err(WorkerV3VerificationProtocolErrorV2::IdentityMismatch(
                "current record",
            ));
        }
        Ok(decoded)
    }

    /// Returns the exact canonical submission bytes.
    pub const fn encode_canonical(&self) -> &[u8; WORKER_V3_VERIFICATION_CURRENT_RECORD_BYTES_V2] {
        &self.canonical_bytes
    }

    /// Checks exact Begin-request and reservation association.
    pub fn matches_session(
        &self,
        request: &WorkerV3VerificationRequestV1,
        reservation: &WorkerV3VerificationChallengeReservationV2,
    ) -> bool {
        self.request_identity == *request.identity().as_bytes()
            && self.challenge == *reservation.challenge_bytes()
            && self.reservation_identity == *reservation.reservation_identity()
    }

    /// Returns the canonical compiler-current-record verification.
    pub const fn verification(&self) -> &CompilerExecutionCurrentRecordVerificationV3 {
        &self.verification
    }

    /// Returns the canonical signed compiler-current-record attestation.
    pub const fn attestation(&self) -> &CompilerExecutionCurrentRecordAttestationV3 {
        &self.attestation
    }

    /// Reports that strict framing alone grants no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for WorkerV3VerificationCurrentRecordFrameV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3VerificationCurrentRecordFrameV2")
            .field("request_identity", &self.request_identity)
            .field("reservation_identity", &self.reservation_identity)
            .field("verification", &self.verification.identity())
            .field("attestation", &self.attestation.identity())
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

/// Disposition of the sole V2 terminal response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3VerificationTerminalDispositionV2 {
    /// The application returned one opaque bounded response.
    ApplicationResponse,
    /// The service or application rejected the session without a response payload.
    Rejected,
}

impl WorkerV3VerificationTerminalDispositionV2 {
    const fn wire_tag(self) -> u16 {
        match self {
            Self::ApplicationResponse => 1,
            Self::Rejected => 2,
        }
    }

    const fn from_wire_tag(tag: u16) -> Option<Self> {
        match tag {
            1 => Some(Self::ApplicationResponse),
            2 => Some(Self::Rejected),
            _ => None,
        }
    }
}

/// One canonical bounded terminal response bound to the exact V2 session.
pub struct WorkerV3VerificationTerminalFrameV2 {
    disposition: WorkerV3VerificationTerminalDispositionV2,
    request_identity: [u8; SHA256_BYTES],
    challenge: [u8; SHA256_BYTES],
    reservation_identity: [u8; SHA256_BYTES],
    application_response: Vec<u8>,
    canonical_bytes: Vec<u8>,
}

impl WorkerV3VerificationTerminalFrameV2 {
    /// Constructs one bounded opaque application response.
    pub fn application_response(
        request: &WorkerV3VerificationRequestV1,
        reservation: &WorkerV3VerificationChallengeReservationV2,
        response: Vec<u8>,
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV2> {
        Self::encode(
            WorkerV3VerificationTerminalDispositionV2::ApplicationResponse,
            *request.identity().as_bytes(),
            *reservation.challenge_bytes(),
            *reservation.reservation_identity(),
            response,
        )
    }

    /// Constructs the generic empty rejection terminal.
    pub fn rejected(
        request: &WorkerV3VerificationRequestV1,
        reservation: &WorkerV3VerificationChallengeReservationV2,
    ) -> Self {
        Self::encode(
            WorkerV3VerificationTerminalDispositionV2::Rejected,
            *request.identity().as_bytes(),
            *reservation.challenge_bytes(),
            *reservation.reservation_identity(),
            Vec::new(),
        )
        .expect("empty rejection is always within the response bound")
    }

    /// Strictly decodes one complete bounded terminal frame.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3VerificationProtocolErrorV2> {
        if !(MIN_WORKER_V3_VERIFICATION_TERMINAL_BYTES_V2
            ..=MAX_WORKER_V3_VERIFICATION_TERMINAL_BYTES_V2)
            .contains(&bytes.len())
        {
            return Err(WorkerV3VerificationProtocolErrorV2::InvalidLength {
                frame: "terminal",
                actual: bytes.len(),
            });
        }
        let mut reader = Reader::new(bytes);
        require_header(&mut reader, TERMINAL_MAGIC, bytes.len())?;
        let disposition_tag = reader.u16()?;
        let disposition = WorkerV3VerificationTerminalDispositionV2::from_wire_tag(disposition_tag)
            .ok_or(WorkerV3VerificationProtocolErrorV2::UnknownDisposition {
                frame: "terminal",
                actual: disposition_tag,
            })?;
        if reader.u16()? != 0 {
            return Err(WorkerV3VerificationProtocolErrorV2::NoncanonicalReservedBytes);
        }
        let response_len = reader.u32()? as usize;
        let request_identity = reader.fixed()?;
        let challenge = reader.fixed()?;
        let reservation_identity = reader.fixed()?;
        let response_digest = reader.fixed::<SHA256_BYTES>()?;
        let application_response = reader.take(response_len)?.to_vec();
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(WorkerV3VerificationProtocolErrorV2::TrailingBytes);
        }
        let decoded = Self::encode(
            disposition,
            request_identity,
            challenge,
            reservation_identity,
            application_response,
        )?;
        let expected_digest = match disposition {
            WorkerV3VerificationTerminalDispositionV2::ApplicationResponse => {
                Sha256::digest(decoded.application_response.as_slice()).into()
            }
            WorkerV3VerificationTerminalDispositionV2::Rejected => [0; SHA256_BYTES],
        };
        if response_digest != expected_digest
            || decoded.canonical_bytes.as_slice() != bytes
            || derive_identity(
                TERMINAL_IDENTITY_DOMAIN,
                &bytes[..bytes.len() - SHA256_BYTES],
            ) != declared_identity
        {
            return Err(WorkerV3VerificationProtocolErrorV2::IdentityMismatch(
                "terminal",
            ));
        }
        Ok(decoded)
    }

    fn encode(
        disposition: WorkerV3VerificationTerminalDispositionV2,
        request_identity: [u8; SHA256_BYTES],
        challenge: [u8; SHA256_BYTES],
        reservation_identity: [u8; SHA256_BYTES],
        application_response: Vec<u8>,
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV2> {
        if challenge == [0; SHA256_BYTES] {
            return Err(WorkerV3VerificationProtocolErrorV2::ZeroChallenge);
        }
        if reservation_identity == [0; SHA256_BYTES] {
            return Err(WorkerV3VerificationProtocolErrorV2::ZeroReservationIdentity);
        }
        if application_response.len() > MAX_WORKER_V3_VERIFICATION_APPLICATION_RESPONSE_BYTES_V2 {
            return Err(
                WorkerV3VerificationProtocolErrorV2::ApplicationResponseTooLarge {
                    actual: application_response.len(),
                    maximum: MAX_WORKER_V3_VERIFICATION_APPLICATION_RESPONSE_BYTES_V2,
                },
            );
        }
        if disposition == WorkerV3VerificationTerminalDispositionV2::Rejected
            && !application_response.is_empty()
        {
            return Err(WorkerV3VerificationProtocolErrorV2::RejectionHasPayload);
        }
        let total_len = MIN_WORKER_V3_VERIFICATION_TERMINAL_BYTES_V2
            .checked_add(application_response.len())
            .ok_or(WorkerV3VerificationProtocolErrorV2::LengthOverflow)?;
        let mut canonical_bytes = Vec::new();
        canonical_bytes
            .try_reserve_exact(total_len)
            .map_err(|_| WorkerV3VerificationProtocolErrorV2::AllocationFailed(total_len))?;
        canonical_bytes.extend_from_slice(&TERMINAL_MAGIC);
        canonical_bytes.extend_from_slice(&VERSION.to_le_bytes());
        canonical_bytes.extend_from_slice(&0_u16.to_le_bytes());
        canonical_bytes.extend_from_slice(&(total_len as u64).to_le_bytes());
        canonical_bytes.extend_from_slice(&0_u32.to_le_bytes());
        canonical_bytes.extend_from_slice(&disposition.wire_tag().to_le_bytes());
        canonical_bytes.extend_from_slice(&0_u16.to_le_bytes());
        canonical_bytes.extend_from_slice(&(application_response.len() as u32).to_le_bytes());
        canonical_bytes.extend_from_slice(&request_identity);
        canonical_bytes.extend_from_slice(&challenge);
        canonical_bytes.extend_from_slice(&reservation_identity);
        let response_digest: [u8; SHA256_BYTES] = match disposition {
            WorkerV3VerificationTerminalDispositionV2::ApplicationResponse => {
                Sha256::digest(application_response.as_slice()).into()
            }
            WorkerV3VerificationTerminalDispositionV2::Rejected => [0; SHA256_BYTES],
        };
        canonical_bytes.extend_from_slice(&response_digest);
        canonical_bytes.extend_from_slice(&application_response);
        debug_assert_eq!(canonical_bytes.len(), total_len - SHA256_BYTES);
        let identity = derive_identity(TERMINAL_IDENTITY_DOMAIN, &canonical_bytes);
        canonical_bytes.extend_from_slice(&identity);
        Ok(Self {
            disposition,
            request_identity,
            challenge,
            reservation_identity,
            application_response,
            canonical_bytes,
        })
    }

    /// Returns the complete canonical terminal bytes.
    pub fn encode_canonical(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the terminal disposition.
    pub const fn disposition(&self) -> WorkerV3VerificationTerminalDispositionV2 {
        self.disposition
    }

    /// Returns the opaque application response, empty for rejection.
    pub fn application_response_bytes(&self) -> &[u8] {
        &self.application_response
    }

    /// Checks exact Begin-request and reservation association.
    pub fn matches_session(
        &self,
        request: &WorkerV3VerificationRequestV1,
        reservation: &WorkerV3VerificationChallengeReservationV2,
    ) -> bool {
        self.request_identity == *request.identity().as_bytes()
            && self.challenge == *reservation.challenge_bytes()
            && self.reservation_identity == *reservation.reservation_identity()
    }

    /// Reports that opaque application bytes grant no protocol authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for WorkerV3VerificationTerminalFrameV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerV3VerificationTerminalFrameV2")
            .field("disposition", &self.disposition)
            .field("request_identity", &self.request_identity)
            .field("reservation_identity", &self.reservation_identity)
            .field("application_response_len", &self.application_response.len())
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

/// Construction or strict canonical-decoding failure for V2 phase frames.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3VerificationProtocolErrorV2 {
    /// The service current-record challenge used the forbidden zero sentinel.
    ZeroChallenge,
    /// The opaque service reservation identity used the forbidden zero sentinel.
    ZeroReservationIdentity,
    /// A phase frame had a length outside its exact bound.
    InvalidLength {
        /// Phase frame name.
        frame: &'static str,
        /// Observed complete byte length.
        actual: usize,
    },
    /// A phase frame header was malformed or named another schema.
    InvalidHeader(&'static str),
    /// A phase disposition tag is not defined.
    UnknownDisposition {
        /// Phase frame name.
        frame: &'static str,
        /// Observed tag.
        actual: u16,
    },
    /// Reserved bytes were not canonical zeroes.
    NoncanonicalReservedBytes,
    /// A fixed-width field was truncated.
    Truncated,
    /// Bytes remained after the complete declared frame.
    TrailingBytes,
    /// A phase identity or canonical reconstruction mismatched.
    IdentityMismatch(&'static str),
    /// Compiler-current-record canonical decoding failed.
    CurrentRecord(CompilerExecutionCurrentRecordVerificationErrorV3),
    /// The attestation did not bind the reserved challenge.
    ChallengeMismatch,
    /// The separate verification differed from the attestation's nested verification.
    VerificationAttestationMismatch,
    /// An application response exceeded the fixed protocol bound.
    ApplicationResponseTooLarge {
        /// Observed byte length.
        actual: usize,
        /// Protocol maximum.
        maximum: usize,
    },
    /// A rejection illegally carried application-owned response bytes.
    RejectionHasPayload,
    /// A rejected challenge frame illegally carried reservation coordinates.
    RejectedChallengeCoordinates,
    /// Length arithmetic overflowed.
    LengthOverflow,
    /// A bounded allocation failed.
    AllocationFailed(usize),
}

impl fmt::Display for WorkerV3VerificationProtocolErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroChallenge => formatter.write_str("service current-record challenge is zero"),
            Self::ZeroReservationIdentity => {
                formatter.write_str("service challenge reservation identity is zero")
            }
            Self::InvalidLength { frame, actual } => {
                write!(formatter, "invalid {frame} frame length {actual}")
            }
            Self::InvalidHeader(frame) => write!(formatter, "invalid {frame} frame header"),
            Self::UnknownDisposition { frame, actual } => {
                write!(formatter, "unknown {frame} disposition {actual}")
            }
            Self::NoncanonicalReservedBytes => formatter.write_str("noncanonical reserved bytes"),
            Self::Truncated => formatter.write_str("phase frame is truncated"),
            Self::TrailingBytes => formatter.write_str("phase frame has trailing bytes"),
            Self::IdentityMismatch(frame) => write!(formatter, "{frame} frame identity mismatch"),
            Self::CurrentRecord(source) => {
                write!(formatter, "current-record frame failed: {source}")
            }
            Self::ChallengeMismatch => {
                formatter.write_str("current-record attestation challenge mismatch")
            }
            Self::VerificationAttestationMismatch => {
                formatter.write_str("current-record verification and nested attestation mismatch")
            }
            Self::ApplicationResponseTooLarge { actual, maximum } => write!(
                formatter,
                "application response length {actual} exceeds {maximum} bytes"
            ),
            Self::RejectionHasPayload => {
                formatter.write_str("terminal rejection carries forbidden payload bytes")
            }
            Self::RejectedChallengeCoordinates => formatter
                .write_str("rejected challenge frame carries forbidden reservation coordinates"),
            Self::LengthOverflow => formatter.write_str("phase frame length overflowed"),
            Self::AllocationFailed(bytes) => {
                write!(
                    formatter,
                    "could not allocate bounded {bytes}-byte phase frame"
                )
            }
        }
    }
}

impl Error for WorkerV3VerificationProtocolErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentRecord(source) => Some(source),
            _ => None,
        }
    }
}

fn encode_header<const N: usize>(bytes: &mut [u8; N], magic: [u8; 8], total: usize) -> usize {
    let mut offset = 0;
    put(bytes, &mut offset, &magic);
    put(bytes, &mut offset, &VERSION.to_le_bytes());
    put(bytes, &mut offset, &0_u16.to_le_bytes());
    put(bytes, &mut offset, &(total as u64).to_le_bytes());
    put(bytes, &mut offset, &0_u32.to_le_bytes());
    offset
}

fn require_header(
    reader: &mut Reader<'_>,
    magic: [u8; 8],
    actual_len: usize,
) -> Result<(), WorkerV3VerificationProtocolErrorV2> {
    let frame = if magic == CHALLENGE_MAGIC {
        "challenge"
    } else if magic == CURRENT_RECORD_MAGIC {
        "current record"
    } else {
        "terminal"
    };
    if reader.fixed::<8>()? != magic
        || reader.u16()? != VERSION
        || reader.u16()? != 0
        || reader.u64()? != actual_len as u64
        || reader.u32()? != 0
    {
        return Err(WorkerV3VerificationProtocolErrorV2::InvalidHeader(frame));
    }
    Ok(())
}

fn put<const N: usize>(target: &mut [u8; N], offset: &mut usize, source: &[u8]) {
    let end = *offset + source.len();
    target[*offset..end].copy_from_slice(source);
    *offset = end;
}

fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], WorkerV3VerificationProtocolErrorV2> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(WorkerV3VerificationProtocolErrorV2::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(WorkerV3VerificationProtocolErrorV2::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], WorkerV3VerificationProtocolErrorV2> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerV3VerificationProtocolErrorV2::Truncated)
    }

    fn u16(&mut self) -> Result<u16, WorkerV3VerificationProtocolErrorV2> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, WorkerV3VerificationProtocolErrorV2> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerV3VerificationProtocolErrorV2> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        WorkerV3VerificationEntryCoordinateV1, WorkerV3VerificationFdPayloadDescriptorV1,
        WorkerV3VerificationFreshChallengeV1, WorkerV3VerificationMeasurementIdentityV1,
        WorkerV3VerificationPolicyIdentityV1, WorkerV3VerificationRosterIdentityV1,
    };

    fn verification_request(seed: u8) -> WorkerV3VerificationRequestV1 {
        WorkerV3VerificationRequestV1::new(
            WorkerV3VerificationFreshChallengeV1::new([seed; 32]).unwrap(),
            WorkerV3VerificationRosterIdentityV1::new([2; 32]).unwrap(),
            WorkerV3VerificationPolicyIdentityV1::new([3; 32]).unwrap(),
            WorkerV3VerificationMeasurementIdentityV1::new([4; 32]).unwrap(),
            WorkerV3VerificationFdPayloadDescriptorV1::load_envelope_v2(1, [5; 32]).unwrap(),
            WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(1, [6; 32]).unwrap(),
            vec![
                WorkerV3VerificationEntryCoordinateV1::new(
                    0,
                    "kernel",
                    "kernel_export",
                    [7; 32],
                    [8; 32],
                    [9; 32],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn reservation_requires_two_nonzero_coordinates_and_is_move_only() {
        assert!(matches!(
            WorkerV3VerificationChallengeReservationV2::new([0; 32], [1; 32]),
            Err(WorkerV3VerificationProtocolErrorV2::ZeroChallenge)
        ));
        assert!(matches!(
            WorkerV3VerificationChallengeReservationV2::new([1; 32], [0; 32]),
            Err(WorkerV3VerificationProtocolErrorV2::ZeroReservationIdentity)
        ));
        let reservation =
            WorkerV3VerificationChallengeReservationV2::new([1; 32], [2; 32]).unwrap();
        assert_eq!(reservation.into_bytes(), ([1; 32], [2; 32]));
    }

    #[test]
    fn challenge_frames_are_exact_canonical_and_request_bound() {
        let request = verification_request(1);
        let other = verification_request(2);
        let reservation =
            WorkerV3VerificationChallengeReservationV2::new([10; 32], [11; 32]).unwrap();
        let frame = WorkerV3VerificationChallengeFrameV2::reserved(&request, &reservation);
        assert_eq!(
            frame.encode_canonical().len(),
            WORKER_V3_VERIFICATION_CHALLENGE_BYTES_V2
        );
        let decoded =
            WorkerV3VerificationChallengeFrameV2::decode_canonical(frame.encode_canonical())
                .unwrap();
        assert!(decoded.matches_request(&request));
        assert!(!decoded.matches_request(&other));
        assert_eq!(decoded.reservation().unwrap().challenge_bytes(), &[10; 32]);
        assert!(!decoded.grants_authority());

        let rejected = WorkerV3VerificationChallengeFrameV2::rejected(&request);
        let mut illegal = *rejected.encode_canonical();
        illegal[HEADER_BYTES + 8 + SHA256_BYTES] = 1;
        assert!(matches!(
            WorkerV3VerificationChallengeFrameV2::decode_canonical(&illegal),
            Err(WorkerV3VerificationProtocolErrorV2::RejectedChallengeCoordinates)
        ));
        let mut mutated = *frame.encode_canonical();
        *mutated.last_mut().unwrap() ^= 1;
        assert!(matches!(
            WorkerV3VerificationChallengeFrameV2::decode_canonical(&mutated),
            Err(WorkerV3VerificationProtocolErrorV2::IdentityMismatch(
                "challenge"
            ))
        ));
    }

    #[test]
    fn terminal_frames_enforce_bound_disposition_digest_and_session() {
        let request = verification_request(3);
        let other = verification_request(4);
        let reservation =
            WorkerV3VerificationChallengeReservationV2::new([12; 32], [13; 32]).unwrap();
        let frame = WorkerV3VerificationTerminalFrameV2::application_response(
            &request,
            &reservation,
            b"opaque".to_vec(),
        )
        .unwrap();
        let decoded =
            WorkerV3VerificationTerminalFrameV2::decode_canonical(frame.encode_canonical())
                .unwrap();
        assert_eq!(decoded.application_response_bytes(), b"opaque");
        assert!(decoded.matches_session(&request, &reservation));
        assert!(!decoded.matches_session(&other, &reservation));
        assert!(!decoded.grants_authority());

        let rejected = WorkerV3VerificationTerminalFrameV2::rejected(&request, &reservation);
        assert_eq!(
            rejected.encode_canonical().len(),
            MIN_WORKER_V3_VERIFICATION_TERMINAL_BYTES_V2
        );
        assert_eq!(
            WorkerV3VerificationTerminalFrameV2::decode_canonical(rejected.encode_canonical())
                .unwrap()
                .disposition(),
            WorkerV3VerificationTerminalDispositionV2::Rejected
        );
        assert!(matches!(
            WorkerV3VerificationTerminalFrameV2::application_response(
                &request,
                &reservation,
                vec![0; MAX_WORKER_V3_VERIFICATION_APPLICATION_RESPONSE_BYTES_V2 + 1]
            ),
            Err(WorkerV3VerificationProtocolErrorV2::ApplicationResponseTooLarge { .. })
        ));

        let mut mutated = frame.encode_canonical().to_vec();
        mutated[TERMINAL_FIXED_PREIMAGE_BYTES] ^= 1;
        assert!(WorkerV3VerificationTerminalFrameV2::decode_canonical(&mutated).is_err());
        mutated.push(0);
        assert!(WorkerV3VerificationTerminalFrameV2::decode_canonical(&mutated).is_err());
    }
}
