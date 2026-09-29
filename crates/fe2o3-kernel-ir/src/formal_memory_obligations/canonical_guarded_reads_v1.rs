use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceErrorV1 as VerificationError,
    CanonicalKernelIrWorkLedgerIdentityV1, CanonicalKirBlockCoordinateV1 as BlockCoordinate,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as Coordinate, ControlFlowLimits, KirLocalMemoryEffectRefV1,
    VerifiedCanonicalKernelIrModuleV12, VerifiedCanonicalKernelIrModuleV18,
};
use meter::LiveGuardMeter;
use predicates::PredicateRow;
use runtime_slice_read_v1::RuntimeSliceReadConditionsV1;
use std::cell::RefCell;

#[path = "canonical_guarded_reads_queries_v1.rs"]
mod queries;
pub use queries::*;

#[path = "canonical_guarded_stores_v24.rs"]
mod stores_v24;
pub use stores_v24::*;

/// Local caps, distinct from the caller's cumulative live ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalGuardedGlobalReadLimitsV1 {
    /// Complete module function roster, including declarations.
    pub functions: usize,
    /// Existing CFG limits and its inherited requested-logical-cell accounting.
    pub control_flow: ControlFlowLimits,
    /// Conservative guarded-index work cap for each defined function.
    pub per_function_work: usize,
    /// Cumulative new allocation requests per function, measured in bytes.
    pub per_function_new_bytes: usize,
    /// Cumulative new row-capacity requests per function.
    pub per_function_records: usize,
}
impl Default for CanonicalGuardedGlobalReadLimitsV1 {
    fn default() -> Self {
        Self {
            functions: 16_384,
            control_flow: ControlFlowLimits::DEFAULT,
            per_function_work: crate::MAX_CFG_ANALYSIS_WORK as usize,
            per_function_new_bytes: MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1,
            per_function_records: MAX_FORMAL_MEMORY_RECORDS_V1,
        }
    }
}

/// Located graph/query/resource refusal. No source or runtime authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalGuardedGlobalReadErrorV1 {
    /// Local guarded limit or external ledger refusal.
    Resource(FormalGuardedMemoryResourceErrorV1),
    /// The existing CFG engine refused this graph or its local limits.
    ControlFlow(crate::ControlFlowError),
    /// Complete module roster exceeds the caller's analysis cap.
    FunctionLimit {
        /// Complete actual module function count.
        actual: usize,
        /// The independently supplied local analysis cap.
        limit: usize,
    },
    /// Coordinate is not an operation of the exact borrowed owner.
    Coordinate(Coordinate),
    /// A callback or rejected payload unwound.
    Panicked,
}
impl From<ResourceError> for CanonicalGuardedGlobalReadErrorV1 {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}
impl From<VerificationError> for CanonicalGuardedGlobalReadErrorV1 {
    fn from(error: VerificationError) -> Self {
        Self::Resource(error.into())
    }
}
impl fmt::Display for CanonicalGuardedGlobalReadErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical guarded read: {self:?}")
    }
}
impl Error for CanonicalGuardedGlobalReadErrorV1 {}
type Failure = CanonicalGuardedGlobalReadErrorV1;
type Result<T> = std::result::Result<T, Failure>;

/// Unsupported local condition; absence never means safe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalGuardedGlobalReadReasonV1 {
    /// The occurrence is not an ordinary nonvolatile Global scalar Load.
    NotOrdinaryGlobalRead,
    /// Slice provenance, representation, alignment, bound or dominance is unknown.
    MissingBoundOrProvenance,
    /// No exact dominating predicate of the requested polarity exists.
    MissingPredicate,
    /// Not an exact supported unsigned/Index checked value/overflow pair.
    NotUnsignedCheckedArithmetic,
}

struct ReadRow {
    coordinate: Coordinate,
    conditions: Option<RuntimeSliceReadConditionsV1>,
    reason: CanonicalGuardedGlobalReadReasonV1,
}

