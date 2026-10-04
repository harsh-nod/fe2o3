//! Native source-bearing journal over the same external-anchor state machine.
use crate::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V3 as T,
    CompilerExecutionExternalAnchorTransactionV3 as Transaction,
    CompilerExecutionNativeJournalErrorV3 as TransactionError,
    CompilerExecutionWorkerAnchorJournalErrorV1 as FrameError,
    CompilerExecutionWorkerAnchorJournalStageV1 as Stage,
    attestation_resources::{CompilerExecutionAttestationStorageV3 as Storage, STRICT_VERIFY_WORK},
    worker_anchor_journal_codec as codec,
};
use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1 as C, ANCHOR_TRANSITION_RECEIPT_BYTES_V1 as R, AnchorChallengeV1,
    AnchorProtocolErrorV1, AnchorTransitionReceiptV1, ChallengeKindV1, HashChainHeadV1,
    PinnedAnchorKeyV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, mem::size_of};

pub const COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V3: usize = 32 + T + C + R + 64;
const N: usize = COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V3;
const SCHEMA: codec::Schema = codec::Schema {
    magic: *b"F2O3CAJ3",
    version: 3,
    domain: b"FE2O3/COMPILER-EXECUTION-WORKER-ANCHOR-JOURNAL/V3\0",
};
/// Inert, move-only native journal. A decoded journal is not proof of durability.
/// All constructors borrow inputs and return the full additional owner charge.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionWorkerAnchorJournalV3;
/// fn duplicate(value: CompilerExecutionWorkerAnchorJournalV3) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionWorkerAnchorJournalV1, CompilerExecutionWorkerAnchorJournalV3};
/// fn convert(value: CompilerExecutionWorkerAnchorJournalV1) -> CompilerExecutionWorkerAnchorJournalV3 { value.into() }
/// ```
/// The transaction is an actual V3 owner, with V3 policy, request and publication.
/// Stage and external-anchor primitives retain their existing V1 semantics.
/// All operations restore entry storage and retain accepted work; keep borrowed
/// inputs prepaid on the same original ledger, then reserve the full returned
/// charge before retaining the new journal. Retire it only after dropping the owner.
///
/// ```
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionExternalAnchorTransactionV3 as Transaction,
///     CompilerExecutionWorkerAnchorJournalV3 as Journal,
///     CompilerExecutionWorkerAnchorJournalErrorV3 as Error,
///     CompilerExecutionAttestationStorageV3 as Storage,
/// };
/// use fe2o3_external_anchor_protocol::AnchorChallengeV1 as Challenge;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn prepare(t: &Transaction, c: &Challenge, b: &mut Budget<'_>)
///     -> Result<(Journal, Storage), Error>
/// {
///     let (journal, charge) = Journal::prepared(t, c, b)?;
///     let _: &Transaction = journal.transaction();
///     Ok((journal, charge))
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionExternalAnchorTransactionV2 as Transaction,
///     CompilerExecutionWorkerAnchorJournalV3 as Journal,
/// };
/// use fe2o3_external_anchor_protocol::AnchorChallengeV1 as Challenge;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(t: &Transaction, c: &Challenge, b: &mut Budget<'_>) {
///     let _ = Journal::prepared(t, c, b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionWorkerAnchorJournalV2 as V2,
///     CompilerExecutionWorkerAnchorJournalV3 as V3,
/// };
/// fn upgrade(value: V2) -> V3 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionWorkerAnchorJournalV2 as V2,
///     CompilerExecutionWorkerAnchorJournalV3 as V3,
/// };
/// fn downgrade(value: V3) -> V2 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionWorkerAnchorJournalV2 as V2,
///     CompilerExecutionWorkerAnchorJournalV3 as V3,
/// };
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(next: &V3, prior: &V2, b: &mut Budget<'_>) {
///     let _ = next.is_legal_successor_of(prior, b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionExternalAnchorTransactionV2 as Transaction,
///     CompilerExecutionWorkerAnchorJournalV3 as Journal,
/// };
/// fn mix(journal: &Journal) -> &Transaction { journal.transaction() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionWorkerAnchorJournalErrorV3 as Error,
///     CompilerExecutionNativeJournalErrorV2 as TransactionError,
/// };
/// fn mix(error: TransactionError) -> Error { Error::Transaction(error) }
/// ```
#[derive(Debug)]
pub struct CompilerExecutionWorkerAnchorJournalV3 {
    transaction: Transaction,
    challenge: AnchorChallengeV1,
    receipt: Option<AnchorTransitionReceiptV1>,
    worker: [u8; 32],
    stage: Stage,
    canonical: [u8; N],
}
type Journal = CompilerExecutionWorkerAnchorJournalV3;
#[path = "worker_anchor_journal_adapter.rs"]
mod adapter;
adapter::worker_anchor_journal!(CompilerExecutionWorkerAnchorJournalErrorV3);

#[cfg(test)]
#[path = "worker_anchor_journal_tests.rs"]
mod tests;
