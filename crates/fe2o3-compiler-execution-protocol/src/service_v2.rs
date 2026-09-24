//! Native service packets, not another service engine or an authority boundary.
//!
//! V2 has explicit framing and identity domains, with no V1 retry. Operations
//! retain the V1 state-machine meanings. Each packet owns only its canonical
//! bytes: construction borrows prepaid leaves; extraction explicitly re-decodes
//! them on the caller's ledger. There are no hidden clones or signing APIs.
//!
//! Current-record V3 is an identity-only wire family and is carried unchanged.
//! Native current-record joins use the explicitly metered native-carriage
//! constructor/authenticator. Packet decoding alone does not establish protected
//! journal custody or independently administered anchor deployment.

use crate::{
    CompilerExecutionAttestationChallengeV2 as Challenge,
    CompilerExecutionAttestationErrorV2 as AttestationError,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionCurrentRecordAttestationV3 as Current,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionReceiptCarriageV2 as Carriage,
    CompilerExecutionReceiptPublicationAckV2 as Ack,
    CompilerExecutionReceiptPublicationErrorV2 as PublicationError,
    CompilerExecutionReceiptPublicationV2 as Publication,
    CompilerExecutionServiceProtocolErrorV1 as Framing,
    CompilerExecutionServicePublishDispositionV1 as Disposition,
    attestation_challenge_v2 as challenge, attestation_receipt_v2 as receipt,
    attestation_request_v2 as request,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    receipt_carriage_v2 as carriage, receipt_publication_v2 as publication,
    service::{Reader, decode_versioned_header, derive_identity, encode_versioned_header, put},
};
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV2 as SubjectError,
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2 as SUBJECT_BYTES,
    INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V2 as SUBJECT_STORAGE,
    INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2 as SUBJECT_WORK,
    InertCompilerExecutionSubjectV2 as Subject,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error as StdError, fmt, mem::size_of};

const VERSION: u16 = 2;
const REQUEST_MAGIC: [u8; 8] = *b"F2O3CSQ2";
const RESPONSE_MAGIC: [u8; 8] = *b"F2O3CSP2";
const REQUEST_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-REQUEST/V2\0";
const RESPONSE_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-RESPONSE/V2\0";
const REQUEST_PREFIX: usize = 24 + 32 + 8 + 32;
const RESPONSE_PREFIX: usize = REQUEST_PREFIX + 32;
const Q: usize = request::COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V2;
const U: usize = publication::COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V2;
const C: usize = carriage::COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V2;
const H: usize = challenge::COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V2;
const A: usize = publication::COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V2;
const V: usize = crate::COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3;

pub const COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V2: usize = REQUEST_PREFIX + 32;
pub const COMPILER_EXECUTION_SERVICE_PREPARE_REQUEST_BYTES_V2: usize = REQUEST_PREFIX + 32;
pub const COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V2: usize = REQUEST_PREFIX + Q + 32;
pub const COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V2: usize = REQUEST_PREFIX + Q + U + 32;
pub const COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V2: usize =
    REQUEST_PREFIX + SUBJECT_BYTES + 32;
pub const COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V2: usize =
    REQUEST_PREFIX + C + 64;
pub const MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V2: usize = maximum(&[
    COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V2,
]);
pub const COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V2: usize = RESPONSE_PREFIX + 32;
pub const COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V2: usize = RESPONSE_PREFIX + H + 32;
pub const COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V2: usize = RESPONSE_PREFIX + U + 32;
pub const COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V2: usize =
    RESPONSE_PREFIX + 8 + A + 32;
pub const COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V2: usize = RESPONSE_PREFIX + C + 32;
pub const COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V2: usize =
    RESPONSE_PREFIX + V + 32;
pub const MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V2: usize = maximum(&[
    COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V2,
]);
const NQ: usize = MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V2;
const NP: usize = MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V2;
const QR: usize = size_of::<CompilerExecutionServiceRequestV2>() + size_of::<Storage>();
const PR: usize = size_of::<CompilerExecutionServiceResponseV2>() + size_of::<Storage>();
/// Fixed logical copy/hash/control work. Nested decoders charge separately.
pub const COMPILER_EXECUTION_SERVICE_REQUEST_WORK_V2: usize = resources::ENTRY_WORK + 32 * NQ;
pub const COMPILER_EXECUTION_SERVICE_RESPONSE_WORK_V2: usize = resources::ENTRY_WORK + 32 * NP;
/// Fixed outer logical frames, NOT generated stack, heap capacity or RSS bounds.
/// Four owner/byte allowances cover staging, returned result and unwind frames;
/// the fixed control allowance covers bounded views, scalar joins and errors.
pub const COMPILER_EXECUTION_SERVICE_REQUEST_STORAGE_V2: usize =
    4 * QR + 4 * NQ + 2 * size_of::<sha2::Sha256>() + 8192;
