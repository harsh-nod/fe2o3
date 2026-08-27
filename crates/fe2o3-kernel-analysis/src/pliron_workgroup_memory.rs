//! Workgroup-memory initialization, publication, and race verification.

use std::fmt;

use dialect_kernel::{AccessKindAttr, MemorySpaceAttr, RankedViewOp};
use pliron::{builtin::ops::FuncOp, context::Context, operation::Operation};

use crate::pliron_analysis_manager::{PlironAnalysisManagerV1, PlironMemoryOrderAnalysisFailureV1};
use crate::pliron_barrier::run_pliron_barrier_convergence_check_with_analyses_v1;
use crate::pliron_memory_order::{PlironMemoryOrderFailureV1, PlironMemoryOrderIssueV1};
use crate::pliron_ranked_bounds::run_pliron_ranked_bounds_check_with_analyses_v1;
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1, trace_failure_detail};

pub const MAX_PLIRON_WORKGROUP_FINDINGS_V1: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironWorkgroupMemoryFindingV1 {
    BoundsPrerequisiteRejected,
    BarrierPrerequisiteRejected,
    AnalysisIncomplete {
        detail: String,
    },
    ReadBeforeInitialization {
        invocation: Vec<u64>,
        block: usize,
        operation: usize,
        indices: Vec<u64>,
    },
    ConflictingEffects {
        indices: Vec<u64>,
        first_invocation: Vec<u64>,
        first_block: usize,
        first_operation: usize,
        first_access: AccessKindAttr,
        second_invocation: Vec<u64>,
        second_block: usize,
        second_operation: usize,
        second_access: AccessKindAttr,
    },
    FindingLimitExceeded,
}

impl PlironWorkgroupMemoryFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::ReadBeforeInitialization { .. } | Self::ConflictingEffects { .. } => {
                KernelCheckStatusV1::Rejected
            }
            Self::BoundsPrerequisiteRejected
            | Self::BarrierPrerequisiteRejected
            | Self::AnalysisIncomplete { .. }
            | Self::FindingLimitExceeded => KernelCheckStatusV1::Incomplete,
        }
    }
}

impl fmt::Display for PlironWorkgroupMemoryFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundsPrerequisiteRejected => formatter.write_str(
                "error[FE2O3-WORKGROUP-000]: bounds prerequisite rejected before workgroup-memory analysis",
            ),
            Self::BarrierPrerequisiteRejected => formatter.write_str(
                "error[FE2O3-WORKGROUP-000]: barrier-convergence prerequisite rejected before workgroup-memory analysis",
            ),
            Self::AnalysisIncomplete { detail } => write!(
                formatter,
                "error[FE2O3-WORKGROUP-003]: cannot prove workgroup-memory safety: {detail}",
            ),
            Self::ReadBeforeInitialization {
                invocation,
                block,
                operation,
                indices,
            } => write!(
                formatter,
                "error[FE2O3-WORKGROUP-001]: invocation {invocation:?} reads uninitialized workgroup address {indices:?} at block {block} op {operation}; failed proof: the address is not initialized by this invocation and no convergent workgroup-memory barrier published a prior write; help: initialize the address and publish it with a workgroup acquire-release barrier before the read",
            ),
            Self::ConflictingEffects {
                indices,
                first_invocation,
                first_block,
                first_operation,
                first_access,
                second_invocation,
                second_block,
                second_operation,
                second_access,
            } => write!(
                formatter,
                "error[FE2O3-WORKGROUP-002]: conflicting {first_access:?}/{second_access:?} workgroup-memory effects at address {indices:?}; invocation {first_invocation:?} block {first_block} op {first_operation} conflicts with invocation {second_invocation:?} block {second_block} op {second_operation}; help: use disjoint coordinates, a convergent workgroup barrier between epochs, or compatible atomic operations",
            ),
            Self::FindingLimitExceeded => formatter.write_str(
                "error[FE2O3-WORKGROUP-003]: workgroup-memory finding limit exceeded",
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironWorkgroupMemoryReportV1 {
    findings: Vec<PlironWorkgroupMemoryFindingV1>,
}

impl PlironWorkgroupMemoryReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::WorkgroupMemory
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }

    pub fn findings(&self) -> &[PlironWorkgroupMemoryFindingV1] {
        &self.findings
    }

    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironWorkgroupMemoryCheckErrorV1 {
    report: PlironWorkgroupMemoryReportV1,
}

