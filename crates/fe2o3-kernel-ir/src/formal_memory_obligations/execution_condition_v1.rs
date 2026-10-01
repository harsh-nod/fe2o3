use super::*;

/// A checked upper bound of at most global invocation zero.
/// Coordinates are descriptive, not a portable proof or runtime authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalSingletonExecutionV1 {
    invocation: u64,
    index: ValueId,
    threshold: ValueId,
    predicate: ValueId,
    path: FormalGuardedPathV1,
}

impl FormalSingletonExecutionV1 {
    pub const fn invocation(self) -> u64 {
        self.invocation
    }
    pub const fn index(self) -> ValueId {
        self.index
    }
    pub const fn threshold(self) -> ValueId {
        self.threshold
    }
    pub const fn predicate(self) -> ValueId {
        self.predicate
    }
    pub const fn path(self) -> FormalGuardedPathV1 {
        self.path
    }
}

/// Execution information for one actual ordinary memory operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalAccessExecutionConditionV1 {
    location: FunctionOperationLocation,
    singleton: Option<FormalSingletonExecutionV1>,
}

impl FormalAccessExecutionConditionV1 {
    pub const fn location(self) -> FunctionOperationLocation {
        self.location
    }
    /// None is unrestricted; Some does not prove that the access executes.
    pub const fn singleton(self) -> Option<FormalSingletonExecutionV1> {
        self.singleton
    }
}

/// Borrowed descriptive facts for one verified kernel and launch input.
///
/// The immutable module borrow prevents mutation while these facts are held.
/// Pair queries check the exact module object and kernel, not structural
/// equality. Launch inputs remain unauthenticated. Existing bounds, conflicts
/// and receipt formats are unchanged. Callee and unmodeled effects are not
/// covered; this is not a complete memory analysis.
#[derive(Debug)]
pub struct FormalAccessExecutionAnalysisV1<'module> {
    module: &'module Module,
    kernel: &'module KernelId,
    entry: &'module FunctionId,
    launch: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
    invocations: Option<InvocationRange1d>,
    accesses: Vec<FormalAccessExecutionConditionV1>,
    work: usize,
    storage: usize,
}

impl FormalAccessExecutionAnalysisV1<'_> {
    pub fn kernel(&self) -> &KernelId {
        self.kernel
    }
    pub fn entry(&self) -> &FunctionId {
        self.entry
    }
    pub const fn launch(&self) -> ExplicitLaunchExtent {
        self.launch
    }
    pub const fn index_width(&self) -> FormalIndexWidth {
        self.index_width
    }
    /// The full launch universe, never narrowed by an execution condition.
    pub const fn invocations(&self) -> Option<InvocationRange1d> {
        self.invocations
    }
    pub fn accesses(&self) -> &[FormalAccessExecutionConditionV1] {
        &self.accesses
    }
    pub const fn work(&self) -> usize {
        self.work
    }
    /// Conservative logical storage charge, including released CFG scratch.
    /// This is not an allocator or RSS measurement.
    pub const fn storage(&self) -> usize {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Describes exclusion of distinct invocations under the recorded launch
    /// and width, not bounds, aliasing or admission. Both locations are
    /// interpreted only in this analysis's selected kernel entry.
    pub fn excludes_distinct_invocations(
        &self,
        verified: VerifiedKernelIrModuleV1<'_>,
        kernel: &KernelId,
        left: FunctionOperationLocation,
        right: FunctionOperationLocation,
    ) -> bool {
        if !std::ptr::eq(self.module, verified.module()) || kernel != self.kernel {
            return false;
        }
        let singleton = |location| {
            self.accesses
                .binary_search_by_key(&location, |row| row.location)
                .ok()
                .and_then(|index| self.accesses[index].singleton)
        };
        same_singleton(singleton(left), singleton(right))
    }
}

fn same_singleton(
    left: Option<FormalSingletonExecutionV1>,
    right: Option<FormalSingletonExecutionV1>,
) -> bool {
    matches!((left, right), (Some(left), Some(right)) if left.invocation == right.invocation)
}

/// Derives execution conditions independently of formal memory bounds.
///
/// Only direct INDEX GlobalX < Index(1) predicates whose unique true edge
/// dominates an actual access are recognized. Casts, helper/block-parameter
/// transport and other predicates remain unrestricted. The rank-one, Bits64,
/// nonwrapping launch requirements use the existing launch resolver. Shared
/// bounded guarded-analysis accounting reports exhaustion as an error, never
/// a successful unrestricted result.
pub fn derive_formal_access_execution_conditions_v1<'module>(
    verified: VerifiedKernelIrModuleV1<'module>,
    kernel: &KernelId,
    launch: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
) -> Result<FormalAccessExecutionAnalysisV1<'module>, FormalMemoryObligationError> {
    derive_with_limits(
        verified,
        kernel,
        launch,
        index_width,
        crate::MAX_CFG_ANALYSIS_WORK as usize,
        MAX_NEW_BYTES,
    )
}

