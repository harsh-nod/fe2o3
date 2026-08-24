//! Workgroup-memory initialization, publication, and race verification.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

use dialect_gpu::{AddressSpaceAttr, HierarchyAttr};
use dialect_kernel::{AccessKindAttr, MemorySpaceAttr, RankedViewOp};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    context::Context,
    linked_list::ContainsLinkedList,
    operation::Operation,
    value::Value,
};

use crate::pliron_invocation_trace::{
    PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceLocationV1, trace_pliron_invocations_v1,
};
use crate::{
    KernelCheckPassKindV1, run_pliron_barrier_convergence_check_v1,
    run_pliron_ranked_bounds_check_v1, trace_failure_detail,
};

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

    pub fn findings(&self) -> &[PlironWorkgroupMemoryFindingV1] {
        &self.findings
    }

    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct AddressV1 {
    allocation_class: u64,
    indices: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ViewContractV1 {
    allocation_origin: u64,
    noalias_class: u64,
    signature: (u32, Vec<u64>),
    writes_memory: bool,
}

#[derive(Clone)]
struct EffectV1 {
    invocation: Vec<u64>,
    location: PlironTraceLocationV1,
    access: AccessKindAttr,
}

#[derive(Default)]
struct EffectWitnessSetV1 {
    first: Option<EffectV1>,
    alternate: Option<EffectV1>,
}

impl EffectWitnessSetV1 {
    fn different_from(&self, invocation: &[u64]) -> Option<&EffectV1> {
        self.first
            .as_ref()
            .filter(|effect| effect.invocation != invocation)
            .or_else(|| {
                self.alternate
                    .as_ref()
                    .filter(|effect| effect.invocation != invocation)
            })
    }

    fn insert(&mut self, effect: EffectV1) {
        match &self.first {
            None => self.first = Some(effect),
            Some(first) if first.invocation != effect.invocation && self.alternate.is_none() => {
                self.alternate = Some(effect);
            }
            Some(_) => {}
        }
    }
}

#[derive(Default)]
struct AddressStateV1 {
    reads: EffectWitnessSetV1,
    writes: EffectWitnessSetV1,
    atomic_reads: EffectWitnessSetV1,
    atomic_writes: EffectWitnessSetV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ConflictClassV1 {
    first: PlironTraceLocationV1,
    second: PlironTraceLocationV1,
    first_access: AccessKindAttr,
    second_access: AccessKindAttr,
}

pub fn run_pliron_workgroup_memory_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironWorkgroupMemoryReportV1 {
    if !run_pliron_ranked_bounds_check_v1(context, function).is_clean() {
        return one(PlironWorkgroupMemoryFindingV1::BoundsPrerequisiteRejected);
    }
    if !run_pliron_barrier_convergence_check_v1(context, function).is_clean() {
        return one(PlironWorkgroupMemoryFindingV1::BarrierPrerequisiteRejected);
    }
    run_pliron_workgroup_memory_check_after_prerequisites_v1(context, function)
}

pub(crate) fn run_pliron_workgroup_memory_check_after_prerequisites_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironWorkgroupMemoryReportV1 {
    if !function
        .get_region(context)
        .deref(context)
        .iter(context)
        .any(|block| {
            block.deref(context).iter(context).any(|operation| {
                let operation = Operation::get_op_dyn(operation, context);
                operation
                    .downcast_ref::<RankedViewOp>()
                    .is_some_and(|view| {
                        view.memory_space(context) == Some(MemorySpaceAttr::Workgroup)
                    })
            })
        })
    {
        return PlironWorkgroupMemoryReportV1 { findings: vec![] };
    }
    let traces = match trace_pliron_invocations_v1(context, function) {
        Ok(traces) => traces,
        Err(failure) => {
            return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                detail: trace_failure_detail(failure),
            });
        }
    };
    if traces.iter().any(|trace| {
        trace.events.iter().any(|event| {
            matches!(
                event,
                PlironTraceEventV1::Barrier {
                    execution_scope: HierarchyAttr::Subgroup,
                    address_space: AddressSpaceAttr::Workgroup,
                    ..
                }
            )
        })
    }) {
        return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
            detail: "subgroup-local LDS publication requires a retained per-subgroup epoch/read-from relation; a subgroup barrier never publishes to sibling waves".to_owned(),
        });
    }
    if traces.iter().any(|trace| {
        trace.events.iter().any(|event| {
            matches!(
                event,
                PlironTraceEventV1::Fence {
                    address_space: AddressSpaceAttr::Workgroup,
                    ..
                }
            )
        })
    }) {
        return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
            detail: "fence-mediated LDS publication requires a retained read-from/synchronizes-with relation; a non-collective fence is not a workgroup barrier".to_owned(),
        });
    }
    analyze_scoped_traces(traces)
}

