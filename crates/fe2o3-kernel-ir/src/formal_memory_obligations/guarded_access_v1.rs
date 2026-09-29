use super::*;
use crate::verification_index_v1::{verification_bounded_sort_by_v1, verification_find_last_by_v1};
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as VerificationResourceError,
    CanonicalKernelIrWorkBudgetV1, ComparePredicate, IndexedControlFlow, Terminator,
};
use std::mem::size_of;

#[path = "runtime_slice_read_v1.rs"]
mod runtime_slice_read_v1;

#[path = "canonical_guarded_reads_v1.rs"]
mod canonical_reads;
#[path = "guarded_meter_v1.rs"]
mod meter;
#[path = "guarded_origins_v1.rs"]
pub(super) mod origins;
#[path = "guarded_predicates_v1.rs"]
mod predicates;
#[path = "source_context_bytes_v2.rs"]
pub(super) mod source_context_bytes_v2;

#[path = "affine_source_bytes_v18.rs"]
pub(super) mod affine_source_bytes_v18;
pub use canonical_reads::*;
use meter::GuardMeter;

impl FormalAliasRegionV1 {
    pub(super) fn union(self, other: Self) -> Self {
        match (self, other) {
            (Self::FixedBytes(a), Self::FixedBytes(b)) => Self::FixedBytes(FormalByteRange {
                start: a.start.min(b.start),
                end_exclusive: a.end_exclusive.max(b.end_exclusive),
            }),
            _ => Self::WholeFormalAllocation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalGuardedMemoryResourceErrorV1 {
    Work(crate::CanonicalKernelIrWorkLimitV1),
    Storage { actual: usize, limit: usize },
    Allocation,
    Accounting,
    Arithmetic,
}

impl fmt::Display for FormalGuardedMemoryResourceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Work(error) => error.fmt(formatter),
            Self::Storage { actual, limit } => {
                write!(formatter, "guarded formal storage {actual} exceeds {limit}")
            }
            Self::Allocation => formatter.write_str("guarded formal allocation failed"),
            Self::Accounting => formatter.write_str("guarded formal accounting mismatch"),
            Self::Arithmetic => formatter.write_str("guarded formal resource arithmetic overflow"),
        }
    }
}
impl Error for FormalGuardedMemoryResourceErrorV1 {}
impl From<VerificationResourceError> for FormalGuardedMemoryResourceErrorV1 {
    fn from(error: VerificationResourceError) -> Self {
        match error {
            VerificationResourceError::Work(error) => Self::Work(error),
            VerificationResourceError::Storage(error) => Self::Storage {
                actual: error.actual(),
                limit: error.limit(),
            },
            VerificationResourceError::Allocation => Self::Allocation,
            VerificationResourceError::Accounting => Self::Accounting,
            VerificationResourceError::Arithmetic => Self::Arithmetic,
        }
    }
}
use FormalGuardedMemoryResourceErrorV1 as ResourceError;
pub(super) type GuardedResourceErrorV1 = FormalGuardedMemoryResourceErrorV1;
const RECIPE_WORK: usize = 128;
const USE_WORK: usize = 64;
pub(super) const BOUNDS_WORK: usize = 16;
// Reuse the decoder's conservative numeric ceiling, not its accounting domain.
const MAX_NEW_BYTES: usize = MAX_FORMAL_MEMORY_DECODER_AUXILIARY_BYTES_V1;

pub(super) struct GuardLedger {
    work: CanonicalKernelIrWorkBudgetV1,
    bytes: usize,
    records: usize,
}