fn derive_with_limits<'module>(
    verified: VerifiedKernelIrModuleV1<'module>,
    kernel_id: &KernelId,
    launch: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
    work_limit: usize,
    storage_limit: usize,
) -> Result<FormalAccessExecutionAnalysisV1<'module>, FormalMemoryObligationError> {
    let module = verified.module();
    // Reserve unavailable capacity rather than changing the legacy ledger's
    // layout, size-based charges or established allocation policy.
    let storage_floor = MAX_NEW_BYTES - storage_limit.min(MAX_NEW_BYTES);
    let mut ledger = GuardLedger {
        work: CanonicalKernelIrWorkBudgetV1::new(
            work_limit.min(crate::MAX_CFG_ANALYSIS_WORK as usize),
        ),
        bytes: storage_floor,
        records: 0,
    };
    ledger.charge(module.kernels.len())?;
    let kernel = module
        .kernels
        .iter()
        .find(|kernel| &kernel.id == kernel_id)
        .ok_or_else(|| FormalMemoryObligationError::MissingKernel {
            kernel: kernel_id.clone(),
        })?;
    ledger.charge(module.functions.len())?;
    let function = module
        .function(&kernel.entry)
        .ok_or(ResourceError::Accounting)?;
    let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
    let mut reasons = BTreeSet::new();
    ledger.charge(64)?;
    let invocations = resolve_invocations(&kernel.domain, launch, &mut reasons)?;
    let eligible = index_width == FormalIndexWidth::Bits64
        && kernel.domain.rank() == 1
        && invocations.is_some();
    ledger.storage(size_of::<FormalAccessExecutionAnalysisV1<'_>>())?;
    let (seed, cfg_peak) = execution_control_index(function, ledger)?;
    let GuardedControlV1 {
        ledger,
        rows,
        entry,
    } = seed;
    let mut index = GuardedAnalysisV1 {
        ledger,
        control: rows,
        definitions: Vec::new(),
        parameters: Vec::new(),
        truths: Vec::new(),
        recipes: Vec::new(),
        runtime_reads: runtime_slice_read_v1::RuntimeReadState::default(),
        rank_one: kernel.domain.rank() == 1,
    };
    for block in &body.blocks {
        index.ledger.charge(2)?;
        if index
            .control_row(block.id)?
            .is_none_or(|row| row.interval.is_none())
        {
            continue;
        }
        for operation in &block.operations {
            index.ledger.charge(2)?;
            for result in &operation.results {
                index.ledger.push(
                    &mut index.definitions,
                    DefinitionRow {
                        value: result.id,
                        operation,
                    },
                )?;
            }
        }
    }
    index
        .ledger
        .sort(&mut index.definitions, 1, |a, b| a.value.cmp(&b.value))?;
    index.collect_truths(function, entry)?;
    let mut singletons = Vec::new();
    if eligible {
        for ordinal in 0..index.truths.len() {
            let truth = index.truths[ordinal];
            index.ledger.charge(32)?;
            if let Some(singleton) = singleton_condition(&mut index, truth)? {
                index
                    .ledger
                    .push(&mut singletons, (truth.interval, singleton))?;
            }
        }
    }
    let mut accesses = Vec::new();
    for block in &body.blocks {
        index.ledger.charge(2)?;
        let Some(interval) = index.control_row(block.id)?.and_then(|row| row.interval) else {
            continue;
        };
        let mut singleton = None;
        for (guard, condition) in &singletons {
            index.ledger.charge(4)?;
            if guard.0 <= interval.0 && interval.1 <= guard.1 {
                singleton = Some(*condition);
                break;
            }
        }
        for (ordinal, operation) in block.operations.iter().enumerate() {
            index.ledger.charge(2)?;
            if !matches!(
                operation.kind,
                OperationKind::Load { .. }
                    | OperationKind::Store { .. }
                    | OperationKind::GuardedLoad { .. }
                    | OperationKind::GuardedStore { .. }
                    | OperationKind::Atomic(_)
            ) {
                continue;
            }
            index.ledger.push(
                &mut accesses,
                FormalAccessExecutionConditionV1 {
                    location: FunctionOperationLocation::new(block.id, ordinal),
                    singleton,
                },
            )?;
        }
    }
    index
        .ledger
        .sort(&mut accesses, 2, |a, b| a.location.cmp(&b.location))?;
    Ok(FormalAccessExecutionAnalysisV1 {
        module,
        kernel: &kernel.id,
        entry: &kernel.entry,
        launch,
        index_width,
        invocations,
        accesses,
        work: index.ledger.work.work(),
        storage: cfg_peak.max(index.ledger.bytes) - storage_floor,
    })
}

