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
    BranchArgsOp, DeterministicJoinOp, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexLessThanBranchArgsOp, IndexUnsignedCastOp, PipelineCreateOp, PipelineEventKindAttr,
    PipelineEventOp, RankedViewOp,
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
    header: usize,
    body: Vec<usize>,
    exit: usize,
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
    pub fn body(&self) -> &[usize] {
        &self.body
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
    let loops = discover_epoch_loops(context, &inventory);
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
            .or_else(|| match noalias_class {
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
        creates.push((site.pointer(), *site, create.pipeline_type(context)));
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
    let mut certificates = Vec::new();
    for (pointer, site, pipeline_type) in creates {
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
            site,
            pipeline_type.buffers(),
            pipeline_type.prefetch_distance(),
            &schedule,
            &loops,
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
struct CanonicalEpochLoopV1 {
    entry: usize,
    header: usize,
    body: Vec<usize>,
    body_members: HashSet<usize>,
    exit: usize,
    inductions: HashMap<usize, Value>,
    header_induction: Value,
    bound: Value,
}

fn verify_one_pipeline(
    context: &Context,
    pipeline: PlironOperationSiteV1,
    buffers: u32,
    distance: u32,
    schedule: &[EventSiteV1],
    loops: &[CanonicalEpochLoopV1],
    uniform_roots: &HashSet<Value>,
) -> Result<PlironPipelineProtocolCertificateV1, PlironPipelineProtocolFindingV1> {
    let containing = loops
        .iter()
        .filter(|summary| {
            schedule
                .iter()
                .any(|event| summary.body_members.contains(&event.site.block()))
        })
        .collect::<Vec<_>>();
    if containing.is_empty() {
        let epochs = verify_concrete_schedule(context, pipeline, buffers, schedule)?;
        return Ok(PlironPipelineProtocolCertificateV1 {
            pipeline_block: pipeline.block(),
            pipeline_operation: pipeline.operation(),
            buffers,
            prefetch_distance: distance,
            dynamic_loop: None,
            concrete_epochs: epochs,
        });
    }
    let [summary] = containing.as_slice() else {
        return Err(invalid(
            pipeline,
            None,
            "one pipeline lifecycle crosses more than one dynamic loop",
        ));
    };
    if summary.entry != 0 {
        return Err(invalid(
            pipeline,
            None,
            "the dynamic pipeline prologue is not in the function entry block, so full-workgroup participation is unproved",
        ));
    }
    if !is_uniform_value(context, summary.bound, uniform_roots, &mut HashSet::new()) {
        return Err(invalid(
            pipeline,
            None,
            "the runtime loop bound is not proved workgroup-uniform",
        ));
    }
    verify_dynamic_schedule(context, pipeline, buffers, distance, schedule, summary)?;
    Ok(PlironPipelineProtocolCertificateV1 {
        pipeline_block: pipeline.block(),
        pipeline_operation: pipeline.operation(),
        buffers,
        prefetch_distance: distance,
        dynamic_loop: Some(EpochAwareLoopSummaryV1 {
            prologue: summary.entry,
            header: summary.header,
            body: summary.body.clone(),
            exit: summary.exit,
            induction: summary.header_induction.unique_name(context).to_string(),
            bound: summary.bound.unique_name(context).to_string(),
            step: 1,
            prefetched_epochs: distance,
            live_epoch_window: distance + 1,
            drained_epochs: distance,
        }),
        concrete_epochs: 0,
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
    summary: &CanonicalEpochLoopV1,
) -> Result<(), PlironPipelineProtocolFindingV1> {
    let mut prologue = Vec::new();
    let mut body = Vec::new();
    let mut drain = Vec::new();
    let positions = summary
        .body
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<HashMap<_, _>>();
    for event in schedule {
        if event.site.block() == summary.entry {
            prologue.push(*event);
        } else if let Some(position) = positions.get(&event.site.block()).copied() {
            body.push((position, *event));
        } else if event.site.block() == summary.exit {
            drain.push(*event);
        } else {
            return Err(invalid(
                pipeline,
                Some(event.site),
                "event is outside the canonical prologue, loop body, or drain block",
            ));
        }
    }
    prologue.sort_by_key(|event| event.site.operation());
    body.sort_by_key(|(position, event)| (*position, event.site.operation()));
    drain.sort_by_key(|event| event.site.operation());

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
    Ok(())
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
) -> Result<usize, PlironPipelineProtocolFindingV1> {
    let first_block = schedule[0].site.block();
    if first_block != 0 {
        return Err(invalid(
            pipeline,
            None,
            "a straight-line pipeline lifecycle is not in the function entry block, so full-workgroup participation is unproved",
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
    for event in ordered {
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
    Ok(epochs.len())
}

fn discover_epoch_loops(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Vec<CanonicalEpochLoopV1> {
    let block_indices = inventory
        .blocks()
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<_, _>>();
    let mut predecessors = vec![Vec::new(); inventory.blocks().len()];
    for (source, block) in inventory.blocks().iter().copied().enumerate() {
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            continue;
        };
        for successor in terminator.deref(context).successors() {
            if let Some(target) = block_indices.get(&successor).copied() {
                predecessors[target].push(source);
            }
        }
    }
    let mut loops = Vec::new();
    for (header_index, header) in inventory.blocks().iter().copied().enumerate() {
        let header_ref = header.deref(context);
        if header_ref.get_num_arguments() != 1 {
            continue;
        }
        let Some(terminator) = header_ref.get_terminator(context) else {
            continue;
        };
        let operation = Operation::get_op_dyn(terminator, context);
        let Some(branch) = operation.downcast_ref::<IndexLessThanBranchArgsOp>() else {
            continue;
        };
        let induction = header_ref.get_argument(0);
        if branch.lhs(context) != induction
            || branch.true_arguments(context).as_slice() != [induction]
        {
            continue;
        }
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
        let mut body = Vec::new();
        let mut body_members = HashSet::new();
        let mut inductions = HashMap::new();
        let mut current = body_start;
        let mut previous = header_index;
        let mut latch = None;
        loop {
            if current == header_index || !body_members.insert(current) {
                break;
            }
            if predecessors[current].as_slice() != [previous] {
                break;
            }
            let block = inventory.blocks()[current];
            let block_ref = block.deref(context);
            if block_ref.get_num_arguments() != 1 {
                break;
            }
            let body_induction = block_ref.get_argument(0);
            inductions.insert(current, body_induction);
            body.push(current);
            let Some(terminator) = block_ref.get_terminator(context) else {
                break;
            };
            let operation = Operation::get_op_dyn(terminator, context);
            let Some(forward) = operation.downcast_ref::<BranchArgsOp>() else {
                break;
            };
            let successors = operation
                .get_operation()
                .deref(context)
                .successors()
                .collect::<Vec<_>>();
            let [successor] = successors.as_slice() else {
                break;
            };
            let Some(next_block) = block_indices.get(successor).copied() else {
                break;
            };
            let arguments = forward.arguments(context);
            let [next] = arguments.as_slice() else {
                break;
            };
            if next_block == header_index {
                if index_offset(context, *next, body_induction) == Some(1) {
                    latch = Some(current);
                }
                break;
            }
            if *next != body_induction {
                break;
            }
            previous = current;
            current = next_block;
        }
        let Some(latch) = latch else {
            continue;
        };
        let external = predecessors[header_index]
            .iter()
            .copied()
            .filter(|predecessor| *predecessor != latch)
            .collect::<Vec<_>>();
        let [entry] = external.as_slice() else {
            continue;
        };
        if predecessors[header_index].len() != 2
            || predecessors[exit].as_slice() != [header_index]
            || !entry_initializes_zero(context, inventory.blocks()[*entry], header)
            || body.is_empty()
            || exit == header_index
        {
            continue;
        }
        loops.push(CanonicalEpochLoopV1 {
            entry: *entry,
            header: header_index,
            body,
            body_members,
            exit,
            inductions,
            header_induction: induction,
            bound: branch.rhs(context),
        });
    }
    loops
}

fn entry_initializes_zero(
    context: &Context,
    entry: Ptr<BasicBlock>,
    header: Ptr<BasicBlock>,
) -> bool {
    let Some(terminator) = entry.deref(context).get_terminator(context) else {
        return false;
    };
    let operation = Operation::get_op_dyn(terminator, context);
    let Some(branch) = operation.downcast_ref::<BranchArgsOp>() else {
        return false;
    };
    operation.get_operation().deref(context).get_successor(0) == header
        && branch.arguments(context).len() == 1
        && index_constant(context, branch.arguments(context)[0]) == Some(0)
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
        && remainder.lhs(context) == epoch
        && index_constant(context, remainder.rhs(context)) == Some(u64::from(buffers))
}

fn index_offset(context: &Context, value: Value, base: Value) -> Option<u64> {
    if value == base {
        return Some(0);
    }
    let definition = value.defining_op()?;
    let operation = Operation::get_op_dyn(definition, context);
    let add = operation.downcast_ref::<IndexBinaryOp>()?;
    if add.kind(context) != Some(IndexBinaryKindAttr::Add) {
        return None;
    }
    if add.lhs(context) == base {
        index_constant(context, add.rhs(context))
    } else if add.rhs(context) == base {
        index_constant(context, add.lhs(context))
    } else {
        None
    }
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
