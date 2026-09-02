use crate::{ControlFlowAnalysis, ControlFlowErrors, analyze_control_flow};
use fe2o3_kernel_ir::{
    AssemblyOperandKind, BasicBlock, BlockId, Function, FunctionId, MAX_BLOCKS_V1,
    MAX_FUNCTIONS_V1, Module, Operation, OperationKind, Terminator, ValueId,
    VerifiedCanonicalKernelIrErrorV9, VerifiedCanonicalKernelIrIdentityV9,
    VerifiedCanonicalKernelIrV9, encode_module_v9,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

pub const MAX_KIR_ANALYSIS_FUNCTIONS: usize = MAX_FUNCTIONS_V1;
pub const MAX_POST_DOMINANCE_BLOCKS: usize = MAX_BLOCKS_V1;
pub const MAX_POST_DOMINANCE_EDGES: usize = MAX_BLOCKS_V1 * 2;
pub const MAX_POST_DOMINANCE_STORAGE_ITEMS: usize = MAX_BLOCKS_V1 * 48 + 8;
pub const MAX_POST_DOMINANCE_WORK_UNITS: usize = MAX_BLOCKS_V1 * 128;
pub const MAX_LIVENESS_VALUES: usize = MAX_BLOCKS_V1 * 16;
pub const MAX_LIVENESS_OPERANDS: usize = MAX_BLOCKS_V1 * 64;
pub const MAX_LIVENESS_FACTS: usize = MAX_BLOCKS_V1 * 64;
pub const MAX_LIVENESS_STORAGE_ITEMS: usize =
    MAX_LIVENESS_VALUES + MAX_LIVENESS_FACTS + MAX_BLOCKS_V1 * 7;
pub const MAX_LIVENESS_WORK_UNITS: usize = MAX_BLOCKS_V1 * 256;

/// Explicit ceilings for reusable Kernel IR analyses.
///
/// These limits cover the post-dominance and liveness stages. Their shared
/// control-flow prerequisite has separate fixed hard limits in
/// [`crate::ControlFlowResource`] and is cached by the manager. The defaults
/// are deliberately independent from the manager cache size: a caller cannot
/// obtain more downstream analysis work by repeatedly evicting entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KirAnalysisLimits {
    pub functions: usize,
    pub post_dominance_blocks: usize,
    pub post_dominance_edges: usize,
    pub post_dominance_storage_items: usize,
    pub post_dominance_work_units: usize,
    pub liveness_values: usize,
    pub liveness_operands: usize,
    pub liveness_facts: usize,
    pub liveness_storage_items: usize,
    pub liveness_work_units: usize,
}

impl KirAnalysisLimits {
    pub const DEFAULT: Self = Self {
        functions: MAX_KIR_ANALYSIS_FUNCTIONS,
        post_dominance_blocks: MAX_POST_DOMINANCE_BLOCKS,
        post_dominance_edges: MAX_POST_DOMINANCE_EDGES,
        post_dominance_storage_items: MAX_POST_DOMINANCE_STORAGE_ITEMS,
        post_dominance_work_units: MAX_POST_DOMINANCE_WORK_UNITS,
        liveness_values: MAX_LIVENESS_VALUES,
        liveness_operands: MAX_LIVENESS_OPERANDS,
        liveness_facts: MAX_LIVENESS_FACTS,
        liveness_storage_items: MAX_LIVENESS_STORAGE_ITEMS,
        liveness_work_units: MAX_LIVENESS_WORK_UNITS,
    };
}

impl Default for KirAnalysisLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnalysisEpoch(u64);

impl AnalysisEpoch {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum KirAnalysisResource {
    Functions,
    PostDominanceBlocks,
    PostDominanceEdges,
    PostDominanceStorageItems,
    PostDominanceWorkUnits,
    LivenessValues,
    LivenessOperands,
    LivenessFacts,
    LivenessStorageItems,
    LivenessWorkUnits,
}

impl fmt::Display for KirAnalysisResource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Functions => "analysis-manager functions",
            Self::PostDominanceBlocks => "post-dominance blocks",
            Self::PostDominanceEdges => "post-dominance edges",
            Self::PostDominanceStorageItems => "post-dominance storage items",
            Self::PostDominanceWorkUnits => "post-dominance work units",
            Self::LivenessValues => "liveness values",
            Self::LivenessOperands => "liveness operand occurrences",
            Self::LivenessFacts => "liveness facts",
            Self::LivenessStorageItems => "liveness storage items",
            Self::LivenessWorkUnits => "liveness work units",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KirAnalysisError {
    InvalidLimit {
        resource: KirAnalysisResource,
        requested: usize,
        maximum: usize,
    },
    ResourceLimit {
        resource: KirAnalysisResource,
        required: usize,
        limit: usize,
    },
    DuplicateFunction(FunctionId),
    UnknownFunction(FunctionId),
    ModuleAdmission(VerifiedCanonicalKernelIrErrorV9),
    EpochDidNotAdvance {
        current: AnalysisEpoch,
        requested: AnalysisEpoch,
    },
    SubjectMismatch {
        report: AnalysisSubject,
        manager: AnalysisSubject,
    },
    ControlFlow(ControlFlowErrors),
    MalformedLiveness(LivenessDiagnostic),
}

impl fmt::Display for KirAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimit {
                resource,
                requested,
                maximum,
            } => write!(
                formatter,
                "configured {resource} limit {requested} exceeds the hard maximum {maximum}"
            ),
            Self::ResourceLimit {
                resource,
                required,
                limit,
            } => write!(
                formatter,
                "{resource} require {required} items, exceeding the deterministic limit {limit}"
            ),
            Self::DuplicateFunction(function) => {
                write!(formatter, "duplicate Kernel IR function {function}")
            }
            Self::UnknownFunction(function) => {
                write!(formatter, "unknown Kernel IR function {function}")
            }
            Self::ModuleAdmission(error) => {
                write!(formatter, "cannot admit Kernel IR for analysis: {error}")
            }
            Self::EpochDidNotAdvance { current, requested } => write!(
                formatter,
                "analysis epoch {} must advance beyond {} before invalidation",
                requested.value(),
                current.value()
            ),
            Self::SubjectMismatch { report, manager } => write!(
                formatter,
                "analysis subject {:?} at epoch {} does not match manager subject {:?} at epoch {}",
                report.module_identity,
                report.epoch.value(),
                manager.module_identity,
                manager.epoch.value()
            ),
            Self::ControlFlow(error) => error.fmt(formatter),
            Self::MalformedLiveness(diagnostic) => diagnostic.fmt(formatter),
        }
    }
}

impl Error for KirAnalysisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ControlFlow(error) => Some(error),
            Self::ModuleAdmission(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ControlFlowErrors> for KirAnalysisError {
    fn from(error: ControlFlowErrors) -> Self {
        Self::ControlFlow(error)
    }
}

impl From<VerifiedCanonicalKernelIrErrorV9> for KirAnalysisError {
    fn from(error: VerifiedCanonicalKernelIrErrorV9) -> Self {
        Self::ModuleAdmission(error)
    }
}

/// Exact immutable subject of a cached analysis result.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnalysisSubject {
    epoch: AnalysisEpoch,
    module_identity: VerifiedCanonicalKernelIrIdentityV9,
}

impl AnalysisSubject {
    pub const fn epoch(self) -> AnalysisEpoch {
        self.epoch
    }

    pub const fn module_identity(self) -> VerifiedCanonicalKernelIrIdentityV9 {
        self.module_identity
    }
}

/// Cheap detached cache handle whose facts remain inaccessible until matched
/// against the current analysis-manager subject.
#[derive(Clone, Debug)]
pub struct SubjectStampedAnalysis<T> {
    subject: AnalysisSubject,
    analysis: Arc<T>,
}

impl<T> SubjectStampedAnalysis<T> {
    pub const fn subject(&self) -> AnalysisSubject {
        self.subject
    }

