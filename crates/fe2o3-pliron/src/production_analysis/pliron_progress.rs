//! Workload-neutral termination and progress checks over ranked PLIRON CFGs.
//!
//! The analysis proves only a closed canonical induction form. Other cyclic
//! control flow is rejected when nontermination is structural, or reported as
//! incomplete when a ranking function would require a stronger solver.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    fmt::Write as _,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_kernel::{
    AnalysisSplitOp, BranchArgsOp, BranchOp, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexEqualBranchArgsOp, IndexLessThanBranchArgsOp, IndexUnsignedCastOp,
};
#[cfg(test)]
use pliron::operation::verify_operation;
use pliron::{
    basic_block::BasicBlock,
    builtin::ops::FuncOp,
    common_traits::{Named, Verify},
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::{Op, OpBox},
    operation::Operation,
    printable::Printable,
};

use crate::KernelCheckStatusV1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};

/// Maximum aggregate nested blocks inventoried before recursive verification.
pub const MAX_PLIRON_PROGRESS_BLOCKS_V1: usize = 4_096;
/// Maximum aggregate successor records inventoried before recursive verification.
pub const MAX_PLIRON_PROGRESS_EDGES_V1: usize = 16_384;
/// Maximum aggregate operations below the function root.
pub const MAX_PLIRON_PROGRESS_OPERATIONS_V1: usize = 65_536;
/// Maximum aggregate regions inventoried before recursive verification.
pub const MAX_PLIRON_PROGRESS_REGIONS_V1: usize = 4_096;
/// Maximum aggregate operation operands inventoried before recursive verification.
pub const MAX_PLIRON_PROGRESS_OPERANDS_V1: usize = 65_536;
/// Maximum aggregate operation results inventoried before recursive verification.
pub const MAX_PLIRON_PROGRESS_RESULTS_V1: usize = 65_536;
/// Maximum aggregate operation and block attribute entries.
pub const MAX_PLIRON_PROGRESS_ATTRIBUTES_V1: usize = 65_536;
/// Maximum aggregate block arguments inventoried before recursive verification.
pub const MAX_PLIRON_PROGRESS_BLOCK_ARGUMENTS_V1: usize = 65_536;
/// Maximum operation nesting depth admitted to PLIRON's recursive verifier.
pub const MAX_PLIRON_PROGRESS_NESTING_DEPTH_V1: usize = 128;
/// Maximum cumulative work units for inventory, graph construction, and analysis.
pub const MAX_PLIRON_PROGRESS_WORK_UNITS_V1: usize = 262_144;
const MAX_PLIRON_PROGRESS_DIAGNOSTIC_BYTES_V1: usize = 1_024;
// Pinned Value::id is `v` plus at most 20 u64 digits. This bounds requested
// String capacity, including formatter growth, not allocator rounding.
const NUMERIC_PROGRESS_LABEL_BYTES_V1: usize = 64;

include!("pliron_progress/resources_v1.rs");
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironProgressCertificateV1 {
    header: usize,
    body: usize,
    exit: usize,
    induction: String,
    bound: String,
    step: u64,
}

impl PlironProgressCertificateV1 {
    pub const fn header(&self) -> usize {
        self.header
    }
    pub const fn body(&self) -> usize {
        self.body
    }
    pub const fn exit(&self) -> usize {
        self.exit
    }
    pub fn induction(&self) -> &str {
        &self.induction
    }
    pub fn bound(&self) -> &str {
        &self.bound
    }
    pub const fn step(&self) -> u64 {
        self.step
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironProgressFindingV1 {
    StructuralPrerequisiteRejected {
        reason: String,
    },
    ResourceLimitExceeded {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    NonTerminatingCycle {
        blocks: Vec<usize>,
        reason: &'static str,
        counterexample: String,
    },
    ProgressIncomplete {
        blocks: Vec<usize>,
        reason: &'static str,
    },
}

impl PlironProgressFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::NonTerminatingCycle { .. } | Self::StructuralPrerequisiteRejected { .. } => {
                KernelCheckStatusV1::Rejected
            }
            Self::ResourceLimitExceeded { .. } | Self::ProgressIncomplete { .. } => {
                KernelCheckStatusV1::Incomplete
            }
        }
    }
}

impl fmt::Display for PlironProgressFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StructuralPrerequisiteRejected { reason } => write!(
                formatter,
                "error[FE2O3-PROGRESS-000]: PLIRON structural verification failed before progress analysis: {reason}; help: repair the malformed operation, type, SSA operand, block argument, or CFG edge"
            ),
            Self::ResourceLimitExceeded {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "error[FE2O3-PROGRESS-003]: progress analysis has {actual} {resource}, exceeding limit {limit}; help: simplify or split the kernel so its bounded structural inventory fits the reported resource limit"
            ),
            Self::NonTerminatingCycle {
                blocks,
                reason,
                counterexample,
            } => write!(
                formatter,
                "error[FE2O3-PROGRESS-001]: control-flow cycle {blocks:?} does not terminate: {reason}; counterexample: {counterexample}; help: add an exit controlled by a finite induction variable and advance it on every backedge"
            ),
            Self::ProgressIncomplete { blocks, reason } => write!(
                formatter,
                "error[FE2O3-PROGRESS-002]: termination proof for control-flow cycle {blocks:?} is incomplete: {reason}; help: express the loop as `i < bound` with a positive constant backedge step and a statically proved no-wrap update, or provide a future supported ranking-function contract"
            ),
        }
    }
}