fn read_row_frame_bytes() -> Result<usize> {
    size_of::<ReadRow>()
        .checked_add(size_of::<(
            Option<RuntimeSliceReadConditionsV1>,
            CanonicalGuardedGlobalReadReasonV1,
        )>())
        .ok_or(ResourceError::Arithmetic.into())
}

fn read_origin_query_frame_bytes<O>() -> Result<usize> {
    size_of::<&CanonicalGuardedGlobalReadFactV1<'_, '_, O>>()
        .checked_add(size_of::<CanonicalGuardedReadIndexOriginV1>())
        .and_then(|n| n.checked_add(size_of::<ValueId>()))
        .and_then(|n| n.checked_add(size_of::<(ValueId, ValueId)>()))
        .ok_or(ResourceError::Arithmetic.into())
}
struct FunctionFacts<'g> {
    function: &'g Function,
    controls: Vec<ControlRow>,
    predicates: Vec<PredicateRow>,
    reads: Vec<ReadRow>,
    global_read_occurrences: usize,
    other_global_effects: usize,
    unresolved_calls: usize,
}
impl FunctionFacts<'_> {
    fn retained_bytes(&self) -> Result<usize> {
        self.controls
            .capacity()
            .checked_mul(size_of::<ControlRow>())
            .and_then(|n| {
                self.predicates
                    .capacity()
                    .checked_mul(size_of::<PredicateRow>())
                    .and_then(|v| n.checked_add(v))
            })
            .and_then(|n| {
                self.reads
                    .capacity()
                    .checked_mul(size_of::<ReadRow>())
                    .and_then(|v| n.checked_add(v))
            })
            .ok_or(ResourceError::Arithmetic.into())
    }
}
struct Facts<'g, O = VerifiedCanonicalKernelIrModuleV12> {
    owner: &'g O,
    functions: Vec<FunctionFacts<'g>>,
}
struct Accounting {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    failure: RefCell<Option<Failure>>,
}
impl Accounting {
    fn valid(&self, budget: &Budget<'_>) -> bool {
        self.slot == std::ptr::from_ref(budget) as usize
            && self.ledger == budget.work_ledger_identity_v1()
            && budget.storage() >= self.floor
    }
    fn save<T>(&self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result {
            if self.failure.borrow().is_none() {
                *self.failure.borrow_mut() = Some(error.clone());
            }
        }
        result
    }
    fn enter(&self, budget: &mut Budget<'_>) -> Result<()> {
        if !self.valid(budget) {
            return self.save(Err(ResourceError::Accounting.into()));
        }
        if let Some(error) = self.failure.borrow().as_ref() {
            return Err(error.clone());
        }
        self.save(budget.charge_work(1).map_err(Into::into))
    }
}

/// Exact borrowed graph facts with paid queries. Not Clone, a source proof,
/// a private-cell certificate, a fixed-nine report or runtime allocation grant.
///
/// Storage observations during construction combine inherited CFG requested
/// logical cells with new-buffer actual-capacity bytes. That aggregate is NOT
/// a whole-path byte/RSS bound. The CFG is dropped and its cells refunded before
/// this callback; retained fact buffers are byte-accounted separately.
pub struct CheckedCanonicalGuardedGlobalReadsV1<'scope, 'g, O = VerifiedCanonicalKernelIrModuleV12>
{
    facts: &'scope Facts<'g, O>,
    accounting: &'scope Accounting,
}