    pub fn current<'analysis, 'module>(
        &'analysis self,
        manager: &'analysis KirAnalysisManager<'module>,
    ) -> Result<CurrentAnalysis<'analysis, 'module, T>, KirAnalysisError> {
        if self.subject != manager.subject {
            return Err(KirAnalysisError::SubjectMismatch {
                report: self.subject,
                manager: manager.subject,
            });
        }
        Ok(CurrentAnalysis {
            analysis: &self.analysis,
            _manager: manager,
        })
    }

    pub fn shares_cached_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.analysis, &other.analysis)
    }
}

/// Facts proven current for the immutably borrowed manager subject.
pub struct CurrentAnalysis<'analysis, 'module, T> {
    analysis: &'analysis T,
    _manager: &'analysis KirAnalysisManager<'module>,
}

impl<T> Deref for CurrentAnalysis<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.analysis
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PostDominanceResourceUsage {
    blocks: usize,
    edges: usize,
    storage_items: usize,
    work_units: usize,
}

impl PostDominanceResourceUsage {
    pub const fn blocks(self) -> usize {
        self.blocks
    }

    pub const fn edges(self) -> usize {
        self.edges
    }

    pub const fn storage_items(self) -> usize {
        self.storage_items
    }

    pub const fn work_units(self) -> usize {
        self.work_units
    }
}

/// Immutable post-dominance facts for the reachable, exit-reaching CFG.
///
/// Reachable blocks from which no terminal block is reachable intentionally
/// have no post-dominance facts. Multiple terminal blocks are represented by
/// an internal virtual exit; a block immediately post-dominated only by that
/// virtual exit has no real immediate post-dominator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostDominanceAnalysis {
    function: FunctionId,
    available: BTreeSet<BlockId>,
    immediate: BTreeMap<BlockId, Option<BlockId>>,
    tree_preorder: BTreeMap<BlockId, usize>,
    tree_subtree_end: BTreeMap<BlockId, usize>,
    resource_usage: PostDominanceResourceUsage,
}

impl PostDominanceAnalysis {
    pub fn function(&self) -> &FunctionId {
        &self.function
    }

    pub fn available_blocks(&self) -> &BTreeSet<BlockId> {
        &self.available
    }

    pub fn is_available(&self, block: BlockId) -> bool {
        self.available.contains(&block)
    }

    /// Returns `None` for an unavailable block. A terminal block or a block
    /// whose immediate parent is the virtual exit is `Some(None)`.
    pub fn immediate_post_dominator(&self, block: BlockId) -> Option<Option<BlockId>> {
        self.immediate.get(&block).copied()
    }

    pub fn post_dominates(&self, post_dominator: BlockId, block: BlockId) -> bool {
        let (Some(start), Some(candidate), Some(end)) = (
            self.tree_preorder.get(&post_dominator),
            self.tree_preorder.get(&block),
            self.tree_subtree_end.get(&post_dominator),
        ) else {
            return false;
        };
        start <= candidate && candidate < end
    }

