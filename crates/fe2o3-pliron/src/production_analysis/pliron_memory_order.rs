//! Workload-neutral memory-version and happens-before analysis for PLIRON.
//!
//! Facts come only from executed access, barrier, fence, and atomic events.
//! An ordering attribute is never treated as proof of a read-from edge.

use std::collections::{BTreeMap, HashMap, hash_map::Entry};

use crate::production_analysis::pliron_invocation_trace::{
    PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceLocationV1,
    ProductionInvocationTraceResourceAdmissionV1,
};
use crate::production_analysis::pliron_provenance_alias::{
    MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1, PlironProvenanceAliasAnalysisV1,
};
use crate::production_analysis::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use dialect_gpu::{
    AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr as GpuMemoryOrderAttr, MemoryScopeAttr,
};
use dialect_kernel::{AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, MemorySpaceAttr};

pub const MAX_PLIRON_MEMORY_VERSIONS_V1: usize = 1_048_576;
pub const MAX_PLIRON_MEMORY_PUBLICATION_EDGES_V1: usize = 1_048_576;
pub const MAX_PLIRON_MEMORY_ORDER_ISSUES_V1: usize = 4_096;
// A release-write epoch can simultaneously retain the release roster,
// published-address table, epoch state/version tables, and per-invocation
// local-version tables. Their address/invocation vectors require at most six
// rank words per event; eight rank words plus fixed map/vector records leaves
// explicit capacity and growth-copy slack.
const MEMORY_ORDER_TEMPORARY_RANK_ITEMS_PER_EVENT_V1: usize = 8;
const MEMORY_ORDER_TEMPORARY_FIXED_ITEMS_PER_EVENT_V1: usize = 32;
const MEMORY_ORDER_WORK_RANK_ITEMS_PER_EVENT_V1: usize = 16;
const MEMORY_ORDER_WORK_FIXED_ITEMS_PER_EVENT_V1: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionMemoryOrderResourceAdmissionV1 {
    upper_bound: ProductionAnalysisResourceUpperBoundV1,
    issue_upper_bound: usize,
    vector_rank: usize,
}

impl ProductionMemoryOrderResourceAdmissionV1 {
    pub(crate) const fn upper_bound(self) -> ProductionAnalysisResourceUpperBoundV1 {
        self.upper_bound
    }

    pub(crate) const fn issue_upper_bound(self) -> usize {
        self.issue_upper_bound
    }

    pub(crate) const fn vector_rank(self) -> usize {
        self.vector_rank
    }
}

fn memory_order_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::MemoryOrder,
        resource: "memory-order resource upper bound",
    }
}

fn checked_memory_order_mul_v1(
    lhs: usize,
    rhs: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs)
        .ok_or_else(memory_order_resource_overflow_v1)
}

fn checked_memory_order_sum_v1(
    items: &[usize],
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    items.iter().try_fold(0_usize, |total, item| {
        total
            .checked_add(*item)
            .ok_or_else(memory_order_resource_overflow_v1)
    })
}

