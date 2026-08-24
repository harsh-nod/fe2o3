//! Generic concurrent-effect verification for ranked Pliron memory.
//!
//! Sparse SSA propagation supplies index formulas. A conservative symbolic
//! fast path proves equal full-rank affine maps injective for any launch size.
//! Remaining cases are evaluated over a bounded static launch domain and
//! indexed by logical allocation plus element coordinate. Exact fallback is
//! O(invocations * effects * rank), never pairwise in invocation count.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use dialect_gpu::{AddressSpaceAttr, FenceOp};
use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, MemorySpaceAttr, RankedAccessOp,
    RankedViewOp,
};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    common_traits::Named,
    context::Context,
    linked_list::ContainsLinkedList,
    operation::Operation,
    value::Value,
};

use crate::pliron_invocation_trace::{PlironTraceFailureV1, pliron_execution_layout_v1};
use crate::pliron_sparse_index::SparseAffineIndexV1;
use crate::{
    KernelCheckPassKindV1, SparseIndexAnalysisV1, SparseIndexFailureV1,
    analyze_pliron_sparse_indices_v1, run_pliron_ranked_bounds_check_v1,
};

pub const MAX_PLIRON_RACE_INVOCATIONS_V1: u64 = 65_536;
pub const MAX_PLIRON_RACE_EFFECT_INSTANCES_V1: usize = 1_048_576;
pub const MAX_PLIRON_RACE_FINDINGS_V1: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RankedRaceLocationV1 {
    block: usize,
    operation: usize,
}

impl RankedRaceLocationV1 {
    pub const fn block(self) -> usize {
        self.block
    }