    pub const fn resource_usage(&self) -> PostDominanceResourceUsage {
        self.resource_usage
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LivenessResourceUsage {
    values: usize,
    operands: usize,
    facts: usize,
    storage_items: usize,
    work_units: usize,
}

impl LivenessResourceUsage {
    pub const fn values(self) -> usize {
        self.values
    }

    pub const fn operands(self) -> usize {
        self.operands
    }

    pub const fn facts(self) -> usize {
        self.facts
    }

    pub const fn storage_items(self) -> usize {
        self.storage_items
    }

    pub const fn work_units(self) -> usize {
        self.work_units
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LivenessDiagnostic {
    DuplicateValueDefinition {
        value: ValueId,
    },
    UnknownValueUse {
        value: ValueId,
    },
    EdgeArgumentArity {
        source: BlockId,
        target: BlockId,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for LivenessDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateValueDefinition { value } => {
                write!(formatter, "SSA value {value} has multiple definitions")
            }
            Self::UnknownValueUse { value } => {
                write!(formatter, "SSA value {value} is used without a definition")
            }
            Self::EdgeArgumentArity {
                source,
                target,
                expected,
                actual,
            } => write!(
                formatter,
                "edge {source} -> {target} supplies {actual} arguments for {expected} block parameters"
            ),
        }
    }
}

/// Immutable block-level SSA liveness facts.
///
/// `live_in` is the program point after block parameters are defined and
/// before the first operation. `live_out` is the edge-dependent point after
/// local operations: live block parameters are translated to the matching
/// edge argument, so unused parameters do not keep arguments alive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LivenessAnalysis {
    function: FunctionId,
    live_in: BTreeMap<BlockId, BTreeSet<ValueId>>,
    live_out: BTreeMap<BlockId, BTreeSet<ValueId>>,
    resource_usage: LivenessResourceUsage,
}

impl LivenessAnalysis {
    pub fn function(&self) -> &FunctionId {
        &self.function
    }

    pub fn live_in(&self, block: BlockId) -> Option<&BTreeSet<ValueId>> {
        self.live_in.get(&block)
    }

    pub fn live_out(&self, block: BlockId) -> Option<&BTreeSet<ValueId>> {
        self.live_out.get(&block)
    }

    pub fn is_live_in(&self, block: BlockId, value: ValueId) -> bool {
        self.live_in
            .get(&block)
            .is_some_and(|values| values.contains(&value))
    }

    pub fn is_live_out(&self, block: BlockId, value: ValueId) -> bool {
        self.live_out
            .get(&block)
            .is_some_and(|values| values.contains(&value))
    }

    pub const fn resource_usage(&self) -> LivenessResourceUsage {
        self.resource_usage
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KirAnalysisCacheStats {
    pub control_flow_runs: usize,
    pub post_dominance_runs: usize,
    pub liveness_runs: usize,
}

/// Epoch-scoped cache over one immutably borrowed Kernel IR module.
///
/// Construction performs bounded canonical V9 admission and semantic
/// verification once. Mutation is excluded by the module borrow. Moving to a
/// transformed module consumes the manager, requires a strictly newer epoch,
/// and starts with empty caches. Detached cache handles must be matched back to
/// this manager's exact module identity and epoch before their facts are
/// accessible.
///
/// Legacy uniformity analysis is intentionally not exposed here: it owns a
/// separate unmetered fixed-point implementation and cannot satisfy this
/// manager's resource contract until it consumes these cached prerequisites.
pub struct KirAnalysisManager<'module> {
    module: &'module Module,
    epoch: AnalysisEpoch,
    subject: AnalysisSubject,
    limits: KirAnalysisLimits,
    functions: BTreeMap<FunctionId, usize>,
    control_flow: BTreeMap<FunctionId, Result<Arc<ControlFlowAnalysis>, Arc<ControlFlowErrors>>>,
    post_dominance: BTreeMap<FunctionId, Result<Arc<PostDominanceAnalysis>, Arc<KirAnalysisError>>>,
    liveness: BTreeMap<FunctionId, Result<Arc<LivenessAnalysis>, Arc<KirAnalysisError>>>,
    stats: KirAnalysisCacheStats,
}

impl<'module> KirAnalysisManager<'module> {
    pub fn new(module: &'module Module, epoch: AnalysisEpoch) -> Result<Self, KirAnalysisError> {
        Self::with_limits(module, epoch, KirAnalysisLimits::DEFAULT)
    }

    pub fn with_limits(
        module: &'module Module,
        epoch: AnalysisEpoch,
        limits: KirAnalysisLimits,
    ) -> Result<Self, KirAnalysisError> {
        validate_limits(limits)?;
        check_limit(
            KirAnalysisResource::Functions,
            module.functions.len(),
            limits.functions,
        )?;
        // Encode through the borrowed module first. The encoder's 16 MiB hard
        // ceiling applies before any proportional copy of caller-owned IR.
        let canonical_bytes =
            encode_module_v9(module).map_err(VerifiedCanonicalKernelIrErrorV9::Encode)?;
        let (canonical, decoded) =
            VerifiedCanonicalKernelIrV9::from_canonical_bytes_with_module(canonical_bytes)?;
        if &decoded != module {
            return Err(KirAnalysisError::ModuleAdmission(
                VerifiedCanonicalKernelIrErrorV9::RoundTripMismatch,
            ));
        }
        drop(decoded);
        let subject = AnalysisSubject {
            epoch,
            module_identity: *canonical.identity(),
        };
        let mut functions = BTreeMap::new();
        for (index, function) in module.functions.iter().enumerate() {
            if functions.insert(function.id.clone(), index).is_some() {
                return Err(KirAnalysisError::DuplicateFunction(function.id.clone()));
            }
        }
        Ok(Self {
            module,
            epoch,
            subject,
            limits,
            functions,
            control_flow: BTreeMap::new(),
            post_dominance: BTreeMap::new(),
            liveness: BTreeMap::new(),
            stats: KirAnalysisCacheStats::default(),
        })
    }

    pub const fn epoch(&self) -> AnalysisEpoch {
        self.epoch
    }

    pub const fn limits(&self) -> KirAnalysisLimits {
        self.limits
    }

    pub const fn subject(&self) -> AnalysisSubject {
        self.subject
    }

    pub const fn cache_stats(&self) -> KirAnalysisCacheStats {
        self.stats
    }

    pub fn invalidate<'next>(
        self,
        module: &'next Module,
        next_epoch: AnalysisEpoch,
    ) -> Result<KirAnalysisManager<'next>, KirAnalysisError> {
        if next_epoch <= self.epoch {
            return Err(KirAnalysisError::EpochDidNotAdvance {
                current: self.epoch,
                requested: next_epoch,
            });
        }
        KirAnalysisManager::with_limits(module, next_epoch, self.limits)
    }

    pub fn control_flow(
        &mut self,
        function: &FunctionId,
    ) -> Result<SubjectStampedAnalysis<ControlFlowAnalysis>, KirAnalysisError> {
        self.function(function)?;
        if let Some(cached) = self.control_flow.get(function) {
            let analysis = cached
                .clone()
                .map_err(|error| KirAnalysisError::ControlFlow((*error).clone()))?;
            return Ok(self.stamp(analysis));
        }
        self.stats.control_flow_runs += 1;
        let result = analyze_control_flow(self.function(function)?)
            .map(Arc::new)
            .map_err(Arc::new);
        self.control_flow.insert(function.clone(), result.clone());
        let analysis = result.map_err(|error| KirAnalysisError::ControlFlow((*error).clone()))?;
        Ok(self.stamp(analysis))
    }

    pub fn post_dominance(
        &mut self,
        function: &FunctionId,
    ) -> Result<SubjectStampedAnalysis<PostDominanceAnalysis>, KirAnalysisError> {
        self.function(function)?;
        if let Some(cached) = self.post_dominance.get(function) {
            return clone_cached(cached).map(|analysis| self.stamp(analysis));
        }
        let control_flow = match self.control_flow(function) {
            Ok(analysis) => analysis,
            Err(error) => {
                self.post_dominance
                    .insert(function.clone(), Err(Arc::new(error.clone())));
                return Err(error);
            }
        };
        self.stats.post_dominance_runs += 1;
        let result = analyze_post_dominance_from_control_flow(
            self.function(function)?,
            control_flow.current(self)?.analysis,
            self.limits,
        )
        .map(Arc::new)
        .map_err(Arc::new);
        self.post_dominance.insert(function.clone(), result.clone());
        clone_cached(&result).map(|analysis| self.stamp(analysis))
    }

    pub fn liveness(
        &mut self,
        function: &FunctionId,
    ) -> Result<SubjectStampedAnalysis<LivenessAnalysis>, KirAnalysisError> {
        self.function(function)?;
        if let Some(cached) = self.liveness.get(function) {
            return clone_cached(cached).map(|analysis| self.stamp(analysis));
        }
        let control_flow = match self.control_flow(function) {
            Ok(analysis) => analysis,
            Err(error) => {
                self.liveness
                    .insert(function.clone(), Err(Arc::new(error.clone())));
                return Err(error);
            }
        };
        self.stats.liveness_runs += 1;
        let result = analyze_liveness_from_control_flow(
            self.function(function)?,
            control_flow.current(self)?.analysis,
            self.limits,
        )
        .map(Arc::new)
        .map_err(Arc::new);
        self.liveness.insert(function.clone(), result.clone());
        clone_cached(&result).map(|analysis| self.stamp(analysis))
    }

    fn function(&self, function: &FunctionId) -> Result<&Function, KirAnalysisError> {
        let Some(index) = self.functions.get(function).copied() else {
            return Err(KirAnalysisError::UnknownFunction(function.clone()));
        };
        Ok(&self.module.functions[index])
    }

    fn stamp<T>(&self, analysis: Arc<T>) -> SubjectStampedAnalysis<T> {
        SubjectStampedAnalysis {
            subject: self.subject,
            analysis,
        }
    }
}

fn clone_cached<T>(
    cached: &Result<Arc<T>, Arc<KirAnalysisError>>,
) -> Result<Arc<T>, KirAnalysisError> {
    cached.clone().map_err(|error| (*error).clone())
}

#[cfg(test)]
fn analyze_post_dominance(function: &Function) -> Result<PostDominanceAnalysis, KirAnalysisError> {
    analyze_post_dominance_with_limits(function, KirAnalysisLimits::DEFAULT)
}

#[cfg(test)]
fn analyze_post_dominance_with_limits(
    function: &Function,
    limits: KirAnalysisLimits,
) -> Result<PostDominanceAnalysis, KirAnalysisError> {
    validate_limits(limits)?;
    let control_flow = analyze_control_flow(function)?;
    analyze_post_dominance_from_control_flow(function, &control_flow, limits)
}

fn analyze_post_dominance_from_control_flow(
    function: &Function,
    control_flow: &ControlFlowAnalysis,
    limits: KirAnalysisLimits,
) -> Result<PostDominanceAnalysis, KirAnalysisError> {
    let body = function.body.as_ref().ok_or_else(|| {
        KirAnalysisError::ControlFlow(
            analyze_control_flow(function).expect_err("a declaration has no CFG"),
        )
    })?;
    let mut meter = AnalysisMeter::new(
        KirAnalysisResource::PostDominanceWorkUnits,
        limits.post_dominance_work_units,
    );
    let input_block_count = body.blocks.len();
    let input_edge_count = control_flow.resource_usage().edges();
    check_limit(
        KirAnalysisResource::PostDominanceBlocks,
        input_block_count,
        limits.post_dominance_blocks,
    )?;
    check_limit(
        KirAnalysisResource::PostDominanceEdges,
        input_edge_count,
        limits.post_dominance_edges,
    )?;
    meter.charge(checked_add(input_block_count, input_edge_count))?;

    // Bound all temporary and retained collections before constructing any of
    // them. The estimate intentionally uses every input block and edge even
    // though exitless regions produce no retained post-dominance facts.
    let storage_items = checked_add(
        checked_multiply(input_block_count, 40),
        checked_add(checked_multiply(input_edge_count, 4), 8),
    );
    check_limit(
        KirAnalysisResource::PostDominanceStorageItems,
        storage_items,
        limits.post_dominance_storage_items,
    )?;

    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let exits = control_flow
        .reachable_blocks()
        .iter()
        .copied()
        .filter(|block| {
            matches!(
                blocks[block].terminator,
                Some(Terminator::Return { .. } | Terminator::Unreachable)
            )
        })
        .collect::<BTreeSet<_>>();
    meter.charge(control_flow.reachable_blocks().len())?;
    let mut available = BTreeSet::new();
    let mut pending = exits.iter().copied().collect::<Vec<_>>();
    while let Some(block) = pending.pop() {
        meter.charge(1)?;
        if !available.insert(block) {
            continue;
        }
        let predecessors = control_flow
            .predecessors(block)
            .expect("reachable block belongs to validated CFG");
        meter.charge(predecessors.len())?;
        pending.extend(
            predecessors
                .iter()
                .rev()
                .filter(|predecessor| control_flow.is_reachable(**predecessor))
                .copied(),
        );
    }
    meter.charge(available.len())?;
    let block_ids = available.iter().copied().collect::<Vec<_>>();
    meter.charge(block_ids.len())?;
    let positions = block_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(index, block)| (block, index))
        .collect::<BTreeMap<_, _>>();
    let virtual_exit = block_ids.len();
    let node_count = checked_add(block_ids.len(), 1);