pub const COMPILER_EXECUTION_SERVICE_RESPONSE_STORAGE_V2: usize =
    4 * PR + 4 * NP + 2 * size_of::<sha2::Sha256>() + 8192;
const QW: usize = COMPILER_EXECUTION_SERVICE_REQUEST_WORK_V2;
const PW: usize = COMPILER_EXECUTION_SERVICE_RESPONSE_WORK_V2;
const QS: usize = COMPILER_EXECUTION_SERVICE_REQUEST_STORAGE_V2;
const PS: usize = COMPILER_EXECUTION_SERVICE_RESPONSE_STORAGE_V2;
/// Borrowed Publish construction additionally checks the policy-pinned receipt.
pub const COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_WORK_V2: usize =
    QW + receipt::COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V2;
pub const COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_STORAGE_V2: usize =
    QS + receipt::COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2;

// V3 decode performs four anchor signature verifications (including its two
// re-verifications), then one attestation signature verification. Key admission,
// bounded anchor codecs and all framing/hash/copy work are prepaid here. This
// is the existing fixed crypto cost model, not an instruction/time guarantee.
const CURRENT_WORK: usize = resources::ENTRY_WORK
    + 128 * V
    + 5 * (resources::STRICT_VERIFY_WORK + resources::KEY_VALIDATION_WORK);
const CURRENT_RETAINED: usize = size_of::<Current>() + size_of::<Storage>();
const CURRENT_STORAGE: usize =
    16 * CURRENT_RETAINED + 16 * V + 4 * size_of::<sha2::Sha256>() + 8192;
/// Upper quotas over all valid packet operations, including real nested calls.
/// Constructors need only their outer quota, except Publish also verifies its
/// borrowed receipt under the supplied policy and needs receipt verification.
pub const MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_WORK_V2: usize = QW
    + maximum(&[
        SUBJECT_WORK,
        request::COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2
            + publication::COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V2,
        carriage::COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V2,
    ]);
pub const MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_STORAGE_V2: usize = QS
    + maximum(&[
        SUBJECT_STORAGE,
        request::COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V2,
        request::RETAINED + publication::COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V2,
        carriage::COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V2,
    ]);
pub const MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_WORK_V2: usize = PW
    + maximum(&[
        challenge::COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V2,
        publication::COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V2,
        publication::COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V2,
        carriage::COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V2,
        CURRENT_WORK,
    ]);
pub const MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_STORAGE_V2: usize = PS
    + maximum(&[
        challenge::COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V2,
        publication::COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V2,
        publication::COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V2,
        carriage::COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V2,
        CURRENT_STORAGE,
    ]);