impl GuardLedger {
    fn new(cfg_work: u64) -> Result<Self, ResourceError> {
        let mut result = Self {
            work: CanonicalKernelIrWorkBudgetV1::new(crate::MAX_CFG_ANALYSIS_WORK as usize),
            bytes: 0,
            records: 0,
        };
        result.charge(usize::try_from(cfg_work).map_err(|_| ResourceError::Arithmetic)?)?;
        Ok(result)
    }
    pub(super) fn charge(&mut self, work: usize) -> Result<(), ResourceError> {
        self.work.charge_work(work).map_err(ResourceError::Work)
    }
    fn storage(&mut self, bytes: usize) -> Result<(), ResourceError> {
        let actual = self
            .bytes
            .checked_add(bytes)
            .ok_or(ResourceError::Arithmetic)?;
        if actual > MAX_NEW_BYTES {
            return Err(ResourceError::Storage {
                actual,
                limit: MAX_NEW_BYTES,
            });
        }
        self.bytes = actual;
        Ok(())
    }
    fn reserve<T>(&mut self, rows: &mut Vec<T>, count: usize) -> Result<(), ResourceError> {
        self.charge(2)?;
        if rows.capacity() >= count {
            return Ok(());
        }
        // Reallocation can move every existing fixed-size row.
        self.charge(rows.len())?;
        let records = self
            .records
            .checked_add(count)
            .ok_or(ResourceError::Arithmetic)?;
        if records > MAX_FORMAL_MEMORY_RECORDS_V1 {
            return Err(ResourceError::Storage {
                actual: records,
                limit: MAX_FORMAL_MEMORY_RECORDS_V1,
            });
        }
        self.storage(
            count
                .checked_mul(size_of::<T>())
                .ok_or(ResourceError::Arithmetic)?,
        )?;
        self.records = records;
        rows.try_reserve_exact(
            count
                .checked_sub(rows.len())
                .ok_or(ResourceError::Accounting)?,
        )
        .map_err(|_| ResourceError::Allocation)?;
        let excess = rows
            .capacity()
            .checked_sub(count)
            .ok_or(ResourceError::Accounting)?;
        self.storage(
            excess
                .checked_mul(size_of::<T>())
                .ok_or(ResourceError::Arithmetic)?,
        )?;
        let records = self
            .records
            .checked_add(excess)
            .ok_or(ResourceError::Arithmetic)?;
        if records > MAX_FORMAL_MEMORY_RECORDS_V1 {
            return Err(ResourceError::Storage {
                actual: records,
                limit: MAX_FORMAL_MEMORY_RECORDS_V1,
            });
        }
        self.records = records;
        Ok(())
    }
    pub(super) fn push<T>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), ResourceError> {
        self.charge(1)?;
        if rows.len() == rows.capacity() {
            let count = rows
                .capacity()
                .max(1)
                .checked_mul(2)
                .ok_or(ResourceError::Arithmetic)?;
            self.reserve(rows, count)?;
        }
        rows.push(value);
        Ok(())
    }
    pub(super) fn sort<T>(
        &mut self,
        rows: &mut [T],
        width: usize,
        compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
    ) -> Result<(), ResourceError> {
        verification_bounded_sort_by_v1(rows, width, &mut Budget::new(&mut self.work, 0), compare)
            .map_err(Into::into)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Edge {
    source: BlockId,
    ordinal: usize,
    target: BlockId,
}
#[derive(Clone, Copy)]
struct ControlRow {
    block: BlockId,
    interval: Option<(u32, u32)>,
    // Unique reachable entering edge; dominated backedges cannot first enter.
    incoming: Option<Edge>,
}
pub(super) struct GuardedControlV1<M = GuardLedger> {
    ledger: M,
    rows: Vec<ControlRow>,
    entry: BlockId,
}

enum GuardedControlCollectionV1<M> {
    Selected(GuardedControlV1<M>),
    Unselected(M),
}

impl GuardedControlV1 {
    pub(super) fn collect(
        function: &Function,
        flow: &IndexedControlFlow,
    ) -> Result<Option<Self>, ResourceError> {
        Self::collect_with_ledger(function, flow, GuardLedger::new(flow.work().total)?)
    }
}

impl<M: GuardMeter> GuardedControlV1<M> {
    fn collect_with_ledger(
        function: &Function,
        flow: &IndexedControlFlow,
        ledger: M,
    ) -> Result<Option<Self>, ResourceError> {
        Ok(
            match Self::collect_preserving_ledger(function, flow, ledger)? {
                GuardedControlCollectionV1::Selected(control) => Some(control),
                GuardedControlCollectionV1::Unselected(_) => None,
            },
        )
    }

    fn collect_preserving_ledger(
        function: &Function,
        flow: &IndexedControlFlow,
        mut ledger: M,
    ) -> Result<GuardedControlCollectionV1<M>, ResourceError> {
        let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
        ledger.charge(32)?;
        let mut selected = false;
        for block in &body.blocks {
            ledger.charge(2)?;
            for operation in &block.operations {
                ledger.charge(2)?;
                selected |= matches!(operation.kind, OperationKind::Select { .. })
                    || matches!(operation.kind, OperationKind::Load { access, .. }
                        if matches!(access.address_space, AddressSpace::Global | AddressSpace::Generic)
                            && !access.volatile);
            }
        }
        if !selected {
            // The live caller must continue its census on this same local meter.
            return Ok(GuardedControlCollectionV1::Unselected(ledger));
        }
        let headers = size_of::<GuardedAnalysisV1<'_, M>>()
            .checked_add(5 * size_of::<Vec<()>>())
            .ok_or(ResourceError::Arithmetic)?;
        ledger.storage(headers)?;
        let mut rows = Vec::new();
        ledger.reserve(&mut rows, body.blocks.len())?;
        let logarithm = crate::verification_index_v1::verification_ceil_log2_v1(body.blocks.len());
        let lookup = logarithm.checked_add(4).ok_or(ResourceError::Arithmetic)?;
        for block in &body.blocks {
            // Two sparse CFG queries, then one paid reachability query per incoming edge.
            ledger.charge(
                lookup
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(8))
                    .ok_or(ResourceError::Arithmetic)?,
            )?;
            let interval = flow.formal_guard_dominator_interval_v1(block.id);
            let mut incoming = None;
            let mut count = 0_usize;
            for edge in flow
                .incoming_edges(block.id)
                .ok_or(ResourceError::Accounting)?
            {
                ledger.charge(lookup.checked_add(8).ok_or(ResourceError::Arithmetic)?)?;
                let source = flow.edge_source(*edge).ok_or(ResourceError::Accounting)?;
                if flow.is_reachable(source) {
                    ledger.charge(lookup.checked_add(4).ok_or(ResourceError::Arithmetic)?)?;
                    let source_interval = flow.formal_guard_dominator_interval_v1(source);
                    if matches!((interval, source_interval),
                        (Some((start, end)), Some((source_start, source_end)))
                        if start <= source_start && source_end <= end)
                    {
                        continue;
                    }
                    count = count.checked_add(1).ok_or(ResourceError::Arithmetic)?;
                    incoming = Some(Edge {
                        source,
                        ordinal: flow.edge(*edge).ok_or(ResourceError::Accounting)?.ordinal(),
                        target: block.id,
                    });
                }
            }
            rows.push(ControlRow {
                block: block.id,
                interval,
                incoming: (count == 1 && block.id != body.blocks[0].id)
                    .then_some(incoming)
                    .flatten(),
            });
        }
        ledger.sort(&mut rows, 1, |a, b| a.block.cmp(&b.block))?;
        Ok(GuardedControlCollectionV1::Selected(Self {
            ledger,
            rows,
            entry: body.blocks[0].id,
        }))
    }
}