    // Successors and predecessors in the reversed graph. The virtual root has
    // an edge to every real exit.
    let mut reverse_successors = vec![Vec::new(); node_count];
    let mut reverse_predecessors = vec![Vec::new(); node_count];
    for (block, position) in &positions {
        meter.charge(1)?;
        for predecessor in control_flow.predecessors(*block).into_iter().flatten() {
            meter.charge(1)?;
            if let Some(predecessor_position) = positions.get(predecessor).copied() {
                reverse_successors[*position].push(predecessor_position);
                reverse_predecessors[predecessor_position].push(*position);
            }
        }
    }
    for exit in &exits {
        meter.charge(1)?;
        let position = positions[exit];
        reverse_successors[virtual_exit].push(position);
        reverse_predecessors[position].push(virtual_exit);
    }
    for edges in reverse_successors
        .iter_mut()
        .chain(reverse_predecessors.iter_mut())
    {
        meter.charge(checked_add(edges.len(), 1))?;
        edges.sort_unstable();
        edges.dedup();
    }

    let reverse_postorder = reverse_postorder(virtual_exit, &reverse_successors, &mut meter)?;
    let mut order = vec![usize::MAX; node_count];
    for (position, block) in reverse_postorder.iter().copied().enumerate() {
        meter.charge(1)?;
        order[block] = position;
    }
    let mut immediate = vec![None; node_count];
    immediate[virtual_exit] = Some(virtual_exit);
    loop {
        let mut changed = false;
        for block in reverse_postorder.iter().copied().skip(1) {
            meter.charge(1)?;
            let mut candidates = reverse_predecessors[block]
                .iter()
                .copied()
                .filter(|candidate| immediate[*candidate].is_some());
            let Some(mut next) = candidates.next() else {
                continue;
            };
            meter.charge(1)?;
            for candidate in candidates {
                meter.charge(1)?;
                next = intersect_tree(next, candidate, &immediate, &order, &mut meter)?;
            }
            if immediate[block] != Some(next) {
                immediate[block] = Some(next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    immediate[virtual_exit] = None;

    let mut children = vec![Vec::new(); node_count];
    for (block, parent) in immediate.iter().copied().enumerate() {
        meter.charge(1)?;
        if let Some(parent) = parent {
            children[parent].push(block);
        }
    }
    for values in &mut children {
        meter.charge(checked_add(values.len(), 1))?;
        values.sort_unstable();
    }
    let (tree_preorder, tree_subtree_end) = tree_intervals(virtual_exit, &children, &mut meter)?;

    meter.charge(checked_multiply(block_ids.len(), 3))?;
    let immediate_report = block_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(position, block)| {
            let parent = immediate[position].and_then(|parent| {
                if parent == virtual_exit {
                    None
                } else {
                    Some(block_ids[parent])
                }
            });
            (block, parent)
        })
        .collect();
    let preorder_report = block_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(position, block)| (block, tree_preorder[position]))
        .collect();
    let subtree_report = block_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(position, block)| (block, tree_subtree_end[position]))
        .collect();

    Ok(PostDominanceAnalysis {
        function: function.id.clone(),
        available,
        immediate: immediate_report,
        tree_preorder: preorder_report,
        tree_subtree_end: subtree_report,
        resource_usage: PostDominanceResourceUsage {
            blocks: input_block_count,
            edges: input_edge_count,
            storage_items,
            work_units: meter.used,
        },
    })
}

#[cfg(test)]
fn analyze_liveness(function: &Function) -> Result<LivenessAnalysis, KirAnalysisError> {
    analyze_liveness_with_limits(function, KirAnalysisLimits::DEFAULT)
}

#[cfg(test)]
fn analyze_liveness_with_limits(
    function: &Function,
    limits: KirAnalysisLimits,
) -> Result<LivenessAnalysis, KirAnalysisError> {
    validate_limits(limits)?;
    let control_flow = analyze_control_flow(function)?;
    analyze_liveness_from_control_flow(function, &control_flow, limits)
}

fn analyze_liveness_from_control_flow(
    function: &Function,
    control_flow: &ControlFlowAnalysis,
    limits: KirAnalysisLimits,
) -> Result<LivenessAnalysis, KirAnalysisError> {
    let body = function.body.as_ref().ok_or_else(|| {
        KirAnalysisError::ControlFlow(
            analyze_control_flow(function).expect_err("a declaration has no CFG"),
        )
    })?;
    let mut meter = AnalysisMeter::new(
        KirAnalysisResource::LivenessWorkUnits,
        limits.liveness_work_units,
    );
    let preflight = preflight_liveness(body, limits, &mut meter)?;
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    for block in &body.blocks {
        meter.charge(1)?;
        validate_edge_arities(block, &blocks, &mut meter)?;
    }

    let mut definitions = BTreeSet::new();
    for value in &body.parameters {
        meter.charge(1)?;
        insert_liveness_definition(&mut definitions, *value, limits.liveness_values)?;
    }
    for block in &body.blocks {
        meter.charge(1)?;
        for definition in &block.parameters {
            meter.charge(1)?;
            insert_liveness_definition(&mut definitions, definition.id, limits.liveness_values)?;
        }
        for operation in &block.operations {
            meter.charge(1)?;
            for definition in &operation.results {
                meter.charge(1)?;
                insert_liveness_definition(
                    &mut definitions,
                    definition.id,
                    limits.liveness_values,
                )?;
            }
        }
    }
    check_limit(
        KirAnalysisResource::LivenessValues,
        definitions.len(),
        limits.liveness_values,
    )?;
    for block in &body.blocks {
        meter.charge(1)?;
        for operation in &block.operations {
            meter.charge(1)?;
            visit_operation_operands(operation, |operand| {
                meter.charge(1)?;
                if !definitions.contains(&operand) {
                    return Err(KirAnalysisError::MalformedLiveness(
                        LivenessDiagnostic::UnknownValueUse { value: operand },
                    ));
                }
                Ok(())
            })?;
        }
        if let Some(terminator) = &block.terminator {
            meter.charge(1)?;
            meter.charge(terminator_successor_count(terminator))?;
            visit_terminator_operands(terminator, |operand| {
                meter.charge(1)?;
                if !definitions.contains(&operand) {
                    return Err(KirAnalysisError::MalformedLiveness(
                        LivenessDiagnostic::UnknownValueUse { value: operand },
                    ));
                }
                Ok(())
            })?;
        }
    }

    meter.charge(body.blocks.len())?;
    let parameter_fact_count = preflight.parameter_facts;
    check_limit(
        KirAnalysisResource::LivenessFacts,
        parameter_fact_count,
        limits.liveness_facts,
    )?;
    check_liveness_storage(
        preflight.base_storage_items,
        parameter_fact_count,
        limits.liveness_storage_items,
    )?;
    meter.charge(checked_add(body.blocks.len(), parameter_fact_count))?;
    let parameter_positions = body
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| (parameter.id, index))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut fact_count = parameter_fact_count;