pub(crate) fn require_pliron_workgroup_memory_after_prerequisites_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironWorkgroupMemoryReportV1, PlironWorkgroupMemoryCheckErrorV1> {
    let report = run_pliron_workgroup_memory_check_after_prerequisites_v1(context, function);
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

fn analyze_scoped_traces(traces: Vec<PlironInvocationTraceV1>) -> PlironWorkgroupMemoryReportV1 {
    let has_unknown_alias = match validate_workgroup_allocation_contract(&traces) {
        Ok(has_unknown_alias) => has_unknown_alias,
        Err(detail) => {
            return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete { detail });
        }
    };
    let mut by_workgroup: BTreeMap<(u64, u64), Vec<PlironInvocationTraceV1>> = BTreeMap::new();
    for trace in traces {
        by_workgroup
            .entry((trace.grid, trace.workgroup))
            .or_default()
            .push(trace);
    }
    let mut findings = Vec::new();
    for traces in by_workgroup.values() {
        let report = analyze_workgroup_traces(traces, has_unknown_alias);
        for finding in report.findings {
            if findings.len() == MAX_PLIRON_WORKGROUP_FINDINGS_V1 {
                return one(PlironWorkgroupMemoryFindingV1::FindingLimitExceeded);
            }
            findings.push(finding);
        }
    }
    PlironWorkgroupMemoryReportV1 { findings }
}