/// Bounds memory-version construction before the trace is grouped or scanned.
/// Pair work covers same-address conflict search, acquire/release candidate
/// search, and publication-edge expansion. The analysis enforces the matching
/// version, edge, and issue cardinality caps while constructing its cache.
pub(crate) fn preflight_memory_order_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    trace: ProductionInvocationTraceResourceAdmissionV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionMemoryOrderResourceAdmissionV1, ProductionAnalysisResourceLimitV1> {
    let events = trace.event_upper_bound();
    let invocations = trace.invocation_count();
    let launch_rank = trace.launch_rank();
    let pairs = checked_memory_order_mul_v1(events, events)?;
    let vector_rank = launch_rank.max(dialect_kernel::MAX_RANKED_MEMORY_RANK);
    // Even one acquire-release RMW is visited while collecting release
    // summaries and again during epoch construction. Sixteen rank traversals
    // cover address materialization/cloning, hashing and equality in both
    // passes; the fixed term covers event matching and map lifecycle actions.
    let event_work = vector_rank
        .checked_mul(MEMORY_ORDER_WORK_RANK_ITEMS_PER_EVENT_V1)
        .and_then(|items| items.checked_add(MEMORY_ORDER_WORK_FIXED_ITEMS_PER_EVENT_V1))
        .ok_or_else(memory_order_resource_overflow_v1)?;
    // Each read/prefix pair can clone a rank-sized candidate address, hash it,
    // compare the address key, and compare the summarized release invocation.
    // Four rank traversals plus eight fixed event/map fields bound that path.
    let pair_work = vector_rank
        .checked_mul(4)
        .and_then(|items| items.checked_add(8))
        .ok_or_else(memory_order_resource_overflow_v1)?;
    let work = checked_memory_order_sum_v1(&[
        census.operations,
        invocations,
        checked_memory_order_mul_v1(events, event_work)?,
        checked_memory_order_mul_v1(pairs, pair_work)?,
    ])?;

    let versions = events.min(MAX_PLIRON_MEMORY_VERSIONS_V1);
    let edges = pairs.min(MAX_PLIRON_MEMORY_PUBLICATION_EDGES_V1);
    let issues = pairs.min(MAX_PLIRON_MEMORY_ORDER_ISSUES_V1);
    let retained = checked_memory_order_sum_v1(&[
        checked_memory_order_mul_v1(
            versions,
            vector_rank
                .checked_mul(2)
                .and_then(|n| n.checked_add(8))
                .ok_or_else(memory_order_resource_overflow_v1)?,
        )?,
        checked_memory_order_mul_v1(
            edges,
            launch_rank
                .checked_add(5)
                .ok_or_else(memory_order_resource_overflow_v1)?,
        )?,
        checked_memory_order_mul_v1(
            issues,
            vector_rank
                .checked_mul(3)
                .and_then(|n| n.checked_add(132))
                .ok_or_else(memory_order_resource_overflow_v1)?,
        )?,
        MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1,
    ])?;
    let temporary = checked_memory_order_sum_v1(&[
        checked_memory_order_mul_v1(invocations, 3)?,
        checked_memory_order_mul_v1(
            events,
            vector_rank
                .checked_mul(MEMORY_ORDER_TEMPORARY_RANK_ITEMS_PER_EVENT_V1)
                .and_then(|n| n.checked_add(MEMORY_ORDER_TEMPORARY_FIXED_ITEMS_PER_EVENT_V1))
                .ok_or_else(memory_order_resource_overflow_v1)?,
        )?,
        versions,
        // One rank-sized address candidate is live during a prefix lookup.
        vector_rank,
    ])?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::MemoryOrder,
        work,
        retained,
        temporary,
    )?;
    let upper_bound = limits.require(ProductionAnalysisResourcePhaseV1::MemoryOrder, bound)?;
    Ok(ProductionMemoryOrderResourceAdmissionV1 {
        upper_bound,
        issue_upper_bound: issues,
        vector_rank,
    })
}

pub(crate) fn preflight_memory_order_attempt_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    trace: Option<ProductionInvocationTraceResourceAdmissionV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionMemoryOrderResourceAdmissionV1, ProductionAnalysisResourceLimitV1> {
    if let Some(trace) = trace {
        return preflight_memory_order_resource_upper_bound_v1(census, trace, limits);
    }
    let phase = ProductionAnalysisResourcePhaseV1::MemoryOrder;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        phase,
        4,
        MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1,
        dialect_kernel::MAX_RANKED_MEMORY_RANK,
    )?;
    let upper_bound = limits.require(phase, bound)?;
    Ok(ProductionMemoryOrderResourceAdmissionV1 {
        upper_bound,
        issue_upper_bound: 0,
        vector_rank: dialect_kernel::MAX_RANKED_MEMORY_RANK,
    })
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlironMemoryLocationV1 {
    block: usize,
    operation: usize,
}

impl From<PlironTraceLocationV1> for PlironMemoryLocationV1 {
    fn from(location: PlironTraceLocationV1) -> Self {
        Self {
            block: location.block,
            operation: location.operation,
        }
    }
}

impl PlironMemoryLocationV1 {
    pub const fn block(self) -> usize {
        self.block
    }