    let mut local_uses = BTreeMap::new();
    let mut operation_definitions = BTreeMap::new();
    for block_id in control_flow.reachable_blocks() {
        meter.charge(1)?;
        let block = blocks[block_id];
        let mut defined = BTreeSet::new();
        let mut used = BTreeSet::new();
        for operation in &block.operations {
            meter.charge(1)?;
            visit_operation_operands(operation, |operand| {
                meter.charge(1)?;
                if !defined.contains(&operand) {
                    insert_liveness_fact(
                        &mut used,
                        operand,
                        &mut fact_count,
                        limits.liveness_facts,
                        preflight.base_storage_items,
                        limits.liveness_storage_items,
                    )?;
                }
                Ok(())
            })?;
            for result in operation.result_ids() {
                meter.charge(1)?;
                insert_liveness_fact(
                    &mut defined,
                    result,
                    &mut fact_count,
                    limits.liveness_facts,
                    preflight.base_storage_items,
                    limits.liveness_storage_items,
                )?;
            }
        }
        let terminator = block
            .terminator
            .as_ref()
            .expect("control-flow analysis validated terminators");
        meter.charge(1)?;
        visit_terminator_local_operands(terminator, |operand| {
            meter.charge(1)?;
            if !defined.contains(&operand) {
                insert_liveness_fact(
                    &mut used,
                    operand,
                    &mut fact_count,
                    limits.liveness_facts,
                    preflight.base_storage_items,
                    limits.liveness_storage_items,
                )?;
            }
            Ok(())
        })?;
        local_uses.insert(*block_id, used);
        operation_definitions.insert(*block_id, defined);
    }

    let mut live_in = control_flow
        .reachable_blocks()
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut live_out = live_in.clone();
    let mut pending = control_flow.reachable_blocks().clone();
    while let Some(block_id) = pending.pop_last() {
        meter.charge(1)?;
        let block = blocks[&block_id];
        let terminator = block
            .terminator
            .as_ref()
            .expect("control-flow analysis validated terminators");
        let mut next_out = BTreeSet::new();
        let mut temporary_fact_count = fact_count;
        visit_edges(terminator, |target, arguments| {
            if !control_flow.is_reachable(target) {
                return Ok(());
            }
            meter.charge(1)?;
            for value in &live_in[&target] {
                meter.charge(1)?;
                let transferred = parameter_positions[&target]
                    .get(value)
                    .map_or(*value, |index| arguments[*index]);
                insert_liveness_fact(
                    &mut next_out,
                    transferred,
                    &mut temporary_fact_count,
                    limits.liveness_facts,
                    preflight.base_storage_items,
                    limits.liveness_storage_items,
                )?;
            }
            Ok(())
        })?;
        let mut next_in = BTreeSet::new();
        for value in &local_uses[&block_id] {
            meter.charge(1)?;
            insert_liveness_fact(
                &mut next_in,
                *value,
                &mut temporary_fact_count,
                limits.liveness_facts,
                preflight.base_storage_items,
                limits.liveness_storage_items,
            )?;
        }
        for value in &next_out {
            meter.charge(1)?;
            if !operation_definitions[&block_id].contains(value) {
                insert_liveness_fact(
                    &mut next_in,
                    *value,
                    &mut temporary_fact_count,
                    limits.liveness_facts,
                    preflight.base_storage_items,
                    limits.liveness_storage_items,
                )?;
            }
        }
        let old_in_len = live_in[&block_id].len();
        let old_out_len = live_out[&block_id].len();
        let next_fact_count = fact_count
            .saturating_sub(old_in_len)
            .saturating_sub(old_out_len)
            .saturating_add(next_in.len())
            .saturating_add(next_out.len());
        check_limit(
            KirAnalysisResource::LivenessFacts,
            next_fact_count,
            limits.liveness_facts,
        )?;
        fact_count = next_fact_count;
        let in_changed = live_in[&block_id] != next_in;
        live_in.insert(block_id, next_in);
        live_out.insert(block_id, next_out);
        if in_changed {
            let predecessors = control_flow
                .predecessors(block_id)
                .expect("reachable block belongs to validated CFG");
            meter.charge(predecessors.len())?;
            pending.extend(
                predecessors
                    .iter()
                    .filter(|predecessor| control_flow.is_reachable(**predecessor))
                    .copied(),
            );
        }
    }