fn validate_workgroup_allocation_contract(
    traces: &[PlironInvocationTraceV1],
) -> Result<bool, String> {
    let mut contracts = HashMap::<Value, ViewContractV1>::new();
    for event in traces.iter().flat_map(|trace| &trace.events) {
        let PlironTraceEventV1::Memory {
            view,
            memory_space: MemorySpaceAttr::Workgroup,
            access,
            allocation_origin,
            noalias_class,
            view_signature,
            ..
        } = event
        else {
            continue;
        };
        let candidate = ViewContractV1 {
            allocation_origin: *allocation_origin,
            noalias_class: *noalias_class,
            signature: view_signature.clone(),
            writes_memory: access.writes_memory(),
        };
        if let Some(previous) = contracts.get_mut(view) {
            if previous.allocation_origin != candidate.allocation_origin
                || previous.noalias_class != candidate.noalias_class
                || previous.signature != candidate.signature
            {
                return Err(
                    "one workgroup view carries inconsistent allocation metadata".to_owned(),
                );
            }
            previous.writes_memory |= candidate.writes_memory;
        } else {
            contracts.insert(*view, candidate);
        }
    }

    let mut classes_by_origin = HashMap::new();
    for contract in contracts.values() {
        if contract.noalias_class != 0 && contract.allocation_origin == 0 {
            return Err(format!(
                "workgroup view claims no-alias class {} without a compiler-issued allocation origin",
                contract.noalias_class
            ));
        }
        if contract.allocation_origin != 0
            && classes_by_origin
                .insert(contract.allocation_origin, contract.noalias_class)
                .is_some_and(|previous| previous != contract.noalias_class)
        {
            return Err(format!(
                "workgroup allocation origin {} is assigned inconsistent no-alias classes",
                contract.allocation_origin
            ));
        }
    }

    let all_origins = contracts
        .values()
        .map(|contract| contract.allocation_origin)
        .collect::<HashSet<_>>();
    if contracts
        .values()
        .any(|contract| contract.noalias_class == 0)
        && contracts.values().any(|contract| contract.writes_memory)
        && all_origins.len() > 1
    {
        return Err(
            "an unknown-alias workgroup view may overlap a distinct allocation origin, but ranked IR does not retain their relative base offset"
                .to_owned(),
        );
    }
    let mut origins_by_class = HashMap::<u64, HashSet<u64>>::new();
    let mut writable_classes = HashSet::new();
    for contract in contracts.values() {
        if contract.noalias_class == 0 {
            continue;
        }
        origins_by_class
            .entry(contract.noalias_class)
            .or_default()
            .insert(contract.allocation_origin);
        if contract.writes_memory {
            writable_classes.insert(contract.noalias_class);
        }
    }
    if let Some((&class, _)) = origins_by_class
        .iter()
        .find(|(class, origins)| origins.len() > 1 && writable_classes.contains(class))
    {
        return Err(format!(
            "potentially aliasing workgroup class {class} contains writable views from distinct allocation origins, but ranked IR does not retain their relative base offset"
        ));
    }

    let has_unknown_alias = contracts
        .values()
        .any(|contract| contract.noalias_class == 0);
    let effective_class = |contract: &ViewContractV1| {
        if has_unknown_alias {
            0
        } else {
            contract.noalias_class
        }
    };
    let classes_with_writes = contracts
        .values()
        .filter_map(|contract| contract.writes_memory.then_some(effective_class(contract)))
        .collect::<HashSet<_>>();
    let mut signatures_by_class = HashMap::new();
    for contract in contracts.values() {
        let class = effective_class(contract);
        if !classes_with_writes.contains(&class) {
            continue;
        }
        if signatures_by_class
            .insert(class, contract.signature.clone())
            .is_some_and(|previous| previous != contract.signature)
        {
            return Err(
                "potentially aliasing workgroup views have incompatible element widths or rank/shapes"
                    .to_owned(),
            );
        }
    }
    Ok(has_unknown_alias)
}

fn analyze_workgroup_traces(
    traces: &[PlironInvocationTraceV1],
    has_unknown_alias: bool,
) -> PlironWorkgroupMemoryReportV1 {
    let mut published = HashSet::new();
    let mut cursors = vec![0_usize; traces.len()];
    let mut findings = Vec::new();
    let mut conflict_classes = HashSet::new();
    loop {
        let mut epoch_effects: HashMap<AddressV1, AddressStateV1> = HashMap::new();
        let mut all_writes = HashSet::new();
        let mut saw_workgroup_barrier = false;
        let mut any_event = false;
        for (trace_index, trace) in traces.iter().enumerate() {
            let mut local_writes = HashSet::new();
            while let Some(event) = trace.events.get(cursors[trace_index]) {
                cursors[trace_index] += 1;
                any_event = true;
                match event {
                    PlironTraceEventV1::Barrier {
                        execution_scope,
                        address_space,
                        ..
                    } if *execution_scope == HierarchyAttr::Workgroup
                        && *address_space == AddressSpaceAttr::Workgroup =>
                    {
                        saw_workgroup_barrier = true;
                        break;
                    }
                    PlironTraceEventV1::Barrier { .. }
                    | PlironTraceEventV1::Fence { .. }
                    | PlironTraceEventV1::TensorInstruction { .. } => {}
                    PlironTraceEventV1::Memory { memory_space, .. }
                        if *memory_space != MemorySpaceAttr::Workgroup => {}
                    PlironTraceEventV1::Memory {
                        location,
                        access,
                        indices,
                        noalias_class,
                        ..
                    } => {
                        let Some(indices) = indices.iter().copied().collect::<Option<Vec<_>>>()
                        else {
                            return one(PlironWorkgroupMemoryFindingV1::AnalysisIncomplete {
                                detail: format!(
                                    "workgroup address at block {} op {} is unresolved",
                                    location.block, location.operation
                                ),
                            });
                        };
                        let address = AddressV1 {
                            allocation_class: if has_unknown_alias { 0 } else { *noalias_class },
                            indices: indices.clone(),
                        };
                        if access.reads_memory()
                            && !published.contains(&address)
                            && !local_writes.contains(&address)
                            && let Err(report) = push_finding(
                                &mut findings,
                                PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization {
                                    invocation: trace.invocation.clone(),
                                    block: location.block,
                                    operation: location.operation,
                                    indices: indices.clone(),
                                },
                            )
                        {
                            return report;
                        }
                        let effect = EffectV1 {
                            invocation: trace.invocation.clone(),
                            location: *location,
                            access: *access,
                        };
                        let state = epoch_effects.entry(address.clone()).or_default();
                        if let Some(first) = conflicting_effect(state, &effect) {
                            let class = ConflictClassV1 {
                                first: first.location,
                                second: effect.location,
                                first_access: first.access,
                                second_access: effect.access,
                            };
                            if conflict_classes.insert(class)
                                && let Err(report) = push_finding(
                                    &mut findings,
                                    PlironWorkgroupMemoryFindingV1::ConflictingEffects {
                                        indices: indices.clone(),
                                        first_invocation: first.invocation.clone(),
                                        first_block: first.location.block,
                                        first_operation: first.location.operation,
                                        first_access: first.access,
                                        second_invocation: effect.invocation.clone(),
                                        second_block: effect.location.block,
                                        second_operation: effect.location.operation,
                                        second_access: effect.access,
                                    },
                                )
                            {
                                return report;
                            }
                        }
                        insert_effect(state, effect);
                        if access.writes_memory() {
                            local_writes.insert(address.clone());
                            all_writes.insert(address);
                        }
                    }
                }
            }
        }
        if !any_event {
            break;
        }
        if saw_workgroup_barrier {
            published.extend(all_writes);
        }
        if cursors
            .iter()
            .zip(traces)
            .all(|(cursor, trace)| *cursor == trace.events.len())
        {
            break;
        }
    }
    PlironWorkgroupMemoryReportV1 { findings }
}