// The existing CFG contract charges one cell per scalar or nested Vec header,
// two per (BlockId, usize), (usize, usize), Range or Option<usize>, and four per
// edge. The assertions bind the byte conversion to those exact payload types.
// The retained CFG owner is charged separately. As in the shared engine, this
// is logical payload accounting, not allocator capacity or generated stack.
const CFG_CELL_BYTES: usize = size_of::<Vec<usize>>();
const _: () = {
    assert!(size_of::<BlockId>() <= CFG_CELL_BYTES);
    assert!(size_of::<usize>() <= CFG_CELL_BYTES);
    assert!(size_of::<bool>() <= CFG_CELL_BYTES);
    assert!(size_of::<u32>() <= CFG_CELL_BYTES);
    assert!(size_of::<(BlockId, usize)>() <= 2 * CFG_CELL_BYTES);
    assert!(size_of::<(usize, usize)>() <= 2 * CFG_CELL_BYTES);
    assert!(size_of::<std::ops::Range<usize>>() <= 2 * CFG_CELL_BYTES);
    assert!(size_of::<Option<usize>>() <= 2 * CFG_CELL_BYTES);
    assert!(size_of::<crate::IndexedControlFlowEdge>() <= 4 * CFG_CELL_BYTES);
};

fn execution_control_index(
    function: &Function,
    mut ledger: GuardLedger,
) -> Result<(GuardedControlV1, usize), ResourceError> {
    ledger.storage(size_of::<crate::MeteredIndexedControlFlowV1>())?;
    let cfg_floor = ledger.bytes;
    let mut budget = Budget::new(
        &mut ledger.work,
        (MAX_NEW_BYTES - cfg_floor) / CFG_CELL_BYTES,
    );
    let flow = crate::analyze_control_flow_with_verification_budget_v1(
        function,
        crate::ControlFlowLimits::DEFAULT,
        &mut budget,
    )
    .map_err(|error| match error {
        crate::MeteredControlFlowErrorV1::Resource(VerificationResourceError::Storage(error)) => {
            match (
                error
                    .actual()
                    .checked_mul(CFG_CELL_BYTES)
                    .and_then(|n| cfg_floor.checked_add(n)),
                error
                    .limit()
                    .checked_mul(CFG_CELL_BYTES)
                    .and_then(|n| cfg_floor.checked_add(n)),
            ) {
                (Some(actual), Some(limit)) => ResourceError::Storage { actual, limit },
                _ => ResourceError::Arithmetic,
            }
        }
        crate::MeteredControlFlowErrorV1::Resource(error) => ResourceError::from(error),
        crate::MeteredControlFlowErrorV1::ControlFlow(
            crate::ControlFlowError::ArithmeticOverflow(_),
        ) => ResourceError::Arithmetic,
        // A previously verified graph must satisfy the unchanged CFG shape
        // limits. A mismatch is a typed accounting refusal, never a panic.
        crate::MeteredControlFlowErrorV1::ControlFlow(_) => ResourceError::Accounting,
    })?;
    let retained_cells = budget.storage();
    let cfg_peak = budget
        .peak_storage()
        .checked_mul(CFG_CELL_BYTES)
        .and_then(|bytes| cfg_floor.checked_add(bytes))
        .ok_or(ResourceError::Arithmetic)?;
    drop(budget);
    ledger.storage(
        retained_cells
            .checked_mul(CFG_CELL_BYTES)
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    let mut control =
        GuardedControlV1::collect_with_ledger(function, flow.indexed_v15(), ledger, true)?
            .ok_or(ResourceError::Accounting)?;
    // Restore the exact CFG owner's logical reservation for its consuming
    // release. Guarded accounting retains a conservative charge after release.
    let mut release_budget = Budget::new(&mut control.ledger.work, retained_cells);
    release_budget.reserve_storage(retained_cells)?;
    flow.release(&mut release_budget)?;
    drop(release_budget);
    Ok((control, cfg_peak))
}

fn singleton_condition(
    index: &mut GuardedAnalysisV1<'_>,
    truth: TrueRow,
) -> Result<Option<FormalSingletonExecutionV1>, ResourceError> {
    if truth.ambiguous {
        return Ok(None);
    }
    let Some(predicate) = index.definition(truth.predicate)? else {
        return Ok(None);
    };
    let OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs,
        rhs,
    } = predicate.kind
    else {
        return Ok(None);
    };
    let Some(global) = index.definition(lhs)? else {
        return Ok(None);
    };
    if !single_type(global, &Type::INDEX)
        || !matches!(&global.kind, OperationKind::Intrinsic(intrinsic)
        if intrinsic.kind == (IntrinsicKind::InvocationIndex {
            kind: IndexKind::Global, axis: Axis::X,
        }))
    {
        return Ok(None);
    }
    let Some(threshold) = index.definition(rhs)? else {
        return Ok(None);
    };
    if !single_type(threshold, &Type::INDEX)
        || !matches!(threshold.kind, OperationKind::Constant(Constant::Index(1)))
    {
        return Ok(None);
    }
    Ok(Some(FormalSingletonExecutionV1 {
        invocation: 0,
        index: lhs,
        threshold: rhs,
        predicate: truth.predicate,
        path: FormalGuardedPathV1::TrueEdge {
            source: truth.edge.source,
            ordinal: truth.edge.ordinal,
            target: truth.edge.target,
        },
    }))
}

#[cfg(test)]
#[path = "execution_condition_v1_tests.rs"]
mod tests;