#[derive(Default)]
struct ProgressWorkBudgetV1 {
    work_units: usize,
}

impl ProgressWorkBudgetV1 {
    fn charge(&mut self, units: usize) -> Result<(), PlironProgressFindingV1> {
        let actual = self.work_units.saturating_add(units);
        if actual > MAX_PLIRON_PROGRESS_WORK_UNITS_V1 {
            return Err(PlironProgressFindingV1::ResourceLimitExceeded {
                resource: "work units",
                actual,
                limit: MAX_PLIRON_PROGRESS_WORK_UNITS_V1,
            });
        }
        self.work_units = actual;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironProgressReportV1 {
    findings: Vec<PlironProgressFindingV1>,
    certificates: Vec<PlironProgressCertificateV1>,
}

impl PlironProgressReportV1 {
    #[cfg(test)]
    pub(super) fn validation_payload_test_report_v1() -> Self {
        let mut induction = String::with_capacity(32);
        induction.push_str("iv");
        let mut bound = String::with_capacity(64);
        bound.push_str("limit");
        let mut certificates = Vec::with_capacity(3);
        certificates.push(PlironProgressCertificateV1 {
            header: 7,
            body: 11,
            exit: 19,
            induction,
            bound,
            step: 3,
        });
        Self {
            findings: Vec::new(),
            certificates,
        }
    }

    #[cfg(test)]
    pub(super) fn set_validation_findings_for_test_v1(&mut self, nonempty: bool) {
        self.findings.reserve_exact(1);
        if nonempty {
            self.findings
                .push(PlironProgressFindingV1::ResourceLimitExceeded {
                    resource: "test finding",
                    actual: 2,
                    limit: 1,
                });
        }
    }

    pub(super) fn has_empty_validation_findings_v1(&self) -> bool {
        self.findings.is_empty() && self.findings.capacity() == 0
    }

    pub(super) fn validation_certificate_capacity_v1(&self) -> usize {
        self.certificates.capacity()
    }

    pub(super) fn validation_text_storage_v1(
        &self,
    ) -> Result<(usize, usize), super::pliron_resource_envelope::ProductionAnalysisResourceLimitV1>
    {
        use super::pliron_report_payload_receipt::payload_sum_v1;
        let mut owned = 0;
        let mut copied = 0;
        for certificate in &self.certificates {
            owned = payload_sum_v1(&[
                owned,
                certificate.induction.capacity(),
                certificate.bound.capacity(),
            ])?;
            copied =
                payload_sum_v1(&[copied, certificate.induction.len(), certificate.bound.len()])?;
        }
        Ok((owned, copied))
    }

    pub(super) fn try_clone_validation_payload_v1(
        &self,
    ) -> Result<Self, super::pliron_resource_envelope::ProductionAnalysisResourceLimitV1> {
        use super::pliron_report_payload_receipt::{
            payload_limit_v1, try_clone_payload_string_v1, try_reserve_payload_v1,
        };
        if !self.has_empty_validation_findings_v1() {
            return Err(payload_limit_v1("report payload shape changed"));
        }
        let mut certificates = Vec::new();
        try_reserve_payload_v1(&mut certificates, self.certificates.len())?;
        for certificate in &self.certificates {
            certificates.push(PlironProgressCertificateV1 {
                header: certificate.header,
                body: certificate.body,
                exit: certificate.exit,
                induction: try_clone_payload_string_v1(&certificate.induction)?,
                bound: try_clone_payload_string_v1(&certificate.bound)?,
                step: certificate.step,
            });
        }
        Ok(Self {
            findings: Vec::new(),
            certificates,
        })
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }
    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }
    pub fn findings(&self) -> &[PlironProgressFindingV1] {
        &self.findings
    }
    pub fn certificates(&self) -> &[PlironProgressCertificateV1] {
        &self.certificates
    }
    pub const fn grants_launch_or_liveness_authority(&self) -> bool {
        false
    }
    pub(crate) fn clean() -> Self {
        Self {
            findings: Vec::new(),
            certificates: Vec::new(),
        }
    }
}

include!("pliron_progress/nested_loops_v1.rs");
include!("pliron_progress/scoped_input_v67.rs");
include!("pliron_progress/scoped_resource_v67_tests.rs");
include!("pliron_progress/structural_inventory_v1.rs");
include!("pliron_progress/loop_graph_v1.rs");
include!("pliron_progress/resource_tests.rs");
