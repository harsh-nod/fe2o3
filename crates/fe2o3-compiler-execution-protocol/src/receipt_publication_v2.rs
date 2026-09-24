//! Native publication claims. Durability must be independently established.
use crate::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V2 as RECEIPT_WORK,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2 as RECEIPT_STORAGE,
    CompilerExecutionAttestationErrorV2 as AttestationError,
    CompilerExecutionAttestationReceiptIdentityV2 as ReceiptIdentity,
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionReceiptPublicationErrorV1 as Framing,
    attestation_receipt_v2::RETAINED as RECEIPT_RETAINED,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    receipt_publication_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error as StdError, fmt, mem::size_of};

pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V2: usize = codec::PUBLICATION_BYTES;
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V2: usize = codec::ACK_BYTES;
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V2: usize =
    resources::ENTRY_WORK + 32 * codec::PUBLICATION_BYTES;
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V2: usize =
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V2 + RECEIPT_WORK;
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V2: usize =
    resources::ENTRY_WORK + 32 * codec::ACK_BYTES;
pub(crate) const PUBLICATION_RETAINED: usize =
    size_of::<CompilerExecutionReceiptPublicationV2>() + size_of::<Storage>();
pub(crate) const ACK_RETAINED: usize =
    size_of::<CompilerExecutionReceiptPublicationAckV2>() + size_of::<Storage>();
/// Additional fixed logical peak; not generated stack, heap or RSS usage.
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V2: usize =
    4 * PUBLICATION_RETAINED + 4 * codec::PUBLICATION_BYTES + 2 * size_of::<sha2::Sha256>() + 4096;
/// Outer frame stays reserved while the actual nested receipt decoder runs.
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V2: usize =
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V2 + RECEIPT_STORAGE;
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V2: usize = 4 * ACK_RETAINED
    + 4 * size_of::<codec::AckFields>()
    + 4 * codec::ACK_BYTES
    + 2 * size_of::<sha2::Sha256>()
    + 4096;
const _: () = {
    assert!(PUBLICATION_RETAINED >= RECEIPT_RETAINED);
    assert!(RECEIPT_STORAGE >= RECEIPT_RETAINED);
    assert!(size_of::<(CompilerExecutionReceiptPublicationV2, Storage)>() <= PUBLICATION_RETAINED);
    assert!(size_of::<(CompilerExecutionReceiptPublicationAckV2, Storage)>() <= ACK_RETAINED);
    assert!(
        8 * size_of::<CompilerExecutionReceiptPublicationErrorV2>()
            + 64 * size_of::<usize>()
            + 4 * size_of::<codec::PublicationParts<'static>>()
            + 4 * size_of::<codec::Bindings>()
            + 4 * (size_of::<
                std::thread::Result<Result<(CompilerExecutionReceiptPublicationV2, Storage)>>,
            >() - size_of::<(CompilerExecutionReceiptPublicationV2, Storage)>())
            <= 4096
    );
    assert!(size_of::<std::thread::Result<Result<bool>>>() <= 4096);
    assert!(size_of::<std::thread::Result<Result<()>>>() <= 4096);
    assert!(
        size_of::<std::thread::Result<Result<(CompilerExecutionReceiptPublicationV2, Storage)>>>()
            <= PUBLICATION_RETAINED + 4096
    );
    assert!(
        size_of::<std::thread::Result<Result<(CompilerExecutionReceiptPublicationAckV2, Storage)>>>(
        ) <= ACK_RETAINED + 4096
    );
};
pub(crate) type Result<T> = std::result::Result<T, CompilerExecutionReceiptPublicationErrorV2>;