    pub const fn operation(self) -> usize {
        self.operation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedRaceWitnessV1 {
    location: RankedRaceLocationV1,
    access: AccessKindAttr,
    invocation: Vec<u64>,
    grid: u64,
    workgroup: Option<u64>,
    subgroup: Option<u64>,
    lane: Option<u64>,
    atomic_scope: Option<AtomicScopeAttr>,
}

impl RankedRaceWitnessV1 {
    pub const fn location(&self) -> RankedRaceLocationV1 {
        self.location
    }

    pub const fn access(&self) -> AccessKindAttr {
        self.access
    }

    pub fn invocation(&self) -> &[u64] {
        &self.invocation
    }

    pub const fn grid(&self) -> u64 {
        self.grid
    }

    pub const fn workgroup(&self) -> Option<u64> {
        self.workgroup
    }

    pub const fn subgroup(&self) -> Option<u64> {
        self.subgroup
    }

    pub const fn lane(&self) -> Option<u64> {
        self.lane
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RankedRaceFindingV1 {
    BoundsPrerequisiteRejected,
    SparseIndexAnalysisFailed {
        detail: String,
    },
    DynamicLaunchExtent {
        dimension: usize,
    },
    LaunchDomainTooLarge {
        invocations: u64,
        limit: u64,
    },
    UnresolvedIndex {
        block: usize,
        operation: usize,
        dimension: usize,
        value: String,
    },
    EffectInstanceLimitExceeded {
        actual: usize,
        limit: usize,
    },
    FindingLimitExceeded {
        actual: usize,
        limit: usize,
    },
    ConflictingEffects {
        view: String,
        indices: Vec<u64>,
        first: RankedRaceWitnessV1,
        second: RankedRaceWitnessV1,
    },
    ExecutionLayoutUnavailable {
        detail: String,
    },
    AllocationContractUnavailable {
        detail: String,
    },
    InsufficientAtomicScope {
        view: String,
        indices: Vec<u64>,
        first: RankedRaceWitnessV1,
        second: RankedRaceWitnessV1,
    },
    HappensBeforeIncomplete {
        view: String,
        detail: String,
    },
}

impl fmt::Display for RankedRaceFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundsPrerequisiteRejected => formatter.write_str(
                "error[FE2O3-RACE-000]: ranked bounds prerequisite rejected before race analysis",
            ),
            Self::SparseIndexAnalysisFailed { detail } => write!(
                formatter,
                "error[FE2O3-RACE-003]: sparse index analysis failed before race analysis: {detail}",
            ),
            Self::DynamicLaunchExtent { dimension } => write!(
                formatter,
                "error[FE2O3-RACE-002]: cannot prove race freedom for dynamic launch dimension {dimension}; help: retain a bounded launch contract or supply a symbolic disjointness proof",
            ),
            Self::LaunchDomainTooLarge { invocations, limit } => write!(
                formatter,
                "error[FE2O3-RACE-003]: static launch has {invocations} invocations, exceeding exact race-analysis limit {limit}",
            ),
            Self::UnresolvedIndex {
                block,
                operation,
                dimension,
                value,
            } => write!(
                formatter,
                "error[FE2O3-RACE-002]: cannot prove race freedom at block {block} op {operation}; access dimension {dimension} has unresolved index {value}",
            ),
            Self::EffectInstanceLimitExceeded { actual, limit } => write!(
                formatter,
                "error[FE2O3-RACE-003]: concurrent effect instance count {actual} exceeds analysis limit {limit}",
            ),
            Self::FindingLimitExceeded { actual, limit } => write!(
                formatter,
                "error[FE2O3-RACE-003]: race finding count {actual} exceeds analysis limit {limit}",
            ),
            Self::ConflictingEffects {
                view,
                indices,
                first,
                second,
            } => write!(
                formatter,
                "error[FE2O3-RACE-001]: potentially conflicting incompatible {:?}/{:?} effects on {view}{indices:?}; first writer/reader: invocation {:?} at block {} op {}; second writer/reader: invocation {:?} at block {} op {}; failed proof: distinct concurrent invocations do not imply disjoint memory coordinates; help: include an invocation-owned coordinate, use a disjoint view, or use a compatible atomic operation",
                first.access,
                second.access,
                first.invocation,
                first.location.block,
                first.location.operation,
                second.invocation,
                second.location.block,
                second.location.operation,
            ),
            Self::ExecutionLayoutUnavailable { detail } => write!(
                formatter,
                "error[FE2O3-RACE-002]: scoped concurrency analysis is incomplete: {detail}",
            ),
            Self::AllocationContractUnavailable { detail } => write!(
                formatter,
                "error[FE2O3-RACE-002]: allocation alias analysis is incomplete: {detail}",
            ),
            Self::InsufficientAtomicScope {
                view,
                indices,
                first,
                second,
            } => write!(
                formatter,
                "error[FE2O3-RACE-004]: overlapping atomic effects on {view}{indices:?} use scopes {:?}/{:?} that do not cover invocations {:?}/{:?}; failed proof: cross-workgroup overlap requires compatible device-scope atomics",
                first.atomic_scope, second.atomic_scope, first.invocation, second.invocation,
            ),
            Self::HappensBeforeIncomplete { view, detail } => write!(
                formatter,
                "error[FE2O3-RACE-002]: happens-before analysis for conflicting ordinary effects on {view} is incomplete: {detail}",
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RankedRaceStatusV1 {
    Clean,
    Incomplete,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedRaceReportV1 {
    findings: Vec<RankedRaceFindingV1>,
}

impl RankedRaceReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::RaceFreedom
    }

    pub fn status(&self) -> RankedRaceStatusV1 {
        if self.findings.is_empty() {
            RankedRaceStatusV1::Clean
        } else if self.findings.iter().all(RankedRaceFindingV1::is_incomplete) {
            RankedRaceStatusV1::Incomplete
        } else {
            RankedRaceStatusV1::Rejected
        }
    }

    pub fn findings(&self) -> &[RankedRaceFindingV1] {
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
pub struct RankedRaceCheckErrorV1 {
    report: RankedRaceReportV1,
}

impl RankedRaceCheckErrorV1 {
    pub fn report(&self) -> &RankedRaceReportV1 {
        &self.report
    }
}

impl fmt::Display for RankedRaceCheckErrorV1 {
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

impl std::error::Error for RankedRaceCheckErrorV1 {}

#[derive(Clone)]
struct EffectV1 {
    view: Value,
    view_name: String,
    kind: AccessKindAttr,
    location: RankedRaceLocationV1,
    indices: Vec<Value>,
    atomic_scope: Option<AtomicScopeAttr>,
    atomic_ordering: Option<AtomicOrderingAttr>,
    allocation_origin: u64,
    noalias_class: u64,
    view_signature: (u32, Vec<u64>),
}

impl RankedRaceFindingV1 {
    const fn is_incomplete(&self) -> bool {
        matches!(
            self,
            Self::SparseIndexAnalysisFailed { .. }
                | Self::DynamicLaunchExtent { .. }
                | Self::LaunchDomainTooLarge { .. }
                | Self::UnresolvedIndex { .. }
                | Self::EffectInstanceLimitExceeded { .. }
                | Self::FindingLimitExceeded { .. }
                | Self::ExecutionLayoutUnavailable { .. }
                | Self::AllocationContractUnavailable { .. }
                | Self::HappensBeforeIncomplete { .. }
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct AddressKeyV1 {
    allocation_class: u64,
    indices: Vec<u64>,
}

#[derive(Clone, Debug, Default)]
struct WitnessPairV1 {
    first: Option<RankedRaceWitnessV1>,
    second: Option<RankedRaceWitnessV1>,
}

impl WitnessPairV1 {
    fn different_from(&self, invocation: &[u64]) -> Option<&RankedRaceWitnessV1> {
        self.first
            .as_ref()
            .filter(|witness| witness.invocation != invocation)
            .or_else(|| {
                self.second
                    .as_ref()
                    .filter(|witness| witness.invocation != invocation)
            })
    }

    fn insert(&mut self, witness: RankedRaceWitnessV1) {
        if self.first.is_none() {
            self.first = Some(witness);
        } else if self
            .first
            .as_ref()
            .is_some_and(|first| first.invocation != witness.invocation)
            && self.second.is_none()
        {
            self.second = Some(witness);
        }
    }
}

#[derive(Clone, Debug, Default)]
struct AddressStateV1 {
    reads: WitnessPairV1,
    writes: WitnessPairV1,
    atomic_reads: WitnessPairV1,
    atomic_writes: WitnessPairV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ConflictClassV1 {
    view: Value,
    first: RankedRaceLocationV1,
    second: RankedRaceLocationV1,
    first_kind: AccessKindAttr,
    second_kind: AccessKindAttr,
}

pub fn run_pliron_ranked_race_check_v1(context: &Context, function: &FuncOp) -> RankedRaceReportV1 {
    if !run_pliron_ranked_bounds_check_v1(context, function).is_clean() {
        return one(RankedRaceFindingV1::BoundsPrerequisiteRejected);
    }
    run_pliron_ranked_race_check_after_bounds_v1(context, function)
}

pub(crate) fn run_pliron_ranked_race_check_after_bounds_v1(
    context: &Context,
    function: &FuncOp,
) -> RankedRaceReportV1 {
    let sparse = match analyze_pliron_sparse_indices_v1(context, function) {
        Ok(sparse) => sparse,
        Err(failure) => {
            return one(RankedRaceFindingV1::SparseIndexAnalysisFailed {
                detail: sparse_failure(failure),
            });
        }
    };
    let mut effects = Vec::new();
    let mut has_global_fence = false;
    for (block_index, block) in function
        .get_region(context)
        .deref(context)
        .iter(context)
        .enumerate()
    {
        for (operation_index, operation) in block.deref(context).iter(context).enumerate() {
            let operation = Operation::get_op_dyn(operation, context);
            if operation
                .downcast_ref::<FenceOp>()
                .is_some_and(|fence| fence.address_space(context) == Some(AddressSpaceAttr::Global))
            {
                has_global_fence = true;
            }
            let Some(access) = operation.downcast_ref::<RankedAccessOp>() else {
                continue;
            };
            let view = access.view(context);
            let Some(definition) = view.defining_op() else {
                return one(RankedRaceFindingV1::UnresolvedIndex {
                    block: block_index,
                    operation: operation_index,
                    dimension: 0,
                    value: "view-without-definition".to_owned(),
                });
            };
            let definition = Operation::get_op_dyn(definition, context);
            let Some(view_op) = definition.downcast_ref::<RankedViewOp>() else {
                return one(RankedRaceFindingV1::UnresolvedIndex {
                    block: block_index,
                    operation: operation_index,
                    dimension: 0,
                    value: "foreign-view-definition".to_owned(),
                });
            };
            match view_op.memory_space(context) {
                Some(MemorySpaceAttr::Private) => continue,
                // Workgroup effects are checked with barrier epochs by the
                // mandatory workgroup-memory pass that follows this pass.
                Some(MemorySpaceAttr::Workgroup) => continue,
                Some(MemorySpaceAttr::Global) => {}
                None => {
                    return one(RankedRaceFindingV1::UnresolvedIndex {
                        block: block_index,
                        operation: operation_index,
                        dimension: 0,
                        value: "view-without-memory-space".to_owned(),
                    });
                }
            }
            let Some(kind) = access.kind(context) else {
                return one(RankedRaceFindingV1::UnresolvedIndex {
                    block: block_index,
                    operation: operation_index,
                    dimension: 0,
                    value: "access-without-kind".to_owned(),
                });
            };
            effects.push(EffectV1 {
                view,
                view_name: view.unique_name(context).to_string(),
                kind,
                location: RankedRaceLocationV1 {
                    block: block_index,
                    operation: operation_index,
                },
                indices: access.indices(context),
                atomic_scope: access.atomic_scope(context),
                atomic_ordering: access.atomic_ordering(context),
                allocation_origin: view_op.allocation_origin(context).unwrap_or(0),
                noalias_class: view_op.noalias_class(context).unwrap_or(0),
                view_signature: view_op
                    .view_type(context)
                    .map(|ty| {
                        let ty = ty.deref(context);
                        (ty.element_width(), ty.shape().to_vec())
                    })
                    .unwrap_or_default(),
            });
        }
    }

    let mut classes_by_origin = HashMap::new();
    for effect in &effects {
        if effect.noalias_class != 0 && effect.allocation_origin == 0 {
            return one(RankedRaceFindingV1::AllocationContractUnavailable {
                detail: format!(
                    "view {} claims no-alias class {} without a compiler-issued allocation origin",
                    effect.view_name, effect.noalias_class
                ),
            });
        }
        if effect.allocation_origin != 0
            && classes_by_origin
                .insert(effect.allocation_origin, effect.noalias_class)
                .is_some_and(|previous| previous != effect.noalias_class)
        {
            return one(RankedRaceFindingV1::AllocationContractUnavailable {
                detail: format!(
                    "allocation origin {} is assigned inconsistent no-alias classes",
                    effect.allocation_origin
                ),
            });
        }
    }
    let distinct_views = effects
        .iter()
        .map(|effect| effect.view)
        .collect::<HashSet<_>>();
    if effects.iter().any(|effect| effect.noalias_class == 0)
        && effects.iter().any(|effect| effect.kind.writes_memory())
        && distinct_views.len() > 1
    {
        return one(RankedRaceFindingV1::AllocationContractUnavailable {
            detail: "an unknown-alias view may overlap a distinct allocation origin, but ranked IR does not retain their relative base offset"
                .to_owned(),
        });
    }
    let mut origins_by_class = HashMap::<u64, HashSet<u64>>::new();
    let mut writable_classes = HashSet::new();
    for effect in &effects {
        if effect.noalias_class == 0 {
            continue;
        }
        origins_by_class
            .entry(effect.noalias_class)
            .or_default()
            .insert(effect.allocation_origin);
        if effect.kind.writes_memory() {
            writable_classes.insert(effect.noalias_class);
        }
    }
    if let Some((&class, _)) = origins_by_class
        .iter()
        .find(|(class, origins)| origins.len() > 1 && writable_classes.contains(class))
    {
        return one(RankedRaceFindingV1::AllocationContractUnavailable {
            detail: format!(
                "potentially aliasing class {class} contains writable views from distinct allocation origins, but ranked IR does not retain their relative base offset"
            ),
        });
    }
    let has_unknown_alias = effects.iter().any(|effect| effect.noalias_class == 0);
    for effect in &mut effects {
        if has_unknown_alias {
            effect.noalias_class = 0;
        }
    }
    let classes_with_writes = effects
        .iter()
        .filter_map(|effect| effect.kind.writes_memory().then_some(effect.noalias_class))
        .collect::<HashSet<_>>();
    let mut signatures_by_class = HashMap::new();
    for effect in &effects {
        if !classes_with_writes.contains(&effect.noalias_class) {
            continue;
        }
        if signatures_by_class
            .insert(effect.noalias_class, effect.view_signature.clone())
            .is_some_and(|previous| previous != effect.view_signature)
        {
            return one(RankedRaceFindingV1::AllocationContractUnavailable {
                detail: format!(
                    "potentially aliasing view {} has an incompatible element width or rank/shape",
                    effect.view_name
                ),
            });
        }
    }

    let layout = match pliron_execution_layout_v1(context, function) {
        Ok(layout) => layout,
        Err(failure) => {
            return one(RankedRaceFindingV1::ExecutionLayoutUnavailable {
                detail: match failure {
                    PlironTraceFailureV1::InvalidExecutionLayout => {
                        "gpu.execution_layout is malformed or duplicated".to_owned()
                    }
                    _ => format!("execution layout extraction failed: {failure:?}"),
                },
            });
        }
    };
    let launch_extents = if let Some(layout) = layout {
        for dimension in 0..sparse.launch_extents().len().max(3) {
            if let Some(declared) = sparse.declared_launch_extent(dimension) {
                let Some(layout_extent) = layout.global_extents.get(dimension).copied() else {
                    return one(RankedRaceFindingV1::ExecutionLayoutUnavailable {
                        detail: format!(
                            "invocation coordinate axis {dimension} is outside the three-dimensional gpu.execution_layout"
                        ),
                    });
                };
                if declared != 0 && layout_extent != declared {
                    return one(RankedRaceFindingV1::ExecutionLayoutUnavailable {
                        detail: format!(
                            "invocation coordinate axis {dimension} declares extent {declared}, inconsistent with gpu.execution_layout"
                        ),
                    });
                }
            }
        }
        layout.global_extents.to_vec()
    } else if sparse.has_declared_launch_extent() {
        sparse.launch_extents().to_vec()
    } else if effects.is_empty() {
        vec![1]
    } else {
        return one(RankedRaceFindingV1::ExecutionLayoutUnavailable {
            detail: "concurrent memory effects require a declared execution domain even when the kernel does not read an invocation coordinate".to_owned(),
        });
    };

    if symbolically_proves_disjoint(&effects, &sparse, &launch_extents) {
        return clean();
    }
    let release_signal_views = effects
        .iter()
        .filter_map(|effect| {
            (effect.kind.is_atomic()
                && effect.kind.writes_memory()
                && matches!(
                    effect.atomic_ordering,
                    Some(
                        AtomicOrderingAttr::Release
                            | AtomicOrderingAttr::AcquireRelease
                            | AtomicOrderingAttr::SequentiallyConsistent
                    )
                )
                && effect
                    .atomic_scope
                    .is_some_and(|scope| scope.rank() >= AtomicScopeAttr::Agent.rank()))
            .then_some(effect.noalias_class)
        })
        .collect::<HashSet<_>>();
    let acquire_signal_views = effects
        .iter()
        .filter_map(|effect| {
            (effect.kind.is_atomic()
                && effect.kind.reads_memory()
                && matches!(
                    effect.atomic_ordering,
                    Some(
                        AtomicOrderingAttr::Acquire
                            | AtomicOrderingAttr::AcquireRelease
                            | AtomicOrderingAttr::SequentiallyConsistent
                    )
                )
                && effect
                    .atomic_scope
                    .is_some_and(|scope| scope.rank() >= AtomicScopeAttr::Agent.rank()))
            .then_some(effect.noalias_class)
        })
        .collect::<HashSet<_>>();
    let atomic_signal_views = release_signal_views
        .intersection(&acquire_signal_views)
        .copied()
        .collect::<HashSet<_>>();
    if let Some(dimension) = launch_extents.iter().position(|extent| *extent == 0) {
        return one(RankedRaceFindingV1::DynamicLaunchExtent { dimension });
    }
    let Some(invocation_count) = launch_extents
        .iter()
        .try_fold(1_u64, |total, extent| total.checked_mul(*extent))
    else {
        return one(RankedRaceFindingV1::LaunchDomainTooLarge {
            invocations: u64::MAX,
            limit: MAX_PLIRON_RACE_INVOCATIONS_V1,
        });
    };
    if invocation_count > MAX_PLIRON_RACE_INVOCATIONS_V1 {
        return one(RankedRaceFindingV1::LaunchDomainTooLarge {
            invocations: invocation_count,
            limit: MAX_PLIRON_RACE_INVOCATIONS_V1,
        });
    }
    if invocation_count <= 1 {
        return clean();
    }

    let zero_invocation = vec![0; launch_extents.len()];
    for effect in &effects {
        for (dimension, index) in effect.indices.iter().copied().enumerate() {
            if sparse.fact(index).evaluate(&zero_invocation).is_none() {
                return one(RankedRaceFindingV1::UnresolvedIndex {
                    block: effect.location.block,
                    operation: effect.location.operation,
                    dimension,
                    value: index.unique_name(context).to_string(),
                });
            }
        }
    }

    let mut addresses: HashMap<AddressKeyV1, AddressStateV1> = HashMap::new();
    let mut findings = Vec::new();
    let mut conflict_classes = HashSet::new();
    let mut effect_instances = 0_usize;
    for linear_invocation in 0..invocation_count {
        let invocation = decode_invocation(linear_invocation, &launch_extents);
        for effect in &effects {
            effect_instances = effect_instances.saturating_add(1);
            if effect_instances > MAX_PLIRON_RACE_EFFECT_INSTANCES_V1 {
                return one(RankedRaceFindingV1::EffectInstanceLimitExceeded {
                    actual: effect_instances,
                    limit: MAX_PLIRON_RACE_EFFECT_INSTANCES_V1,
                });
            }
            let Some(indices) = effect
                .indices
                .iter()
                .map(|index| sparse.fact(*index).evaluate(&invocation))
                .collect::<Option<Vec<_>>>()
            else {
                let (dimension, value) = effect
                    .indices
                    .iter()
                    .copied()
                    .enumerate()
                    .find(|(_, index)| sparse.fact(*index).evaluate(&invocation).is_none())
                    .expect("failed index evaluation identifies an unresolved index");
                return one(RankedRaceFindingV1::UnresolvedIndex {
                    block: effect.location.block,
                    operation: effect.location.operation,
                    dimension,
                    value: value.unique_name(context).to_string(),
                });
            };
            let key = AddressKeyV1 {
                allocation_class: effect.noalias_class,
                indices,
            };
            let scoped_identity = layout.and_then(|layout| layout.scoped_identity(&invocation));
            let witness = RankedRaceWitnessV1 {
                location: effect.location,
                access: effect.kind,
                invocation: invocation.clone(),
                grid: layout.map_or(0, |layout| layout.grid),
                workgroup: scoped_identity.map(|identity| identity.0),
                subgroup: scoped_identity.map(|identity| identity.1),
                lane: scoped_identity.map(|identity| identity.2),
                atomic_scope: effect.atomic_scope,
            };
            let state = addresses.entry(key.clone()).or_default();
            let conflict = conflicting_witness(state, effect.kind, &witness).cloned();
            if let Some(first) = conflict {
                let class = ConflictClassV1 {
                    view: effect.view,
                    first: first.location,
                    second: effect.location,
                    first_kind: first.access,
                    second_kind: effect.kind,
                };
                if conflict_classes.insert(class) {
                    let finding = if first.access.is_atomic() && witness.access.is_atomic() {
                        if layout.is_none() {
                            RankedRaceFindingV1::ExecutionLayoutUnavailable {
                                detail: format!(
                                    "overlapping narrow-scope atomics on {} require retained workgroup identity",
                                    effect.view_name
                                ),
                            }
                        } else {
                            RankedRaceFindingV1::InsufficientAtomicScope {
                                view: effect.view_name.clone(),
                                indices: key.indices,
                                first,
                                second: witness.clone(),
                            }
                        }
                    } else if has_global_fence
                        || atomic_signal_views
                            .iter()
                            .any(|class| *class != effect.noalias_class)
                    {
                        RankedRaceFindingV1::HappensBeforeIncomplete {
                            view: effect.view_name.clone(),
                            detail: if atomic_signal_views
                                .iter()
                                .any(|class| *class != effect.noalias_class)
                            {
                                "release/acquire atomics require an authenticated read-from relation before they can publish ordinary memory across invocations".to_owned()
                            } else {
                                "a non-collective fence alone does not establish a cross-invocation synchronizes-with edge".to_owned()
                            },
                        }
                    } else {
                        RankedRaceFindingV1::ConflictingEffects {
                            view: effect.view_name.clone(),
                            indices: key.indices,
                            first,
                            second: witness.clone(),
                        }
                    };
                    findings.push(finding);
                    if findings.len() > MAX_PLIRON_RACE_FINDINGS_V1 {
                        return one(RankedRaceFindingV1::FindingLimitExceeded {
                            actual: findings.len(),
                            limit: MAX_PLIRON_RACE_FINDINGS_V1,
                        });
                    }
                }
            }
            insert_witness(state, witness);
        }
    }
    RankedRaceReportV1 { findings }
}

pub(crate) fn require_pliron_ranked_race_freedom_after_bounds_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<RankedRaceReportV1, RankedRaceCheckErrorV1> {
    let report = run_pliron_ranked_race_check_after_bounds_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(RankedRaceCheckErrorV1 { report })
    }
}

pub fn require_pliron_ranked_race_freedom_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<RankedRaceReportV1, RankedRaceCheckErrorV1> {
    let report = run_pliron_ranked_race_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(RankedRaceCheckErrorV1 { report })
    }
}

fn conflicting_witness<'a>(
    state: &'a AddressStateV1,
    access: AccessKindAttr,
    witness: &RankedRaceWitnessV1,
) -> Option<&'a RankedRaceWitnessV1> {
    match access {
        AccessKindAttr::Read => state
            .writes
            .different_from(&witness.invocation)
            .or_else(|| state.atomic_writes.different_from(&witness.invocation)),
        AccessKindAttr::Write => state
            .writes
            .different_from(&witness.invocation)
            .or_else(|| state.reads.different_from(&witness.invocation))
            .or_else(|| state.atomic_reads.different_from(&witness.invocation))
            .or_else(|| state.atomic_writes.different_from(&witness.invocation)),
        AccessKindAttr::AtomicRead => state
            .writes
            .different_from(&witness.invocation)
            .or_else(|| incompatible_atomic(&state.atomic_writes, witness)),
        AccessKindAttr::AtomicWrite | AccessKindAttr::AtomicReadModifyWrite => state
            .writes
            .different_from(&witness.invocation)
            .or_else(|| state.reads.different_from(&witness.invocation))
            .or_else(|| incompatible_atomic(&state.atomic_reads, witness))
            .or_else(|| incompatible_atomic(&state.atomic_writes, witness)),
    }
}

fn incompatible_atomic<'a>(
    state: &'a WitnessPairV1,
    witness: &RankedRaceWitnessV1,
) -> Option<&'a RankedRaceWitnessV1> {
    [state.first.as_ref(), state.second.as_ref()]
        .into_iter()
        .flatten()
        .find(|other| {
            other.invocation != witness.invocation
                && (!atomic_scope_covers_pair(other.atomic_scope, other, witness)
                    || !atomic_scope_covers_pair(witness.atomic_scope, witness, other))
        })
}

fn atomic_scope_covers_pair(
    scope: Option<AtomicScopeAttr>,
    first: &RankedRaceWitnessV1,
    second: &RankedRaceWitnessV1,
) -> bool {
    if first.invocation == second.invocation {
        return true;
    }
    match (first.workgroup, second.workgroup) {
        (Some(first), Some(second)) if first == second => matches!(
            scope,
            Some(
                AtomicScopeAttr::Workgroup
                    | AtomicScopeAttr::Agent
                    | AtomicScopeAttr::Device
                    | AtomicScopeAttr::System
            )
        ),
        _ => matches!(
            scope,
            Some(AtomicScopeAttr::Agent | AtomicScopeAttr::Device | AtomicScopeAttr::System)
        ),
    }
}

fn insert_witness(state: &mut AddressStateV1, witness: RankedRaceWitnessV1) {
    match witness.access {
        AccessKindAttr::Read => state.reads.insert(witness),
        AccessKindAttr::Write => state.writes.insert(witness),
        AccessKindAttr::AtomicRead => state.atomic_reads.insert(witness),
        AccessKindAttr::AtomicWrite | AccessKindAttr::AtomicReadModifyWrite => {
            state.atomic_writes.insert(witness);
        }
    }
}

fn symbolically_proves_disjoint(
    effects: &[EffectV1],
    sparse: &SparseIndexAnalysisV1,
    launch_extents: &[u64],
) -> bool {
    let mut by_view: HashMap<u64, Vec<&EffectV1>> = HashMap::new();
    for effect in effects {
        by_view
            .entry(effect.noalias_class)
            .or_default()
            .push(effect);
    }
    for effects in by_view.values() {
        let has_plain_write = effects
            .iter()
            .any(|effect| effect.kind == AccessKindAttr::Write);
        let has_plain_read = effects
            .iter()
            .any(|effect| effect.kind == AccessKindAttr::Read);
        let has_atomic_write = effects.iter().any(|effect| {
            matches!(
                effect.kind,
                AccessKindAttr::AtomicWrite | AccessKindAttr::AtomicReadModifyWrite
            )
        });
        if !has_plain_write && !has_plain_read && has_atomic_write {
            if effects.iter().all(|effect| {
                matches!(
                    effect.atomic_scope,
                    Some(
                        AtomicScopeAttr::Agent | AtomicScopeAttr::Device | AtomicScopeAttr::System
                    )
                )
            }) {
                continue;
            }
            let mut representative = None;
            for effect in effects {
                let Some(first) = representative else {
                    if !affine_map_is_injective(&effect.indices, sparse, launch_extents) {
                        return false;
                    }
                    representative = Some(&effect.indices);
                    continue;
                };
                if !same_index_formula(first, &effect.indices, sparse) {
                    return false;
                }
            }
            continue;
        }
        if !(has_plain_write || has_plain_read && has_atomic_write) {
            continue;
        }
        let relevant = effects
            .iter()
            .copied()
            .filter(|effect| {
                has_plain_write
                    || effect.kind == AccessKindAttr::Read
                    || matches!(
                        effect.kind,
                        AccessKindAttr::AtomicWrite | AccessKindAttr::AtomicReadModifyWrite
                    )
            })
            .collect::<Vec<_>>();
        if tiled_2d_effect_family_is_injective(&relevant, sparse, launch_extents) {
            continue;
        }
        let mut representative = None;
        for effect in relevant {
            let Some(first) = representative else {
                if !affine_map_is_injective(&effect.indices, sparse, launch_extents) {
                    return false;
                }
                representative = Some(&effect.indices);
                continue;
            };
            if !same_index_formula(first, &effect.indices, sparse) {
                return false;
            }
        }
    }
    true
}

fn tiled_2d_effect_family_is_injective(
    effects: &[&EffectV1],
    sparse: &SparseIndexAnalysisV1,
    launch_extents: &[u64],
) -> bool {
    let Some(first_index) = effects.first().and_then(|effect| effect.indices.first()) else {
        return false;
    };
    if effects.iter().any(|effect| effect.indices.len() != 1) {
        return false;
    }
    let first_fact = sparse.fact(*first_index);
    let Some(first) = first_fact.checked_tiled_2d() else {
        return false;
    };
    for effect in effects.iter().skip(1) {
        let index_fact = sparse.fact(effect.indices[0]);
        let Some(index) = index_fact.checked_tiled_2d() else {
            return false;
        };
        if index.invocation() != first.invocation()
            || index.runtime_layout() != first.runtime_layout()
            || index.geometry() != first.geometry()
        {
            return false;
        }
    }
    affine_facts_are_injective(&[first.invocation().clone()], launch_extents)
}

fn same_index_formula(first: &[Value], second: &[Value], sparse: &SparseIndexAnalysisV1) -> bool {
    first.len() == second.len()
        && first
            .iter()
            .zip(second)
            .all(|(first, second)| sparse.fact(*first) == sparse.fact(*second))
}

fn affine_map_is_injective(
    indices: &[Value],
    sparse: &SparseIndexAnalysisV1,
    launch_extents: &[u64],
) -> bool {
    let facts = indices
        .iter()
        .map(|index| sparse.fact(*index).affine().cloned())
        .collect::<Option<Vec<_>>>();
    facts.is_some_and(|facts| affine_facts_are_injective(&facts, launch_extents))
}

fn affine_facts_are_injective(facts: &[SparseAffineIndexV1], launch_extents: &[u64]) -> bool {
    if !facts
        .iter()
        .all(|affine| affine_is_total_over_launch(affine, launch_extents))
    {
        return false;
    }
    let active_dimensions = launch_extents
        .iter()
        .enumerate()
        .filter_map(|(dimension, extent)| (*extent != 1).then_some(dimension))
        .collect::<Vec<_>>();
    if active_dimensions.is_empty() {
        return true;
    }
    let matrix = facts
        .iter()
        .map(|affine| {
            active_dimensions
                .iter()
                .map(|dimension| affine.coefficients()[*dimension])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    modular_rank(matrix) == active_dimensions.len()
}

fn affine_is_total_over_launch(affine: &SparseAffineIndexV1, launch_extents: &[u64]) -> bool {
    let mut maximum = affine.constant_term();
    let mut dynamic_dimensions = 0_usize;
    for (dimension, coefficient) in affine.coefficients().iter().copied().enumerate() {
        if coefficient == 0 {
            continue;
        }
        let Some(extent) = launch_extents.get(dimension).copied() else {
            return false;
        };
        if extent == 0 {
            if coefficient != 1 {
                return false;
            }
            dynamic_dimensions += 1;
            continue;
        }
        let maximum_coordinate = extent - 1;
        let Some(contribution) = coefficient.checked_mul(maximum_coordinate) else {
            return false;
        };
        let Some(next) = maximum.checked_add(contribution) else {
            return false;
        };
        maximum = next;
    }
    dynamic_dimensions == 0 || dynamic_dimensions == 1 && maximum == 0
}

// Full rank modulo a prime implies full rank over the integers. A rank loss
// modulo this prime is treated as unknown and falls back to exact analysis.
fn modular_rank(mut matrix: Vec<Vec<u64>>) -> usize {
    const PRIME: u64 = (1_u64 << 61) - 1;
    let row_count = matrix.len();
    let column_count = matrix.first().map_or(0, Vec::len);
    for row in &mut matrix {
        for value in row {
            *value %= PRIME;
        }
    }
    let mut rank = 0_usize;
    for column in 0..column_count {
        let Some(pivot) = (rank..row_count).find(|row| matrix[*row][column] != 0) else {
            continue;
        };
        matrix.swap(rank, pivot);
        let inverse = modular_power(matrix[rank][column], PRIME - 2, PRIME);
        for value in &mut matrix[rank][column..column_count] {
            *value = modular_multiply(*value, inverse, PRIME);
        }
        let pivot_row = matrix[rank].clone();
        for (row_index, row) in matrix.iter_mut().enumerate() {
            if row_index == rank || row[column] == 0 {
                continue;
            }
            let factor = row[column];
            for (value, pivot) in row[column..column_count]
                .iter_mut()
                .zip(&pivot_row[column..column_count])
            {
                let product = modular_multiply(factor, *pivot, PRIME);
                *value = (*value + PRIME - product) % PRIME;
            }
        }
        rank += 1;
        if rank == row_count {
            break;
        }
    }
    rank
}

fn modular_power(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = modular_multiply(result, base, modulus);
        }
        base = modular_multiply(base, base, modulus);
        exponent >>= 1;
    }
    result
}

fn modular_multiply(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    ((u128::from(lhs) * u128::from(rhs)) % u128::from(modulus)) as u64
}

fn decode_invocation(mut linear: u64, extents: &[u64]) -> Vec<u64> {
    let mut invocation = Vec::with_capacity(extents.len());
    for extent in extents {
        invocation.push(linear % extent);
        linear /= extent;
    }
    invocation
}

fn sparse_failure(failure: SparseIndexFailureV1) -> String {
    match failure {
        SparseIndexFailureV1::ResourceLimit {
            resource,
            limit,
            actual,
        } => format!("{resource} count {actual} exceeds {limit}"),
        SparseIndexFailureV1::InconsistentLaunchExtent {
            dimension,
            first,
            second,
        } => format!(
            "invocation dimension {dimension} has inconsistent launch extents {first} and {second}"
        ),
    }
}

fn one(finding: RankedRaceFindingV1) -> RankedRaceReportV1 {
    RankedRaceReportV1 {
        findings: vec![finding],
    }
}

fn clean() -> RankedRaceReportV1 {
    RankedRaceReportV1 {
        findings: Vec::new(),
    }
}
