//! Workgroup-memory initialization, publication, and race verification.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
use dialect_kernel::{
    AccessKindAttr, AllocationEffectOp, MemorySpaceAttr, PipelineCreateOp, RankedAccessOp,
    RankedViewOp, is_supported_allocation_effect_contract_v1,
};
use pliron::{builtin::ops::FuncOp, context::Context, operation::Operation, value::Value};

use crate::pliron_analysis_manager::{PlironAnalysisManagerV1, PlironMemoryOrderAnalysisFailureV1};
use crate::pliron_barrier::run_pliron_barrier_convergence_check_with_analyses_v1;
use crate::pliron_invocation_trace::{
    PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceLocationV1,
};
use crate::pliron_memory_order::{PlironMemoryOrderFailureV1, PlironMemoryOrderIssueV1};
use crate::pliron_pipeline_protocol::run_pliron_pipeline_protocol_check_with_analyses_v1;
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
    let (workgroup_access_views, pipeline_views, collective_effect_sites) =
        inventory.operations().iter().fold(
            (
                HashSet::<Value>::new(),
                Vec::new(),
                HashSet::<PlironTraceLocationV1>::new(),
            ),
            |(mut accesses, mut pipelines, mut collective_effects), site| {
                let operation = Operation::get_op_dyn(site.pointer(), context);
                if let Some(access) = operation.downcast_ref::<RankedAccessOp>()
                    && access
                        .view(context)
                        .defining_op()
                        .is_some_and(|definition| {
                            Operation::get_op_dyn(definition, context)
                                .downcast_ref::<RankedViewOp>()
                                .is_some_and(|view| {
                                    view.memory_space(context) == Some(MemorySpaceAttr::Workgroup)
                                })
                        })
                {
                    accesses.insert(access.view(context));
                }
                if let Some(create) = operation.downcast_ref::<PipelineCreateOp>() {
                    pipelines.push((site.block(), site.operation(), create.view(context)));
                }
                if operation
                    .downcast_ref::<AllocationEffectOp>()
                    .is_some_and(|effect| {
                        effect.memory_space(context) == Some(MemorySpaceAttr::Workgroup)
                    })
                {
                    collective_effects.insert(PlironTraceLocationV1 {
                        block: site.block(),
                        operation: site.operation(),
                    });
                }
                (accesses, pipelines, collective_effects)
            },
        );
    if !collective_effect_sites.is_empty() {
        analyses.prepare_exact_trace(context, function);
        let traces = match analyses.exact_trace() {
            Ok(traces) => traces,
            Err(failure) => {
                return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                    detail: trace_failure_detail(failure),
                });
            }
        };
        if let Err(detail) =
            validate_collective_transpose_lifecycle_v1(traces, &collective_effect_sites)
        {
            return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete { detail });
        }
    }
    if workgroup_access_views.is_empty() {
        return PlironWorkgroupMemoryReportV1 { findings: vec![] };
    }
    if !pipeline_views.is_empty() {
        let pipeline =
            run_pliron_pipeline_protocol_check_with_analyses_v1(context, function, analyses);
        if pipeline.is_clean() {
            let certified = pipeline
                .certificates()
                .iter()
                .filter(|certificate| certificate.access_refinement_proven())
                .filter_map(|certificate| {
                    pipeline_views
                        .iter()
                        .find(|(block, operation, _)| {
                            *block == certificate.pipeline_block()
                                && *operation == certificate.pipeline_operation()
                        })
                        .map(|(_, _, view)| *view)
                })
                .collect::<HashSet<_>>();
            if workgroup_access_views.is_subset(&certified) {
                return PlironWorkgroupMemoryReportV1 { findings: vec![] };
            }
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectiveTransposePhaseV1 {
    Staged,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CollectiveTransposeEventV1 {
    location: PlironTraceLocationV1,
    access: AccessKindAttr,
    allocation_origin: u64,
    noalias_class: u64,
}

fn collective_trace_v1(trace: &PlironInvocationTraceV1) -> Vec<CollectiveTransposeEventV1> {
    trace
        .events
        .iter()
        .filter_map(|event| {
            let PlironTraceEventV1::CollectiveAllocation {
                location,
                access,
                memory_space: MemorySpaceAttr::Workgroup,
                allocation_origin,
                noalias_class,
            } = event
            else {
                return None;
            };
            Some(CollectiveTransposeEventV1 {
                location: *location,
                access: *access,
                allocation_origin: *allocation_origin,
                noalias_class: *noalias_class,
            })
        })
        .collect()
}

fn validate_collective_transpose_lifecycle_v1(
    traces: &[PlironInvocationTraceV1],
    expected_sites: &HashSet<PlironTraceLocationV1>,
) -> Result<(), String> {
    let mut workgroups = BTreeMap::<(u64, u64), Vec<&PlironInvocationTraceV1>>::new();
    for trace in traces {
        workgroups
            .entry((trace.grid, trace.workgroup))
            .or_default()
            .push(trace);
    }
    if workgroups.is_empty() {
        return Err("the collective transpose lifecycle has no exact invocation traces".to_owned());
    }

    for ((grid, workgroup), group) in workgroups {
        let lanes = group.iter().map(|trace| trace.lane).collect::<HashSet<_>>();
        let subgroups = group
            .iter()
            .map(|trace| trace.subgroup)
            .collect::<HashSet<_>>();
        if group.len() != 64
            || lanes.len() != 64
            || lanes.iter().any(|lane| *lane >= 64)
            || subgroups.len() != 1
        {
            return Err(format!(
                "grid {grid} workgroup {workgroup} does not execute one full physical Wave64 for the coordinate-free gfx950 transpose tile"
            ));
        }

        let expected_trace = collective_trace_v1(group[0]);
        let observed_sites = expected_trace
            .iter()
            .map(|event| event.location)
            .collect::<HashSet<_>>();
        if observed_sites != *expected_sites || expected_trace.len() != expected_sites.len() {
            return Err(format!(
                "grid {grid} workgroup {workgroup} does not execute every collective transpose effect exactly once"
            ));
        }
        for trace in &group[1..] {
            if collective_trace_v1(trace) != expected_trace {
                return Err(format!(
                    "grid {grid} workgroup {workgroup} has a lane-divergent collective transpose effect trace"
                ));
            }
        }

        for trace in group {
            let mut phases = HashMap::<(u64, u64), CollectiveTransposePhaseV1>::new();
            for event in &trace.events {
                match event {
                    PlironTraceEventV1::CollectiveAllocation {
                        location,
                        access: AccessKindAttr::Write,
                        memory_space: MemorySpaceAttr::Workgroup,
                        allocation_origin,
                        noalias_class,
                    } => {
                        if !is_supported_allocation_effect_contract_v1(
                            AccessKindAttr::Write,
                            MemorySpaceAttr::Workgroup,
                            *allocation_origin,
                            *noalias_class,
                        ) {
                            return Err(format!(
                                "invocation {:?} block {} op {} uses a non-reserved collective transpose identity",
                                trace.invocation, location.block, location.operation
                            ));
                        }
                        if !phases.is_empty() {
                            return Err(format!(
                                "invocation {:?} block {} op {} interleaves or duplicates a collective transpose stage",
                                trace.invocation, location.block, location.operation
                            ));
                        }
                        phases.insert(
                            (*allocation_origin, *noalias_class),
                            CollectiveTransposePhaseV1::Staged,
                        );
                    }
                    PlironTraceEventV1::Barrier {
                        execution_scope: HierarchyAttr::Workgroup,
                        memory_scope: MemoryScopeAttr::Workgroup,
                        address_space: AddressSpaceAttr::Workgroup,
                        order: MemoryOrderAttr::AcquireRelease,
                        ..
                    } => {
                        if phases
                            .values()
                            .any(|phase| *phase == CollectiveTransposePhaseV1::Published)
                        {
                            return Err(format!(
                                "invocation {:?} crosses a duplicate collective transpose publication barrier",
                                trace.invocation
                            ));
                        }
                        for phase in phases.values_mut() {
                            if *phase == CollectiveTransposePhaseV1::Staged {
                                *phase = CollectiveTransposePhaseV1::Published;
                            }
                        }
                    }
                    PlironTraceEventV1::CollectiveAllocation {
                        location,
                        access: AccessKindAttr::Read,
                        memory_space: MemorySpaceAttr::Workgroup,
                        allocation_origin,
                        noalias_class,
                    } => {
                        if !is_supported_allocation_effect_contract_v1(
                            AccessKindAttr::Read,
                            MemorySpaceAttr::Workgroup,
                            *allocation_origin,
                            *noalias_class,
                        ) {
                            return Err(format!(
                                "invocation {:?} block {} op {} uses a non-reserved collective transpose identity",
                                trace.invocation, location.block, location.operation
                            ));
                        }
                        let key = (*allocation_origin, *noalias_class);
                        if phases.len() != 1
                            || phases.remove(&key) != Some(CollectiveTransposePhaseV1::Published)
                        {
                            return Err(format!(
                                "invocation {:?} block {} op {} reads a collective transpose tile without the exact matching staged and published format",
                                trace.invocation, location.block, location.operation
                            ));
                        }
                    }
                    PlironTraceEventV1::CollectiveAllocation { location, .. } => {
                        return Err(format!(
                            "invocation {:?} block {} op {} has an unsupported collective transpose effect",
                            trace.invocation, location.block, location.operation
                        ));
                    }
                    _ => {}
                }
            }
            if !phases.is_empty() {
                return Err(format!(
                    "invocation {:?} leaves a collective transpose stage unpublished or unread",
                    trace.invocation
                ));
            }
        }
    }
    Ok(())
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