impl PlironWorkgroupMemoryCheckErrorV1 {
    pub fn report(&self) -> &PlironWorkgroupMemoryReportV1 {
        &self.report
    }
}

impl fmt::Display for PlironWorkgroupMemoryCheckErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, finding) in self.report.findings.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for PlironWorkgroupMemoryCheckErrorV1 {}

pub fn run_pliron_workgroup_memory_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironWorkgroupMemoryReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    if !run_pliron_ranked_bounds_check_with_analyses_v1(context, function, &mut analyses).is_clean()
    {
        return one(PlironWorkgroupMemoryFindingV1::BoundsPrerequisiteRejected);
    }
    if !run_pliron_barrier_convergence_check_with_analyses_v1(context, function, &mut analyses)
        .is_clean()
    {
        return one(PlironWorkgroupMemoryFindingV1::BarrierPrerequisiteRejected);
    }
    run_pliron_workgroup_memory_check_with_analyses_v1(context, function, &mut analyses)
}

pub(crate) fn run_pliron_workgroup_memory_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironWorkgroupMemoryReportV1 {
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(_) => {
            return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                detail: "the bounded function inventory limit was exceeded".to_owned(),
            });
        }
    };
    if !inventory.operations().iter().any(|site| {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        operation
            .downcast_ref::<RankedViewOp>()
            .is_some_and(|view| view.memory_space(context) == Some(MemorySpaceAttr::Workgroup))
    }) {
        return PlironWorkgroupMemoryReportV1 { findings: vec![] };
    }
    analyses.prepare_memory_order(context, function);
    let memory_order = match analyses.memory_order() {
        Ok(analysis) => analysis,
        Err(failure) => {
            return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                detail: memory_order_failure_detail(failure),
            });
        }
    };
    let mut findings = Vec::new();
    for issue in memory_order.issues() {
        let finding = match issue {
            PlironMemoryOrderIssueV1::ReadBeforeInitialization {
                invocation,
                location,
                address,
            } => PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization {
                invocation: invocation.clone(),
                block: location.block(),
                operation: location.operation(),
                indices: address.indices().to_vec(),
            },
            PlironMemoryOrderIssueV1::ConflictingEffects {
                address,
                first_invocation,
                first_location,
                first_access,
                second_invocation,
                second_location,
                second_access,
            } => PlironWorkgroupMemoryFindingV1::ConflictingEffects {
                indices: address.indices().to_vec(),
                first_invocation: first_invocation.clone(),
                first_block: first_location.block(),
                first_operation: first_location.operation(),
                first_access: *first_access,
                second_invocation: second_invocation.clone(),
                second_block: second_location.block(),
                second_operation: second_location.operation(),
                second_access: *second_access,
            },
            PlironMemoryOrderIssueV1::AtomicReadFromUnresolved {
                invocation,
                location,
                address,
                detail,
            } => PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                detail: format!(
                    "invocation {invocation:?} block {} op {} cannot derive read-from for workgroup address {:?}: {detail}",
                    location.block(),
                    location.operation(),
                    address.indices(),
                ),
            },
        };
        if findings.len() == MAX_PLIRON_WORKGROUP_FINDINGS_V1 {
            return one(PlironWorkgroupMemoryFindingV1::FindingLimitExceeded);
        }
        findings.push(finding);
    }
    PlironWorkgroupMemoryReportV1 { findings }
}