    pub const fn operation(self) -> usize {
        self.operation
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PlironMemoryAddressV1 {
    allocation_class: u64,
    indices: Vec<u64>,
}

impl PlironMemoryAddressV1 {
    pub const fn allocation_class(&self) -> u64 {
        self.allocation_class
    }

    pub fn indices(&self) -> &[u64] {
        &self.indices
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironMemoryVersionV1 {
    id: usize,
    grid: u64,
    workgroup: u64,
    epoch: usize,
    invocation: Vec<u64>,
    location: PlironMemoryLocationV1,
    address: PlironMemoryAddressV1,
    access: AccessKindAttr,
}

impl PlironMemoryVersionV1 {
    pub const fn id(&self) -> usize {
        self.id
    }

    pub const fn epoch(&self) -> usize {
        self.epoch
    }

    pub const fn grid(&self) -> u64 {
        self.grid
    }

    pub const fn workgroup(&self) -> u64 {
        self.workgroup
    }

    pub fn invocation(&self) -> &[u64] {
        &self.invocation
    }

    pub const fn location(&self) -> PlironMemoryLocationV1 {
        self.location
    }

    pub const fn address(&self) -> &PlironMemoryAddressV1 {
        &self.address
    }

    pub const fn access(&self) -> AccessKindAttr {
        self.access
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironPublicationEdgeV1 {
    version: usize,
    barrier: PlironMemoryLocationV1,
    reader_invocation: Vec<u64>,
    read: PlironMemoryLocationV1,
}

impl PlironPublicationEdgeV1 {
    pub const fn version(&self) -> usize {
        self.version
    }

    pub const fn barrier(&self) -> PlironMemoryLocationV1 {
        self.barrier
    }

    pub fn reader_invocation(&self) -> &[u64] {
        &self.reader_invocation
    }

    pub const fn read(&self) -> PlironMemoryLocationV1 {
        self.read
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironMemoryOrderIssueV1 {
    ReadBeforeInitialization {
        invocation: Vec<u64>,
        location: PlironMemoryLocationV1,
        address: PlironMemoryAddressV1,
    },
    ConflictingEffects {
        address: PlironMemoryAddressV1,
        first_invocation: Vec<u64>,
        first_location: PlironMemoryLocationV1,
        first_access: AccessKindAttr,
        second_invocation: Vec<u64>,
        second_location: PlironMemoryLocationV1,
        second_access: AccessKindAttr,
    },
    AtomicReadFromUnresolved {
        invocation: Vec<u64>,
        location: PlironMemoryLocationV1,
        address: PlironMemoryAddressV1,
        detail: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironMemoryOrderFailureV1 {
    UnresolvedAddress {
        location: PlironMemoryLocationV1,
    },
    MismatchedBarrierPhase {
        grid: u64,
        workgroup: u64,
        epoch: usize,
    },
    SubgroupPublicationUnsupported {
        location: PlironMemoryLocationV1,
    },
    FencePublicationUnsupported {
        location: PlironMemoryLocationV1,
    },
    VersionLimitExceeded,
    PublicationEdgeLimitExceeded,
    IssueLimitExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironMemoryOrderAnalysisV1 {
    versions: Vec<PlironMemoryVersionV1>,
    publication_edges: Vec<PlironPublicationEdgeV1>,
    issues: Vec<PlironMemoryOrderIssueV1>,
}

impl PlironMemoryOrderAnalysisV1 {
    pub fn versions(&self) -> &[PlironMemoryVersionV1] {
        &self.versions
    }

    pub fn publication_edges(&self) -> &[PlironPublicationEdgeV1] {
        &self.publication_edges
    }

    pub fn issues(&self) -> &[PlironMemoryOrderIssueV1] {
        &self.issues
    }
}

#[derive(Clone)]
struct EffectV1 {
    invocation: Vec<u64>,
    location: PlironMemoryLocationV1,
    access: AccessKindAttr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ReleaseAtomicSummaryV1 {
    UniqueInvocation {
        invocation: Vec<u64>,
        earliest_event: usize,
    },
    MultipleInvocations,
}

#[derive(Default)]
struct EpochAddressStateV1 {
    effects: Vec<EffectV1>,
}

#[derive(Clone, Debug)]
struct PublishedVersionV1 {
    id: usize,
    barrier: PlironMemoryLocationV1,
}

pub(crate) fn analyze_pliron_memory_order_v1(
    traces: &[PlironInvocationTraceV1],
    provenance: &PlironProvenanceAliasAnalysisV1,
) -> Result<PlironMemoryOrderAnalysisV1, PlironMemoryOrderFailureV1> {
    if let Some(location) = traces
        .iter()
        .flat_map(|trace| &trace.events)
        .find_map(|event| match event {
            PlironTraceEventV1::Fence {
                location,
                address_space: AddressSpaceAttr::Workgroup,
                ..
            } => Some((*location).into()),
            _ => None,
        })
    {
        return Err(PlironMemoryOrderFailureV1::FencePublicationUnsupported { location });
    }
    let mut grouped = BTreeMap::<(u64, u64), Vec<&PlironInvocationTraceV1>>::new();
    for trace in traces {
        grouped
            .entry((trace.grid, trace.workgroup))
            .or_default()
            .push(trace);
    }

    let mut analysis = PlironMemoryOrderAnalysisV1 {
        versions: Vec::new(),
        publication_edges: Vec::new(),
        issues: Vec::new(),
    };
    for ((grid, workgroup), group) in grouped {
        analyze_workgroup(grid, workgroup, &group, provenance, &mut analysis)?;
    }
    Ok(analysis)
}

fn analyze_workgroup(
    grid: u64,
    workgroup: u64,
    traces: &[&PlironInvocationTraceV1],
    provenance: &PlironProvenanceAliasAnalysisV1,
    analysis: &mut PlironMemoryOrderAnalysisV1,
) -> Result<(), PlironMemoryOrderFailureV1> {
    let mut cursors = vec![0_usize; traces.len()];
    let mut epoch = 0_usize;
    let mut published = HashMap::<PlironMemoryAddressV1, Vec<PublishedVersionV1>>::new();
    let release_atomics = collect_release_atomics(traces, provenance);

    loop {
        let mut epoch_states = HashMap::<PlironMemoryAddressV1, EpochAddressStateV1>::new();
        let mut epoch_versions = HashMap::<PlironMemoryAddressV1, Vec<usize>>::new();
        let mut local_versions =
            vec![HashMap::<PlironMemoryAddressV1, Vec<usize>>::new(); traces.len()];
        let mut barriers = Vec::new();
        let mut any_event = false;

        for (trace_index, trace) in traces.iter().enumerate() {
            while let Some(event) = trace.events.get(cursors[trace_index]) {
                cursors[trace_index] += 1;
                any_event = true;
                match event {
                    PlironTraceEventV1::Barrier {
                        location,
                        execution_scope: HierarchyAttr::Workgroup,
                        memory_scope:
                            MemoryScopeAttr::Workgroup
                            | MemoryScopeAttr::Device
                            | MemoryScopeAttr::System,
                        address_space: AddressSpaceAttr::Workgroup,
                        order:
                            GpuMemoryOrderAttr::AcquireRelease
                            | GpuMemoryOrderAttr::SequentiallyConsistent,
                        ..
                    } => {
                        barriers.push((*location).into());
                        break;
                    }
                    PlironTraceEventV1::Barrier {
                        location,
                        execution_scope: HierarchyAttr::Subgroup,
                        address_space: AddressSpaceAttr::Workgroup,
                        ..
                    } => {
                        return Err(PlironMemoryOrderFailureV1::SubgroupPublicationUnsupported {
                            location: (*location).into(),
                        });
                    }
                    PlironTraceEventV1::Memory {
                        location,
                        memory_space: MemorySpaceAttr::Workgroup,
                        access,
                        indices,
                        noalias_class,
                        ..
                    } => {
                        let location = (*location).into();
                        let Some(indices) = indices.iter().copied().collect::<Option<Vec<_>>>()
                        else {
                            return Err(PlironMemoryOrderFailureV1::UnresolvedAddress { location });
                        };
                        let address = PlironMemoryAddressV1 {
                            allocation_class: provenance
                                .canonical_class(MemorySpaceAttr::Workgroup, *noalias_class),
                            indices,
                        };
                        let effect = EffectV1 {
                            invocation: trace.invocation.clone(),
                            location,
                            access: *access,
                        };

                        if access.reads_memory()
                            && !local_versions[trace_index].contains_key(&address)
                            && !published.contains_key(&address)
                        {
                            let issue = if has_plausible_atomic_publication(
                                trace,
                                cursors[trace_index],
                                provenance,
                                &release_atomics,
                            ) {
                                PlironMemoryOrderIssueV1::AtomicReadFromUnresolved {
                                    invocation: trace.invocation.clone(),
                                    location,
                                    address: address.clone(),
                                    detail: "an acquire/release declaration does not identify the write observed by this read".to_owned(),
                                }
                            } else {
                                PlironMemoryOrderIssueV1::ReadBeforeInitialization {
                                    invocation: trace.invocation.clone(),
                                    location,
                                    address: address.clone(),
                                }
                            };
                            push_issue(analysis, issue)?;
                        }

                        if let Some(state) = epoch_states.get(&address)
                            && let Some(first) = state.effects.iter().find(|first| {
                                first.invocation != effect.invocation
                                    && effects_conflict(first.access, effect.access)
                            })
                        {
                            push_issue(
                                analysis,
                                PlironMemoryOrderIssueV1::ConflictingEffects {
                                    address: address.clone(),
                                    first_invocation: first.invocation.clone(),
                                    first_location: first.location,
                                    first_access: first.access,
                                    second_invocation: effect.invocation.clone(),
                                    second_location: effect.location,
                                    second_access: effect.access,
                                },
                            )?;
                        }
                        epoch_states
                            .entry(address.clone())
                            .or_default()
                            .effects
                            .push(effect);

                        if access.reads_memory()
                            && !local_versions[trace_index].contains_key(&address)
                            && let Some(visible) = published.get(&address)
                        {
                            for version in visible {
                                if analysis.publication_edges.len()
                                    == MAX_PLIRON_MEMORY_PUBLICATION_EDGES_V1
                                {
                                    return Err(
                                        PlironMemoryOrderFailureV1::PublicationEdgeLimitExceeded,
                                    );
                                }
                                analysis.publication_edges.push(PlironPublicationEdgeV1 {
                                    version: version.id,
                                    barrier: version.barrier,
                                    reader_invocation: trace.invocation.clone(),
                                    read: location,
                                });
                            }
                        }
                        if access.writes_memory() {
                            if analysis.versions.len() == MAX_PLIRON_MEMORY_VERSIONS_V1 {
                                return Err(PlironMemoryOrderFailureV1::VersionLimitExceeded);
                            }
                            let id = analysis.versions.len();
                            analysis.versions.push(PlironMemoryVersionV1 {
                                id,
                                grid,
                                workgroup,
                                epoch,
                                invocation: trace.invocation.clone(),
                                location,
                                address: address.clone(),
                                access: *access,
                            });
                            epoch_versions.entry(address.clone()).or_default().push(id);
                            local_versions[trace_index]
                                .entry(address)
                                .or_default()
                                .push(id);
                        }
                    }
                    PlironTraceEventV1::Barrier { .. }
                    | PlironTraceEventV1::Fence { .. }
                    | PlironTraceEventV1::TensorInstruction { .. }
                    | PlironTraceEventV1::Trap { .. }
                    | PlironTraceEventV1::Memory { .. }
                    | PlironTraceEventV1::CollectiveAllocation { .. } => {}
                }
            }
        }

        if !any_event {
            break;
        }
        if !barriers.is_empty() {
            if barriers.len() != traces.len()
                || barriers.iter().any(|barrier| *barrier != barriers[0])
            {
                return Err(PlironMemoryOrderFailureV1::MismatchedBarrierPhase {
                    grid,
                    workgroup,
                    epoch,
                });
            }
            for (address, versions) in epoch_versions {
                published.insert(
                    address,
                    versions
                        .into_iter()
                        .map(|id| PublishedVersionV1 {
                            id,
                            barrier: barriers[0],
                        })
                        .collect(),
                );
            }
            epoch = epoch.saturating_add(1);
        }
        if cursors
            .iter()
            .zip(traces)
            .all(|(cursor, trace)| *cursor == trace.events.len())
        {
            break;
        }
    }
    Ok(())
}

fn effects_conflict(first: AccessKindAttr, second: AccessKindAttr) -> bool {
    if !first.writes_memory() && !second.writes_memory() {
        return false;
    }
    !(first.is_atomic() && second.is_atomic())
}

fn collect_release_atomics(
    traces: &[&PlironInvocationTraceV1],
    provenance: &PlironProvenanceAliasAnalysisV1,
) -> HashMap<PlironMemoryAddressV1, ReleaseAtomicSummaryV1> {
    let mut releases = HashMap::<PlironMemoryAddressV1, ReleaseAtomicSummaryV1>::new();
    for trace in traces {
        for (event_index, event) in trace.events.iter().enumerate() {
            let PlironTraceEventV1::Memory {
                memory_space: MemorySpaceAttr::Workgroup,
                access,
                atomic_ordering:
                    Some(
                        AtomicOrderingAttr::Release
                        | AtomicOrderingAttr::AcquireRelease
                        | AtomicOrderingAttr::SequentiallyConsistent,
                    ),
                atomic_scope: Some(scope),
                indices,
                noalias_class,
                ..
            } = event
            else {
                continue;
            };
            if !access.is_atomic()
                || !access.writes_memory()
                || scope.rank() < AtomicScopeAttr::Workgroup.rank()
            {
                continue;
            }
            let Some(indices) = indices.iter().copied().collect::<Option<Vec<_>>>() else {
                continue;
            };
            record_release_atomic_v1(
                &mut releases,
                PlironMemoryAddressV1 {
                    allocation_class: provenance
                        .canonical_class(MemorySpaceAttr::Workgroup, *noalias_class),
                    indices,
                },
                &trace.invocation,
                event_index,
            );
        }
    }
    releases
}

fn record_release_atomic_v1(
    releases: &mut HashMap<PlironMemoryAddressV1, ReleaseAtomicSummaryV1>,
    address: PlironMemoryAddressV1,
    invocation: &[u64],
    event: usize,
) {
    match releases.entry(address) {
        Entry::Vacant(slot) => {
            slot.insert(ReleaseAtomicSummaryV1::UniqueInvocation {
                invocation: invocation.to_vec(),
                earliest_event: event,
            });
        }
        Entry::Occupied(mut slot) => match slot.get_mut() {
            ReleaseAtomicSummaryV1::UniqueInvocation {
                invocation: recorded_invocation,
                earliest_event,
            } if recorded_invocation.as_slice() == invocation => {
                *earliest_event = (*earliest_event).min(event);
            }
            ReleaseAtomicSummaryV1::UniqueInvocation { .. } => {
                slot.insert(ReleaseAtomicSummaryV1::MultipleInvocations);
            }
            ReleaseAtomicSummaryV1::MultipleInvocations => {}
        },
    }
}

fn has_plausible_atomic_publication(
    trace: &PlironInvocationTraceV1,
    cursor: usize,
    provenance: &PlironProvenanceAliasAnalysisV1,
    releases: &HashMap<PlironMemoryAddressV1, ReleaseAtomicSummaryV1>,
) -> bool {
    trace.events[..cursor]
        .iter()
        .enumerate()
        .any(|(acquire_event, event)| {
            let PlironTraceEventV1::Memory {
                memory_space: MemorySpaceAttr::Workgroup,
                access,
                atomic_ordering:
                    Some(
                        AtomicOrderingAttr::Acquire
                        | AtomicOrderingAttr::AcquireRelease
                        | AtomicOrderingAttr::SequentiallyConsistent,
                    ),
                atomic_scope: Some(acquire_scope),
                indices,
                noalias_class,
                ..
            } = event
            else {
                return false;
            };
            if !access.is_atomic()
                || !access.reads_memory()
                || acquire_scope.rank() < AtomicScopeAttr::Workgroup.rank()
            {
                return false;
            }
            let Some(indices) = indices.iter().copied().collect::<Option<Vec<_>>>() else {
                return false;
            };
            let address = PlironMemoryAddressV1 {
                allocation_class: provenance
                    .canonical_class(MemorySpaceAttr::Workgroup, *noalias_class),
                indices,
            };
            releases.get(&address).is_some_and(|summary| {
                release_summary_can_publish_v1(summary, &trace.invocation, acquire_event)
            })
        })
}

fn release_summary_can_publish_v1(
    summary: &ReleaseAtomicSummaryV1,
    reader_invocation: &[u64],
    acquire_event: usize,
) -> bool {
    match summary {
        ReleaseAtomicSummaryV1::UniqueInvocation {
            invocation,
            earliest_event,
        } => invocation != reader_invocation || *earliest_event < acquire_event,
        ReleaseAtomicSummaryV1::MultipleInvocations => true,
    }
}

fn push_issue(
    analysis: &mut PlironMemoryOrderAnalysisV1,
    issue: PlironMemoryOrderIssueV1,
) -> Result<(), PlironMemoryOrderFailureV1> {
    if analysis.issues.len() == MAX_PLIRON_MEMORY_ORDER_ISSUES_V1 {
        return Err(PlironMemoryOrderFailureV1::IssueLimitExceeded);
    }
    analysis.issues.push(issue);
    Ok(())
}

#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;
    use dialect_kernel::{DIALECT_NAME, IndexConstantOp, register_dialect};
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
        dialect::DialectName,
    };

    fn trace_admission() -> ProductionInvocationTraceResourceAdmissionV1 {
        super::super::pliron_invocation_trace::invocation_trace_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 {
                blocks: 2,
                operations: 7,
                max_operation_arity: 3,
                max_successor_arity: 2,
                ..ProductionAnalysisInputCensusV1::default()
            },
            4,
            3,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap()
    }

    #[test]
    fn memory_order_bound_has_exact_and_one_under_admission() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 7,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_memory_order_resource_upper_bound_v1(
            census,
            trace_admission(),
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap()
        .upper_bound();
        assert!(exact.work_upper_bound() > 0);
        assert!(exact.retained_storage_upper_bound() > 0);
        assert_eq!(
            preflight_memory_order_resource_upper_bound_v1(
                census,
                trace_admission(),
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - 1,
                    exact.peak_storage_upper_bound(),
                ),
            )
            .map(ProductionMemoryOrderResourceAdmissionV1::upper_bound),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::MemoryOrder,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_memory_order_resource_upper_bound_v1(
                census,
                trace_admission(),
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound() - 1,
                ),
            )
            .map(ProductionMemoryOrderResourceAdmissionV1::upper_bound),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::MemoryOrder,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn release_write_epoch_peak_is_literal_and_rejects_one_under() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 7,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_memory_order_resource_upper_bound_v1(
            census,
            trace_admission(),
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap()
        .upper_bound();

        // The authenticated trace admits 1,048,576 events at vector rank 8.
        // Retained output is 34,194,432 items. The release-write epoch owns
        // 4*3 invocation slots + 1,048,576*(8*8+32) event slots + 1,048,576
        // version slots + one rank-8 lookup candidate = 101,711,892 temporary
        // items. Pair work is 1,048,576^2*(4*8+8); linear event work is
        // 1,048,576*(16*8+32), plus seven operations and four invocations.
        const EXACT_WORK: usize = 43_980_632_883_211;
        const RELEASE_WRITE_EPOCH_PEAK: usize = 135_906_324;
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.peak_storage_upper_bound(), RELEASE_WRITE_EPOCH_PEAK);
        assert!(
            preflight_memory_order_resource_upper_bound_v1(
                census,
                trace_admission(),
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK - 1, RELEASE_WRITE_EPOCH_PEAK),
            )
            .is_err()
        );
        assert!(
            preflight_memory_order_resource_upper_bound_v1(
                census,
                trace_admission(),
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    RELEASE_WRITE_EPOCH_PEAK - 1,
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn release_acquire_candidates_stay_constant_size_under_adversarial_repetition() {
        let address = PlironMemoryAddressV1 {
            allocation_class: 9,
            indices: vec![3, 5, 7],
        };
        let mut releases = HashMap::new();
        for event in (0..4_096).rev() {
            record_release_atomic_v1(&mut releases, address.clone(), &[0, 0, 0], event);
        }
        assert_eq!(releases.len(), 1);
        let summary = releases.get(&address).unwrap();
        assert_eq!(
            summary,
            &ReleaseAtomicSummaryV1::UniqueInvocation {
                invocation: vec![0, 0, 0],
                earliest_event: 0,
            }
        );
        assert!(!release_summary_can_publish_v1(summary, &[0, 0, 0], 0));
        assert!(release_summary_can_publish_v1(summary, &[0, 0, 0], 1));
        assert!(release_summary_can_publish_v1(summary, &[1, 0, 0], 0));

        record_release_atomic_v1(&mut releases, address.clone(), &[1, 0, 0], 4_096);
        let summary = releases.get(&address).unwrap();
        assert_eq!(summary, &ReleaseAtomicSummaryV1::MultipleInvocations);
        assert!(release_summary_can_publish_v1(summary, &[0, 0, 0], 0));
        assert!(release_summary_can_publish_v1(summary, &[1, 0, 0], 0));
    }

    #[test]
    fn one_acquire_release_rmw_executes_both_event_traversals_under_the_linear_bound() {
        let mut context = Context::new();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        let function_type = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(&mut context, "one_rmw".try_into().unwrap(), function_type);
        let provenance = crate::analyze_pliron_provenance_alias_v1(&context, &function).unwrap();
        let placeholder = IndexConstantOp::new(&mut context, 0).result(&context);
        let traces = [PlironInvocationTraceV1 {
            invocation: vec![0],
            grid: 0,
            workgroup: 0,
            subgroup: 0,
            lane: 0,
            events: vec![PlironTraceEventV1::Memory {
                location: PlironTraceLocationV1 {
                    block: 0,
                    operation: 0,
                },
                view: placeholder,
                memory_space: MemorySpaceAttr::Workgroup,
                access: AccessKindAttr::AtomicReadModifyWrite,
                atomic_ordering: Some(AtomicOrderingAttr::AcquireRelease),
                atomic_scope: Some(AtomicScopeAttr::Workgroup),
                indices: vec![Some(0)],
                allocation_origin: 0,
                noalias_class: 0,
                view_signature: (32, vec![1]),
            }],
        }];
        let analysis = analyze_pliron_memory_order_v1(&traces, &provenance).unwrap();
        assert_eq!(analysis.versions().len(), 1);
        assert_eq!(analysis.issues().len(), 1);
        assert!(analysis.publication_edges().is_empty());

        // At the minimum effective vector rank of eight, one RMW costs
        // 16*8+32 linear units. One operation, one invocation, and the one
        // pair at 4*8+8 bring the exact work bound to 202.
        let event_work = 8 * MEMORY_ORDER_WORK_RANK_ITEMS_PER_EVENT_V1
            + MEMORY_ORDER_WORK_FIXED_ITEMS_PER_EVENT_V1;
        assert_eq!(1 + 1 + event_work + (4 * 8 + 8), 202);
    }

    #[test]
    fn rejected_trace_memory_cache_is_admitted_at_exact_and_one_under_limits() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 7,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_memory_order_attempt_resource_upper_bound_v1(
            census,
            None,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert_eq!(exact.issue_upper_bound(), 0);
        let bound = exact.upper_bound();
        assert!(bound.work_upper_bound() > 0);
        assert!(bound.retained_storage_upper_bound() > 0);

        assert_eq!(
            preflight_memory_order_attempt_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound() - 1,
                    bound.peak_storage_upper_bound(),
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::MemoryOrder,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_memory_order_attempt_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound() - 1,
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::MemoryOrder,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn memory_order_bound_rejects_pair_overflow_before_analysis() {
        let trace = super::super::pliron_invocation_trace::invocation_trace_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 {
                operations: usize::MAX,
                ..ProductionAnalysisInputCensusV1::default()
            },
            1,
            1,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        );
        let error = trace.expect_err("overflowing trace shape must fail before analysis");
        assert_eq!(
            error.phase,
            ProductionAnalysisResourcePhaseV1::InvocationTrace
        );
        assert_eq!(error.resource, "invocation trace resource upper bound");
    }
}