    Ok(LivenessAnalysis {
        function: function.id.clone(),
        live_in,
        live_out,
        resource_usage: LivenessResourceUsage {
            values: definitions.len(),
            operands: preflight.operands,
            facts: fact_count,
            storage_items: checked_add(preflight.base_storage_items, fact_count),
            work_units: meter.used,
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LivenessPreflight {
    operands: usize,
    parameter_facts: usize,
    base_storage_items: usize,
}

fn preflight_liveness(
    body: &fe2o3_kernel_ir::FunctionBody,
    limits: KirAnalysisLimits,
    meter: &mut AnalysisMeter,
) -> Result<LivenessPreflight, KirAnalysisError> {
    let mut definitions = body.parameters.len();
    let mut operands = 0usize;
    let mut parameter_facts = 0usize;
    meter.charge(body.parameters.len())?;
    check_limit(
        KirAnalysisResource::LivenessValues,
        definitions,
        limits.liveness_values,
    )?;

    for block in &body.blocks {
        meter.charge(1)?;
        definitions = checked_add(definitions, block.parameters.len());
        parameter_facts = checked_add(parameter_facts, block.parameters.len());
        meter.charge(block.parameters.len())?;
        check_limit(
            KirAnalysisResource::LivenessValues,
            definitions,
            limits.liveness_values,
        )?;
        check_limit(
            KirAnalysisResource::LivenessFacts,
            parameter_facts,
            limits.liveness_facts,
        )?;
        for operation in &block.operations {
            meter.charge(1)?;
            definitions = checked_add(definitions, operation.results.len());
            meter.charge(operation.results.len())?;
            check_limit(
                KirAnalysisResource::LivenessValues,
                definitions,
                limits.liveness_values,
            )?;
            visit_operation_operands(operation, |_| {
                operands = checked_add(operands, 1);
                meter.charge(1)?;
                check_limit(
                    KirAnalysisResource::LivenessOperands,
                    operands,
                    limits.liveness_operands,
                )
            })?;
        }
        if let Some(terminator) = &block.terminator {
            meter.charge(1)?;
            meter.charge(terminator_successor_count(terminator))?;
            visit_terminator_operands(terminator, |_| {
                operands = checked_add(operands, 1);
                meter.charge(1)?;
                check_limit(
                    KirAnalysisResource::LivenessOperands,
                    operands,
                    limits.liveness_operands,
                )
            })?;
        }
    }

    // Entries in the definition set plus outer block-indexed maps and the
    // block worklist. Inner set entries are charged dynamically as facts.
    let base_storage_items = checked_add(definitions, checked_multiply(body.blocks.len(), 7));
    check_liveness_storage(
        base_storage_items,
        parameter_facts,
        limits.liveness_storage_items,
    )?;
    Ok(LivenessPreflight {
        operands,
        parameter_facts,
        base_storage_items,
    })
}

fn validate_edge_arities(
    block: &BasicBlock,
    blocks: &BTreeMap<BlockId, &BasicBlock>,
    meter: &mut AnalysisMeter,
) -> Result<(), KirAnalysisError> {
    let terminator = block
        .terminator
        .as_ref()
        .expect("control-flow analysis validated terminators");
    visit_edges(terminator, |target, arguments| {
        meter.charge(checked_add(arguments.len(), 1))?;
        let expected = blocks[&target].parameters.len();
        if arguments.len() != expected {
            return Err(KirAnalysisError::MalformedLiveness(
                LivenessDiagnostic::EdgeArgumentArity {
                    source: block.id,
                    target,
                    expected,
                    actual: arguments.len(),
                },
            ));
        }
        Ok(())
    })
}

fn insert_liveness_fact(
    facts: &mut BTreeSet<ValueId>,
    value: ValueId,
    fact_count: &mut usize,
    limit: usize,
    base_storage_items: usize,
    storage_limit: usize,
) -> Result<(), KirAnalysisError> {
    if facts.contains(&value) {
        return Ok(());
    }
    let required = checked_add(*fact_count, 1);
    check_limit(KirAnalysisResource::LivenessFacts, required, limit)?;
    check_liveness_storage(base_storage_items, required, storage_limit)?;
    facts.insert(value);
    *fact_count = required;
    Ok(())
}

fn check_liveness_storage(
    base_storage_items: usize,
    facts: usize,
    limit: usize,
) -> Result<(), KirAnalysisError> {
    check_limit(
        KirAnalysisResource::LivenessStorageItems,
        checked_add(base_storage_items, facts),
        limit,
    )
}

fn insert_liveness_definition(
    definitions: &mut BTreeSet<ValueId>,
    value: ValueId,
    limit: usize,
) -> Result<(), KirAnalysisError> {
    if definitions.contains(&value) {
        return Err(KirAnalysisError::MalformedLiveness(
            LivenessDiagnostic::DuplicateValueDefinition { value },
        ));
    }
    let required = checked_add(definitions.len(), 1);
    check_limit(KirAnalysisResource::LivenessValues, required, limit)?;
    definitions.insert(value);
    Ok(())
}

/// Visits operands without cloning caller-controlled argument vectors. The
/// fallback arm is deliberately exhaustive over only fixed-arity operations,
/// so adding another variable-arity operation requires updating this policy.
fn visit_operation_operands(
    operation: &Operation,
    mut visitor: impl FnMut(ValueId) -> Result<(), KirAnalysisError>,
) -> Result<(), KirAnalysisError> {
    match &operation.kind {
        OperationKind::Call { arguments, .. } => {
            for operand in arguments {
                visitor(*operand)?;
            }
        }
        OperationKind::InlineAssembly(assembly) => {
            for operand in &assembly.operands {
                match &operand.kind {
                    AssemblyOperandKind::Input(value)
                    | AssemblyOperandKind::InOut { input: value, .. } => visitor(*value)?,
                    AssemblyOperandKind::Output { .. } | AssemblyOperandKind::ImmediateI32(_) => {}
                }
            }
        }
        OperationKind::Constant(_)
        | OperationKind::Intrinsic(_)
        | OperationKind::MemoryIntrinsic(_)
        | OperationKind::Unary { .. }
        | OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::Cast { .. }
        | OperationKind::Select { .. }
        | OperationKind::Alloca { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. }
        | OperationKind::Load { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::Store { .. }
        | OperationKind::Barrier(_)
        | OperationKind::Atomic(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::Matrix(_)
        | OperationKind::Gfx950LdsTranspose(_)
        | OperationKind::Wave(_) => {
            for operand in operation.operands() {
                visitor(operand)?;
            }
        }
    }
    Ok(())
}

fn visit_terminator_operands(
    terminator: &Terminator,
    mut visitor: impl FnMut(ValueId) -> Result<(), KirAnalysisError>,
) -> Result<(), KirAnalysisError> {
    visit_terminator_local_operands(terminator, &mut visitor)?;
    visit_edges(terminator, |_, arguments| {
        for operand in arguments {
            visitor(*operand)?;
        }
        Ok(())
    })
}

fn terminator_successor_count(terminator: &Terminator) -> usize {
    match terminator {
        Terminator::Branch { .. } => 1,
        Terminator::ConditionalBranch { .. } => 2,
        Terminator::Switch { cases, .. } => cases.len().saturating_add(1),
        Terminator::IntegerSwitch { cases, .. } => cases.len().saturating_add(1),
        Terminator::Return { .. } | Terminator::Unreachable => 0,
    }
}

fn visit_edges(
    terminator: &Terminator,
    mut visitor: impl FnMut(BlockId, &[ValueId]) -> Result<(), KirAnalysisError>,
) -> Result<(), KirAnalysisError> {
    match terminator {
        Terminator::Branch { target, arguments } => visitor(*target, arguments),
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => {
            visitor(*then_target, then_arguments)?;
            visitor(*else_target, else_arguments)
        }
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            for case in cases {
                visitor(case.target, &case.arguments)?;
            }
            visitor(*default_target, default_arguments)
        }
        Terminator::IntegerSwitch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            for case in cases {
                visitor(case.target, &case.arguments)?;
            }
            visitor(*default_target, default_arguments)
        }
        Terminator::Return { .. } | Terminator::Unreachable => Ok(()),
    }
}

fn visit_terminator_local_operands(
    terminator: &Terminator,
    mut visitor: impl FnMut(ValueId) -> Result<(), KirAnalysisError>,
) -> Result<(), KirAnalysisError> {
    match terminator {
        Terminator::ConditionalBranch { condition, .. } => visitor(*condition)?,
        Terminator::Switch { selector, .. } | Terminator::IntegerSwitch { selector, .. } => {
            visitor(*selector)?;
        }
        Terminator::Return { values } => {
            for value in values {
                visitor(*value)?;
            }
        }
        Terminator::Branch { .. } | Terminator::Unreachable => {}
    }
    Ok(())
}

fn reverse_postorder(
    root: usize,
    successors: &[Vec<usize>],
    meter: &mut AnalysisMeter,
) -> Result<Vec<usize>, KirAnalysisError> {
    let mut visited = vec![false; successors.len()];
    let mut postorder = Vec::with_capacity(successors.len());
    let mut pending = vec![(root, 0usize)];
    visited[root] = true;
    while let Some((block, next_successor)) = pending.last_mut() {
        if *next_successor == successors[*block].len() {
            postorder.push(*block);
            pending.pop();
            continue;
        }
        let successor = successors[*block][*next_successor];
        *next_successor += 1;
        meter.charge(1)?;
        if !visited[successor] {
            visited[successor] = true;
            pending.push((successor, 0));
        }
    }
    postorder.reverse();
    Ok(postorder)
}

fn intersect_tree(
    mut left: usize,
    mut right: usize,
    immediate: &[Option<usize>],
    order: &[usize],
    meter: &mut AnalysisMeter,
) -> Result<usize, KirAnalysisError> {
    while left != right {
        while order[left] > order[right] {
            meter.charge(1)?;
            left = immediate[left].expect("visited reverse-CFG node has a dominator");
        }
        while order[right] > order[left] {
            meter.charge(1)?;
            right = immediate[right].expect("visited reverse-CFG node has a dominator");
        }
    }
    Ok(left)
}

fn tree_intervals(
    root: usize,
    children: &[Vec<usize>],
    meter: &mut AnalysisMeter,
) -> Result<(Vec<usize>, Vec<usize>), KirAnalysisError> {
    let mut preorder = vec![usize::MAX; children.len()];
    let mut subtree_end = vec![usize::MAX; children.len()];
    let mut next = 0usize;
    let mut pending = vec![(root, false)];
    while let Some((block, finish)) = pending.pop() {
        meter.charge(1)?;
        if finish {
            subtree_end[block] = next;
            continue;
        }
        preorder[block] = next;
        next += 1;
        pending.push((block, true));
        pending.extend(
            children[block]
                .iter()
                .rev()
                .copied()
                .map(|child| (child, false)),
        );
    }
    Ok((preorder, subtree_end))
}

struct AnalysisMeter {
    resource: KirAnalysisResource,
    limit: usize,
    used: usize,
}

impl AnalysisMeter {
    const fn new(resource: KirAnalysisResource, limit: usize) -> Self {
        Self {
            resource,
            limit,
            used: 0,
        }
    }

    fn charge(&mut self, amount: usize) -> Result<(), KirAnalysisError> {
        self.used = checked_add(self.used, amount);
        check_limit(self.resource, self.used, self.limit)
    }
}

fn checked_add(left: usize, right: usize) -> usize {
    left.saturating_add(right)
}

fn checked_multiply(left: usize, right: usize) -> usize {
    left.saturating_mul(right)
}

fn validate_limits(limits: KirAnalysisLimits) -> Result<(), KirAnalysisError> {
    for (resource, requested, maximum) in [
        (
            KirAnalysisResource::Functions,
            limits.functions,
            MAX_KIR_ANALYSIS_FUNCTIONS,
        ),
        (
            KirAnalysisResource::PostDominanceBlocks,
            limits.post_dominance_blocks,
            MAX_POST_DOMINANCE_BLOCKS,
        ),
        (
            KirAnalysisResource::PostDominanceEdges,
            limits.post_dominance_edges,
            MAX_POST_DOMINANCE_EDGES,
        ),
        (
            KirAnalysisResource::PostDominanceStorageItems,
            limits.post_dominance_storage_items,
            MAX_POST_DOMINANCE_STORAGE_ITEMS,
        ),
        (
            KirAnalysisResource::PostDominanceWorkUnits,
            limits.post_dominance_work_units,
            MAX_POST_DOMINANCE_WORK_UNITS,
        ),
        (
            KirAnalysisResource::LivenessValues,
            limits.liveness_values,
            MAX_LIVENESS_VALUES,
        ),
        (
            KirAnalysisResource::LivenessOperands,
            limits.liveness_operands,
            MAX_LIVENESS_OPERANDS,
        ),
        (
            KirAnalysisResource::LivenessFacts,
            limits.liveness_facts,
            MAX_LIVENESS_FACTS,
        ),
        (
            KirAnalysisResource::LivenessStorageItems,
            limits.liveness_storage_items,
            MAX_LIVENESS_STORAGE_ITEMS,
        ),
        (
            KirAnalysisResource::LivenessWorkUnits,
            limits.liveness_work_units,
            MAX_LIVENESS_WORK_UNITS,
        ),
    ] {
        if requested > maximum {
            return Err(KirAnalysisError::InvalidLimit {
                resource,
                requested,
                maximum,
            });
        }
    }
    Ok(())
}

fn check_limit(
    resource: KirAnalysisResource,
    required: usize,
    limit: usize,
) -> Result<(), KirAnalysisError> {
    if required > limit {
        Err(KirAnalysisError::ResourceLimit {
            resource,
            required,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        AddressSpace, BarrierSemantics, Constant, Convergence, MemoryOrdering, Operation,
        OperationKind, Signature, SynchronizationScope, Type, ValueDef, WorkgroupBarrier,
    };

    fn block(id: u32, terminator: Terminator) -> BasicBlock {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(terminator);
        block
    }

    fn branch(id: u32, target: u32, arguments: Vec<ValueId>) -> BasicBlock {
        block(
            id,
            Terminator::Branch {
                target: BlockId(target),
                arguments,
            },
        )
    }

    fn conditional(id: u32, then_target: u32, else_target: u32) -> BasicBlock {
        let condition = ValueId(100 + id);
        let mut block = block(
            id,
            Terminator::ConditionalBranch {
                condition,
                then_target: BlockId(then_target),
                then_arguments: Vec::new(),
                else_target: BlockId(else_target),
                else_arguments: Vec::new(),
            },
        );
        block.operations.push(Operation::effect_free(
            ValueDef::new(condition, Type::BOOL),
            OperationKind::Constant(Constant::Bool(true)),
        ));
        block
    }

    fn returning(id: u32, values: Vec<ValueId>) -> BasicBlock {
        block(id, Terminator::Return { values })
    }

    fn function(blocks: Vec<BasicBlock>) -> Function {
        Function::definition("test", Signature::new(vec![], vec![]), vec![], blocks)
    }

    fn module(function: Function) -> Module {
        let mut module = Module::new("test");
        module.functions.push(function);
        module
    }

    #[test]
    fn post_dominance_handles_diamond_and_multiple_exits() {
        let diamond = function(vec![
            conditional(0, 1, 2),
            branch(1, 3, vec![]),
            branch(2, 3, vec![]),
            returning(3, vec![]),
        ]);
        let report = analyze_post_dominance(&diamond).unwrap();
        assert_eq!(
            report.immediate_post_dominator(BlockId(0)),
            Some(Some(BlockId(3)))
        );
        assert!(report.post_dominates(BlockId(3), BlockId(1)));
        assert!(!report.post_dominates(BlockId(1), BlockId(2)));

        let split_exit = function(vec![
            conditional(0, 1, 2),
            returning(1, vec![]),
            returning(2, vec![]),
        ]);
        let report = analyze_post_dominance(&split_exit).unwrap();
        assert_eq!(report.immediate_post_dominator(BlockId(0)), Some(None));
        assert!(!report.post_dominates(BlockId(1), BlockId(0)));
    }

    #[test]
    fn post_dominance_excludes_non_terminating_region() {
        let function = function(vec![
            conditional(0, 1, 2),
            branch(1, 1, vec![]),
            returning(2, vec![]),
        ]);
        let report = analyze_post_dominance(&function).unwrap();
        assert_eq!(
            report.available_blocks(),
            &BTreeSet::from([BlockId(0), BlockId(2)])
        );
        assert_eq!(report.immediate_post_dominator(BlockId(1)), None);
        assert_eq!(
            report.immediate_post_dominator(BlockId(0)),
            Some(Some(BlockId(2)))
        );
    }

    #[test]
    fn liveness_translates_only_live_block_parameters_to_edge_arguments() {
        let incoming = ValueId(1);
        let dead_argument = ValueId(2);
        let parameter = ValueId(3);
        let dead_parameter = ValueId(4);
        let result = ValueId(5);
        let mut entry = branch(0, 1, vec![incoming, dead_argument]);
        entry.operations.push(Operation::effect_free(
            ValueDef::new(incoming, Type::BOOL),
            OperationKind::Constant(Constant::Bool(true)),
        ));
        entry.operations.push(Operation::effect_free(
            ValueDef::new(dead_argument, Type::BOOL),
            OperationKind::Constant(Constant::Bool(false)),
        ));
        let mut exit = returning(1, vec![result]);
        exit.parameters = vec![
            ValueDef::new(parameter, Type::BOOL),
            ValueDef::new(dead_parameter, Type::BOOL),
        ];
        exit.operations.push(Operation::effect_free(
            ValueDef::new(result, Type::BOOL),
            OperationKind::Unary {
                op: fe2o3_kernel_ir::UnaryOp::Not,
                operand: parameter,
            },
        ));
        let function = function(vec![entry, exit]);
        let report = analyze_liveness(&function).unwrap();
        assert_eq!(
            report.live_in(BlockId(1)),
            Some(&BTreeSet::from([parameter]))
        );
        assert_eq!(
            report.live_out(BlockId(0)),
            Some(&BTreeSet::from([incoming]))
        );
        assert!(!report.is_live_out(BlockId(0), dead_argument));
        assert_eq!(report.live_in(BlockId(0)), Some(&BTreeSet::new()));
    }

    #[test]
    fn manager_caches_reports_and_consuming_invalidation_clears_them() {
        let first_module = module(function(vec![returning(0, vec![])]));
        let id = FunctionId::new("test");
        let mut manager = KirAnalysisManager::new(&first_module, AnalysisEpoch::new(7)).unwrap();
        let first = manager.control_flow(&id).unwrap();
        let second = manager.control_flow(&id).unwrap();
        assert!(first.shares_cached_storage_with(&second));
        assert_eq!(first.current(&manager).unwrap().blocks().len(), 1);
        assert_eq!(manager.cache_stats().control_flow_runs, 1);

        let second_module = module(function(vec![branch(0, 1, vec![]), returning(1, vec![])]));
        let mut manager = manager
            .invalidate(&second_module, AnalysisEpoch::new(8))
            .unwrap();
        assert_eq!(manager.cache_stats(), KirAnalysisCacheStats::default());
        let current = manager.control_flow(&id).unwrap();
        assert_eq!(current.current(&manager).unwrap().blocks().len(), 2);
        assert!(matches!(
            first.current(&manager),
            Err(KirAnalysisError::SubjectMismatch { .. })
        ));
    }

    #[test]
    fn manager_propagates_irreducible_cfg_and_reuses_failure() {
        let function = function(vec![
            conditional(0, 1, 2),
            branch(1, 2, vec![]),
            conditional(2, 1, 3),
            returning(3, vec![]),
        ]);
        let module = module(function);
        let id = FunctionId::new("test");
        let mut manager = KirAnalysisManager::new(&module, AnalysisEpoch::new(0)).unwrap();
        assert!(matches!(
            manager.post_dominance(&id),
            Err(KirAnalysisError::ControlFlow(_))
        ));
        assert!(matches!(
            manager.post_dominance(&id),
            Err(KirAnalysisError::ControlFlow(_))
        ));
        assert_eq!(manager.cache_stats().control_flow_runs, 1);
        assert_eq!(manager.cache_stats().post_dominance_runs, 0);
    }

    #[test]
    fn custom_limits_fail_with_typed_resource() {
        let empty_function = function(vec![returning(0, vec![])]);
        let mut limits = KirAnalysisLimits::DEFAULT;
        limits.post_dominance_blocks = 0;
        assert_eq!(
            analyze_post_dominance_with_limits(&empty_function, limits),
            Err(KirAnalysisError::ResourceLimit {
                resource: KirAnalysisResource::PostDominanceBlocks,
                required: 1,
                limit: 0,
            })
        );

        let mut limits = KirAnalysisLimits::DEFAULT;
        limits.liveness_values = 0;
        let mut value_function = function(vec![returning(0, vec![ValueId(1)])]);
        value_function
            .body
            .as_mut()
            .unwrap()
            .parameters
            .push(ValueId(1));
        assert_eq!(
            analyze_liveness_with_limits(&value_function, limits),
            Err(KirAnalysisError::ResourceLimit {
                resource: KirAnalysisResource::LivenessValues,
                required: 1,
                limit: 0,
            })
        );
    }

    #[test]
    fn custom_limits_cannot_raise_hard_maximum() {
        let function = function(vec![returning(0, vec![])]);
        let mut limits = KirAnalysisLimits::DEFAULT;
        limits.post_dominance_work_units = MAX_POST_DOMINANCE_WORK_UNITS + 1;
        assert_eq!(
            analyze_post_dominance_with_limits(&function, limits),
            Err(KirAnalysisError::InvalidLimit {
                resource: KirAnalysisResource::PostDominanceWorkUnits,
                requested: MAX_POST_DOMINANCE_WORK_UNITS + 1,
                maximum: MAX_POST_DOMINANCE_WORK_UNITS,
            })
        );
    }

    #[test]
    fn stale_handles_reject_a_new_epoch_even_for_identical_module_content() {
        let module = module(function(vec![returning(0, vec![])]));
        let id = FunctionId::new("test");
        let mut manager = KirAnalysisManager::new(&module, AnalysisEpoch::new(11)).unwrap();
        let stale = manager.liveness(&id).unwrap();
        let manager = manager.invalidate(&module, AnalysisEpoch::new(12)).unwrap();

        assert_eq!(stale.subject().epoch(), AnalysisEpoch::new(11));
        assert_eq!(manager.subject().epoch(), AnalysisEpoch::new(12));
        assert_eq!(
            stale.subject().module_identity(),
            manager.subject().module_identity()
        );
        assert!(matches!(
            stale.current(&manager),
            Err(KirAnalysisError::SubjectMismatch { .. })
        ));
    }

    #[test]
    fn manager_rejects_non_dominating_ssa_during_typed_admission() {
        let value = ValueId(7);
        let entry = returning(0, vec![value]);
        let mut unreachable = returning(1, vec![]);
        unreachable.operations.push(Operation::effect_free(
            ValueDef::new(value, Type::Scalar(fe2o3_kernel_ir::ScalarType::U32)),
            OperationKind::Constant(Constant::U32(1)),
        ));
        let mut function = function(vec![entry, unreachable]);
        function.signature.results = vec![Type::Scalar(fe2o3_kernel_ir::ScalarType::U32)];
        let module = module(function);

        assert!(matches!(
            KirAnalysisManager::new(&module, AnalysisEpoch::new(0)),
            Err(KirAnalysisError::ModuleAdmission(_))
        ));
    }

    #[test]
    fn liveness_work_limit_counts_zero_result_zero_operand_operations() {
        let barrier = Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        );
        let mut block = returning(0, vec![]);
        block.operations = vec![barrier; 32];
        let function = function(vec![block]);
        let mut limits = KirAnalysisLimits::DEFAULT;
        limits.liveness_work_units = 8;

        assert!(matches!(
            analyze_liveness_with_limits(&function, limits),
            Err(KirAnalysisError::ResourceLimit {
                resource: KirAnalysisResource::LivenessWorkUnits,
                limit: 8,
                ..
            })
        ));
    }

    #[test]
    fn liveness_work_counts_empty_argument_edges_in_each_operand_scan() {
        let function = function(vec![
            conditional(0, 1, 2),
            returning(1, vec![]),
            returning(2, vec![]),
        ]);

        let report = analyze_liveness(&function).unwrap();
        assert_eq!(report.resource_usage().work_units(), 51);

        let mut limits = KirAnalysisLimits::DEFAULT;
        limits.liveness_work_units = 50;
        assert!(matches!(
            analyze_liveness_with_limits(&function, limits),
            Err(KirAnalysisError::ResourceLimit {
                resource: KirAnalysisResource::LivenessWorkUnits,
                required: 51,
                limit: 50,
            })
        ));
    }

    #[test]
    fn post_dominance_block_limit_covers_unreachable_input_blocks() {
        let function = function(vec![
            returning(0, vec![]),
            returning(1, vec![]),
            returning(2, vec![]),
            returning(3, vec![]),
        ]);
        let mut limits = KirAnalysisLimits::DEFAULT;
        limits.post_dominance_blocks = 1;

        assert_eq!(
            analyze_post_dominance_with_limits(&function, limits),
            Err(KirAnalysisError::ResourceLimit {
                resource: KirAnalysisResource::PostDominanceBlocks,
                required: 4,
                limit: 1,
            })
        );
    }

    #[test]
    fn manager_borrowed_admission_rejects_oversized_input() {
        let oversized = Module::new("x".repeat(fe2o3_kernel_ir::MAX_TEXT_BYTES_V1 + 1));
        assert!(matches!(
            KirAnalysisManager::new(&oversized, AnalysisEpoch::new(0)),
            Err(KirAnalysisError::ModuleAdmission(
                VerifiedCanonicalKernelIrErrorV9::Encode(_)
            ))
        ));
    }

    #[test]
    fn liveness_rejects_bad_edge_arity_in_unreachable_blocks() {
        let entry = returning(0, vec![]);
        let source = branch(1, 2, vec![]);
        let mut target = returning(2, vec![]);
        target.parameters = vec![ValueDef::new(ValueId(7), Type::BOOL)];

        assert_eq!(
            analyze_liveness(&function(vec![entry, source, target])),
            Err(KirAnalysisError::MalformedLiveness(
                LivenessDiagnostic::EdgeArgumentArity {
                    source: BlockId(1),
                    target: BlockId(2),
                    expected: 1,
                    actual: 0,
                }
            ))
        );
    }

    #[test]
    fn liveness_preflights_borrowed_operands_and_auxiliary_storage() {
        let value = ValueId(1);
        let mut entry = returning(0, vec![]);
        entry.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("callee"),
                arguments: vec![value; 9],
            },
        ));
        let mut hostile = function(vec![entry]);
        hostile.body.as_mut().unwrap().parameters.push(value);
        hostile.signature.parameters.push(Type::BOOL);

        let mut limits = KirAnalysisLimits::DEFAULT;
        limits.liveness_operands = 8;
        assert_eq!(
            analyze_liveness_with_limits(&hostile, limits),
            Err(KirAnalysisError::ResourceLimit {
                resource: KirAnalysisResource::LivenessOperands,
                required: 9,
                limit: 8,
            })
        );

        let mut limits = KirAnalysisLimits::DEFAULT;
        limits.liveness_storage_items = 6;
        assert_eq!(
            analyze_liveness_with_limits(&function(vec![returning(0, vec![])]), limits),
            Err(KirAnalysisError::ResourceLimit {
                resource: KirAnalysisResource::LivenessStorageItems,
                required: 7,
                limit: 6,
            })
        );
    }
}
