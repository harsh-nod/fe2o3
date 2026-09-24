//! Complete native carriage with same-ledger nested decoding and exact joins.
use crate::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2 as VS,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V2 as VW,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V2 as QS,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2 as QW,
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2 as PS,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as PW,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V2 as AS,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V2 as AW,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V2 as US,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V2 as UW,
    CompilerExecutionAttestationRequestV2 as Request, CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionReceiptPublicationAckV2 as Ack,
    CompilerExecutionReceiptPublicationV2 as Publication,
    attestation_request_v2::RETAINED as REQUEST_RETAINED,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    issuer_policy_v2::RETAINED as POLICY_RETAINED,
    receipt_publication_codec as codec,
    receipt_publication_v2::{
        ACK_RETAINED, CompilerExecutionReceiptPublicationErrorV2 as Error, PUBLICATION_RETAINED,
        Result,
    },
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

pub const COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V2: usize = codec::CARRIAGE_BYTES;
/// Fixed outer framing work, also used for identity matching.
pub const COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2: usize =
    resources::ENTRY_WORK + 32 * codec::CARRIAGE_BYTES;
pub const COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_WORK_V2: usize =
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2 + VW + AW;
pub const COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V2: usize =
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2 + PW + QW + UW + AW + VW + AW;
const INHERITED: usize = POLICY_RETAINED + REQUEST_RETAINED + PUBLICATION_RETAINED + ACK_RETAINED;
const RETAINED: usize = size_of::<CompilerExecutionReceiptCarriageV2>() + size_of::<Storage>();
/// Fixed additional logical outer frame; excludes nested operations' scratch.
pub const COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2: usize =
    4 * RETAINED + 4 * codec::CARRIAGE_BYTES + 2 * size_of::<sha2::Sha256>() + 4096;
pub const COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_STORAGE_V2: usize =
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2 + maximum(&[VS, AS]);
/// Peak over the actual sequential child-retention schedule, above entry storage.
pub const COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V2: usize =
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2
        + maximum(&[
            PS,
            POLICY_RETAINED + QS,
            POLICY_RETAINED + REQUEST_RETAINED + US,
            POLICY_RETAINED + REQUEST_RETAINED + PUBLICATION_RETAINED + AS,
            INHERITED + VS,
            INHERITED + AS,
        ]);
const fn maximum(values: &[usize]) -> usize {
    let mut result = 0;
    let mut i = 0;
    while i < values.len() {
        if values[i] > result {
            result = values[i];
        }
        i += 1;
    }
    result
}
const _: () = {
    assert!(RETAINED >= INHERITED);
    assert!(PS >= POLICY_RETAINED);
    assert!(QS >= REQUEST_RETAINED);
    assert!(US >= PUBLICATION_RETAINED);
    assert!(AS >= ACK_RETAINED);
    assert!(size_of::<(CompilerExecutionReceiptCarriageV2, Storage)>() <= RETAINED);
    assert!(
        size_of::<std::thread::Result<Result<(CompilerExecutionReceiptCarriageV2, Storage)>>>()
            <= RETAINED + 4096
    );
    assert!(
        8 * size_of::<Error>()
            + 64 * size_of::<usize>()
            + 4 * size_of::<codec::CarriageParts<'static>>()
            + 4 * (size_of::<
                std::thread::Result<Result<(CompilerExecutionReceiptCarriageV2, Storage)>>,
            >() - size_of::<(CompilerExecutionReceiptCarriageV2, Storage)>())
            <= 4096
    );
    assert!(size_of::<std::thread::Result<Result<bool>>>() <= 4096);
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionReceiptCarriageIdentityV2([u8; 32]);
impl CompilerExecutionReceiptCarriageIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
        resources::fixed(
            budget,
            resources::fixed_input_floor(bytes, codec::CARRIAGE_BYTES),
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2,
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2,
            || Ok(codec::V2.matches_carriage(self.0, bytes)),
        )
    }
}

