//! Native source-bearing journal over the same external-anchor state machine.
use crate::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V2 as T,
    CompilerExecutionExternalAnchorTransactionV2 as Transaction,
    CompilerExecutionNativeJournalErrorV2 as TransactionError,
    CompilerExecutionWorkerAnchorJournalErrorV1 as FrameError,
    CompilerExecutionWorkerAnchorJournalStageV1 as Stage,
    attestation_resources::{CompilerExecutionAttestationStorageV2 as Storage, STRICT_VERIFY_WORK},
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

pub const COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V2: usize = 32 + T + C + R + 64;
const N: usize = COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V2;
const SCHEMA: codec::Schema = codec::Schema {
    magic: *b"F2O3CAJ2",
    version: 2,
    domain: b"FE2O3/COMPILER-EXECUTION-WORKER-ANCHOR-JOURNAL/V2\0",
};
/// Inert, move-only native journal. A decoded journal is not proof of durability.
/// All constructors borrow inputs and return the full additional owner charge.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionWorkerAnchorJournalV2;
/// fn duplicate(value: CompilerExecutionWorkerAnchorJournalV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionWorkerAnchorJournalV1, CompilerExecutionWorkerAnchorJournalV2};
/// fn convert(value: CompilerExecutionWorkerAnchorJournalV1) -> CompilerExecutionWorkerAnchorJournalV2 { value.into() }
/// ```
#[derive(Debug)]
pub struct CompilerExecutionWorkerAnchorJournalV2 {
    transaction: Transaction,
    challenge: AnchorChallengeV1,
    receipt: Option<AnchorTransitionReceiptV1>,
    worker: [u8; 32],
    stage: Stage,
    canonical: [u8; N],
}
type Journal = CompilerExecutionWorkerAnchorJournalV2;
#[path = "worker_anchor_journal_adapter.rs"]
mod adapter;
adapter::worker_anchor_journal!(CompilerExecutionWorkerAnchorJournalErrorV2);

#[cfg(test)]
#[path = "worker_anchor_journal_tests.rs"]
mod tests;
