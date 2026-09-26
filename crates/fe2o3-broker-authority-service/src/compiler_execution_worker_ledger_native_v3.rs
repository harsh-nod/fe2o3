//! Native Worker records consumed under the issuer's one directory singleton.
use super::ledger::Stored;
use super::*;
use super::{ANCHOR_JOURNAL_BYTES as A, TRANSACTION_BYTES as T};
use crate::compiler_execution_journal_recovery::{
    JournalNames, NATIVE_ANCHOR_STATE_FILES, NATIVE_WORKER_STATE_FILES,
};
use fe2o3_artifact_transaction::{
    NoRetainedDurableDirectoryHooksV1 as NoHooks, RetainedDurableDirectoryHooksV1 as Hooks,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionWorkerAnchorJournalStageV1 as Stage;
use fe2o3_external_anchor_protocol::{
    ANCHOR_TRANSITION_RECEIPT_BYTES_V1 as R, AnchorChallengeV1 as AnchorChallenge,
    AnchorPositionV1, AnchorTransitionReceiptV1 as AnchorReceipt, AnchoredStateV1, CallerNonceV1,
    ChallengeKindV1, HashChainHeadV1, PinnedAnchorKeyV1,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

include!("compiler_execution_worker_ledger_native_body.rs");
native_worker!(
    external_anchor_currentness_challenge_native_v3,
    new_native_v3
);