/// Borrowed local guarded-read facts from one actual V18 graph and table.
///
/// This uses the same local analysis as the V12 entry. It does not certify
/// storage initialization, runtime allocation, source equivalence or GPU execution.
///
/// ```
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV18,
///     CanonicalKernelIrVerificationResourceBudgetV1, CanonicalGuardedGlobalReadLimitsV1,
///     CanonicalGuardedGlobalReadErrorV1, with_canonical_guarded_global_reads_v18};
/// fn inspect(owner: &VerifiedCanonicalKernelIrModuleV18,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> Result<usize, CanonicalGuardedGlobalReadErrorV1> {
///     with_canonical_guarded_global_reads_v18(owner,
///         CanonicalGuardedGlobalReadLimitsV1::default(), budget,
///         |view, budget| view.function_count(budget))
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::{CheckedCanonicalGuardedGlobalReadsV18,
///     VerifiedCanonicalKernelIrModuleV18, CanonicalKernelIrVerificationResourceBudgetV1,
///     CanonicalGuardedGlobalReadLimitsV1, with_canonical_guarded_global_reads_v18};
/// fn escape<'g>(owner: &'g VerifiedCanonicalKernelIrModuleV18,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> CheckedCanonicalGuardedGlobalReadsV18<'g, 'g> {
///     with_canonical_guarded_global_reads_v18(owner,
///         CanonicalGuardedGlobalReadLimitsV1::default(), budget,
///         |view, _| Ok(*view)).unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::{CheckedCanonicalGuardedGlobalReadsV18,
///     VerifiedCanonicalKernelIrModuleV18, CanonicalKernelIrVerificationResourceBudgetV1,
///     CanonicalGuardedGlobalReadLimitsV1, with_canonical_guarded_global_reads_v18};
/// fn borrow_facts<'g>(owner: &'g VerifiedCanonicalKernelIrModuleV18,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> &'g CheckedCanonicalGuardedGlobalReadsV18<'g, 'g> {
///     with_canonical_guarded_global_reads_v18(owner,
///         CanonicalGuardedGlobalReadLimitsV1::default(), budget,
///         |view, _| Ok(view)).unwrap()
/// }
/// ```
pub type CheckedCanonicalGuardedGlobalReadsV18<'scope, 'g> =
    CheckedCanonicalGuardedGlobalReadsV1<'scope, 'g, VerifiedCanonicalKernelIrModuleV18>;

fn drain<T>(value: T) {
    let mut result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = result {
        result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)));
    }
}

/// Derives every defined function on this actual owner, retaining declarations
/// and all unresolved calls/effects. No external truth, extent or launch input.
///
/// The caller retains its owner floor. Callback scratch may exceed its captured
/// floor, but must be dropped/refunded before return. Foreign/moved ledgers,
/// undercut, ignored query failure and leaked reservations are typed errors.
/// Rejected results/panic payloads drain before backing credit is refunded.
pub fn with_canonical_guarded_global_reads_v1<'g, 'w, T>(
    owner: &'g VerifiedCanonicalKernelIrModuleV12,
    limits: CanonicalGuardedGlobalReadLimitsV1,
    budget: &mut Budget<'w>,
    consume: impl for<'scope> FnOnce(
        &CheckedCanonicalGuardedGlobalReadsV1<'scope, 'g>,
        &mut Budget<'w>,
    ) -> Result<T>,
) -> Result<T> {
    with_owner::<false, _, _>(owner, owner.module(), limits, budget, consume)
}

/// Derives paid local guarded-read facts from the exact borrowed V18 owner.
///
/// The callback, sticky failures and scratch accounting follow the V12 entry.
/// Unsupported storage or pointer provenance remains unproved; no layout row is
/// copied, erased or converted into an old-profile verification token.
pub fn with_canonical_guarded_global_reads_v18<'g, 'w, T>(
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    limits: CanonicalGuardedGlobalReadLimitsV1,
    budget: &mut Budget<'w>,
    consume: impl for<'scope> FnOnce(
        &CheckedCanonicalGuardedGlobalReadsV18<'scope, 'g>,
        &mut Budget<'w>,
    ) -> Result<T>,
) -> Result<T> {
    with_owner::<false, _, _>(owner, owner.module(), limits, budget, consume)
}

