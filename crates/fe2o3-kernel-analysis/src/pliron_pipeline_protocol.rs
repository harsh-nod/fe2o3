//! Epoch-aware verification for target-neutral staged workgroup pipelines.
//!
//! Constant schedules are interpreted exactly. Runtime-bounded canonical
//! loops are proved from a finite epoch-window invariant, so verification
//! never unrolls a dynamic trip count.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use dialect_kernel::{
    AccessKindAttr, AnalysisSplitOp, BranchArgsOp, DeterministicJoinOp, IndexBinaryKindAttr,
    IndexBinaryOp, IndexConstantOp, IndexEqualBranchArgsOp, IndexLessThanBranchArgsOp,
    IndexUnsignedCastOp, PipelineCreateOp, PipelineEventKindAttr, PipelineEventOp, RankedAccessOp,
    RankedViewOp,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::ops::FuncOp,
    common_traits::Named,
    context::{Context, Ptr},
    operation::Operation,
    value::Value,
};

use crate::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::pliron_function_inventory::{BoundedPlironFunctionInventoryV1, PlironOperationSiteV1};
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1};

pub const MAX_PLIRON_PIPELINE_FINDINGS_V1: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironPipelineProtocolFindingV1 {
    AnalysisIncomplete {
        detail: String,
    },
    OrphanEvent {
        block: usize,
        operation: usize,
    },
    AliasedStorage {
        first_block: usize,
        first_operation: usize,
        second_block: usize,
        second_operation: usize,
    },
    InvalidSchedule {
        pipeline_block: usize,
        pipeline_operation: usize,
        event_block: Option<usize>,
        event_operation: Option<usize>,
        detail: String,
    },
    FindingLimitExceeded,
}

impl PlironPipelineProtocolFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::AnalysisIncomplete { .. } | Self::FindingLimitExceeded => {
                KernelCheckStatusV1::Incomplete
            }
            Self::OrphanEvent { .. }
            | Self::AliasedStorage { .. }
            | Self::InvalidSchedule { .. } => KernelCheckStatusV1::Rejected,
        }
    }
}