/// Move-only complete native policy, request, publication and ACK. Relationships
/// are verified, but protected policy pinning and durable currentness are not.
/// No clone, legacy projection, alternate graph or authority-bearing result.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriageV2;
/// fn duplicate(v: CompilerExecutionReceiptCarriageV2) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionReceiptCarriageV2, CompilerExecutionIssuerPolicyV1, CompilerExecutionAttestationRequestV2, CompilerExecutionReceiptPublicationV2, CompilerExecutionReceiptPublicationAckV2};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(p: CompilerExecutionIssuerPolicyV1,q: CompilerExecutionAttestationRequestV2,
///        u: CompilerExecutionReceiptPublicationV2,a: CompilerExecutionReceiptPublicationAckV2,
///        b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = CompilerExecutionReceiptCarriageV2::new(p,q,u,a,b);
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionReceiptCarriageV2 {
    policy: Policy,
    request: Request,
    publication: Publication,
    acknowledgment: Ack,
    record: codec::Record<{ codec::CARRIAGE_BYTES }>,
}
impl CompilerExecutionReceiptCarriageV2 {
    /// Transfer all four prepaid reservations; reserve only the returned delta.
    /// On refusal owners drop but inherited reservations remain for retirement.
    pub fn new(
        policy: Policy,
        request: Request,
        publication: Publication,
        acknowledgment: Ack,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        resources::nested_fixed(
            budget,
            INHERITED,
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2,
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2,
            |budget| {
                Ok((
                    Self::from_owned(policy, request, publication, acknowledgment, budget)?,
                    Storage(RETAINED - INHERITED),
                ))
            },
        )
    }
    /// Decode each real native child on this ledger. Returns the FULL carriage
    /// charge; internally reserved children do not transfer from borrowed bytes.
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        resources::nested_fixed(
            budget,
            resources::fixed_input_floor(bytes, codec::CARRIAGE_BYTES),
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2,
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2,
            |budget| {
                let parts = codec::V2.carriage_parts(bytes)?;
                let (policy, s) = Policy::decode(parts.policy, budget)?;
                budget.reserve_storage(s.additional_storage())?;
                let (request, s) = Request::decode(parts.request, budget)?;
                budget.reserve_storage(s.additional_storage())?;
                let (publication, s) = Publication::decode(parts.publication, budget)?;
                budget.reserve_storage(s.additional_storage())?;
                let (acknowledgment, s) = Ack::decode(parts.ack, budget)?;
                budget.reserve_storage(s.additional_storage())?;
                let decoded =
                    Self::from_owned(policy, request, publication, acknowledgment, budget)?;
                decoded
                    .record
                    .check(parts.identity, bytes, "compiler receipt carriage")?;
                Ok((decoded, Storage(RETAINED)))
            },
        )
    }
    fn from_owned(
        policy: Policy,
        request: Request,
        publication: Publication,
        acknowledgment: Ack,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        publication.receipt().verify_matches(
            &policy,
            &request,
            request.challenge().prior_rollback_anchor(),
            budget,
        )?;
        acknowledgment.matches_publication(&publication, budget)?;
        let record = codec::V2.carriage(
            policy.canonical_bytes(),
            request.canonical_bytes(),
            publication.canonical_bytes(),
            acknowledgment.canonical_bytes(),
        );
        Ok(Self {
            policy,
            request,
            publication,
            acknowledgment,
            record,
        })
    }
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }
    pub const fn request(&self) -> &Request {
        &self.request
    }
    pub const fn publication(&self) -> &Publication {
        &self.publication
    }
    pub const fn acknowledgment(&self) -> &Ack {
        &self.acknowledgment
    }
    pub const fn identity(&self) -> CompilerExecutionReceiptCarriageIdentityV2 {
        CompilerExecutionReceiptCarriageIdentityV2(self.record.identity)
    }
    pub const fn canonical_bytes(&self) -> &[u8; codec::CARRIAGE_BYTES] {
        &self.record.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
    pub const fn requires_protected_policy_verification(&self) -> bool {
        true
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}
impl fmt::Debug for CompilerExecutionReceiptCarriageV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionReceiptCarriageV2")
            .field("policy_identity", &self.policy.identity())
            .field("request_identity", &self.request.identity())
            .field("publication_identity", &self.publication.identity())
            .field("acknowledgment_identity", &self.acknowledgment.identity())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}