fn conflicting_effect<'a>(state: &'a AddressStateV1, effect: &EffectV1) -> Option<&'a EffectV1> {
    match effect.access {
        AccessKindAttr::Read => state
            .writes
            .different_from(&effect.invocation)
            .or_else(|| state.atomic_writes.different_from(&effect.invocation)),
        AccessKindAttr::Write => state
            .writes
            .different_from(&effect.invocation)
            .or_else(|| state.reads.different_from(&effect.invocation))
            .or_else(|| state.atomic_reads.different_from(&effect.invocation))
            .or_else(|| state.atomic_writes.different_from(&effect.invocation)),
        AccessKindAttr::AtomicRead => state.writes.different_from(&effect.invocation),
        AccessKindAttr::AtomicWrite | AccessKindAttr::AtomicReadModifyWrite => state
            .writes
            .different_from(&effect.invocation)
            .or_else(|| state.reads.different_from(&effect.invocation)),
    }
}

fn insert_effect(state: &mut AddressStateV1, effect: EffectV1) {
    match effect.access {
        AccessKindAttr::Read => state.reads.insert(effect),
        AccessKindAttr::Write => state.writes.insert(effect),
        AccessKindAttr::AtomicRead => state.atomic_reads.insert(effect),
        AccessKindAttr::AtomicWrite | AccessKindAttr::AtomicReadModifyWrite => {
            state.atomic_writes.insert(effect);
        }
    }
}

fn push_finding(
    findings: &mut Vec<PlironWorkgroupMemoryFindingV1>,
    finding: PlironWorkgroupMemoryFindingV1,
) -> Result<(), PlironWorkgroupMemoryReportV1> {
    if findings.len() == MAX_PLIRON_WORKGROUP_FINDINGS_V1 {
        return Err(one(PlironWorkgroupMemoryFindingV1::FindingLimitExceeded));
    }
    findings.push(finding);
    Ok(())
}

fn one(finding: PlironWorkgroupMemoryFindingV1) -> PlironWorkgroupMemoryReportV1 {
    PlironWorkgroupMemoryReportV1 {
        findings: vec![finding],
    }
}