#[derive(Clone, Copy)]
enum PointerPlane {
    Unvisited,
    Pending(Option<usize>),
    Resolved(Option<AddressSpace>),
}

#[derive(Clone, Copy)]
struct DefinitionRow<'module> {
    value: ValueId,
    operation: &'module Operation,
    plane: PointerPlane,
}
#[derive(Clone, Copy)]
struct ParameterRow<'module> {
    value: ValueId,
    ordinal: u32,
    ty: &'module Type,
}
#[derive(Clone, Copy)]
struct TrueRow {
    predicate: ValueId,
    edge: Edge,
    interval: (u32, u32),
    ambiguous: bool,
}

fn single_case_switch_truth_frame_bytes() -> Result<usize, ResourceError> {
    // Only the new one-case path uses this edge-selection helper. The existing
    // two-case matcher keeps its original work and storage debits.
    size_of::<(
        &crate::BasicBlock,
        Option<&Terminator>,
        &Vec<crate::SwitchCase>,
        &[crate::SwitchCase],
        &crate::SwitchCase,
        &BlockId,
        Edge,
        Option<Edge>,
        [usize; 2],
        u64,
        [bool; 2],
    )>()
    .checked_add(size_of::<&mut GuardLedger>())
    .and_then(|n| n.checked_add(2 * size_of::<Result<(), ResourceError>>()))
    .and_then(|n| n.checked_add(2 * size_of::<Result<usize, ResourceError>>()))
    .and_then(|n| n.checked_add(size_of::<Option<usize>>()))
    .ok_or(ResourceError::Arithmetic)
}

fn prepay_single_case_switch_truth<M: GuardMeter>(meter: &mut M) -> Result<(), ResourceError> {
    meter.storage(single_case_switch_truth_frame_bytes()?)?;
    meter.charge(8)
}

// This selects an edge only. The caller must authenticate a ZeroExtend of a
// Bool producer, and the existing unique-incoming/dominance gate still applies.
fn single_case_switch_true_edge(block: &crate::BasicBlock) -> Option<Edge> {
    let Some(Terminator::Switch {
        cases,
        default_target,
        ..
    }) = block.terminator.as_ref()
    else {
        return None;
    };
    let [case] = cases.as_slice() else {
        return None;
    };
    if case.target == *default_target {
        return None;
    }
    match case.value {
        1 => Some(Edge {
            source: block.id,
            ordinal: 0,
            target: case.target,
        }),
        0 => Some(Edge {
            source: block.id,
            ordinal: 1,
            target: *default_target,
        }),
        _ => None,
    }
}
#[derive(Clone, Copy)]
struct Recipe {
    domain: FormalSliceBoundedDomainV1,
    address_space: AddressSpace,
}

pub(super) struct GuardedAnalysisV1<'module, M = GuardLedger> {
    pub(super) ledger: M,
    control: Vec<ControlRow>,
    definitions: Vec<DefinitionRow<'module>>,
    parameters: Vec<ParameterRow<'module>>,
    truths: Vec<TrueRow>,
    recipes: Vec<Recipe>,
    runtime_reads: runtime_slice_read_v1::RuntimeReadState<'module>,
    rank_one: bool,
}

