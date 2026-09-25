//! Passive actual-owner observations for source fixtures. No event admits a proof.
//! Callback entry and acceptance after all postchecks are deliberately distinct.
use crate::production_pipeline::conditional_generated_fields_v1::RetainedConditionalContractV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
};
use fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1 as Request;
use fe2o3_verifier::{
    ProductionConditionalFormulaExecutionV1 as Execution,
    ProductionConditionalFormulaReportV1 as Report,
    RetainedProductionConditionalFormulaV1 as Retained,
};
use serde::Serialize;
use std::cell::RefCell;

#[path = "retained_proof_assertions_v1_tests.rs"]
mod assertions;
pub(crate) use assertions::check;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Resources {
    pub(crate) work: usize,
    pub(crate) failed_work: Option<usize>,
    pub(crate) storage: usize,
    pub(crate) peak_storage: usize,
    pub(crate) failed_storage: Option<usize>,
}
impl Resources {
    pub(super) fn from_owned(ledger: &OwnedBudget) -> Self {
        Self {
            work: ledger.work(),
            failed_work: ledger.failed_work(),
            storage: ledger.storage(),
            peak_storage: ledger.peak_storage(),
            failed_storage: ledger.failed_storage(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Receipt {
    pub(crate) statement: [u8; 32],
    pub(crate) generated_source: [u8; 32],
    pub(crate) execution: [u8; 32],
    pub(crate) receipt: [u8; 32],
    pub(crate) reference_identity: [u8; 32],
    pub(crate) reference_mir: [u8; 32],
    pub(crate) kernel_identity: [u8; 32],
    pub(crate) kernel_mir: [u8; 32],
}
impl Receipt {
    fn from_report(report: Report) -> Self {
        let binding = report.binding();
        Self {
            statement: *report.statement_identity().as_bytes(),
            generated_source: *report.generated_source_identity().as_bytes(),
            execution: *report.execution_identity().as_bytes(),
            receipt: *report.receipt_identity().as_bytes(),
            reference_identity: *binding.safe_reference_identity().as_bytes(),
            reference_mir: *binding.safe_reference_mir_hash().as_bytes(),
            kernel_identity: *binding.kernel_subject_identity().as_bytes(),
            kernel_mir: *binding.kernel_mir_hash().as_bytes(),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub(crate) enum Outcome {
    Accepted,
    Rejected,
    Accounting,
    Unwind,
}

#[derive(Debug, Serialize)]
pub(crate) enum Event {
    Retained {
        root: u32,
        receipt: Receipt,
        reserved_receipt_bytes: usize,
        work: usize,
        storage: usize,
    },
    ReplayCallback {
        root: u32,
        receipt: Receipt,
        wire: Vec<u8>,
        verifying_key: [u8; 32],
        aggregate: [u8; 32],
        source: [u8; 32],
        graph: [u8; 32],
        epoch: u64,
        work: usize,
        storage: usize,
    },
    ReplayAccepted {
        root: u32,
        receipt: Receipt,
        reserved_contract_bytes: usize,
        contract: Vec<u8>,
        work: usize,
        storage: usize,
    },
    PhaseRetained(Resources),
    PhaseFinished {
        before: Resources,
        after: Resources,
        outcome: Outcome,
    },
    PhaseDropped {
        before: Resources,
        after: Resources,
        poisoned: bool,
    },
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Observation {
    pub(crate) events: Vec<Event>,
}
thread_local! {
    static ACTIVE: RefCell<Option<Observation>> = const { RefCell::new(None) };
}
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        ACTIVE.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

/// Wrap the real source transaction and its retained replay. This never drives
/// compilation, changes a gate, injects an owner, or fabricates a success flag.
pub(crate) fn observe<R>(run: impl FnOnce() -> R) -> (R, Observation) {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(
            slot.is_none(),
            "conditional retention observer must not nest"
        );
        *slot = Some(Observation::default());
    });
    let restore = Restore;
    let result = run();
    let observed = ACTIVE.with(|slot| slot.borrow_mut().take().unwrap());
    drop(restore);
    (result, observed)
}

fn record(event: impl FnOnce() -> Event) {
    ACTIVE.with(|slot| {
        if let Some(observation) = slot.borrow_mut().as_mut() {
            observation.events.push(event());
        }
    });
}

pub(crate) fn retained(root: u32, proof: &Retained, budget: &Budget<'_>) {
    record(|| Event::Retained {
        root,
        receipt: Receipt::from_report(proof.report()),
        reserved_receipt_bytes: proof.retained_storage_v1(),
        work: budget.work(),
        storage: budget.storage(),
    });
}

pub(crate) fn replay_callback(
    root: u32,
    request: &Request<'_>,
    proof: &Execution,
    budget: &Budget<'_>,
) {
    record(|| {
        let input = request.pliron_input();
        Event::ReplayCallback {
            root,
            receipt: Receipt::from_report(proof.report()),
            wire: proof.signed_receipt_wire().to_vec(),
            verifying_key: *proof.receipt_verifying_key(),
            aggregate: *input.identity().as_bytes(),
            source: *input.source_semantic_identity().as_bytes(),
            graph: input.exact_graph_identity().digest(),
            epoch: input.graph_snapshot().epoch().sequence(),
            work: budget.work(),
            storage: budget.storage(),
        }
    });
}

pub(crate) fn replay_accepted(
    root: u32,
    proof: &Retained,
    contract: &RetainedConditionalContractV1,
    budget: &Budget<'_>,
) {
    record(|| Event::ReplayAccepted {
        root,
        receipt: Receipt::from_report(proof.report()),
        reserved_contract_bytes: contract.retained_storage_v1(),
        contract: contract.observed_bytes().to_vec(),
        work: budget.work(),
        storage: budget.storage(),
    });
}

pub(super) fn phase_retained(ledger: &OwnedBudget) {
    record(|| Event::PhaseRetained(Resources::from_owned(ledger)));
}
pub(super) fn phase_finished(before: Resources, ledger: &OwnedBudget, outcome: Outcome) {
    record(|| Event::PhaseFinished {
        before,
        after: Resources::from_owned(ledger),
        outcome,
    });
}
pub(super) fn phase_dropped(before: Resources, ledger: &OwnedBudget, poisoned: bool) {
    record(|| Event::PhaseDropped {
        before,
        after: Resources::from_owned(ledger),
        poisoned,
    });
}
