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
const BOUNDS_WORK: usize = 16;
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
    fn find<T>(
        &mut self,
        rows: &[T],
        compare: impl FnMut(&T) -> std::cmp::Ordering,
    ) -> Result<Option<usize>, ResourceError> {
        verification_find_last_by_v1(rows, 1, &mut Budget::new(&mut self.work, 0), compare)
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
    incoming: Option<Edge>,
}
pub(super) struct GuardedControlV1 {
    ledger: GuardLedger,
    rows: Vec<ControlRow>,
    entry: BlockId,
}

impl GuardedControlV1 {
    pub(super) fn collect(
        function: &Function,
        flow: &IndexedControlFlow,
    ) -> Result<Option<Self>, ResourceError> {
        let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
        let mut ledger = GuardLedger::new(flow.work().total)?;
        ledger.charge(32)?;
        let mut selected = false;
        for block in &body.blocks {
            ledger.charge(2)?;
            for operation in &block.operations {
                ledger.charge(2)?;
                selected |= matches!(operation.kind, OperationKind::Select { .. })
                    || matches!(operation.kind, OperationKind::Load { access, .. }
                        if access.address_space == AddressSpace::Global && !access.volatile);
            }
        }
        if !selected {
            return Ok(None);
        }
        let headers = size_of::<GuardedAnalysisV1<'_>>()
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
        Ok(Some(Self {
            ledger,
            rows,
            entry: body.blocks[0].id,
        }))
    }
}

#[derive(Clone, Copy)]
struct DefinitionRow<'module> {
    value: ValueId,
    operation: &'module Operation,
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
#[derive(Clone, Copy)]
struct Recipe {
    domain: FormalSliceBoundedDomainV1,
    address_space: AddressSpace,
}

pub(super) struct GuardedAnalysisV1<'module> {
    pub(super) ledger: GuardLedger,
    control: Vec<ControlRow>,
    definitions: Vec<DefinitionRow<'module>>,
    parameters: Vec<ParameterRow<'module>>,
    truths: Vec<TrueRow>,
    recipes: Vec<Recipe>,
    runtime_reads: runtime_slice_read_v1::RuntimeReadState<'module>,
    rank_one: bool,
}

impl<'module> GuardedAnalysisV1<'module> {
    pub(super) fn new(
        seed: GuardedControlV1,
        definitions: &Definitions<'module>,
        function: &'module Function,
        rank_one: bool,
    ) -> Result<Self, ResourceError> {
        let GuardedControlV1 {
            ledger,
            rows,
            entry,
        } = seed;
        let mut result = Self {
            ledger,
            control: rows,
            definitions: Vec::new(),
            parameters: Vec::new(),
            truths: Vec::new(),
            recipes: Vec::new(),
            runtime_reads: runtime_slice_read_v1::RuntimeReadState::default(),
            rank_one,
        };
        result
            .ledger
            .reserve(&mut result.definitions, definitions.operations.len())?;
        for (value, (operation, _)) in &definitions.operations {
            result.ledger.charge(2)?;
            result.definitions.push(DefinitionRow {
                value: *value,
                operation,
            });
        }
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
                }) if cases.len() == 2 && cases[0].value == 0 && cases[1].value == 1 => {
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
                                Edge {
                                    source: block.id,
                                    ordinal: 1,
                                    target: cases[1].target,
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
            let Some(predicate_op) = result.definition(predicate)? else {
                continue;
            };
            if !single_type(predicate_op, &Type::BOOL) || edge.target == entry {
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
        result.collect_runtime_reads(definitions, function)?;
        result
            .ledger
            .reserve(&mut result.recipes, result.definitions.len())?;
        for index in 0..result.definitions.len() {
            result.ledger.charge(2)?;
            let row = result.definitions[index];
            if !matches!(row.operation.kind, OperationKind::GetElementPointer { .. }) {
                continue;
            }
            if let Some(recipe) = result.recipe(row.value, row.operation)? {
                result.recipes.push(recipe);
            }
        }
        // Definition order is ValueId order; filtering preserves the recipe index order.
        Ok(result)
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
        if pointer_result.id != pointer {
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
        let Some(data) = self.definition(base)? else {
            return Ok(None);
        };
        if !matches!(data.kind, OperationKind::SliceData { slice: actual } if actual == slice)
            || !single_type(data, &pointer_result.ty)
        {
            return Ok(None);
        }
        let Some(parameter) = self
            .ledger
            .find(&self.parameters, |row| row.value.cmp(&slice))?
            .map(|i| self.parameters[i])
        else {
            return Ok(None);
        };
        let Type::Slice(slice_type) = parameter.ty else {
            return Ok(None);
        };
        if slice_type.element != pointer_type.pointee
            || slice_type.address_space != pointer_type.address_space
            || slice_type.access != pointer_type.access
        {
            return Ok(None);
        }
        Ok(Some(Recipe {
            domain: FormalSliceBoundedDomainV1 {
                allocation: FormalAllocationIdentity {
                    parameter_index: parameter.ordinal,
                },
                slice,
                index,
                length,
                predicate: condition,
                selected_offset: offset,
                pointer,
                element_bytes,
                path: FormalGuardedPathV1::ExplicitPredicate,
            },
            address_space: pointer_type.address_space,
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
            address_space: access.address_space,
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

#[cfg(test)]
#[path = "guarded_access_v1_tests.rs"]
mod tests;