impl<'module, M> GuardedAnalysisV1<'module, M> {
    fn replace_meter<N>(self, ledger: N) -> GuardedAnalysisV1<'module, N> {
        GuardedAnalysisV1 {
            ledger,
            control: self.control,
            definitions: self.definitions,
            parameters: self.parameters,
            truths: self.truths,
            recipes: self.recipes,
            runtime_reads: self.runtime_reads,
            rank_one: self.rank_one,
        }
    }
}

impl<'module, M: GuardMeter> GuardedAnalysisV1<'module, M> {
    pub(super) fn new(
        seed: GuardedControlV1<M>,
        definitions: &Definitions<'module>,
        function: &'module Function,
        rank_one: bool,
    ) -> Result<Self, ResourceError> {
        let entry = seed.entry;
        let mut result = Self::empty(seed, rank_one);
        result
            .ledger
            .reserve(&mut result.definitions, definitions.operations.len())?;
        for (value, (operation, _)) in &definitions.operations {
            result.ledger.charge(2)?;
            result.definitions.push(DefinitionRow {
                value: *value,
                operation,
                plane: PointerPlane::Unvisited,
            });
        }
        result.collect_parameters_and_truths(function, entry)?;
        result.collect_runtime_reads(definitions, function)?;
        result.collect_recipes()?;
        Ok(result)
    }

    fn empty(seed: GuardedControlV1<M>, rank_one: bool) -> Self {
        Self {
            ledger: seed.ledger,
            control: seed.rows,
            definitions: Vec::new(),
            parameters: Vec::new(),
            truths: Vec::new(),
            recipes: Vec::new(),
            runtime_reads: runtime_slice_read_v1::RuntimeReadState::default(),
            rank_one,
        }
    }

    fn collect_parameters_and_truths(
        &mut self,
        function: &'module Function,
        entry: BlockId,
    ) -> Result<(), ResourceError> {
        self.collect_parameters_and_truths_impl::<false>(function, entry)
    }

    fn collect_parameters_and_carried_truths(
        &mut self,
        function: &'module Function,
        entry: BlockId,
    ) -> Result<(), ResourceError> {
        self.collect_parameters_and_truths_impl::<true>(function, entry)
    }