impl fmt::Display for PlironPipelineProtocolFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnalysisIncomplete { detail } => write!(
                formatter,
                "error[FE2O3-PIPELINE-003]: cannot prove staged pipeline safety: {detail}"
            ),
            Self::OrphanEvent { block, operation } => write!(
                formatter,
                "error[FE2O3-PIPELINE-001]: pipeline event at block {block} op {operation} does not use a pipeline created by kernel.pipeline_create; help: bind the event to the pipeline that owns the staged workgroup view"
            ),
            Self::AliasedStorage {
                first_block,
                first_operation,
                second_block,
                second_operation,
            } => write!(
                formatter,
                "error[FE2O3-PIPELINE-001]: pipeline at block {second_block} op {second_operation} may alias staged storage already owned by pipeline block {first_block} op {first_operation}; help: use one lifecycle for that view or provide compiler-derived disjoint allocation provenance"
            ),
            Self::InvalidSchedule {
                pipeline_block,
                pipeline_operation,
                event_block,
                event_operation,
                detail,
            } => {
                write!(
                    formatter,
                    "error[FE2O3-PIPELINE-001]: invalid staged pipeline created at block {pipeline_block} op {pipeline_operation}"
                )?;
                if let (Some(block), Some(operation)) = (event_block, event_operation) {
                    write!(formatter, ", detected at block {block} op {operation}")?;
                }
                write!(
                    formatter,
                    ": {detail}; help: stage then commit each future epoch, wait then consume or discard it, release it exactly once, and use slot = epoch % buffer_count"
                )
            }
            Self::FindingLimitExceeded => formatter
                .write_str("error[FE2O3-PIPELINE-003]: staged-pipeline finding limit exceeded"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochAwareLoopSummaryV1 {
    prologue: usize,
    prologue_blocks: Vec<usize>,
    header: usize,
    body: Vec<usize>,
    exit: usize,
    drain_blocks: Vec<usize>,
    induction: String,
    bound: String,
    step: u64,
    prefetched_epochs: u32,
    live_epoch_window: u32,
    drained_epochs: u32,
}

impl EpochAwareLoopSummaryV1 {
    pub const fn prologue(&self) -> usize {
        self.prologue
    }
    pub const fn header(&self) -> usize {
        self.header
    }
    pub fn prologue_blocks(&self) -> &[usize] {
        &self.prologue_blocks
    }
    pub fn body(&self) -> &[usize] {
        &self.body
    }
    pub const fn exit(&self) -> usize {
        self.exit
    }
    pub fn drain_blocks(&self) -> &[usize] {
        &self.drain_blocks
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
    pub const fn prefetched_epochs(&self) -> u32 {
        self.prefetched_epochs
    }
    pub const fn live_epoch_window(&self) -> u32 {
        self.live_epoch_window
    }
    pub const fn drained_epochs(&self) -> u32 {
        self.drained_epochs
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironPipelineProtocolCertificateV1 {
    pipeline_block: usize,
    pipeline_operation: usize,
    buffers: u32,
    prefetch_distance: u32,
    dynamic_loop: Option<EpochAwareLoopSummaryV1>,
    concrete_epochs: usize,
    staged_writes: usize,
    consuming_reads: usize,
    access_refinement_proven: bool,
}

impl PlironPipelineProtocolCertificateV1 {
    pub const fn pipeline_block(&self) -> usize {
        self.pipeline_block
    }
    pub const fn pipeline_operation(&self) -> usize {
        self.pipeline_operation
    }
    pub const fn buffers(&self) -> u32 {
        self.buffers
    }
    pub const fn prefetch_distance(&self) -> u32 {
        self.prefetch_distance
    }
    pub const fn dynamic_loop(&self) -> Option<&EpochAwareLoopSummaryV1> {
        self.dynamic_loop.as_ref()
    }
    pub const fn concrete_epochs(&self) -> usize {
        self.concrete_epochs
    }
    pub const fn staged_writes(&self) -> usize {
        self.staged_writes
    }
    pub const fn consuming_reads(&self) -> usize {
        self.consuming_reads
    }
    pub const fn access_refinement_proven(&self) -> bool {
        self.access_refinement_proven
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironPipelineProtocolReportV1 {
    findings: Vec<PlironPipelineProtocolFindingV1>,
    certificates: Vec<PlironPipelineProtocolCertificateV1>,
}

impl PlironPipelineProtocolReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::PipelineProtocol
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
    pub fn findings(&self) -> &[PlironPipelineProtocolFindingV1] {
        &self.findings
    }
    pub fn certificates(&self) -> &[PlironPipelineProtocolCertificateV1] {
        &self.certificates
    }
    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironPipelineProtocolCheckErrorV1 {
    report: PlironPipelineProtocolReportV1,
}

impl PlironPipelineProtocolCheckErrorV1 {
    pub fn report(&self) -> &PlironPipelineProtocolReportV1 {
        &self.report
    }
}

impl fmt::Display for PlironPipelineProtocolCheckErrorV1 {
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

impl std::error::Error for PlironPipelineProtocolCheckErrorV1 {}

pub fn run_pliron_pipeline_protocol_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironPipelineProtocolReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_pliron_pipeline_protocol_check_with_analyses_v1(context, function, &mut analyses)
}

pub fn require_pliron_pipeline_protocol_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironPipelineProtocolReportV1, PlironPipelineProtocolCheckErrorV1> {
    let report = run_pliron_pipeline_protocol_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironPipelineProtocolCheckErrorV1 { report })
    }
}

pub(crate) fn require_pliron_pipeline_protocol_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<PlironPipelineProtocolReportV1, PlironPipelineProtocolCheckErrorV1> {
    let report = run_pliron_pipeline_protocol_check_with_analyses_v1(context, function, analyses);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironPipelineProtocolCheckErrorV1 { report })
    }
}

pub(crate) fn run_pliron_pipeline_protocol_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironPipelineProtocolReportV1 {
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(_) => {
            return report(PlironPipelineProtocolFindingV1::AnalysisIncomplete {
                detail: "the bounded function inventory limit was exceeded".to_owned(),
            });
        }
    };
    let loop_discovery = discover_epoch_loops(context, &inventory);
    let uniform_roots = function
        .get_entry_block(context)
        .deref(context)
        .arguments()
        .collect::<HashSet<_>>();
    let mut creates = Vec::new();
    let mut create_owners = HashSet::new();
    let mut first_storage_owner = None;
    let mut unknown_class_owner = None;
    let mut view_owners = HashMap::<Value, PlironOperationSiteV1>::new();
    let mut origin_owners = HashMap::<u64, PlironOperationSiteV1>::new();
    let mut class_owners = HashMap::<u64, PlironOperationSiteV1>::new();
    let mut findings = Vec::new();
    for site in inventory.operations() {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let Some(create) = operation.downcast_ref::<PipelineCreateOp>() else {
            continue;
        };
        let view = create.view(context);
        let (origin, noalias_class) = pipeline_storage_contract(context, view);
        let first = view_owners
            .get(&view)
            .or_else(|| origin.and_then(|origin| origin_owners.get(&origin)))
            .or_else(|| noalias_class.and_then(|class| class_owners.get(&class)))
            .or(match noalias_class {
                Some(_) => unknown_class_owner.as_ref(),
                None => first_storage_owner.as_ref(),
            })
            .copied();
        if let Some(first) = first {
            push_finding(
                &mut findings,
                PlironPipelineProtocolFindingV1::AliasedStorage {
                    first_block: first.block(),
                    first_operation: first.operation(),
                    second_block: site.block(),
                    second_operation: site.operation(),
                },
            );
        }
        first_storage_owner.get_or_insert(*site);
        view_owners.entry(view).or_insert(*site);
        if let Some(origin) = origin {
            origin_owners.entry(origin).or_insert(*site);
        }
        if let Some(noalias_class) = noalias_class {
            class_owners.entry(noalias_class).or_insert(*site);
        } else {
            unknown_class_owner.get_or_insert(*site);
        }
        creates.push((site.pointer(), *site, create.pipeline_type(context), view));
        create_owners.insert(site.pointer());
    }
    let mut events = HashMap::<Ptr<Operation>, Vec<EventSiteV1>>::new();
    for site in inventory.operations() {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let Some(event) = operation.downcast_ref::<PipelineEventOp>() else {
            continue;
        };
        let Some(owner) = event.pipeline(context).defining_op() else {
            push_finding(
                &mut findings,
                PlironPipelineProtocolFindingV1::OrphanEvent {
                    block: site.block(),
                    operation: site.operation(),
                },
            );
            continue;
        };
        if !create_owners.contains(&owner) {
            push_finding(
                &mut findings,
                PlironPipelineProtocolFindingV1::OrphanEvent {
                    block: site.block(),
                    operation: site.operation(),
                },
            );
            continue;
        }
        events.entry(owner).or_default().push(EventSiteV1 {
            site: *site,
            kind: event.kind(context),
            epoch: event.epoch(context),
            slot: event.slot(context),
        });
    }
    let mut accesses = HashMap::<Value, Vec<AccessSiteV1>>::new();
    for site in inventory.operations() {
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let Some(access) = operation.downcast_ref::<RankedAccessOp>() else {
            continue;
        };
        let Some(kind) = access.kind(context) else {
            continue;
        };
        let indices = access.indices(context);
        let Some(slot) = indices.first().copied() else {
            continue;
        };
        accesses
            .entry(access.view(context))
            .or_default()
            .push(AccessSiteV1 {
                site: *site,
                kind,
                slot,
                indices,
            });
    }
    let mut certificates = Vec::new();
    for (pointer, site, pipeline_type, view) in creates {
        let Some(pipeline_type) = pipeline_type else {
            push_finding(
                &mut findings,
                invalid(site, None, "pipeline configuration type is malformed"),
            );
            continue;
        };
        let pipeline_type = pipeline_type.deref(context);
        let schedule = events.remove(&pointer).unwrap_or_default();
        if schedule.is_empty() {
            push_finding(
                &mut findings,
                invalid(site, None, "pipeline has no lifecycle events"),
            );
            continue;
        }
        match verify_one_pipeline(
            context,
            &loop_discovery.dominators,
            site,
            pipeline_type.buffers(),
            pipeline_type.prefetch_distance(),
            &schedule,
            accesses.get(&view).map_or(&[], Vec::as_slice),
            &loop_discovery.loops,
            &uniform_roots,
        ) {
            Ok(certificate) => certificates.push(certificate),
            Err(finding) => push_finding(&mut findings, finding),
        }
    }
    PlironPipelineProtocolReportV1 {
        findings,
        certificates,
    }
}

fn pipeline_storage_contract(context: &Context, view: Value) -> (Option<u64>, Option<u64>) {
    let contract = view.defining_op().and_then(|definition| {
        Operation::get_op_dyn(definition, context)
            .downcast_ref::<RankedViewOp>()
            .map(|view| (view.allocation_origin(context), view.noalias_class(context)))
    });
    contract.map_or((None, None), |(origin, noalias_class)| {
        (
            origin.filter(|origin| *origin != 0),
            noalias_class.filter(|class| *class != 0),
        )
    })
}

#[derive(Clone, Copy)]
struct EventSiteV1 {
    site: PlironOperationSiteV1,
    kind: Option<PipelineEventKindAttr>,
    epoch: Value,
    slot: Value,
}

#[derive(Clone)]
struct AccessSiteV1 {
    site: PlironOperationSiteV1,
    kind: AccessKindAttr,
    slot: Value,
    indices: Vec<Value>,
}

#[derive(Clone)]
struct CanonicalEpochLoopV1 {
    prologue: Vec<usize>,
    header: usize,
    body: Vec<usize>,
    body_members: HashSet<usize>,
    drain: Vec<usize>,
    inductions: HashMap<usize, Value>,
    header_induction: Value,
    bound: Value,
}

struct EpochLoopDiscoveryV1 {
    loops: Vec<CanonicalEpochLoopV1>,
    dominators: Vec<HashSet<usize>>,
}

#[allow(clippy::too_many_arguments)]
fn verify_one_pipeline(
    context: &Context,
    dominators: &[HashSet<usize>],
    pipeline: PlironOperationSiteV1,
    buffers: u32,
    distance: u32,
    schedule: &[EventSiteV1],
    accesses: &[AccessSiteV1],
    loops: &[CanonicalEpochLoopV1],
    uniform_roots: &HashSet<Value>,
) -> Result<PlironPipelineProtocolCertificateV1, PlironPipelineProtocolFindingV1> {
    let mut containing = loops
        .iter()
        .filter(|summary| {
            schedule
                .iter()
                .any(|event| summary.body_members.contains(&event.site.block()))
        })
        .collect::<Vec<_>>();
    if containing.is_empty() {
        let (epochs, staged_writes, consuming_reads, access_refinement_proven) =
            verify_concrete_schedule(context, pipeline, buffers, schedule, accesses)?;
        return Ok(PlironPipelineProtocolCertificateV1 {
            pipeline_block: pipeline.block(),
            pipeline_operation: pipeline.operation(),
            buffers,
            prefetch_distance: distance,
            dynamic_loop: None,
            concrete_epochs: epochs,
            staged_writes,
            consuming_reads,
            access_refinement_proven,
        });
    }
    containing.sort_by_key(|summary| summary.body.len());
    let Some(summary) = containing.first().copied() else {
        unreachable!("the empty case returned above")
    };
    if containing[1..].iter().any(|outer| {
        !summary
            .body_members
            .iter()
            .all(|block| outer.body_members.contains(block))
    }) {
        return Err(invalid(
            pipeline,
            None,
            "one pipeline lifecycle crosses non-nested dynamic loops",
        ));
    }
    if summary.body_members.contains(&pipeline.block())
        || summary.header == pipeline.block()
        || !pipeline_creation_dominates_schedule(dominators, pipeline, schedule, accesses)
    {
        return Err(invalid(
            pipeline,
            None,
            "the dynamic pipeline creation does not execute once and dominate its complete lifecycle",
        ));
    }
    let mut prologue_blocks = Vec::with_capacity(summary.prologue.len() + 1);
    prologue_blocks.push(pipeline.block());
    prologue_blocks.extend(
        summary
            .prologue
            .iter()
            .copied()
            .filter(|block| *block != pipeline.block()),
    );
    if !is_uniform_value(context, summary.bound, uniform_roots, &mut HashSet::new()) {
        return Err(invalid(
            pipeline,
            None,
            "the runtime loop bound is not proved workgroup-uniform",
        ));
    }
    let (staged_writes, consuming_reads, access_refinement_proven) = verify_dynamic_schedule(
        context, pipeline, buffers, distance, schedule, accesses, summary,
    )?;
    Ok(PlironPipelineProtocolCertificateV1 {
        pipeline_block: pipeline.block(),
        pipeline_operation: pipeline.operation(),
        buffers,
        prefetch_distance: distance,
        dynamic_loop: Some(EpochAwareLoopSummaryV1 {
            prologue: pipeline.block(),
            prologue_blocks,
            header: summary.header,
            body: summary.body.clone(),
            exit: summary.drain[0],
            drain_blocks: summary.drain.clone(),
            induction: summary.header_induction.unique_name(context).to_string(),
            bound: summary.bound.unique_name(context).to_string(),
            step: 1,
            prefetched_epochs: distance,
            live_epoch_window: distance + 1,
            drained_epochs: distance,
        }),
        concrete_epochs: 0,
        staged_writes,
        consuming_reads,
        access_refinement_proven,
    })
}

fn pipeline_creation_dominates_schedule(
    dominators: &[HashSet<usize>],
    pipeline: PlironOperationSiteV1,
    schedule: &[EventSiteV1],
    accesses: &[AccessSiteV1],
) -> bool {
    schedule
        .iter()
        .map(|event| event.site)
        .chain(accesses.iter().map(|access| access.site))
        .all(|site| {
            if site.block() == pipeline.block() {
                return site.operation() > pipeline.operation();
            }
            dominators
                .get(site.block())
                .is_some_and(|target| target.contains(&pipeline.block()))
        })
}

fn is_uniform_value(
    context: &Context,
    value: Value,
    uniform_roots: &HashSet<Value>,
    visiting: &mut HashSet<Value>,
) -> bool {
    if uniform_roots.contains(&value) {
        return true;
    }
    if !visiting.insert(value) {
        return false;
    }
    let uniform = value.defining_op().is_some_and(|definition| {
        let operation = Operation::get_op_dyn(definition, context);
        if operation.downcast_ref::<IndexConstantOp>().is_some() {
            true
        } else if let Some(binary) = operation.downcast_ref::<IndexBinaryOp>() {
            is_uniform_value(context, binary.lhs(context), uniform_roots, visiting)
                && is_uniform_value(context, binary.rhs(context), uniform_roots, visiting)
        } else if let Some(cast) = operation.downcast_ref::<IndexUnsignedCastOp>() {
            is_uniform_value(context, cast.source(context), uniform_roots, visiting)
        } else if let Some(join) = operation.downcast_ref::<DeterministicJoinOp>() {
            let dependencies = join.dependencies(context);
            !dependencies.is_empty()
                && dependencies.into_iter().all(|dependency| {
                    is_uniform_value(context, dependency, uniform_roots, visiting)
                })
        } else {
            false
        }
    });
    visiting.remove(&value);
    uniform
}

fn verify_dynamic_schedule(
    context: &Context,
    pipeline: PlironOperationSiteV1,
    buffers: u32,
    distance: u32,
    schedule: &[EventSiteV1],
    accesses: &[AccessSiteV1],
    summary: &CanonicalEpochLoopV1,
) -> Result<(usize, usize, bool), PlironPipelineProtocolFindingV1> {
    let mut prologue = Vec::new();
    let mut body = Vec::new();
    let mut drain = Vec::new();
    let prologue_start = summary
        .prologue
        .iter()
        .position(|block| *block == pipeline.block())
        .unwrap_or(0);
    let prologue_positions = summary.prologue[prologue_start..]
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<HashMap<_, _>>();
    let body_positions = summary
        .body
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<HashMap<_, _>>();
    let drain_positions = summary
        .drain
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<HashMap<_, _>>();
    for event in schedule {
        if let Some(position) = prologue_positions.get(&event.site.block()).copied() {
            prologue.push((position, *event));
        } else if let Some(position) = body_positions.get(&event.site.block()).copied() {
            body.push((position, *event));
        } else if let Some(position) = drain_positions.get(&event.site.block()).copied() {
            drain.push((position, *event));
        } else {
            return Err(invalid(
                pipeline,
                Some(event.site),
                "event is outside the canonical prologue, loop body, or drain block",
            ));
        }
    }
    prologue.sort_by_key(|(position, event)| (*position, event.site.operation()));
    body.sort_by_key(|(position, event)| (*position, event.site.operation()));
    drain.sort_by_key(|(position, event)| (*position, event.site.operation()));
    let prologue = prologue
        .into_iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let drain = drain
        .into_iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();

    let expected_prologue = usize::try_from(distance).unwrap_or(usize::MAX) * 2;
    if prologue.len() != expected_prologue {
        return Err(invalid(
            pipeline,
            prologue.first().map(|event| event.site),
            &format!(
                "dynamic pipeline requires {expected_prologue} prologue events for prefetch distance {distance}, found {}",
                prologue.len()
            ),
        ));
    }
    for epoch in 0..u64::from(distance) {
        require_event(
            context,
            pipeline,
            &prologue[(epoch as usize) * 2],
            PipelineEventKindAttr::Stage,
            EpochExpectationV1::Constant(epoch),
            buffers,
        )?;
        require_event(
            context,
            pipeline,
            &prologue[(epoch as usize) * 2 + 1],
            PipelineEventKindAttr::Commit,
            EpochExpectationV1::Constant(epoch),
            buffers,
        )?;
    }

    let body = body.into_iter().map(|(_, event)| event).collect::<Vec<_>>();
    if body.len() != 5 {
        return Err(invalid(
            pipeline,
            body.first().map(|event| event.site),
            &format!(
                "dynamic loop body requires exactly five lifecycle events, found {}",
                body.len()
            ),
        ));
    }
    let body_induction = |event: &EventSiteV1| {
        summary
            .inductions
            .get(&event.site.block())
            .copied()
            .expect("body events are classified only in summarized blocks")
    };
    let expected = [
        (PipelineEventKindAttr::Stage, u64::from(distance)),
        (PipelineEventKindAttr::Commit, u64::from(distance)),
        (PipelineEventKindAttr::Wait, 0),
        (PipelineEventKindAttr::Consume, 0),
        (PipelineEventKindAttr::Release, 0),
    ];
    for (event, (kind, offset)) in body.iter().zip(expected) {
        require_event(
            context,
            pipeline,
            event,
            kind,
            EpochExpectationV1::Offset(body_induction(event), offset),
            buffers,
        )?;
    }

    let expected_drain = usize::try_from(distance).unwrap_or(usize::MAX) * 3;
    if drain.len() != expected_drain {
        return Err(invalid(
            pipeline,
            drain.first().map(|event| event.site),
            &format!(
                "dynamic pipeline requires {expected_drain} drain events for prefetch distance {distance}, found {}",
                drain.len()
            ),
        ));
    }
    for offset in 0..u64::from(distance) {
        for (inner, kind) in [
            PipelineEventKindAttr::Wait,
            PipelineEventKindAttr::Discard,
            PipelineEventKindAttr::Release,
        ]
        .into_iter()
        .enumerate()
        {
            require_event(
                context,
                pipeline,
                &drain[(offset as usize) * 3 + inner],
                kind,
                EpochExpectationV1::Offset(summary.bound, offset),
                buffers,
            )?;
        }
    }
    verify_dynamic_accesses(context, pipeline, buffers, schedule, accesses, summary)
}

#[derive(Clone, Copy)]
enum EpochExpectationV1 {
    Constant(u64),
    Offset(Value, u64),
}

fn require_event(
    context: &Context,
    pipeline: PlironOperationSiteV1,
    event: &EventSiteV1,
    kind: PipelineEventKindAttr,
    epoch: EpochExpectationV1,
    buffers: u32,
) -> Result<(), PlironPipelineProtocolFindingV1> {
    if event.kind != Some(kind) {
        return Err(invalid(
            pipeline,
            Some(event.site),
            &format!("expected {kind:?}, found {:?}", event.kind),
        ));
    }
    let epoch_matches = match epoch {
        EpochExpectationV1::Constant(expected) => {
            index_constant(context, event.epoch) == Some(expected)
        }
        EpochExpectationV1::Offset(base, offset) => {
            index_offset(context, event.epoch, base) == Some(offset)
        }
    };
    if !epoch_matches {
        return Err(invalid(
            pipeline,
            Some(event.site),
            "event uses the wrong epoch for its pipeline phase",
        ));
    }
    if !slot_is_epoch_modulo(context, event.slot, event.epoch, buffers) {
        return Err(invalid(
            pipeline,
            Some(event.site),
            &format!("slot is not epoch % {buffers}"),
        ));
    }
    Ok(())
}

#[derive(Clone)]
enum DynamicPipelineActionV1 {
    Event(EventSiteV1),
    Access(AccessSiteV1),
}

impl DynamicPipelineActionV1 {
    const fn site(&self) -> PlironOperationSiteV1 {
        match self {
            Self::Event(event) => event.site,
            Self::Access(access) => access.site,
        }
    }
}

fn verify_dynamic_accesses(
    context: &Context,
    pipeline: PlironOperationSiteV1,
    buffers: u32,
    schedule: &[EventSiteV1],
    accesses: &[AccessSiteV1],
    summary: &CanonicalEpochLoopV1,
) -> Result<(usize, usize, bool), PlironPipelineProtocolFindingV1> {
    if accesses.is_empty() {
        return Ok((0, 0, true));
    }
    let prologue_start = summary
        .prologue
        .iter()
        .position(|block| *block == pipeline.block())
        .unwrap_or(0);
    let mut ordered_blocks = summary.prologue[prologue_start..].to_vec();
    ordered_blocks.extend(summary.body.iter().copied());
    ordered_blocks.extend(summary.drain.iter().copied());
    let positions = ordered_blocks
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<HashMap<_, _>>();
    let order_key = |site: PlironOperationSiteV1| {
        positions
            .get(&site.block())
            .copied()
            .map(|position| (position, site.operation()))
    };
    let mut actions = Vec::with_capacity(accesses.len() + schedule.len());
    for access in accesses {
        if order_key(access.site).is_none() {
            return Err(invalid(
                pipeline,
                Some(access.site),
                "pipeline-owned storage is accessed outside the canonical prologue, loop body, or drain block",
            ));
        }
        actions.push(DynamicPipelineActionV1::Access(access.clone()));
    }
    for event in schedule {
        actions.push(DynamicPipelineActionV1::Event(*event));
    }
    actions.sort_by_key(|action| {
        order_key(action.site()).unwrap_or((usize::MAX, action.site().operation()))
    });

    let mut staging = None::<(Value, usize)>;
    let mut consuming = None::<(Value, usize)>;
    let mut staging_coordinates = HashSet::<Vec<Value>>::new();
    let mut consuming_coordinates = HashSet::<Vec<Value>>::new();
    let mut canonical_coordinates = None::<HashSet<Vec<Value>>>;
    let mut empty_staging_windows = 0_usize;
    let mut empty_consuming_windows = 0_usize;
    let mut staged_writes = 0;
    let mut consuming_reads = 0;
    for action in actions {
        match action {
            DynamicPipelineActionV1::Event(event) => match event.kind {
                Some(PipelineEventKindAttr::Stage) => {
                    staging = Some((event.epoch, event.site.block()));
                    staging_coordinates.clear();
                }
                Some(PipelineEventKindAttr::Commit) => {
                    if staging_coordinates.is_empty() {
                        empty_staging_windows += 1;
                    } else {
                        require_matching_pipeline_coordinates_v1(
                            context,
                            pipeline,
                            event.site,
                            "staging",
                            &staging_coordinates,
                            &mut canonical_coordinates,
                        )?;
                    }
                    staging = None;
                }
                Some(PipelineEventKindAttr::Consume) => {
                    consuming = Some((event.epoch, event.site.block()));
                    consuming_coordinates.clear();
                }
                Some(PipelineEventKindAttr::Release) => {
                    if consuming.is_some() {
                        if consuming_coordinates.is_empty() {
                            empty_consuming_windows += 1;
                        } else {
                            require_matching_pipeline_coordinates_v1(
                                context,
                                pipeline,
                                event.site,
                                "consuming",
                                &consuming_coordinates,
                                &mut canonical_coordinates,
                            )?;
                        }
                    }
                    consuming_coordinates.clear();
                    consuming = None;
                }
                Some(PipelineEventKindAttr::Wait | PipelineEventKindAttr::Discard) | None => {}
            },
            DynamicPipelineActionV1::Access(access) => {
                let expected = match access.kind {
                    AccessKindAttr::Write => staging.map(|(epoch, block)| (epoch, block, true)),
                    AccessKindAttr::Read => consuming.map(|(epoch, block)| (epoch, block, false)),
                    _ => None,
                };
                let Some((epoch, epoch_block, is_write)) = expected else {
                    return Err(invalid(
                        pipeline,
                        Some(access.site),
                        &format!(
                            "{:?} access is outside its legal stage-to-commit or consume-to-release window",
                            access.kind
                        ),
                    ));
                };
                if !slot_is_epoch_modulo_across_loop_blocks(
                    context,
                    access.slot,
                    access.site.block(),
                    epoch,
                    epoch_block,
                    buffers,
                    summary,
                ) {
                    return Err(invalid(
                        pipeline,
                        Some(access.site),
                        &format!(
                            "{:?} access does not use the live epoch modulo {buffers} as its leading ring index",
                            access.kind
                        ),
                    ));
                }
                if is_write {
                    staging_coordinates.insert(access.indices[1..].to_vec());
                    staged_writes += 1;
                } else {
                    consuming_coordinates.insert(access.indices[1..].to_vec());
                    consuming_reads += 1;
                }
            }
        }
    }
    if consuming_reads != 0 && (empty_staging_windows != 0 || empty_consuming_windows != 0) {
        return Err(invalid(
            pipeline,
            None,
            "a consumed symbolic tile is not initialized in every prologue and steady-state epoch",
        ));
    }
    Ok((staged_writes, consuming_reads, true))
}

fn require_matching_pipeline_coordinates_v1(
    context: &Context,
    pipeline: PlironOperationSiteV1,
    event: PlironOperationSiteV1,
    phase: &str,
    coordinates: &HashSet<Vec<Value>>,
    canonical: &mut Option<HashSet<Vec<Value>>>,
) -> Result<(), PlironPipelineProtocolFindingV1> {
    debug_assert!(!coordinates.is_empty());
    match canonical {
        None => *canonical = Some(coordinates.clone()),
        Some(expected) if coordinate_sets_equivalent_v1(context, expected, coordinates) => {}
        Some(_) => {
            return Err(invalid(
                pipeline,
                Some(event),
                &format!(
                    "{phase} epoch coordinates do not match the staged/consumed symbolic tile"
                ),
            ));
        }
    }
    Ok(())
}

fn coordinate_sets_equivalent_v1(
    context: &Context,
    left: &HashSet<Vec<Value>>,
    right: &HashSet<Vec<Value>>,
) -> bool {
    let contains_equivalent = |haystack: &HashSet<Vec<Value>>, needle: &[Value]| {
        haystack.iter().any(|candidate| {
            candidate.len() == needle.len()
                && candidate
                    .iter()
                    .copied()
                    .zip(needle.iter().copied())
                    .all(|(left, right)| index_values_equivalent(context, left, right))
        })
    };
    left.iter()
        .all(|coordinate| contains_equivalent(right, coordinate))
        && right
            .iter()
            .all(|coordinate| contains_equivalent(left, coordinate))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlotStateV1 {
    Free,
    Staged(u64),
    Committed(u64),
    Ready(u64),
    Consuming(u64),
    Discarding(u64),
}

fn verify_concrete_schedule(
    context: &Context,
    pipeline: PlironOperationSiteV1,
    buffers: u32,
    schedule: &[EventSiteV1],
    accesses: &[AccessSiteV1],
) -> Result<(usize, usize, usize, bool), PlironPipelineProtocolFindingV1> {
    let first_block = schedule[0].site.block();
    if first_block != pipeline.block() {
        return Err(invalid(
            pipeline,
            None,
            "a straight-line pipeline lifecycle is not in its creation block, so per-entry lifecycle ownership is unproved",
        ));
    }
    if schedule
        .iter()
        .any(|event| event.site.block() != first_block)
    {
        return Err(invalid(
            pipeline,
            None,
            "non-loop pipeline events span multiple blocks, so execution order is not unique",
        ));
    }
    let mut ordered = schedule.to_vec();
    ordered.sort_by_key(|event| event.site.operation());
    let mut slots = vec![SlotStateV1::Free; buffers as usize];
    let mut epochs = HashSet::new();
    let mut ordered_accesses = accesses.to_vec();
    ordered_accesses.sort_by_key(|access| access.site.operation());
    let mut next_event = 0;
    let mut next_access = 0;
    let mut staged_writes = 0;
    let mut consuming_reads = 0;
    let mut initialized = HashMap::<u64, HashSet<Vec<Value>>>::new();
    while next_event < ordered.len() || next_access < ordered_accesses.len() {
        let event_precedes = next_access == ordered_accesses.len()
            || (next_event < ordered.len()
                && ordered[next_event].site.operation()
                    < ordered_accesses[next_access].site.operation());
        if !event_precedes {
            let access = ordered_accesses[next_access].clone();
            next_access += 1;
            if access.site.block() != first_block {
                return Err(invalid(
                    pipeline,
                    Some(access.site),
                    "a straight-line pipeline access is outside the entry block",
                ));
            }
            let Some(slot) = index_constant(context, access.slot) else {
                return Err(invalid(
                    pipeline,
                    Some(access.site),
                    "a straight-line pipeline access has a symbolic ring slot",
                ));
            };
            let Some(state) = usize::try_from(slot)
                .ok()
                .and_then(|slot| slots.get(slot))
                .copied()
            else {
                return Err(invalid(
                    pipeline,
                    Some(access.site),
                    &format!("pipeline access slot {slot} is outside {buffers} buffers"),
                ));
            };
            match access.kind {
                AccessKindAttr::Write if matches!(state, SlotStateV1::Staged(_)) => {
                    let SlotStateV1::Staged(epoch) = state else {
                        unreachable!("the guarded state is staged")
                    };
                    initialized
                        .entry(epoch)
                        .or_default()
                        .insert(access.indices[1..].to_vec());
                    staged_writes += 1;
                }
                AccessKindAttr::Read if matches!(state, SlotStateV1::Consuming(_)) => {
                    let SlotStateV1::Consuming(epoch) = state else {
                        unreachable!("the guarded state is consuming")
                    };
                    if !initialized
                        .get(&epoch)
                        .is_some_and(|writes| writes.contains(&access.indices[1..]))
                    {
                        return Err(invalid(
                            pipeline,
                            Some(access.site),
                            "pipeline read coordinate was not initialized in the same epoch",
                        ));
                    }
                    consuming_reads += 1;
                }
                _ => {
                    return Err(invalid(
                        pipeline,
                        Some(access.site),
                        &format!(
                            "{:?} access to slot {slot} occurs while that slot is {state:?}",
                            access.kind
                        ),
                    ));
                }
            }
            continue;
        }
        let event = ordered[next_event];
        next_event += 1;
        let Some(kind) = event.kind else {
            return Err(invalid(
                pipeline,
                Some(event.site),
                "event kind is malformed",
            ));
        };
        let Some(epoch) = index_constant(context, event.epoch) else {
            return Err(invalid(
                pipeline,
                Some(event.site),
                "symbolic event is not enclosed by a supported runtime-bounded loop",
            ));
        };
        let Some(slot) = index_constant(context, event.slot) else {
            return Err(invalid(
                pipeline,
                Some(event.site),
                "constant epoch has a non-constant ring slot",
            ));
        };
        if slot != epoch % u64::from(buffers) {
            return Err(invalid(
                pipeline,
                Some(event.site),
                &format!("slot {slot} is not epoch {epoch} % {buffers}"),
            ));
        }
        let state = &mut slots[slot as usize];
        let next = match (kind, *state) {
            (PipelineEventKindAttr::Stage, SlotStateV1::Free) => {
                epochs.insert(epoch);
                SlotStateV1::Staged(epoch)
            }
            (PipelineEventKindAttr::Commit, SlotStateV1::Staged(current)) if current == epoch => {
                SlotStateV1::Committed(epoch)
            }
            (PipelineEventKindAttr::Wait, SlotStateV1::Committed(current)) if current == epoch => {
                SlotStateV1::Ready(epoch)
            }
            (PipelineEventKindAttr::Consume, SlotStateV1::Ready(current)) if current == epoch => {
                SlotStateV1::Consuming(epoch)
            }
            (PipelineEventKindAttr::Discard, SlotStateV1::Ready(current)) if current == epoch => {
                SlotStateV1::Discarding(epoch)
            }
            (PipelineEventKindAttr::Release, SlotStateV1::Consuming(current))
            | (PipelineEventKindAttr::Release, SlotStateV1::Discarding(current))
                if current == epoch =>
            {
                SlotStateV1::Free
            }
            _ => {
                return Err(invalid(
                    pipeline,
                    Some(event.site),
                    &format!(
                        "{kind:?} for epoch {epoch} is illegal while slot {slot} is {state:?}"
                    ),
                ));
            }
        };
        *state = next;
    }
    if slots.iter().any(|state| *state != SlotStateV1::Free) {
        return Err(invalid(
            pipeline,
            None,
            "pipeline exits with committed or consuming epochs that were not released",
        ));
    }
    Ok((epochs.len(), staged_writes, consuming_reads, true))
}

fn discover_epoch_loops(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> EpochLoopDiscoveryV1 {
    let block_indices = inventory
        .blocks()
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<_, _>>();
    let mut predecessors = vec![Vec::new(); inventory.blocks().len()];
    let mut cfg_successors = vec![Vec::new(); inventory.blocks().len()];
    for (source, block) in inventory.blocks().iter().copied().enumerate() {
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            continue;
        };
        for successor in terminator.deref(context).successors() {
            if let Some(target) = block_indices.get(&successor).copied() {
                predecessors[target].push(source);
                cfg_successors[source].push(target);
            }
        }
    }
    let dominators = pipeline_dominators_v1(&cfg_successors, &predecessors);
    let mut loops = Vec::new();
    'headers: for (header_index, header) in inventory.blocks().iter().copied().enumerate() {
        let header_ref = header.deref(context);
        let Some(terminator) = header_ref.get_terminator(context) else {
            continue;
        };
        let operation = Operation::get_op_dyn(terminator, context);
        let Some(branch) = operation.downcast_ref::<IndexLessThanBranchArgsOp>() else {
            continue;
        };
        let induction = branch.lhs(context);
        let Some(induction_argument) = (0..header_ref.get_num_arguments())
            .find(|argument| header_ref.get_argument(*argument) == induction)
        else {
            continue;
        };
        let successors = operation
            .get_operation()
            .deref(context)
            .successors()
            .collect::<Vec<_>>();
        let [body_block, exit_block] = successors.as_slice() else {
            continue;
        };
        let (Some(body_start), Some(exit)) = (
            block_indices.get(body_block).copied(),
            block_indices.get(exit_block).copied(),
        ) else {
            continue;
        };
        let latches = predecessors[header_index]
            .iter()
            .copied()
            .filter(|predecessor| {
                *predecessor != header_index && dominators[*predecessor].contains(&header_index)
            })
            .collect::<Vec<_>>();
        let [latch] = latches.as_slice() else {
            continue;
        };
        let mut natural_loop = HashSet::from([header_index, *latch]);
        let mut pending = vec![*latch];
        while let Some(block) = pending.pop() {
            for predecessor in predecessors[block].iter().copied() {
                if predecessor != header_index
                    && dominators[predecessor].contains(&header_index)
                    && natural_loop.insert(predecessor)
                {
                    pending.push(predecessor);
                }
            }
        }
        if !natural_loop.contains(&body_start) || natural_loop.contains(&exit) {
            continue;
        }
        if predecessors[exit].as_slice() != [header_index] {
            continue;
        }
        let external = predecessors[header_index]
            .iter()
            .copied()
            .filter(|predecessor| !natural_loop.contains(predecessor))
            .collect::<Vec<_>>();
        let [entry] = external.as_slice() else {
            continue;
        };
        let Some(entry_arguments) = edge_arguments_v1(context, inventory.blocks()[*entry], header)
        else {
            continue;
        };
        if entry_arguments
            .get(induction_argument)
            .and_then(|value| index_constant(context, *value))
            != Some(0)
        {
            continue;
        }

        let mut inductions = HashMap::from([(header_index, induction)]);
        for _ in 0..natural_loop.len() {
            let mut changed = false;
            for source in natural_loop.iter().copied().collect::<Vec<_>>() {
                let Some(source_induction) = inductions.get(&source).copied() else {
                    continue;
                };
                let Some(terminator) = inventory.blocks()[source]
                    .deref(context)
                    .get_terminator(context)
                else {
                    continue;
                };
                for successor in terminator.deref(context).successors() {
                    let Some(target) = block_indices.get(&successor).copied() else {
                        continue;
                    };
                    if target == header_index || !natural_loop.contains(&target) {
                        continue;
                    }
                    let Some(arguments) =
                        edge_arguments_v1(context, inventory.blocks()[source], successor)
                    else {
                        continue;
                    };
                    let matching = arguments
                        .iter()
                        .enumerate()
                        .filter(|(_, argument)| {
                            index_values_equivalent(context, **argument, source_induction)
                        })
                        .map(|(argument, _)| argument)
                        .collect::<Vec<_>>();
                    let [argument] = matching.as_slice() else {
                        continue;
                    };
                    let target_ref = successor.deref(context);
                    if *argument >= target_ref.get_num_arguments() {
                        continue;
                    }
                    let target_induction = target_ref.get_argument(*argument);
                    match inductions.get(&target).copied() {
                        Some(existing) if existing != target_induction => {
                            continue 'headers;
                        }
                        Some(_) => {}
                        None => {
                            inductions.insert(target, target_induction);
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let Some(latch_induction) = inductions.get(latch).copied() else {
            continue;
        };
        let Some(latch_arguments) = edge_arguments_v1(context, inventory.blocks()[*latch], header)
        else {
            continue;
        };
        if latch_arguments
            .get(induction_argument)
            .and_then(|value| index_offset(context, *value, latch_induction))
            != Some(1)
        {
            continue;
        }

        let body_members = natural_loop
            .iter()
            .copied()
            .filter(|block| *block != header_index)
            .collect::<HashSet<_>>();
        let Some(body) = acyclic_loop_body_order_v1(&body_members, &cfg_successors) else {
            continue;
        };
        let mut prologue = vec![*entry];
        let mut current = *entry;
        while let [predecessor] = predecessors[current].as_slice() {
            if *predecessor == header_index
                || body_members.contains(predecessor)
                || predecessors[*predecessor].len() > 1
                || prologue.contains(predecessor)
            {
                break;
            }
            prologue.push(*predecessor);
            current = *predecessor;
        }
        prologue.reverse();

        let mut drain = Vec::new();
        let mut current = exit;
        loop {
            if drain.contains(&current) {
                break;
            }
            drain.push(current);
            let [successor] = cfg_successors[current].as_slice() else {
                break;
            };
            if *successor == header_index
                || body_members.contains(successor)
                || predecessors[*successor].len() > 1
            {
                break;
            }
            current = *successor;
        }
        inductions.remove(&header_index);
        loops.push(CanonicalEpochLoopV1 {
            prologue,
            header: header_index,
            body,
            body_members,
            drain,
            inductions,
            header_induction: induction,
            bound: branch.rhs(context),
        });
    }
    EpochLoopDiscoveryV1 { loops, dominators }
}

fn pipeline_dominators_v1(
    successors: &[Vec<usize>],
    predecessors: &[Vec<usize>],
) -> Vec<HashSet<usize>> {
    let mut reachable = vec![false; successors.len()];
    let mut pending = (!successors.is_empty())
        .then_some(0)
        .into_iter()
        .collect::<Vec<_>>();
    while let Some(block) = pending.pop() {
        if reachable[block] {
            continue;
        }
        reachable[block] = true;
        pending.extend(successors[block].iter().copied());
    }
    let all = reachable
        .iter()
        .enumerate()
        .filter_map(|(block, reachable)| (*reachable).then_some(block))
        .collect::<HashSet<_>>();
    let mut dominators = vec![HashSet::new(); successors.len()];
    for block in all.iter().copied() {
        dominators[block] = all.clone();
    }
    if !successors.is_empty() {
        dominators[0] = HashSet::from([0]);
    }
    let mut changed = true;
    while changed {
        changed = false;
        for block in all.iter().copied().filter(|block| *block != 0) {
            let mut incoming = predecessors[block]
                .iter()
                .copied()
                .filter(|predecessor| reachable[*predecessor]);
            let Some(first) = incoming.next() else {
                continue;
            };
            let mut next = dominators[first].clone();
            for predecessor in incoming {
                next.retain(|dominator| dominators[predecessor].contains(dominator));
            }
            next.insert(block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
    }
    dominators
}

fn edge_arguments_v1(
    context: &Context,
    source: Ptr<BasicBlock>,
    target: Ptr<BasicBlock>,
) -> Option<Vec<Value>> {
    let terminator = source.deref(context).get_terminator(context)?;
    let operation = Operation::get_op_dyn(terminator, context);
    let successor = operation
        .get_operation()
        .deref(context)
        .successors()
        .position(|successor| successor == target)?;
    if let Some(branch) = operation.downcast_ref::<BranchArgsOp>() {
        return (successor == 0).then(|| branch.arguments(context));
    };
    if let Some(branch) = operation.downcast_ref::<IndexLessThanBranchArgsOp>() {
        return match successor {
            0 => Some(branch.true_arguments(context)),
            1 => Some(branch.false_arguments(context)),
            _ => None,
        };
    }
    if let Some(branch) = operation.downcast_ref::<IndexEqualBranchArgsOp>() {
        return match successor {
            0 => Some(branch.true_arguments(context)),
            1 => Some(branch.false_arguments(context)),
            _ => None,
        };
    }
    if let Some(split) = operation.downcast_ref::<AnalysisSplitOp>() {
        return match successor {
            0 => Some(split.first_arguments(context)),
            1 => Some(split.second_arguments(context)),
            _ => None,
        };
    }
    (target.deref(context).get_num_arguments() == 0).then(Vec::new)
}

fn acyclic_loop_body_order_v1(
    members: &HashSet<usize>,
    successors: &[Vec<usize>],
) -> Option<Vec<usize>> {
    let mut incoming = members
        .iter()
        .copied()
        .map(|block| (block, 0_usize))
        .collect::<HashMap<_, _>>();
    for source in members.iter().copied() {
        for target in &successors[source] {
            if members.contains(target) {
                *incoming.get_mut(target)? += 1;
            }
        }
    }
    let mut ready = incoming
        .iter()
        .filter_map(|(block, incoming)| (*incoming == 0).then_some(*block))
        .collect::<Vec<_>>();
    ready.sort_unstable_by(|left, right| right.cmp(left));
    let mut ordered = Vec::with_capacity(members.len());
    while let Some(source) = ready.pop() {
        ordered.push(source);
        for target in successors[source].iter().copied() {
            if !members.contains(&target) {
                continue;
            }
            let count = incoming.get_mut(&target)?;
            *count = count.checked_sub(1)?;
            if *count == 0 {
                ready.push(target);
                ready.sort_unstable_by(|left, right| right.cmp(left));
            }
        }
    }
    (ordered.len() == members.len()).then_some(ordered)
}

fn slot_is_epoch_modulo(context: &Context, slot: Value, epoch: Value, buffers: u32) -> bool {
    if let (Some(slot), Some(epoch)) = (
        index_constant(context, slot),
        index_constant(context, epoch),
    ) {
        return slot == epoch % u64::from(buffers);
    }
    let Some(definition) = slot.defining_op() else {
        return false;
    };
    let operation = Operation::get_op_dyn(definition, context);
    let Some(remainder) = operation.downcast_ref::<IndexBinaryOp>() else {
        return false;
    };
    remainder.kind(context) == Some(IndexBinaryKindAttr::Remainder)
        && index_values_equivalent(context, remainder.lhs(context), epoch)
        && index_constant(context, remainder.rhs(context)) == Some(u64::from(buffers))
}

fn slot_is_epoch_modulo_across_loop_blocks(
    context: &Context,
    slot: Value,
    slot_block: usize,
    epoch: Value,
    epoch_block: usize,
    buffers: u32,
    summary: &CanonicalEpochLoopV1,
) -> bool {
    if let (Some(slot), Some(epoch)) = (
        index_constant(context, slot),
        index_constant(context, epoch),
    ) {
        return slot == epoch % u64::from(buffers);
    }
    let Some(definition) = slot.defining_op() else {
        return false;
    };
    let operation = Operation::get_op_dyn(definition, context);
    let Some(remainder) = operation.downcast_ref::<IndexBinaryOp>() else {
        return false;
    };
    remainder.kind(context) == Some(IndexBinaryKindAttr::Remainder)
        && index_values_equivalent_across_loop_blocks(
            context,
            remainder.lhs(context),
            slot_block,
            epoch,
            epoch_block,
            summary,
        )
        && index_constant(context, remainder.rhs(context)) == Some(u64::from(buffers))
}

fn index_values_equivalent_across_loop_blocks(
    context: &Context,
    left: Value,
    left_block: usize,
    right: Value,
    right_block: usize,
    summary: &CanonicalEpochLoopV1,
) -> bool {
    if index_values_equivalent(context, left, right) {
        return true;
    }
    let Some(left_induction) = summary.inductions.get(&left_block).copied() else {
        return false;
    };
    let Some(right_induction) = summary.inductions.get(&right_block).copied() else {
        return false;
    };
    match (
        index_offset(context, left, left_induction),
        index_offset(context, right, right_induction),
    ) {
        (Some(left_offset), Some(right_offset)) => left_offset == right_offset,
        _ => false,
    }
}

fn index_offset(context: &Context, value: Value, base: Value) -> Option<u64> {
    if index_values_equivalent(context, value, base) {
        return Some(0);
    }
    let definition = value.defining_op()?;
    let operation = Operation::get_op_dyn(definition, context);
    let add = operation.downcast_ref::<IndexBinaryOp>()?;
    if add.kind(context) != Some(IndexBinaryKindAttr::Add) {
        return None;
    }
    if index_values_equivalent(context, add.lhs(context), base) {
        index_constant(context, add.rhs(context))
    } else if index_values_equivalent(context, add.rhs(context), base) {
        index_constant(context, add.lhs(context))
    } else {
        None
    }
}

fn index_values_equivalent(context: &Context, left: Value, right: Value) -> bool {
    const MAX_EQUIVALENCE_WORK_V1: usize = 256;

    fn equivalent(
        context: &Context,
        left: Value,
        right: Value,
        remaining: &mut usize,
        visiting: &mut HashSet<(Value, Value)>,
    ) -> bool {
        if left == right {
            return true;
        }
        if *remaining == 0 || !visiting.insert((left, right)) {
            return false;
        }
        *remaining -= 1;

        let transparent_source = |value: Value| {
            let definition = value.defining_op()?;
            let operation = Operation::get_op_dyn(definition, context);
            let join = operation.downcast_ref::<DeterministicJoinOp>()?;
            let dependencies = join.dependencies(context);
            (dependencies.len() == 1).then(|| dependencies[0])
        };
        let result = if let Some(source) = transparent_source(left) {
            equivalent(context, source, right, remaining, visiting)
        } else if let Some(source) = transparent_source(right) {
            equivalent(context, left, source, remaining, visiting)
        } else {
            let Some(left_definition) = left.defining_op() else {
                visiting.remove(&(left, right));
                return false;
            };
            let Some(right_definition) = right.defining_op() else {
                visiting.remove(&(left, right));
                return false;
            };
            let left_operation = Operation::get_op_dyn(left_definition, context);
            let right_operation = Operation::get_op_dyn(right_definition, context);
            if let (Some(left), Some(right)) = (
                left_operation.downcast_ref::<IndexConstantOp>(),
                right_operation.downcast_ref::<IndexConstantOp>(),
            ) {
                left.value(context) == right.value(context)
            } else if let (Some(left), Some(right)) = (
                left_operation.downcast_ref::<IndexUnsignedCastOp>(),
                right_operation.downcast_ref::<IndexUnsignedCastOp>(),
            ) {
                left.bit_width(context) == right.bit_width(context)
                    && equivalent(
                        context,
                        left.source(context),
                        right.source(context),
                        remaining,
                        visiting,
                    )
            } else if let (Some(left), Some(right)) = (
                left_operation.downcast_ref::<IndexBinaryOp>(),
                right_operation.downcast_ref::<IndexBinaryOp>(),
            ) {
                let same_kind = left.kind(context) == right.kind(context);
                let direct = equivalent(
                    context,
                    left.lhs(context),
                    right.lhs(context),
                    remaining,
                    visiting,
                ) && equivalent(
                    context,
                    left.rhs(context),
                    right.rhs(context),
                    remaining,
                    visiting,
                );
                let commutative = matches!(
                    left.kind(context),
                    Some(IndexBinaryKindAttr::Add | IndexBinaryKindAttr::Multiply)
                ) && equivalent(
                    context,
                    left.lhs(context),
                    right.rhs(context),
                    remaining,
                    visiting,
                ) && equivalent(
                    context,
                    left.rhs(context),
                    right.lhs(context),
                    remaining,
                    visiting,
                );
                same_kind && (direct || commutative)
            } else {
                false
            }
        };
        visiting.remove(&(left, right));
        result
    }

    let mut remaining = MAX_EQUIVALENCE_WORK_V1;
    equivalent(context, left, right, &mut remaining, &mut HashSet::new())
}

fn index_constant(context: &Context, value: Value) -> Option<u64> {
    let definition = value.defining_op()?;
    Operation::get_op_dyn(definition, context)
        .downcast_ref::<IndexConstantOp>()?
        .value(context)
}

fn invalid(
    pipeline: PlironOperationSiteV1,
    event: Option<PlironOperationSiteV1>,
    detail: &str,
) -> PlironPipelineProtocolFindingV1 {
    PlironPipelineProtocolFindingV1::InvalidSchedule {
        pipeline_block: pipeline.block(),
        pipeline_operation: pipeline.operation(),
        event_block: event.map(PlironOperationSiteV1::block),
        event_operation: event.map(PlironOperationSiteV1::operation),
        detail: detail.to_owned(),
    }
}

fn push_finding(
    findings: &mut Vec<PlironPipelineProtocolFindingV1>,
    finding: PlironPipelineProtocolFindingV1,
) {
    if findings.len() < MAX_PLIRON_PIPELINE_FINDINGS_V1 {
        findings.push(finding);
    } else if !matches!(
        findings.last(),
        Some(PlironPipelineProtocolFindingV1::FindingLimitExceeded)
    ) {
        findings.push(PlironPipelineProtocolFindingV1::FindingLimitExceeded);
    }
}

fn report(finding: PlironPipelineProtocolFindingV1) -> PlironPipelineProtocolReportV1 {
    PlironPipelineProtocolReportV1 {
        findings: vec![finding],
        certificates: Vec::new(),
    }
}