#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionReceiptPublicationErrorV2 {
    Framing(Framing),
    Attestation(AttestationError),
    Resource(Resource),
}
impl From<Framing> for CompilerExecutionReceiptPublicationErrorV2 {
    fn from(e: Framing) -> Self {
        Self::Framing(e)
    }
}
impl From<AttestationError> for CompilerExecutionReceiptPublicationErrorV2 {
    fn from(e: AttestationError) -> Self {
        Self::Attestation(e)
    }
}
impl From<Resource> for CompilerExecutionReceiptPublicationErrorV2 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for CompilerExecutionReceiptPublicationErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(e) => e.fmt(f),
            Self::Attestation(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
        }
    }
}
impl StdError for CompilerExecutionReceiptPublicationErrorV2 {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(match self {
            Self::Framing(e) => e,
            Self::Attestation(e) => e,
            Self::Resource(e) => e,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionReceiptPublicationIdentityV2([u8; 32]);
impl CompilerExecutionReceiptPublicationIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
        publication_fixed(
            budget,
            resources::fixed_input_floor(bytes, codec::PUBLICATION_BYTES),
            || Ok(codec::V2.matches_publication(self.0, bytes)),
        )
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionReceiptPublicationAckIdentityV2([u8; 32]);
impl CompilerExecutionReceiptPublicationAckIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
        ack_fixed(
            budget,
            resources::fixed_input_floor(bytes, codec::ACK_BYTES),
            || Ok(codec::V2.matches_ack(self.0, bytes)),
        )
    }
}

/// Move-only sidecar of a signed native receipt. No durable publication authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptPublicationV2;
/// fn duplicate(v: CompilerExecutionReceiptPublicationV2) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationReceiptV1, CompilerExecutionReceiptPublicationV2};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(r: CompilerExecutionAttestationReceiptV1,b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = CompilerExecutionReceiptPublicationV2::new([1;32],[2;32],r,b);
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionReceiptPublicationV2 {
    pub(crate) record: codec::Publication,
    receipt: Receipt,
}
impl CompilerExecutionReceiptPublicationV2 {
    /// Transfer the consumed receipt reservation; reserve only the returned delta.
    /// On refusal its owner drops but the inherited reservation remains.
    pub fn new(
        journal: [u8; 32],
        occurrence: [u8; 32],
        receipt: Receipt,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        publication_fixed(budget, RECEIPT_RETAINED, || {
            Ok((
                Self::from_receipt(journal, occurrence, receipt)?,
                Storage(PUBLICATION_RETAINED - RECEIPT_RETAINED),
            ))
        })
    }
    fn from_receipt(journal: [u8; 32], occurrence: [u8; 32], receipt: Receipt) -> Result<Self> {
        let record = codec::V2.publication(
            codec::Bindings {
                policy: *receipt.policy_identity().as_bytes(),
                journal,
                occurrence,
                receipt: *receipt.identity().as_bytes(),
            },
            receipt.canonical_bytes(),
        )?;
        Ok(Self { record, receipt })
    }
    /// Borrow prepaid bytes; returns the FULL owner charge, not an inherited delta.
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        resources::nested_fixed(
            budget,
            resources::fixed_input_floor(bytes, codec::PUBLICATION_BYTES),
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V2,
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V2,
            |budget| {
                let parts = codec::V2.publication_parts(bytes)?;
                let (receipt, storage) = Receipt::decode(parts.receipt, budget)?;
                budget.reserve_storage(storage.additional_storage())?;
                let record = codec::V2.finish_publication(
                    parts,
                    *receipt.policy_identity().as_bytes(),
                    *receipt.identity().as_bytes(),
                    receipt.canonical_bytes(),
                    bytes,
                )?;
                Ok((Self { record, receipt }, Storage(PUBLICATION_RETAINED)))
            },
        )
    }
    pub const fn policy_identity(&self) -> PolicyIdentity {
        PolicyIdentity::from_bytes_for_protocol(self.record.bindings.policy)
    }
    pub const fn issuer_journal_identity(&self) -> [u8; 32] {
        self.record.bindings.journal
    }
    pub const fn compiler_occurrence_identity(&self) -> [u8; 32] {
        self.record.bindings.occurrence
    }
    pub const fn receipt_identity(&self) -> ReceiptIdentity {
        ReceiptIdentity::from_bytes_for_protocol(self.record.bindings.receipt)
    }
    pub const fn receipt(&self) -> &Receipt {
        &self.receipt
    }
    pub const fn identity(&self) -> CompilerExecutionReceiptPublicationIdentityV2 {
        CompilerExecutionReceiptPublicationIdentityV2(self.record.wire.identity)
    }
    pub const fn canonical_bytes(&self) -> &[u8; codec::PUBLICATION_BYTES] {
        &self.record.wire.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        PUBLICATION_RETAINED
    }
    pub fn matches_issued_record(
        &self,
        policy: PolicyIdentity,
        journal: [u8; 32],
        occurrence: [u8; 32],
        receipt: ReceiptIdentity,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        publication_fixed(budget, PUBLICATION_RETAINED, || {
            Ok(self.record.bindings.matches(codec::Bindings {
                policy: *policy.as_bytes(),
                journal,
                occurrence,
                receipt: *receipt.as_bytes(),
            })?)
        })
    }
    pub const fn proves_durable_publication(&self) -> bool {
        false
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
}
impl fmt::Debug for CompilerExecutionReceiptPublicationV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionReceiptPublicationV2")
            .field("policy_identity", &self.policy_identity())
            .field("issuer_journal_identity", &self.issuer_journal_identity())
            .field(
                "compiler_occurrence_identity",
                &self.compiler_occurrence_identity(),
            )
            .field("receipt_identity", &self.receipt_identity())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

/// Move-only inert ACK claim. Reacquire and verify durable Worker state separately.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptPublicationAckV2;
/// fn duplicate(v: CompilerExecutionReceiptPublicationAckV2) { let _ = v.clone(); }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionReceiptPublicationAckV2 {
    record: codec::Ack,
}
impl CompilerExecutionReceiptPublicationAckV2 {
    /// Borrows the publication; returns a full additional ACK charge.
    pub fn new(
        publication: &CompilerExecutionReceiptPublicationV2,
        worker: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        ack_fixed(budget, PUBLICATION_RETAINED, || {
            Ok((
                Self {
                    record: codec::V2.ack(codec::AckFields {
                        bindings: publication.record.bindings,
                        publication: publication.record.wire.identity,
                        worker,
                        sequence: publication.receipt.sequence(),
                        current: publication.receipt.next_rollback_anchor(),
                    })?,
                },
                Storage(ACK_RETAINED),
            ))
        })
    }
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        ack_fixed(
            budget,
            resources::fixed_input_floor(bytes, codec::ACK_BYTES),
            || {
                Ok((
                    Self {
                        record: codec::V2.decode_ack(bytes)?,
                    },
                    Storage(ACK_RETAINED),
                ))
            },
        )
    }
    pub fn matches_publication(
        &self,
        publication: &CompilerExecutionReceiptPublicationV2,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        ack_fixed(budget, ACK_RETAINED + PUBLICATION_RETAINED, || {
            Ok(self.record.matches(
                &publication.record,
                publication.receipt.sequence(),
                publication.receipt.next_rollback_anchor(),
            )?)
        })
    }
    pub fn matches_worker_ledger_record(
        &self,
        expected: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        ack_fixed(budget, ACK_RETAINED, || {
            Ok(self.record.matches_worker(expected)?)
        })
    }
    pub const fn policy_identity(&self) -> PolicyIdentity {
        PolicyIdentity::from_bytes_for_protocol(self.record.fields.bindings.policy)
    }
    pub const fn issuer_journal_identity(&self) -> [u8; 32] {
        self.record.fields.bindings.journal
    }
    pub const fn compiler_occurrence_identity(&self) -> [u8; 32] {
        self.record.fields.bindings.occurrence
    }
    pub const fn receipt_identity(&self) -> ReceiptIdentity {
        ReceiptIdentity::from_bytes_for_protocol(self.record.fields.bindings.receipt)
    }
    pub const fn publication_identity(&self) -> CompilerExecutionReceiptPublicationIdentityV2 {
        CompilerExecutionReceiptPublicationIdentityV2(self.record.fields.publication)
    }
    pub const fn worker_ledger_record_identity(&self) -> [u8; 32] {
        self.record.fields.worker
    }
    pub const fn sequence(&self) -> u64 {
        self.record.fields.sequence
    }
    pub const fn current_rollback_anchor(&self) -> [u8; 32] {
        self.record.fields.current
    }
    pub const fn identity(&self) -> CompilerExecutionReceiptPublicationAckIdentityV2 {
        CompilerExecutionReceiptPublicationAckIdentityV2(self.record.wire.identity)
    }
    pub const fn canonical_bytes(&self) -> &[u8; codec::ACK_BYTES] {
        &self.record.wire.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        ACK_RETAINED
    }
    pub const fn proves_durable_publication(&self) -> bool {
        false
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
}
impl fmt::Debug for CompilerExecutionReceiptPublicationAckV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionReceiptPublicationAckV2")
            .field("policy_identity", &self.policy_identity())
            .field("issuer_journal_identity", &self.issuer_journal_identity())
            .field(
                "compiler_occurrence_identity",
                &self.compiler_occurrence_identity(),
            )
            .field("receipt_identity", &self.receipt_identity())
            .field("publication_identity", &self.publication_identity())
            .field(
                "worker_ledger_record_identity",
                &self.worker_ledger_record_identity(),
            )
            .field("sequence", &self.sequence())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}
fn publication_fixed<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    f: impl FnOnce() -> Result<T>,
) -> Result<T> {
    resources::fixed(
        budget,
        floor,
        COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V2,
        COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V2,
        f,
    )
}
fn ack_fixed<T>(budget: &mut Budget<'_>, floor: usize, f: impl FnOnce() -> Result<T>) -> Result<T> {
    resources::fixed(
        budget,
        floor,
        COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V2,
        COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V2,
        f,
    )
}