    fn collect_parameters_and_truths_impl<const CARRIED: bool>(
        &mut self,
        function: &'module Function,
        entry: BlockId,
    ) -> Result<(), ResourceError> {
        let result = self;
        let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
        result
            .ledger
            .reserve(&mut result.parameters, body.parameters.len())?;
        for (ordinal, (value, ty)) in body
            .parameters
            .iter()
            .zip(&function.signature.parameters)
            .enumerate()
        {
            result.ledger.charge(3)?;
            result.parameters.push(ParameterRow {
                value: *value,
                ordinal: u32::try_from(ordinal).map_err(|_| ResourceError::Arithmetic)?,
                ty,
            });
        }
        result
            .ledger
            .sort(&mut result.parameters, 1, |a, b| a.value.cmp(&b.value))?;
        result
            .ledger
            .reserve(&mut result.truths, body.blocks.len())?;
        for block in &body.blocks {
            result.ledger.charge(32)?;
            let Some(source) = result.control_row(block.id)? else {
                continue;
            };
            if source.interval.is_none() {
                continue;
            }
            let candidate = match block.terminator.as_ref() {
                Some(Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    ..
                }) => Some((
                    *condition,
                    Edge {
                        source: block.id,
                        ordinal: 0,
                        target: *then_target,
                    },
                )),
                Some(Terminator::Switch {
                    selector, cases, ..
                }) if (cases.len() == 2 && cases[0].value == 0 && cases[1].value == 1)
                    || (cases.len() == 1 && cases[0].value <= 1) =>
                {
                    if cases.len() == 1 {
                        prepay_single_case_switch_truth(&mut result.ledger)?;
                    }
                    result.ledger.charge(16)?;
                    let selector = result.definition(*selector)?;
                    selector.and_then(|op| match &op.kind {
                        OperationKind::Cast {
                            kind: CastKind::ZeroExtend,
                            value,
                            to,
                        } if matches!(
                            to,
                            Type::Scalar(
                                ScalarType::I8
                                    | ScalarType::I16
                                    | ScalarType::I32
                                    | ScalarType::I64
                                    | ScalarType::U8
                                    | ScalarType::U16
                                    | ScalarType::U32
                                    | ScalarType::U64
                            )
                        ) && matches!(op.results.as_slice(), [r] if &r.ty == to) =>
                        {
                            Some((
                                *value,
                                if cases.len() == 1 {
                                    single_case_switch_true_edge(block)?
                                } else {
                                    Edge {
                                        source: block.id,
                                        ordinal: 1,
                                        target: cases[1].target,
                                    }
                                },
                            ))
                        }
                        _ => None,
                    })
                }
                _ => None,
            };
            let Some((predicate, edge)) = candidate else {
                continue;
            };
            let supported = match result.definition(predicate)? {
                Some(operation) => single_type(operation, &Type::BOOL),
                None if CARRIED => result.boolean_carrier_definition(predicate)?.is_some(),
                None => false,
            };
            if !supported || edge.target == entry {
                continue;
            }
            let Some(target) = result.control_row(edge.target)? else {
                continue;
            };
            if target.incoming != Some(edge) {
                continue;
            }
            let Some(interval) = target.interval else {
                continue;
            };
            result.truths.push(TrueRow {
                predicate,
                edge,
                interval,
                ambiguous: false,
            });
        }
        result
            .ledger
            .sort(&mut result.truths, 1, |a, b| a.predicate.cmp(&b.predicate))?;
        for i in 1..result.truths.len() {
            result.ledger.charge(2)?;
            if result.truths[i - 1].predicate == result.truths[i].predicate {
                result.truths[i - 1].ambiguous = true;
                result.truths[i].ambiguous = true;
            }
        }
        Ok(())
    }

    fn collect_recipes(&mut self) -> Result<(), ResourceError> {
        let result = self;
        result
            .ledger
            .reserve(&mut result.recipes, result.definitions.len())?;
        for index in 0..result.definitions.len() {
            result.ledger.charge(2)?;
            let row = result.definitions[index];
            let operation = match row.operation.kind {
                OperationKind::GetElementPointer { .. } => row.operation,
                OperationKind::Cast {
                    kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                    ..
                } => {
                    let Some(origin) = result.peel_pointer_casts(row.value)? else {
                        continue;
                    };
                    let Some(operation) = result.definition(origin)? else {
                        continue;
                    };
                    operation
                }
                _ => continue,
            };
            if let Some(recipe) = result.recipe(row.value, operation)? {
                result.recipes.push(recipe);
            }
        }
        // Definition order is ValueId order; filtering preserves the recipe index order.
        Ok(())
    }

    fn definition(&mut self, value: ValueId) -> Result<Option<&'module Operation>, ResourceError> {
        Ok(self
            .ledger
            .find(&self.definitions, |row| row.value.cmp(&value))?
            .map(|i| self.definitions[i].operation))
    }
    fn control_row(&mut self, block: BlockId) -> Result<Option<ControlRow>, ResourceError> {
        Ok(self
            .ledger
            .find(&self.control, |row| row.block.cmp(&block))?
            .map(|i| self.control[i]))
    }

    pub(super) fn peel_pointer_casts(
        &mut self,
        mut value: ValueId,
    ) -> Result<Option<ValueId>, ResourceError> {
        let bound = self
            .definitions
            .len()
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .ok_or(ResourceError::Arithmetic)?;
        for _ in 0..=bound {
            self.ledger.charge(4)?;
            let Some(origin) = self.runtime_origin(value)? else {
                return Ok(None);
            };
            if origin != value {
                let (Some(actual), Some(source)) =
                    (self.runtime_type(value)?, self.runtime_type(origin)?)
                else {
                    return Ok(None);
                };
                // Only leaf-pointee guards use this bounded alias query.
                if !guard_pointer_leaf(actual) || !guard_pointer_leaf(source) || actual != source {
                    return Ok(None);
                }
                value = origin;
                continue;
            }
            let Some(operation) = self.definition(value)? else {
                return Ok(Some(value));
            };
            let OperationKind::Cast {
                kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                value: source,
                to,
            } = &operation.kind
            else {
                return Ok(Some(value));
            };
            let Some(from) = self.runtime_type(*source)? else {
                return Ok(None);
            };
            if !guard_pointer_leaf(from)
                || !guard_pointer_leaf(to)
                || !matches!(operation.results.as_slice(), [r] if guard_pointer_leaf(&r.ty))
            {
                return Ok(None);
            }
            let Some(source) = checked_pointer_cast_source_v18(operation, from) else {
                return Ok(None);
            };
            value = source;
        }
        Ok(None)
    }

    pub(super) fn peel_slice_casts(
        &mut self,
        mut value: ValueId,
    ) -> Result<Option<ValueId>, ResourceError> {
        let bound = self
            .definitions
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or(ResourceError::Arithmetic)?;
        for _ in 0..=bound {
            self.ledger.charge(4)?;
            let Some(actual) = self.runtime_type(value)? else {
                return Ok(None);
            };
            if !guard_slice_leaf(actual) {
                return Ok(None);
            }
            let Some(origin) = self.runtime_origin(value)? else {
                return Ok(None);
            };
            if origin != value {
                let Some(source) = self.runtime_type(origin)? else {
                    return Ok(None);
                };
                if !guard_slice_leaf(source) || actual != source {
                    return Ok(None);
                }
                value = origin;
                continue;
            }
            let Some(operation) = self.definition(value)? else {
                return Ok(Some(value));
            };
            let OperationKind::Cast {
                kind: CastKind::SliceToGeneric,
                value: source,
                to,
            } = &operation.kind
            else {
                return Ok(Some(value));
            };
            let Some(from) = self.runtime_type(*source)? else {
                return Ok(None);
            };
            if !guard_slice_leaf(from)
                || !guard_slice_leaf(to)
                || !matches!(operation.results.as_slice(), [r] if guard_slice_leaf(&r.ty))
            {
                return Ok(None);
            }
            let Some(source) = checked_slice_cast_source_v18(operation, from) else {
                return Ok(None);
            };
            value = source;
        }
        Ok(None)
    }

    pub(super) fn proven_pointer_space_v18(
        &mut self,
        mut value: ValueId,
    ) -> Result<Option<AddressSpace>, ResourceError> {
        // Thread the pending path through the existing paid definition rows.
        // Each definition is resolved once, without per-query scratch allocation.
        let mut pending = None;
        let result = loop {
            self.ledger.charge(4)?;
            let Some(ty) = self.runtime_type(value)? else {
                break None;
            };
            let space = match ty {
                Type::Pointer(p) => p.address_space,
                Type::Slice(s) if guard_slice_leaf(ty) => s.address_space,
                _ => break None,
            };
            if space != AddressSpace::Generic {
                break Some(space);
            }
            let Some(origin) = self.runtime_origin(value)? else {
                break None;
            };
            if origin != value {
                let Some(source) = self.runtime_type(origin)? else {
                    break None;
                };
                if !(guard_pointer_leaf(source) || guard_slice_leaf(source)) || source != ty {
                    break None;
                }
                value = origin;
                continue;
            }
            let Some(index) = self
                .ledger
                .find(&self.definitions, |row| row.value.cmp(&value))?
            else {
                break None;
            };
            let row = self.definitions[index];
            match row.plane {
                PointerPlane::Resolved(space) => break space,
                PointerPlane::Pending(_) => break None,
                PointerPlane::Unvisited => {}
            }
            self.definitions[index].plane = PointerPlane::Pending(pending);
            pending = Some(index);
            let operation = row.operation;
            value = match operation.kind {
                OperationKind::SliceData { slice } => {
                    let Some(Type::Slice(source)) = self.runtime_type(slice)? else {
                        break None;
                    };
                    if !matches!(ty, Type::Pointer(p) if p.pointee == source.element
                        && p.access == source.access && p.address_space == source.address_space)
                        || !single_type(operation, ty)
                    {
                        break None;
                    }
                    slice
                }
                OperationKind::Cast {
                    kind: CastKind::SliceToGeneric,
                    value: source,
                    ref to,
                } => {
                    let Some(from) = self.runtime_type(source)? else {
                        break None;
                    };
                    if !guard_slice_leaf(from)
                        || !guard_slice_leaf(to)
                        || !matches!(operation.results.as_slice(), [r] if guard_slice_leaf(&r.ty))
                    {
                        break None;
                    }
                    let Some(source) = checked_slice_cast_source_v18(operation, from) else {
                        break None;
                    };
                    source
                }
                OperationKind::GetElementPointer { base, .. }
                | OperationKind::Storage(crate::StorageOperationV1::Project { base, .. }) => base,
                OperationKind::Cast {
                    kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                    value: source,
                    ref to,
                } => {
                    let Some(from) = self.runtime_type(source)? else {
                        break None;
                    };
                    if !guard_pointer_leaf(from)
                        || !guard_pointer_leaf(to)
                        || !matches!(operation.results.as_slice(), [r] if guard_pointer_leaf(&r.ty))
                    {
                        break None;
                    }
                    let Some(source) = checked_pointer_cast_source_v18(operation, from) else {
                        break None;
                    };
                    source
                }
                _ => break None,
            };
        };
        while let Some(index) = pending {
            self.ledger.charge(2)?;
            let PointerPlane::Pending(previous) = self.definitions[index].plane else {
                return Err(ResourceError::Accounting);
            };
            self.definitions[index].plane = PointerPlane::Resolved(result);
            pending = previous;
        }
        Ok(result)
    }

    fn recipe(
        &mut self,
        pointer: ValueId,
        gep: &Operation,
    ) -> Result<Option<Recipe>, ResourceError> {
        self.ledger.charge(RECIPE_WORK)?;
        if !self.rank_one {
            return Ok(None);
        }
        let OperationKind::GetElementPointer { base, offset } = gep.kind else {
            return Ok(None);
        };
        let [pointer_result] = gep.results.as_slice() else {
            return Ok(None);
        };
        if pointer_result.id != pointer
            && self.peel_pointer_casts(pointer)? != Some(pointer_result.id)
        {
            return Ok(None);
        }
        let Type::Pointer(pointer_type) = &pointer_result.ty else {
            return Ok(None);
        };
        // Establish scalar width before structural type comparisons so the
        // fixed recipe charge cannot conceal a recursive pointee walk.
        let Some(element_bytes) = pointer_byte_width(&pointer_result.ty) else {
            return Ok(None);
        };
        let Some(select) = self.definition(offset)? else {
            return Ok(None);
        };
        if !single_type(select, &Type::INDEX) {
            return Ok(None);
        }
        let OperationKind::Select {
            condition,
            true_value: index,
            false_value: zero,
        } = select.kind
        else {
            return Ok(None);
        };
        let Some(zero_op) = self.definition(zero)? else {
            return Ok(None);
        };
        if !single_type(zero_op, &Type::INDEX)
            || !matches!(zero_op.kind, OperationKind::Constant(Constant::Index(0)))
        {
            return Ok(None);
        }
        let Some(compare) = self.definition(condition)? else {
            return Ok(None);
        };
        if !single_type(compare, &Type::BOOL) {
            return Ok(None);
        }
        let OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs: length,
        } = compare.kind
        else {
            return Ok(None);
        };
        if lhs != index {
            return Ok(None);
        }
        let Some(index_op) = self.definition(index)? else {
            return Ok(None);
        };
        if !single_type(index_op, &Type::INDEX)
            || !matches!(index_op.kind, OperationKind::Intrinsic(ref intrinsic) if intrinsic.kind == (IntrinsicKind::InvocationIndex {kind: IndexKind::Global, axis: Axis::X}))
        {
            return Ok(None);
        }
        let Some(length_op) = self.definition(length)? else {
            return Ok(None);
        };
        if !single_type(length_op, &Type::INDEX) {
            return Ok(None);
        }
        let OperationKind::SliceLength { slice } = length_op.kind else {
            return Ok(None);
        };
        let Some(base_operation) = self.definition(base)? else {
            return Ok(None);
        };
        let data = if matches!(
            base_operation.kind,
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                ..
            }
        ) {
            if !single_type(base_operation, &pointer_result.ty) {
                return Ok(None);
            }
            let Some(source) = self.peel_pointer_casts(base)? else {
                return Ok(None);
            };
            let Some(operation) = self.definition(source)? else {
                return Ok(None);
            };
            operation
        } else {
            base_operation
        };
        let [data_result] = data.results.as_slice() else {
            return Ok(None);
        };
        let Type::Pointer(data_type) = &data_result.ty else {
            return Ok(None);
        };
        let OperationKind::SliceData { slice: data_slice } = data.kind else {
            return Ok(None);
        };
        if data_slice != slice {
            let Some(data_origin) = self.peel_slice_casts(data_slice)? else {
                return Ok(None);
            };
            if self.peel_slice_casts(slice)? != Some(data_origin) {
                return Ok(None);
            }
        }
        if data_type.pointee != pointer_type.pointee
            || (std::ptr::eq(data, base_operation) && data_result.ty != pointer_result.ty)
        {
            return Ok(None);
        }
        let direct_parameter = self
            .ledger
            .find(&self.parameters, |row| row.value.cmp(&slice))?
            .map(|i| self.parameters[i]);
        let parameter = match direct_parameter {
            Some(parameter) => parameter,
            None => {
                let Some(origin) = self.peel_slice_casts(slice)? else {
                    return Ok(None);
                };
                let Some(index) = self
                    .ledger
                    .find(&self.parameters, |row| row.value.cmp(&origin))?
                else {
                    return Ok(None);
                };
                self.parameters[index]
            }
        };
        let Type::Slice(slice_type) = parameter.ty else {
            return Ok(None);
        };
        let space_matches = slice_type.address_space == data_type.address_space
            || (data_type.address_space == AddressSpace::Generic
                && self.peel_slice_casts(data_slice)? == Some(parameter.value));
        if slice_type.element != pointer_type.pointee
            || !space_matches
            || slice_type.access != data_type.access
        {
            return Ok(None);
        }
        Ok(Some(Recipe {
            domain: FormalSliceBoundedDomainV1 {
                allocation: FormalAllocationIdentity {
                    parameter_index: parameter.ordinal,
                },
                slice: parameter.value,
                index,
                length,
                predicate: condition,
                selected_offset: offset,
                pointer,
                element_bytes,
                path: FormalGuardedPathV1::ExplicitPredicate,
            },
            address_space: if pointer_result.id == pointer {
                pointer_type.address_space
            } else {
                let Some(Type::Pointer(actual)) = self.runtime_type(pointer)? else {
                    return Ok(None);
                };
                actual.address_space
            },
        }))
    }

    pub(super) fn access(
        &mut self,
        location: FunctionOperationLocation,
        pointer: ValueId,
        kind: FormalMemoryAccessKind,
        access: MemoryAccess,
        invocations: InvocationRange1d,
        predicate: Option<ValueId>,
    ) -> Result<Option<FormalMemoryAccess>, AccessDerivationError> {
        let Some(index) = self
            .ledger
            .find(&self.recipes, |row| row.domain.pointer.cmp(&pointer))?
        else {
            return Ok(None);
        };
        self.ledger.charge(USE_WORK)?;
        let recipe = self.recipes[index];
        let mut domain = recipe.domain;
        let path_error = || FormalMemoryIncompleteReason::GuardedAccessPathUnavailable {
            location,
            predicate: recipe.domain.predicate,
        };
        if access.address_space != recipe.address_space || kind == FormalMemoryAccessKind::Atomic {
            return Err(path_error().into());
        }
        let Some(block) = self.control_row(location.block)? else {
            return Err(path_error().into());
        };
        let Some((start, end)) = block.interval else {
            return Err(path_error().into());
        };
        if predicate != Some(domain.predicate) {
            if predicate.is_some() {
                return Err(path_error().into());
            }
            let Some(index) = self
                .ledger
                .find(&self.truths, |row| row.predicate.cmp(&domain.predicate))?
            else {
                return Err(path_error().into());
            };
            let truth = self.truths[index];
            if truth.ambiguous || truth.interval.0 > start || end > truth.interval.1 {
                return Err(path_error().into());
            }
            domain.path = FormalGuardedPathV1::TrueEdge {
                source: truth.edge.source,
                ordinal: truth.edge.ordinal,
                target: truth.edge.target,
            };
        }
        Ok(Some(FormalMemoryAccess {
            location,
            allocation: domain.allocation,
            kind,
            address_space: if access.address_space == AddressSpace::Generic {
                let Some(Type::Slice(source)) = self.runtime_type(domain.slice)? else {
                    return Err(path_error().into());
                };
                source.address_space
            } else {
                access.address_space
            },
            byte_offset: ByteExpression::Affine {
                constant: 0,
                invocation_coefficient: domain.element_bytes,
            },
            byte_width: domain.element_bytes,
            alignment: u64::from(access.alignment),
            invocations,
            domain: FormalAccessDomainV1::SliceBounded(domain),
        }))
    }
    pub(super) fn bounds_work(&mut self) -> Result<(), ResourceError> {
        self.ledger.charge(BOUNDS_WORK)
    }
}