const fn maximum(values: &[usize]) -> usize {
    let mut max = 0;
    let mut i = 0;
    while i < values.len() {
        if values[i] > max {
            max = values[i];
        }
        i += 1;
    }
    max
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionServiceRequestIdentityV2([u8; 32]);
impl CompilerExecutionServiceRequestIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionServiceResponseIdentityV2([u8; 32]);
impl CompilerExecutionServiceResponseIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum CompilerExecutionServiceRequestKindV2 {
    Inspect = 1,
    Prepare = 2,
    Issue = 3,
    Publish = 4,
    Cancel = 5,
    Recover = 6,
    VerifyCurrent = 7,
}
use CompilerExecutionServiceRequestKindV2 as QKind;
impl QKind {
    fn decode(tag: u16) -> Result<Self> {
        match tag {
            1 => Ok(Self::Inspect),
            2 => Ok(Self::Prepare),
            3 => Ok(Self::Issue),
            4 => Ok(Self::Publish),
            5 => Ok(Self::Cancel),
            6 => Ok(Self::Recover),
            7 => Ok(Self::VerifyCurrent),
            _ => Err(Error::Kind),
        }
    }
    pub const fn packet_bytes(self) -> usize {
        REQUEST_PREFIX
            + 32
            + match self {
                Self::Inspect | Self::Prepare | Self::Cancel => 0,
                Self::Issue => Q,
                Self::Publish => Q + U,
                Self::Recover => SUBJECT_BYTES,
                Self::VerifyCurrent => C + 32,
            }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum CompilerExecutionServiceResponseKindV2 {
    Ready = 1,
    Prepared = 2,
    Issued = 3,
    Published = 4,
    Cancelled = 5,
    Recovered = 6,
    ReceiptAbsent = 7,
    VerifiedCurrent = 8,
}
use CompilerExecutionServiceResponseKindV2 as PKind;
impl PKind {
    fn decode(tag: u16) -> Result<Self> {
        match tag {
            1 => Ok(Self::Ready),
            2 => Ok(Self::Prepared),
            3 => Ok(Self::Issued),
            4 => Ok(Self::Published),
            5 => Ok(Self::Cancelled),
            6 => Ok(Self::Recovered),
            7 => Ok(Self::ReceiptAbsent),
            8 => Ok(Self::VerifiedCurrent),
            _ => Err(Error::Kind),
        }
    }
    pub const fn packet_bytes(self) -> usize {
        RESPONSE_PREFIX
            + 32
            + match self {
                Self::Ready | Self::Cancelled | Self::ReceiptAbsent => 0,
                Self::Prepared => H,
                Self::Issued => U,
                Self::Published => 8 + A,
                Self::Recovered => C,
                Self::VerifiedCurrent => V,
            }
    }
}

/// Borrowed native inputs. Only the packet copy is returned; input reservations
/// remain owned by the caller, and no reference is retained in the packet.
#[derive(Debug)]
pub enum CompilerExecutionServiceRequestPayloadV2<'a> {
    Inspect,
    Prepare {
        sequence: u64,
        prior_rollback_anchor: [u8; 32],
    },
    Issue(&'a Request),
    Publish {
        request: &'a Request,
        publication: &'a Publication,
    },
    Cancel,
    Recover(&'a Subject),
    VerifyCurrent {
        carriage: &'a Carriage,
        verification_challenge: [u8; 32],
    },
}
use CompilerExecutionServiceRequestPayloadV2 as QPayload;
impl QPayload<'_> {
    /// Declared borrowed-owner floor, excluding the separately prepaid policy.
    /// Aliased inputs are conservatively counted independently.
    pub fn input_storage(&self) -> usize {
        match self {
            Self::Inspect | Self::Prepare { .. } | Self::Cancel => 0,
            Self::Issue(q) => q.retained_storage(),
            Self::Publish {
                request,
                publication,
            } => request.retained_storage() + publication.retained_storage(),
            Self::Recover(_) => challenge::SUBJECT_RETAINED,
            Self::VerifyCurrent { carriage, .. } => carriage.retained_storage(),
        }
    }
    fn fields(&self, policy: PolicyIdentity) -> Result<(QKind, Position)> {
        use QKind as K;
        nonzero(policy.as_bytes())?;
        let (kind, position) = match self {
            Self::Inspect => (K::Inspect, Position::ZERO),
            Self::Cancel => (K::Cancel, Position::ZERO),
            Self::Recover(_) => (K::Recover, Position::ZERO),
            Self::Prepare {
                sequence,
                prior_rollback_anchor,
            } => (
                K::Prepare,
                Position::prior(*sequence, *prior_rollback_anchor)?,
            ),
            Self::Issue(q) => (K::Issue, request_position(q, policy)?),
            Self::Publish {
                request: q,
                publication: u,
            } => {
                let position = request_position(q, policy)?;
                let r = u.receipt();
                if u.policy_identity() != policy
                    || r.sequence() != position.sequence
                    || r.prior_rollback_anchor() != position.anchor
                    || r.request_sha256() != q.identity().as_bytes()
                    || r.challenge_identity() != q.challenge().identity()
                    || r.subject() != q.challenge().subject()
                    || r.challenge_nonce() != q.challenge().nonce()
                {
                    return Err(Error::Payload);
                }
                (K::Publish, position)
            }
            Self::VerifyCurrent {
                carriage,
                verification_challenge,
            } => {
                nonzero(verification_challenge)?;
                (K::VerifyCurrent, carriage_position(carriage, policy)?)
            }
        };
        Ok((kind, position))
    }
    fn chunks(&self) -> [&[u8]; 2] {
        match self {
            Self::Inspect | Self::Prepare { .. } | Self::Cancel => [&[], &[]],
            Self::Issue(q) => [q.canonical_bytes(), &[]],
            Self::Publish {
                request,
                publication,
            } => [request.canonical_bytes(), publication.canonical_bytes()],
            Self::Recover(s) => [s.canonical_bytes(), &[]],
            Self::VerifyCurrent {
                carriage,
                verification_challenge,
            } => [carriage.canonical_bytes(), verification_challenge],
        }
    }
}

/// Borrowed response data. `VerifiedCurrent` carries the existing identity-only
/// V3 record but does not authenticate it against a native carriage or policy.
#[derive(Debug)]
pub enum CompilerExecutionServiceResponsePayloadV2<'a> {
    Ready {
        sequence: u64,
        prior_rollback_anchor: [u8; 32],
    },
    Prepared(&'a Challenge),
    Issued(&'a Publication),
    Published {
        acknowledgment: &'a Ack,
        disposition: Disposition,
    },
    Cancelled {
        sequence: u64,
        prior_rollback_anchor: [u8; 32],
    },
    Recovered(&'a Carriage),
    ReceiptAbsent {
        sequence: u64,
        prior_rollback_anchor: [u8; 32],
    },
    VerifiedCurrent(&'a Current),
}
use CompilerExecutionServiceResponsePayloadV2 as PPayload;
impl PPayload<'_> {
    /// Declared borrowed-owner floor, excluding the separately prepaid policy.
    /// Aliased inputs are conservatively counted independently.
    pub fn input_storage(&self) -> usize {
        match self {
            Self::Ready { .. } | Self::Cancelled { .. } | Self::ReceiptAbsent { .. } => 0,
            Self::Prepared(h) => h.retained_storage(),
            Self::Issued(u) => u.retained_storage(),
            Self::Published { acknowledgment, .. } => acknowledgment.retained_storage(),
            Self::Recovered(c) => c.retained_storage(),
            Self::VerifiedCurrent(_) => CURRENT_RETAINED,
        }
    }
    fn fields(&self, policy: PolicyIdentity) -> Result<(PKind, Position)> {
        use PKind as K;
        nonzero(policy.as_bytes())?;
        let (kind, position, actual_policy) = match self {
            Self::Ready {
                sequence,
                prior_rollback_anchor,
            } => (
                K::Ready,
                Position::prior(*sequence, *prior_rollback_anchor)?,
                policy,
            ),
            Self::Cancelled {
                sequence,
                prior_rollback_anchor,
            } => (
                K::Cancelled,
                Position::prior(*sequence, *prior_rollback_anchor)?,
                policy,
            ),
            Self::ReceiptAbsent {
                sequence,
                prior_rollback_anchor,
            } => (
                K::ReceiptAbsent,
                Position::prior(*sequence, *prior_rollback_anchor)?,
                policy,
            ),
            Self::Prepared(h) => (
                K::Prepared,
                Position::prior(h.sequence(), h.prior_rollback_anchor())?,
                h.policy_identity(),
            ),
            Self::Issued(u) => (
                K::Issued,
                Position::prior(u.receipt().sequence(), u.receipt().prior_rollback_anchor())?,
                u.policy_identity(),
            ),
            Self::Published {
                acknowledgment: a, ..
            } => (
                K::Published,
                Position::current(a.sequence(), a.current_rollback_anchor())?,
                a.policy_identity(),
            ),
            Self::Recovered(c) => (
                K::Recovered,
                carriage_position(c, policy)?,
                c.policy().identity(),
            ),
            Self::VerifiedCurrent(a) => {
                let v = a.verification();
                (
                    K::VerifiedCurrent,
                    Position::current(v.sequence(), v.current_rollback_anchor())?,
                    PolicyIdentity::from_bytes_for_protocol(v.policy_identity()),
                )
            }
        };
        if actual_policy != policy {
            return Err(Error::Policy);
        }
        Ok((kind, position))
    }
    fn chunks<'a>(&'a self, disposition: &'a [u8; 8]) -> [&'a [u8]; 2] {
        match self {
            Self::Ready { .. } | Self::Cancelled { .. } | Self::ReceiptAbsent { .. } => [&[], &[]],
            Self::Prepared(h) => [h.canonical_bytes(), &[]],
            Self::Issued(u) => [u.canonical_bytes(), &[]],
            Self::Published { acknowledgment, .. } => {
                [disposition, acknowledgment.canonical_bytes()]
            }
            Self::Recovered(c) => [c.canonical_bytes(), &[]],
            Self::VerifiedCurrent(v) => [v.canonical_bytes(), &[]],
        }
    }
}

/// One move-only native request, with no retained duplicate leaf graph.
/// Reserve the returned FULL charge before retaining it. All borrowed owners
/// stay prepaid. Failure restores storage but never work or denial history.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceRequestV2;
/// fn duplicate(p: CompilerExecutionServiceRequestV2) { let _ = p.clone(); }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct CompilerExecutionServiceRequestV2 {
    wire: Wire<NQ>,
    kind: QKind,
}
impl CompilerExecutionServiceRequestV2 {
    pub fn new(
        policy: &Policy,
        payload: QPayload<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        resources::nested_fixed(
            budget,
            policy.retained_storage() + payload.input_storage(),
            QW,
            QS,
            |budget| {
                let (kind, position) = payload.fields(policy.identity())?;
                if let QPayload::Publish {
                    request,
                    publication,
                } = &payload
                {
                    publication.receipt().verify_matches(
                        policy,
                        request,
                        position.anchor,
                        budget,
                    )?;
                }
                let fields = Fields {
                    kind: kind as u16,
                    policy: policy.identity(),
                    position,
                    request: [0; 32],
                };
                Ok((
                    Self {
                        wire: Wire::encode(REQUEST_FAMILY, fields, payload.chunks()),
                        kind,
                    },
                    Storage(QR),
                ))
            },
        )
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        resources::nested_fixed(
            budget,
            input_floor(bytes, REQUEST_PREFIX + 32, NQ),
            QW,
            QS,
            |budget| {
                let fields = Wire::<NQ>::parse(bytes, REQUEST_FAMILY)?;
                let kind = QKind::decode(fields.kind)?;
                if bytes.len() != kind.packet_bytes() {
                    return Err(Error::Length);
                }
                validate_request(
                    fields,
                    kind,
                    &bytes[REQUEST_PREFIX..bytes.len() - 32],
                    budget,
                )?;
                Ok((
                    Self {
                        wire: Wire::copy(bytes, fields),
                        kind,
                    },
                    Storage(QR),
                ))
            },
        )
    }
    pub const fn kind(&self) -> QKind {
        self.kind
    }
    pub const fn policy_identity(&self) -> PolicyIdentity {
        self.wire.fields.policy
    }
    pub const fn expected_sequence(&self) -> u64 {
        self.wire.fields.position.sequence
    }
    pub const fn expected_rollback_anchor(&self) -> [u8; 32] {
        self.wire.fields.position.anchor
    }
    pub const fn identity(&self) -> CompilerExecutionServiceRequestIdentityV2 {
        CompilerExecutionServiceRequestIdentityV2(self.wire.identity)
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        self.wire.bytes()
    }
    pub const fn retained_storage(&self) -> usize {
        QR
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Explicit same-ledger extraction, not an unmetered clone of a hidden owner.
    pub fn decode_request(&self, budget: &mut Budget<'_>) -> Result<(Request, Storage)> {
        resources::nested_fixed(budget, QR, QW, QS, |budget| {
            if !matches!(self.kind, QKind::Issue | QKind::Publish) {
                return Err(Error::Payload);
            }
            retain(
                Request::decode(&self.wire.bytes[REQUEST_PREFIX..REQUEST_PREFIX + Q], budget),
                budget,
            )
        })
    }
    pub fn decode_publication(&self, budget: &mut Budget<'_>) -> Result<(Publication, Storage)> {
        resources::nested_fixed(budget, QR, QW, QS, |budget| {
            if self.kind != QKind::Publish {
                return Err(Error::Payload);
            }
            retain(
                Publication::decode(
                    &self.wire.bytes[REQUEST_PREFIX + Q..REQUEST_PREFIX + Q + U],
                    budget,
                ),
                budget,
            )
        })
    }
    pub fn decode_subject(&self, budget: &mut Budget<'_>) -> Result<(Subject, Storage)> {
        resources::nested_fixed(budget, QR, QW, QS, |budget| {
            if self.kind != QKind::Recover {
                return Err(Error::Payload);
            }
            let (s, charge) = Subject::decode(self.wire.payload(REQUEST_FAMILY), budget)?;
            retain(
                Ok::<_, Error>((s, Storage(charge.retained_storage()))),
                budget,
            )
        })
    }
    pub fn decode_carriage(&self, budget: &mut Budget<'_>) -> Result<(Carriage, Storage)> {
        resources::nested_fixed(budget, QR, QW, QS, |budget| {
            if self.kind != QKind::VerifyCurrent {
                return Err(Error::Payload);
            }
            retain(
                Carriage::decode(&self.wire.bytes[REQUEST_PREFIX..REQUEST_PREFIX + C], budget),
                budget,
            )
        })
    }
    pub fn verification_challenge(&self) -> Option<[u8; 32]> {
        (self.kind == QKind::VerifyCurrent).then(|| {
            let mut value = [0; 32];
            value.copy_from_slice(&self.wire.bytes[REQUEST_PREFIX + C..REQUEST_PREFIX + C + 32]);
            value
        })
    }
}

/// Move-only canonical response. Correlation and leaf consistency are not
/// protected service authentication, currentness or signing authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceResponseV2;
/// fn duplicate(p: CompilerExecutionServiceResponseV2) { let _ = p.clone(); }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct CompilerExecutionServiceResponseV2 {
    wire: Wire<NP>,
    kind: PKind,
}
impl CompilerExecutionServiceResponseV2 {
    pub fn new(
        request: CompilerExecutionServiceRequestIdentityV2,
        policy: &Policy,
        payload: PPayload<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        resources::fixed(
            budget,
            policy.retained_storage() + payload.input_storage(),
            PW,
            PS,
            || {
                nonzero(&request.0)?;
                let (kind, position) = payload.fields(policy.identity())?;
                let mut disposition = [0; 8];
                if let PPayload::Published { disposition: d, .. } = &payload {
                    disposition[0] = *d as u8;
                }
                let fields = Fields {
                    kind: kind as u16,
                    policy: policy.identity(),
                    position,
                    request: request.0,
                };
                Ok((
                    Self {
                        wire: Wire::encode(RESPONSE_FAMILY, fields, payload.chunks(&disposition)),
                        kind,
                    },
                    Storage(PR),
                ))
            },
        )
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        resources::nested_fixed(
            budget,
            input_floor(bytes, RESPONSE_PREFIX + 32, NP),
            PW,
            PS,
            |budget| {
                let fields = Wire::<NP>::parse(bytes, RESPONSE_FAMILY)?;
                let kind = PKind::decode(fields.kind)?;
                if bytes.len() != kind.packet_bytes() {
                    return Err(Error::Length);
                }
                validate_response(
                    fields,
                    kind,
                    &bytes[RESPONSE_PREFIX..bytes.len() - 32],
                    budget,
                )?;
                Ok((
                    Self {
                        wire: Wire::copy(bytes, fields),
                        kind,
                    },
                    Storage(PR),
                ))
            },
        )
    }
    pub const fn kind(&self) -> PKind {
        self.kind
    }
    pub const fn request_identity(&self) -> CompilerExecutionServiceRequestIdentityV2 {
        CompilerExecutionServiceRequestIdentityV2(self.wire.fields.request)
    }
    pub const fn policy_identity(&self) -> PolicyIdentity {
        self.wire.fields.policy
    }
    pub const fn sequence(&self) -> u64 {
        self.wire.fields.position.sequence
    }
    pub const fn rollback_anchor(&self) -> [u8; 32] {
        self.wire.fields.position.anchor
    }
    pub const fn identity(&self) -> CompilerExecutionServiceResponseIdentityV2 {
        CompilerExecutionServiceResponseIdentityV2(self.wire.identity)
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        self.wire.bytes()
    }
    pub const fn retained_storage(&self) -> usize {
        PR
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub fn disposition(&self) -> Option<Disposition> {
        if self.kind != PKind::Published {
            return None;
        }
        if self.wire.bytes[RESPONSE_PREFIX] == 1 {
            Some(Disposition::Advanced)
        } else {
            Some(Disposition::AlreadyAcknowledged)
        }
    }
    pub fn decode_challenge(&self, budget: &mut Budget<'_>) -> Result<(Challenge, Storage)> {
        self.extract(PKind::Prepared, budget, |bytes, b| {
            Ok(Challenge::decode(bytes, b)?)
        })
    }
    pub fn decode_publication(&self, budget: &mut Budget<'_>) -> Result<(Publication, Storage)> {
        self.extract(PKind::Issued, budget, |bytes, b| {
            Ok(Publication::decode(bytes, b)?)
        })
    }
    pub fn decode_acknowledgment(&self, budget: &mut Budget<'_>) -> Result<(Ack, Storage)> {
        self.extract(PKind::Published, budget, |bytes, b| {
            Ok(Ack::decode(&bytes[8..], b)?)
        })
    }
    pub fn decode_carriage(&self, budget: &mut Budget<'_>) -> Result<(Carriage, Storage)> {
        self.extract(PKind::Recovered, budget, |bytes, b| {
            Ok(Carriage::decode(bytes, b)?)
        })
    }
    /// Inert V3 decoding only. No native protected authentication result exists here.
    pub fn decode_current_record_attestation(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(Current, Storage)> {
        self.extract(PKind::VerifiedCurrent, budget, decode_current)
    }
    fn extract<T>(
        &self,
        kind: PKind,
        budget: &mut Budget<'_>,
        decode: impl FnOnce(&[u8], &mut Budget<'_>) -> Result<(T, Storage)>,
    ) -> Result<(T, Storage)> {
        resources::nested_fixed(budget, PR, PW, PS, |budget| {
            if self.kind != kind {
                return Err(Error::Payload);
            }
            retain(decode(self.wire.payload(RESPONSE_FAMILY), budget), budget)
        })
    }
}

fn validate_request(
    fields: Fields,
    kind: QKind,
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<()> {
    let mut reader = Reader::new(bytes);
    let payload;
    let q;
    let u;
    let s;
    let c;
    match kind {
        QKind::Inspect => payload = QPayload::Inspect,
        QKind::Cancel => payload = QPayload::Cancel,
        QKind::Prepare => {
            payload = QPayload::Prepare {
                sequence: fields.position.sequence,
                prior_rollback_anchor: fields.position.anchor,
            }
        }
        QKind::Issue | QKind::Publish => {
            (q, _) = retain(Request::decode(reader.take(Q)?, budget), budget)?;
            if kind == QKind::Publish {
                (u, _) = retain(Publication::decode(reader.take(U)?, budget), budget)?;
                payload = QPayload::Publish {
                    request: &q,
                    publication: &u,
                };
            } else {
                payload = QPayload::Issue(&q);
            }
        }
        QKind::Recover => {
            let (subject, storage) = Subject::decode(reader.take(SUBJECT_BYTES)?, budget)?;
            budget.reserve_storage(storage.retained_storage())?;
            s = subject;
            payload = QPayload::Recover(&s);
        }
        QKind::VerifyCurrent => {
            (c, _) = retain(Carriage::decode(reader.take(C)?, budget), budget)?;
            payload = QPayload::VerifyCurrent {
                carriage: &c,
                verification_challenge: reader.fixed()?,
            };
        }
    }
    let (actual_kind, actual_position) = payload.fields(fields.policy)?;
    if actual_kind != kind || actual_position != fields.position {
        return Err(Error::Payload);
    }
    Ok(())
}
fn validate_response(
    fields: Fields,
    kind: PKind,
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<()> {
    let payload;
    let h;
    let u;
    let a;
    let c;
    let v;
    match kind {
        PKind::Ready => {
            payload = PPayload::Ready {
                sequence: fields.position.sequence,
                prior_rollback_anchor: fields.position.anchor,
            }
        }
        PKind::Cancelled => {
            payload = PPayload::Cancelled {
                sequence: fields.position.sequence,
                prior_rollback_anchor: fields.position.anchor,
            }
        }
        PKind::ReceiptAbsent => {
            payload = PPayload::ReceiptAbsent {
                sequence: fields.position.sequence,
                prior_rollback_anchor: fields.position.anchor,
            }
        }
        PKind::Prepared => {
            (h, _) = retain(Challenge::decode(bytes, budget), budget)?;
            payload = PPayload::Prepared(&h);
        }
        PKind::Issued => {
            (u, _) = retain(Publication::decode(bytes, budget), budget)?;
            payload = PPayload::Issued(&u);
        }
        PKind::Published => {
            let mut reader = Reader::new(bytes);
            let disposition = match reader.u8()? {
                1 => Disposition::Advanced,
                2 => Disposition::AlreadyAcknowledged,
                _ => return Err(Error::Payload),
            };
            if reader.fixed::<7>()? != [0; 7] {
                return Err(Error::Header);
            }
            (a, _) = retain(Ack::decode(reader.take(A)?, budget), budget)?;
            payload = PPayload::Published {
                acknowledgment: &a,
                disposition,
            };
        }
        PKind::Recovered => {
            (c, _) = retain(Carriage::decode(bytes, budget), budget)?;
            payload = PPayload::Recovered(&c);
        }
        PKind::VerifiedCurrent => {
            (v, _) = retain(decode_current(bytes, budget), budget)?;
            payload = PPayload::VerifiedCurrent(&v);
        }
    }
    let (actual_kind, actual_position) = payload.fields(fields.policy)?;
    if actual_kind != kind || actual_position != fields.position {
        return Err(Error::Payload);
    }
    Ok(())
}

fn decode_current(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Current, Storage)> {
    resources::fixed(
        budget,
        resources::fixed_input_floor(bytes, V),
        CURRENT_WORK,
        CURRENT_STORAGE,
        || {
            Ok((
                Current::decode(bytes).map_err(|_| Error::CurrentRecord)?,
                Storage(CURRENT_RETAINED),
            ))
        },
    )
}
fn retain<T, E: Into<Error>>(
    result: std::result::Result<(T, Storage), E>,
    budget: &mut Budget<'_>,
) -> Result<(T, Storage)> {
    let (owner, storage) = result.map_err(Into::into)?;
    budget.reserve_storage(storage.additional_storage())?;
    Ok((owner, storage))
}
fn request_position(request: &Request, policy: PolicyIdentity) -> Result<Position> {
    let h = request.challenge();
    if h.policy_identity() != policy {
        return Err(Error::Policy);
    }
    Position::prior(h.sequence(), h.prior_rollback_anchor())
}
fn carriage_position(carriage: &Carriage, policy: PolicyIdentity) -> Result<Position> {
    if carriage.policy().identity() != policy {
        return Err(Error::Policy);
    }
    Position::current(
        carriage.acknowledgment().sequence(),
        carriage.acknowledgment().current_rollback_anchor(),
    )
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Position {
    sequence: u64,
    anchor: [u8; 32],
}
impl Position {
    const ZERO: Self = Self {
        sequence: 0,
        anchor: [0; 32],
    };
    fn prior(sequence: u64, anchor: [u8; 32]) -> Result<Self> {
        if sequence == 0 || (sequence == 1) != (anchor == [0; 32]) {
            return Err(Error::Position);
        }
        Ok(Self { sequence, anchor })
    }
    fn current(sequence: u64, anchor: [u8; 32]) -> Result<Self> {
        if sequence == 0 || anchor == [0; 32] {
            return Err(Error::Position);
        }
        Ok(Self { sequence, anchor })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Fields {
    kind: u16,
    policy: PolicyIdentity,
    position: Position,
    request: [u8; 32],
}
#[derive(Clone, Copy)]
struct Family {
    magic: [u8; 8],
    domain: &'static [u8],
    prefix: usize,
}
const REQUEST_FAMILY: Family = Family {
    magic: REQUEST_MAGIC,
    domain: REQUEST_DOMAIN,
    prefix: REQUEST_PREFIX,
};
const RESPONSE_FAMILY: Family = Family {
    magic: RESPONSE_MAGIC,
    domain: RESPONSE_DOMAIN,
    prefix: RESPONSE_PREFIX,
};
#[derive(Eq, PartialEq)]
struct Wire<const N: usize> {
    fields: Fields,
    identity: [u8; 32],
    bytes: [u8; N],
    len: usize,
}
impl<const N: usize> Wire<N> {
    fn encode(family: Family, fields: Fields, chunks: [&[u8]; 2]) -> Self {
        let len = family.prefix + chunks[0].len() + chunks[1].len() + 32;
        let mut bytes = [0; N];
        let mut offset =
            encode_versioned_header(&mut bytes, family.magic, VERSION, fields.kind, len);
        if family.prefix == RESPONSE_PREFIX {
            put(&mut bytes, &mut offset, &fields.request);
        }
        put(&mut bytes, &mut offset, fields.policy.as_bytes());
        put(
            &mut bytes,
            &mut offset,
            &fields.position.sequence.to_le_bytes(),
        );
        put(&mut bytes, &mut offset, &fields.position.anchor);
        for chunk in chunks {
            put(&mut bytes, &mut offset, chunk);
        }
        let identity = derive_identity(family.domain, &bytes[..offset]);
        put(&mut bytes, &mut offset, &identity);
        Self {
            fields,
            identity,
            bytes,
            len,
        }
    }
    fn parse(bytes: &[u8], family: Family) -> Result<Fields> {
        if bytes.len() < family.prefix + 32 || bytes.len() > N {
            return Err(Error::Length);
        }
        let mut reader = Reader::new(bytes);
        let kind = decode_versioned_header(&mut reader, family.magic, VERSION, bytes.len())?;
        let request = if family.prefix == RESPONSE_PREFIX {
            let id = reader.fixed()?;
            nonzero(&id)?;
            id
        } else {
            [0; 32]
        };
        let policy = PolicyIdentity::from_bytes_for_protocol(reader.fixed()?);
        nonzero(policy.as_bytes())?;
        let position = Position {
            sequence: reader.u64()?,
            anchor: reader.fixed()?,
        };
        let prefix = bytes.len() - 32;
        if derive_identity(family.domain, &bytes[..prefix]) != bytes[prefix..] {
            return Err(Error::Identity);
        }
        Ok(Fields {
            kind,
            policy,
            position,
            request,
        })
    }
    fn copy(bytes: &[u8], fields: Fields) -> Self {
        let mut stored = [0; N];
        stored[..bytes.len()].copy_from_slice(bytes);
        let mut identity = [0; 32];
        identity.copy_from_slice(&bytes[bytes.len() - 32..]);
        Self {
            fields,
            identity,
            bytes: stored,
            len: bytes.len(),
        }
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    fn payload(&self, family: Family) -> &[u8] {
        &self.bytes[family.prefix..self.len - 32]
    }
}
impl<const N: usize> fmt::Debug for Wire<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeServicePacket")
            .field("fields", &self.fields)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}
fn nonzero(value: &[u8; 32]) -> Result<()> {
    if *value == [0; 32] {
        Err(Error::Identity)
    } else {
        Ok(())
    }
}
fn input_floor(bytes: &[u8], minimum: usize, maximum: usize) -> usize {
    if (minimum..=maximum).contains(&bytes.len()) {
        bytes.len()
    } else {
        0
    }
}

/// Fixed-size diagnostics. Untrusted data is never formatted or retained here.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionServiceProtocolErrorV2 {
    Length,
    Header,
    Kind,
    Policy,
    Position,
    Payload,
    Identity,
    Subject,
    Attestation,
    Publication,
    CurrentRecord,
    Resource(Resource),
}
use CompilerExecutionServiceProtocolErrorV2 as Error;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<Framing> for Error {
    fn from(_: Framing) -> Self {
        Self::Header
    }
}
impl From<SubjectError> for Error {
    fn from(e: SubjectError) -> Self {
        match e {
            SubjectError::Resource(e) => Self::Resource(e),
            _ => Self::Subject,
        }
    }
}
impl From<AttestationError> for Error {
    fn from(e: AttestationError) -> Self {
        match e {
            AttestationError::Resource(e) => Self::Resource(e),
            AttestationError::Subject(e) => e.into(),
            _ => Self::Attestation,
        }
    }
}
impl From<PublicationError> for Error {
    fn from(e: PublicationError) -> Self {
        match e {
            PublicationError::Resource(e) => Self::Resource(e),
            PublicationError::Attestation(e) => e.into(),
            _ => Self::Publication,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Length => "native service packet length is invalid",
            Self::Header => "native service packet header is invalid",
            Self::Kind => "native service packet operation is unknown",
            Self::Policy => "native service packet policy mismatch",
            Self::Position => "native service packet position is invalid",
            Self::Payload => "native service packet payload mismatch",
            Self::Identity => "native service packet identity mismatch",
            Self::Subject => "native service packet subject rejected",
            Self::Attestation => "native service packet attestation rejected",
            Self::Publication => "native service packet publication rejected",
            Self::CurrentRecord => "native service packet current record rejected",
            Self::Resource(_) => "native service packet resource budget rejected",
        })
    }
}
impl StdError for Error {}
const _: () = {
    assert!(size_of::<Error>() <= 128);
    assert!(size_of::<(CompilerExecutionServiceRequestV2, Storage)>() <= QR);
    assert!(size_of::<(CompilerExecutionServiceResponseV2, Storage)>() <= PR);
    assert!(
        8 * size_of::<Error>()
            + 16 * size_of::<Fields>()
            + 8 * size_of::<QPayload<'static>>()
            + 8 * size_of::<PPayload<'static>>()
            <= 8192
    );
};

#[cfg(test)]
#[path = "service_v2_tests.rs"]
mod tests;