fn with_owner<'g, 'w, const STORE: bool, O, T>(
    owner: &'g O,
    module: &'g Module,
    limits: CanonicalGuardedGlobalReadLimitsV1,
    budget: &mut Budget<'w>,
    consume: impl for<'scope> FnOnce(
        &CheckedCanonicalGuardedGlobalReadsV1<'scope, 'g, O>,
        &mut Budget<'w>,
    ) -> Result<T>,
) -> Result<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let mut returned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Getter carriers must remain paid through the external callback,
        // after each function's construction scratch has been refunded.
        let origin_query_headers = read_origin_query_frame_bytes::<O>()?;
        let headers = size_of::<Facts<'_, O>>()
            .checked_add(size_of::<Accounting>())
            .and_then(|n| {
                n.checked_add(size_of::<CheckedCanonicalGuardedGlobalReadsV1<'_, '_, O>>())
            })
            .and_then(|n| n.checked_add(size_of::<std::thread::Result<Result<T>>>()))
            .and_then(|n| n.checked_add(size_of::<std::thread::Result<Result<T>>>()))
            .and_then(|n| n.checked_add(size_of::<std::thread::Result<()>>()))
            .and_then(|n| n.checked_add(origin_query_headers))
            .ok_or(ResourceError::Arithmetic)?;
        budget.reserve_storage(headers)?;
        let facts = build::<STORE, _>(owner, module, limits, budget)?;
        let accounting = Accounting {
            slot,
            ledger,
            floor: budget.storage(),
            failure: RefCell::new(None),
        };
        let view = CheckedCanonicalGuardedGlobalReadsV1 {
            facts: &facts,
            accounting: &accounting,
        };
        // Keep the sticky query state alive while arbitrary callback code unwinds.
        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| consume(&view, budget)));
        let result = match caught {
            Ok(mut result) => {
                let local = if !accounting.valid(budget) || budget.storage() != accounting.floor {
                    Err(ResourceError::Accounting.into())
                } else {
                    accounting.enter(budget)
                };
                if let Err(error) = local {
                    drain(std::mem::replace(&mut result, Err(error)));
                }
                result
            }
            Err(payload) => {
                drain(payload);
                // Unwind may leave paid scratch credit; the outer scope refunds it
                // only after all payload and fact backing has been destroyed.
                Err(if !accounting.valid(budget) {
                    ResourceError::Accounting.into()
                } else {
                    accounting
                        .failure
                        .borrow()
                        .clone()
                        .unwrap_or(Failure::Panicked)
                })
            }
        };
        drop(facts);
        result
    }));
    if slot != std::ptr::from_ref(&*budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        drain(returned);
        return Err(ResourceError::Accounting.into());
    }
    let result = match &returned {
        Ok(_) => None,
        Err(_) => Some(Failure::Panicked),
    };
    if let Some(error) = result {
        drain(std::mem::replace(&mut returned, Ok(Err(error))));
    }
    let result = match returned {
        Ok(result) => result,
        Err(_) => unreachable!("drained panic replaced"),
    };
    if let Err(error) = budget.release_storage(budget.storage() - floor) {
        drain(result);
        return Err(error.into());
    }
    result
}