fn guard_pointer_leaf(ty: &Type) -> bool {
    matches!(ty, Type::Pointer(p) if matches!(p.pointee.as_ref(), Type::Scalar(_) | Type::StorageObject(_)))
}

fn guard_slice_leaf(ty: &Type) -> bool {
    matches!(ty, Type::Slice(s) if matches!(s.element.as_ref(), Type::Scalar(_) | Type::StorageObject(_)))
}

fn single_type(operation: &Operation, ty: &Type) -> bool {
    matches!(operation.results.as_slice(), [result] if &result.ty == ty)
}

pub(super) fn report_push<T>(
    guarded: &mut Option<GuardedAnalysisV1<'_>>,
    rows: &mut Vec<T>,
    value: T,
) -> Result<(), ResourceError> {
    if let Some(guarded) = guarded {
        guarded.ledger.push(rows, value)
    } else {
        rows.push(value);
        Ok(())
    }
}
pub(super) fn report_work(
    guarded: &mut Option<GuardedAnalysisV1<'_>>,
    amount: usize,
) -> Result<(), ResourceError> {
    if let Some(guarded) = guarded {
        guarded.ledger.charge(amount)?;
    }
    Ok(())
}

impl super::report_construction_v18::ReportMeterV18 for Option<GuardedAnalysisV1<'_>> {
    fn bounds_work(&mut self) -> Result<(), ResourceError> {
        if let Some(guarded) = self {
            guarded.bounds_work()?;
        }
        Ok(())
    }

    fn charge(&mut self, work: usize) -> Result<(), ResourceError> {
        report_work(self, work)
    }

    fn push<T>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), ResourceError> {
        report_push(self, rows, value)
    }

    fn sort<T>(
        &mut self,
        rows: &mut [T],
        compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
    ) -> Result<(), ResourceError> {
        if let Some(guarded) = self {
            guarded.ledger.sort(rows, 1, compare)
        } else {
            rows.sort_unstable_by(compare);
            Ok(())
        }
    }

    fn retire<T>(&mut self, rows: Vec<T>) -> Result<(), ResourceError> {
        drop(rows);
        Ok(())
    }
}

#[cfg(test)]
#[path = "guarded_access_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "guarded_entry_edge_v1_tests.rs"]
mod entry_edge_tests;
