//! Diagnostic state only. Resource decisions still belong to the existing Budget.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum Phase {
    #[default]
    Conditions,
    CandidateScan,
    CfgPreflight,
    InitialState,
    EntryClone,
    Statements,
    TerminatorTransfer,
    TerminatorEdges,
    OutgoingReservation,
    SuccessorClone,
    CallReturn,
    Join,
    StoreSuccessor,
    Publication,
    PublicationStatements,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StorageOperation {
    Check,
    Resize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResourceFailure {
    Work {
        remaining: usize,
        requested: usize,
    },
    Storage {
        operation: StorageOperation,
        retained: usize,
        replaced: usize,
        requested: Option<usize>,
        attempted: Option<usize>,
        limit: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Failure {
    Resource(ResourceFailure),
    ConditionsUnavailable,
    InvalidEntry,
    BlockLimit { blocks: usize, limit: usize },
    MissingEntry,
    InvalidCheckedSite,
    InvalidLocal,
    DepthLimit,
    VariantFieldCount,
    UnsupportedTerminator,
    InvalidSuccessor,
    Accounting,
    UnclassifiedTransfer,
}

// Keep the original live counter and a constant-size first-failure record in
// the same allocation. Dropping a reservation cannot overwrite the failure.
#[derive(Default)]
pub(super) struct SharedStorage {
    live: Cell<usize>,
    failure: Cell<Option<ResourceFailure>>,
}

impl SharedStorage {
    pub(super) fn get(&self) -> usize {
        self.live.get()
    }

    pub(super) fn set(&self, value: usize) {
        self.live.set(value);
    }

    pub(super) fn record(&self, failure: ResourceFailure) {
        if self.failure.get().is_none() {
            self.failure.set(Some(failure));
        }
    }

    pub(super) fn failure(&self) -> Option<ResourceFailure> {
        self.failure.get()
    }
}