fn build<'g, const STORE: bool, O>(
    owner: &'g O,
    module: &'g Module,
    limits: CanonicalGuardedGlobalReadLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<Facts<'g, O>> {
    let count = module.functions.len();
    if count > limits.functions {
        return Err(Failure::FunctionLimit {
            actual: count,
            limit: limits.functions,
        });
    }
    budget.charge_work(1)?;
    let mut functions = Vec::new();
    let bytes = count
        .checked_mul(size_of::<FunctionFacts<'_>>())
        .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    functions
        .try_reserve_exact(count)
        .map_err(|_| ResourceError::Allocation)?;
    budget.reserve_storage(
        functions
            .capacity()
            .checked_sub(count)
            .and_then(|n| n.checked_mul(size_of::<FunctionFacts<'_>>()))
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    for (ordinal, function) in module.functions.iter().enumerate() {
        budget.charge_work(1)?;
        let coordinate =
            FunctionCoordinate(u32::try_from(ordinal).map_err(|_| ResourceError::Arithmetic)?);
        functions.push(build_function::<STORE>(
            function, coordinate, limits, budget,
        )?);
    }
    Ok(Facts { owner, functions })
}

fn build_function<'g, const STORE: bool>(
    function: &'g Function,
    coordinate: FunctionCoordinate,
    limits: CanonicalGuardedGlobalReadLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<FunctionFacts<'g>> {
    let floor = budget.storage();
    let mut result = FunctionFacts {
        function,
        controls: Vec::new(),
        predicates: Vec::new(),
        reads: Vec::new(),
        global_read_occurrences: 0,
        other_global_effects: 0,
        unresolved_calls: 0,
    };
    let Some(body) = function.body.as_ref() else {
        return Ok(result);
    };
    budget.reserve_storage(size_of::<FunctionFacts<'_>>())?;
    let flow = crate::control_flow::analyze_control_flow_with_verification_budget_v1(
        function,
        limits.control_flow,
        budget,
    )
    .map_err(|error| match error {
        crate::control_flow::MeteredControlFlowErrorV1::ControlFlow(error) => {
            Failure::ControlFlow(error)
        }
        crate::control_flow::MeteredControlFlowErrorV1::Resource(error) => error.into(),
    })?;
    {
        let mut meter = LiveGuardMeter::new(
            budget,
            limits.per_function_work,
            limits.per_function_new_bytes,
            limits.per_function_records,
        );
        // Prepay the complete returned state, including the unselected owned
        // meter. The selected analysis keeps its existing independent header.
        meter.storage(size_of::<GuardedControlCollectionV1<LiveGuardMeter<'_, '_>>>())?;
        let seed = GuardedControlV1::collect_preserving_ledger_v24::<STORE>(
            function,
            flow.indexed_v15(),
            meter,
        )?;
        match seed {
            GuardedControlCollectionV1::Selected(seed) => {
                let entry = seed.entry;
                let mut analysis = GuardedAnalysisV1::empty(seed, false);
                collect_actual_definitions(&mut analysis, function)?;
                collect_actual_origins(&mut analysis, function, flow.indexed_v15())?;
                analysis.collect_parameters_and_carried_truths(function, entry)?;
                result.predicates = analysis.expanded_predicates()?;
                analysis.collect_runtime_access_guards_v24::<STORE>(function)?;
                collect_effects::<STORE, _>(&mut result, coordinate, body, Some(&mut analysis))?;
                result.controls = std::mem::take(&mut analysis.control);
            }
            GuardedControlCollectionV1::Unselected(mut meter) => {
                collect_effects_without_reads_profile_v24::<STORE, _>(
                    &mut result,
                    coordinate,
                    body,
                    &mut meter,
                )?;
            }
        }
    }
    flow.release(budget)?;
    let retained = result.retained_bytes()?;
    let excess = budget
        .storage()
        .checked_sub(floor)
        .and_then(|n| n.checked_sub(retained))
        .ok_or(ResourceError::Accounting)?;
    budget.release_storage(excess)?;
    Ok(result)
}

pub(super) fn collect_actual_definitions<'g, M: GuardMeter>(
    analysis: &mut GuardedAnalysisV1<'g, M>,
    function: &'g Function,
) -> std::result::Result<(), ResourceError> {
    let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
    for block in &body.blocks {
        analysis.ledger.charge(2)?;
        if analysis
            .control_row(block.id)?
            .is_none_or(|row| row.interval.is_none())
        {
            continue;
        }
        for operation in &block.operations {
            analysis.ledger.charge(2)?;
            for result in &operation.results {
                analysis.ledger.push(
                    &mut analysis.definitions,
                    DefinitionRow {
                        value: result.id,
                        operation,
                        plane: PointerPlane::Unvisited,
                    },
                )?;
            }
        }
    }
    analysis
        .ledger
        .sort(&mut analysis.definitions, 1, |a, b| a.value.cmp(&b.value))
}

fn collect_actual_origins<'g, M: GuardMeter>(
    analysis: &mut GuardedAnalysisV1<'g, M>,
    function: &'g Function,
    flow: &IndexedControlFlow,
) -> std::result::Result<(), ResourceError> {
    let control = &analysis.control;
    collect_source_origins_v2(
        &mut analysis.ledger,
        function,
        flow,
        &mut analysis.runtime_reads.origins,
        |meter, block| {
            Ok(meter
                .find(control, |row| row.block.cmp(&block))?
                .is_some_and(|index| control[index].interval.is_some()))
        },
    )
}

// Share the exact original-edge census and SCC transfer; callers provide a
// paid reachability query bound to this same immutable function and CFG.
pub(super) fn collect_source_origins_v2<'g, M: GuardMeter>(
    meter: &mut M,
    function: &'g Function,
    flow: &IndexedControlFlow,
    output: &mut Vec<runtime_slice_read_v1::Origin<'g>>,
    reachable: impl FnMut(&mut M, BlockId) -> std::result::Result<bool, ResourceError>,
) -> std::result::Result<(), ResourceError> {
    let retained = collect_source_origin_inputs_v3(meter, function, flow, output, reachable)?;
    drop(retained);
    Ok(())
}