fn memory_order_failure_detail(failure: PlironMemoryOrderAnalysisFailureV1) -> String {
    match failure {
        PlironMemoryOrderAnalysisFailureV1::Trace(failure) => trace_failure_detail(failure),
        PlironMemoryOrderAnalysisFailureV1::Provenance(failure) => failure.to_string(),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::UnresolvedAddress { location },
        ) => format!(
            "workgroup address at block {} op {} is unresolved",
            location.block(),
            location.operation(),
        ),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::MismatchedBarrierPhase {
                grid,
                workgroup,
                epoch,
            },
        ) => format!(
            "grid {grid} workgroup {workgroup} has mismatched workgroup-barrier participation at memory epoch {epoch}"
        ),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::SubgroupPublicationUnsupported { .. },
        ) => "subgroup-local LDS publication requires a retained per-subgroup epoch/read-from relation; a subgroup barrier never publishes to sibling waves".to_owned(),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::FencePublicationUnsupported { .. },
        ) => "fence-mediated LDS publication requires a retained read-from/synchronizes-with relation; a non-collective fence is not a workgroup barrier".to_owned(),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::VersionLimitExceeded,
        ) => "workgroup memory-version limit exceeded".to_owned(),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::IssueLimitExceeded,
        ) => "workgroup memory-order issue limit exceeded".to_owned(),
    }
}

pub(crate) fn require_pliron_workgroup_memory_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<PlironWorkgroupMemoryReportV1, PlironWorkgroupMemoryCheckErrorV1> {
    let report = run_pliron_workgroup_memory_check_with_analyses_v1(context, function, analyses);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironWorkgroupMemoryCheckErrorV1 { report })
    }
}

pub fn require_pliron_workgroup_memory_safety_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironWorkgroupMemoryReportV1, PlironWorkgroupMemoryCheckErrorV1> {
    let report = run_pliron_workgroup_memory_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironWorkgroupMemoryCheckErrorV1 { report })
    }
}

fn one(finding: PlironWorkgroupMemoryFindingV1) -> PlironWorkgroupMemoryReportV1 {
    PlironWorkgroupMemoryReportV1 {
        findings: vec![finding],
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    fn conflict() -> PlironWorkgroupMemoryFindingV1 {
        PlironWorkgroupMemoryFindingV1::ConflictingEffects {
            indices: vec![0],
            first_invocation: vec![0],
            first_block: 0,
            first_operation: 0,
            first_access: AccessKindAttr::Write,
            second_invocation: vec![1],
            second_block: 0,
            second_operation: 0,
            second_access: AccessKindAttr::Read,
        }
    }

    #[test]
    fn every_workgroup_memory_finding_has_the_shared_status() {
        let incomplete = [
            PlironWorkgroupMemoryFindingV1::BoundsPrerequisiteRejected,
            PlironWorkgroupMemoryFindingV1::BarrierPrerequisiteRejected,
            PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                detail: "unresolved".to_owned(),
            },
            PlironWorkgroupMemoryFindingV1::FindingLimitExceeded,
        ];
        for finding in incomplete {
            assert_eq!(finding.status(), KernelCheckStatusV1::Incomplete);
        }

        let rejected = [
            PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization {
                invocation: vec![0],
                block: 0,
                operation: 0,
                indices: vec![0],
            },
            conflict(),
        ];
        for finding in rejected {
            assert_eq!(finding.status(), KernelCheckStatusV1::Rejected);
        }
    }

    #[test]
    fn rejected_workgroup_finding_dominates_an_incomplete_finding() {
        let report = PlironWorkgroupMemoryReportV1 {
            findings: vec![
                PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                    detail: "unresolved".to_owned(),
                },
                conflict(),
            ],
        };
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(!report.is_clean());
        assert_eq!(
            PlironWorkgroupMemoryReportV1 { findings: vec![] }.status(),
            KernelCheckStatusV1::Clean
        );
    }
}