pub(super) fn collect_source_origin_inputs_v3<'g, M: GuardMeter>(
    meter: &mut M,
    function: &'g Function,
    flow: &IndexedControlFlow,
    output: &mut Vec<runtime_slice_read_v1::Origin<'g>>,
    mut reachable: impl FnMut(&mut M, BlockId) -> std::result::Result<bool, ResourceError>,
) -> std::result::Result<(Vec<origins::Input>, Vec<ValueId>), ResourceError> {
    meter.storage(
        3_usize
            .checked_mul(size_of::<Vec<()>>())
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
    let mut inputs = Vec::new();
    let mut incoming = Vec::new();
    let mut types = Vec::new();
    for block in &body.blocks {
        meter.charge(2)?;
        if !reachable(meter, block.id)? {
            continue;
        }
        for (ordinal, parameter) in block.parameters.iter().enumerate() {
            meter.charge(4)?;
            let start = incoming.len();
            if block.id != body.blocks[0].id {
                meter.charge(
                    crate::verification_index_v1::verification_ceil_log2_v1(body.blocks.len())
                        .checked_add(4)
                        .ok_or(ResourceError::Arithmetic)?,
                )?;
                for &edge in flow
                    .incoming_edges(block.id)
                    .ok_or(ResourceError::Accounting)?
                {
                    meter.charge(8)?;
                    let source = flow.edge_source(edge).ok_or(ResourceError::Accounting)?;
                    if reachable(meter, source)? {
                        let value = *flow
                            .edge_arguments(function, edge)
                            .get(ordinal)
                            .ok_or(ResourceError::Accounting)?;
                        meter.push(&mut incoming, value)?;
                    }
                }
            }
            meter.push(
                &mut inputs,
                origins::Input {
                    value: parameter.id,
                    incoming: start..incoming.len(),
                },
            )?;
            meter.push(&mut types, (parameter.id, &parameter.ty))?;
        }
    }
    meter.sort(&mut inputs, 1, |a, b| a.value.cmp(&b.value))?;
    meter.sort(&mut types, 1, |a, b| a.0.cmp(&b.0))?;
    let resolved = origins::resolve(meter, &inputs, &incoming)?;
    meter.reserve(output, inputs.len())?;
    for ((input, origin), (value, ty)) in inputs.iter().zip(resolved).zip(types) {
        meter.charge(3)?;
        if input.value != value {
            return Err(ResourceError::Accounting);
        }
        output.push(runtime_slice_read_v1::Origin { value, origin, ty });
    }
    Ok((inputs, incoming))
}

fn operation_coordinate(
    function: FunctionCoordinate,
    block: usize,
    operation: usize,
) -> Result<Coordinate> {
    Ok(Coordinate {
        block: BlockCoordinate {
            function,
            block: u32::try_from(block).map_err(|_| ResourceError::Arithmetic)?,
        },
        operation: u32::try_from(operation).map_err(|_| ResourceError::Arithmetic)?,
    })
}

fn effect_counts<const STORE: bool, M: GuardMeter>(
    result: &mut FunctionFacts<'_>,
    operation: &Operation,
    generic_may_be_global: bool,
    meter: &mut M,
) -> Result<bool> {
    meter.charge(2)?;
    if matches!(operation.kind, OperationKind::Call { .. }) {
        result.unresolved_calls = result
            .unresolved_calls
            .checked_add(1)
            .ok_or(ResourceError::Arithmetic)?;
    }
    let mut read = false;
    operation.try_visit_local_memory_effects_v1(|effect| -> Result<()> {
        meter.charge(2)?;
        match effect {
            KirLocalMemoryEffectRefV1::Read(space)
            | KirLocalMemoryEffectRefV1::VolatileRead(space)
                if !STORE
                    && (space == AddressSpace::Global
                        || (space == AddressSpace::Generic && generic_may_be_global)) =>
            {
                read = true;
                result.global_read_occurrences = result
                    .global_read_occurrences
                    .checked_add(1)
                    .ok_or(ResourceError::Arithmetic)?;
            }
            KirLocalMemoryEffectRefV1::Write(space)
            | KirLocalMemoryEffectRefV1::VolatileWrite(space)
                if STORE
                    && (space == AddressSpace::Global
                        || (space == AddressSpace::Generic && generic_may_be_global)) =>
            {
                read = true;
                result.global_read_occurrences = result
                    .global_read_occurrences
                    .checked_add(1)
                    .ok_or(ResourceError::Arithmetic)?;
            }
            KirLocalMemoryEffectRefV1::Read(space)
            | KirLocalMemoryEffectRefV1::VolatileRead(space)
                if space == AddressSpace::Global
                    || (space == AddressSpace::Generic && generic_may_be_global) =>
            {
                result.other_global_effects = result
                    .other_global_effects
                    .checked_add(1)
                    .ok_or(ResourceError::Arithmetic)?;
            }
            KirLocalMemoryEffectRefV1::Write(space)
            | KirLocalMemoryEffectRefV1::VolatileWrite(space)
            | KirLocalMemoryEffectRefV1::Allocate(space)
            | KirLocalMemoryEffectRefV1::Atomic {
                address_space: space,
                ..
            } if space == AddressSpace::Global
                || (space == AddressSpace::Generic && generic_may_be_global) =>
            {
                result.other_global_effects = result
                    .other_global_effects
                    .checked_add(1)
                    .ok_or(ResourceError::Arithmetic)?;
            }
            KirLocalMemoryEffectRefV1::Synchronize { address_spaces, .. }
            | KirLocalMemoryEffectRefV1::Fence { address_spaces, .. } => {
                let global = match address_spaces {
                    crate::KirAddressSpacesRefV1::Singleton(space) => {
                        matches!(space, AddressSpace::Global | AddressSpace::Generic)
                    }
                    crate::KirAddressSpacesRefV1::Borrowed(spaces) => {
                        let mut global = false;
                        for space in spaces {
                            meter.charge(1)?;
                            global |= matches!(space, AddressSpace::Global | AddressSpace::Generic);
                        }
                        global
                    }
                };
                if global {
                    result.other_global_effects = result
                        .other_global_effects
                        .checked_add(1)
                        .ok_or(ResourceError::Arithmetic)?;
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(read)
}

fn collect_effects<'g, const STORE: bool, M: GuardMeter>(
    result: &mut FunctionFacts<'g>,
    function: FunctionCoordinate,
    body: &'g crate::FunctionBody,
    analysis: Option<&mut GuardedAnalysisV1<'g, M>>,
) -> Result<()> {
    let analysis = analysis.ok_or(ResourceError::Accounting)?;
    analysis.ledger.storage(read_row_frame_bytes()?)?;
    for (block_ordinal, block) in body.blocks.iter().enumerate() {
        analysis.ledger.charge(1)?;
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let generic_may_be_global = match operation.kind {
                OperationKind::Load { pointer, access }
                | OperationKind::Store {
                    pointer, access, ..
                }
                | OperationKind::GuardedLoad {
                    pointer, access, ..
                }
                | OperationKind::GuardedStore {
                    pointer, access, ..
                } if access.address_space == AddressSpace::Generic => analysis
                    .proven_pointer_space_v18(pointer)?
                    .is_none_or(|space| {
                        matches!(space, AddressSpace::Global | AddressSpace::Generic)
                    }),
                _ => true,
            };
            if !effect_counts::<STORE, _>(
                result,
                operation,
                generic_may_be_global,
                &mut analysis.ledger,
            )? {
                continue;
            }
            let coordinate = operation_coordinate(function, block_ordinal, ordinal)?;
            let (conditions, reason) = match operation.kind {
                OperationKind::Load { pointer, access }
                    if !STORE
                        && !access.volatile
                        && matches!(
                            access.address_space,
                            AddressSpace::Global | AddressSpace::Generic
                        ) =>
                {
                    (
                        analysis.runtime_slice_access_conditions_v24::<false>(
                            FunctionOperationLocation::new(block.id, ordinal),
                            pointer,
                            FormalMemoryAccessKind::Read,
                            access,
                            None,
                        )?,
                        CanonicalGuardedGlobalReadReasonV1::MissingBoundOrProvenance,
                    )
                }
                OperationKind::Store {
                    pointer, access, ..
                } if STORE
                    && !access.volatile
                    && matches!(
                        access.address_space,
                        AddressSpace::Global | AddressSpace::Generic
                    ) =>
                {
                    (
                        analysis.runtime_slice_access_conditions_v24::<true>(
                            FunctionOperationLocation::new(block.id, ordinal),
                            pointer,
                            FormalMemoryAccessKind::Write,
                            access,
                            None,
                        )?,
                        CanonicalGuardedGlobalReadReasonV1::MissingBoundOrProvenance,
                    )
                }
                _ => (
                    None,
                    CanonicalGuardedGlobalReadReasonV1::NotOrdinaryGlobalRead,
                ),
            };
            analysis.ledger.push(
                &mut result.reads,
                ReadRow {
                    coordinate,
                    conditions,
                    reason,
                },
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn collect_effects_without_reads<M: GuardMeter>(
    result: &mut FunctionFacts<'_>,
    function: FunctionCoordinate,
    body: &crate::FunctionBody,
    meter: &mut M,
) -> Result<()> {
    collect_effects_without_reads_profile_v24::<false, _>(result, function, body, meter)
}

fn collect_effects_without_reads_profile_v24<const STORE: bool, M: GuardMeter>(
    result: &mut FunctionFacts<'_>,
    function: FunctionCoordinate,
    body: &crate::FunctionBody,
    meter: &mut M,
) -> Result<()> {
    meter.storage(read_row_frame_bytes()?)?;
    for (block_ordinal, block) in body.blocks.iter().enumerate() {
        meter.charge(1)?;
        for (ordinal, operation) in block.operations.iter().enumerate() {
            if effect_counts::<STORE, _>(result, operation, true, meter)? {
                let coordinate = operation_coordinate(function, block_ordinal, ordinal)?;
                meter.push(
                    &mut result.reads,
                    ReadRow {
                        coordinate,
                        conditions: None,
                        reason: CanonicalGuardedGlobalReadReasonV1::NotOrdinaryGlobalRead,
                    },
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "canonical_guarded_reads_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "canonical_guarded_reads_v18_tests.rs"]
mod v18_tests;

#[cfg(test)]
#[path = "canonical_guarded_reads_switch_truth_v1_tests.rs"]
mod switch_truth_tests;
