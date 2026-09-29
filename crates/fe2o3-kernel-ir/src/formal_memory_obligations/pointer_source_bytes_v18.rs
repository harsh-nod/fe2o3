//! Lexical pointer extraction consuming the genuine retained private-slot owner.
use super::*;
use crate::formal_memory_obligations::pointer_derivation::{
    CachedPointerDerivation, PointerDerivationFailure, PointerExpression, engine,
};

#[path = "access_source_bytes_v18.rs"]
pub(in crate::formal_memory_obligations) mod accesses;

#[derive(Clone, Copy)]
struct PointerRow {
    value: ValueId,
    root: Option<FormalAllocationIdentity>,
    allocation: Option<CachedPointerDerivation<FormalAllocationIdentity>>,
    expression: Option<CachedPointerDerivation<PointerExpression>>,
    active: [usize; 4],
    touched: [usize; 4],
}

struct SparseSet {
    role: usize,
    epoch: usize,
    touched: Vec<ValueId>,
}

#[derive(Clone, Copy)]
enum QueryValue {
    Allocation(FormalAllocationIdentity),
    Expression(PointerExpression),
}

struct Query<'query, 'borrow, 'affine, 'owner, 'work, 'budget, 'budget_work> {
    pointers: &'query mut ActualOwnerPointersV18<'borrow, 'affine, 'owner, 'work>,
    budget: &'budget mut Budget<'budget_work>,
}

impl Query<'_, '_, '_, '_, '_, '_, '_> {
    fn position(&mut self, value: ValueId) -> Result<Option<usize>> {
        Ok(verification_find_last_by_v1(
            &self.pointers.rows,
            1,
            self.budget,
            |row| row.value.cmp(&value),
        )?)
    }
}
impl crate::formal_memory_obligations::exact_origin_v18::Compare
    for Query<'_, '_, '_, '_, '_, '_, '_>
{
    type Error = Failure;
    fn equal(&mut self, left: &Type, right: &Type) -> Result<bool> {
        Ok(verification_types_equal_v1(left, right, self.budget)?)
    }
    fn step(&mut self) -> Result<()> {
        Ok(self.budget.charge_work(1)?)
    }
}
impl<'owner> engine::State<'owner> for Query<'_, '_, '_, 'owner, '_, '_, '_> {
    type Error = Failure;
    type Set = SparseSet;
    fn step(&mut self, work: usize) -> Result<()> {
        Ok(self.budget.charge_work(work)?)
    }
    fn empty<T: Copy>(&mut self) -> Result<Vec<T>> {
        Ok(allocate_vector_v2(0, self.budget)?)
    }
    fn push<T: Copy>(&mut self, rows: &mut Vec<T>, value: T) -> Result<()> {
        Ok(
            meter::LiveGuardMeter::new(self.budget, usize::MAX, usize::MAX, usize::MAX)
                .push(rows, value)?,
        )
    }
    fn sort<T: Copy>(&mut self, rows: &mut [T], key: impl Fn(&T) -> u32 + Copy) -> Result<()> {
        Ok(verification_radix_sort_u32_bytes_v2(
            rows,
            self.budget,
            key,
        )?)
    }
    fn find<T>(
        &mut self,
        rows: &[T],
        compare: impl Fn(&T) -> std::cmp::Ordering,
    ) -> Result<Option<usize>> {
        Ok(verification_find_last_by_v1(rows, 1, self.budget, compare)?)
    }
    fn set(&mut self, role: engine::SetRole) -> Result<Self::Set> {
        self.budget.charge_work(1)?;
        let epoch = self.pointers.next_epoch;
        self.pointers.next_epoch = epoch.checked_add(1).ok_or(ResourceError::Arithmetic)?;
        self.budget
            .reserve_storage(size_of::<SparseSet>() - size_of::<Vec<ValueId>>())?;
        // The retained dense row marks are initialized once, not per query.
        // Each original engine role has at most one simultaneously live set.
        Ok(SparseSet {
            role: role as usize,
            epoch,
            touched: allocate_vector_v2(0, self.budget)?,
        })
    }
    fn insert(&mut self, set: &mut Self::Set, value: ValueId) -> Result<bool> {
        let index = self.position(value)?.ok_or(ResourceError::Accounting)?;
        self.budget.charge_work(1)?;
        if self.pointers.rows[index].active[set.role] == set.epoch {
            return Ok(false);
        }
        if self.pointers.rows[index].touched[set.role] != set.epoch {
            meter::LiveGuardMeter::new(self.budget, usize::MAX, usize::MAX, usize::MAX)
                .push(&mut set.touched, value)?;
            self.pointers.rows[index].touched[set.role] = set.epoch;
        }
        self.pointers.rows[index].active[set.role] = set.epoch;
        Ok(true)
    }
    fn remove(&mut self, set: &mut Self::Set, value: ValueId) -> Result<()> {
        let index = self.position(value)?.ok_or(ResourceError::Accounting)?;
        self.budget.charge_work(1)?;
        self.pointers.rows[index].active[set.role] = 0;
        Ok(())
    }
    fn members(&mut self, set: &Self::Set) -> Result<Vec<ValueId>> {
        let mut result = allocate_vector_v2(set.touched.len(), self.budget)?;
        for &value in &set.touched {
            let index = self.position(value)?.ok_or(ResourceError::Accounting)?;
            self.budget.charge_work(1)?;
            if self.pointers.rows[index].active[set.role] == set.epoch {
                self.budget.charge_work(1)?;
                result.push(value);
            }
        }
        verification_radix_sort_u32_bytes_v2(&mut result, self.budget, |value| value.0)?;
        Ok(result)
    }
    fn phi_count(&mut self, value: ValueId) -> Result<Option<usize>> {
        let affine = &mut self.pointers.slots.affine;
        affine
            .context
            .phi_input_count(affine.source, value, self.budget)
    }
    fn phi_input(&mut self, value: ValueId, ordinal: usize) -> Result<ValueId> {
        let affine = &mut self.pointers.slots.affine;
        affine
            .context
            .phi_input(affine.source, value, ordinal, self.budget)?
            .ok_or_else(|| ResourceError::Accounting.into())
    }
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>> {
        let affine = &mut self.pointers.slots.affine;
        affine
            .context
            .unique_origin(affine.source, value, self.budget)
    }
    fn root(&mut self, value: ValueId, slices: bool) -> Result<Option<FormalAllocationIdentity>> {
        let root = self
            .position(value)?
            .and_then(|index| self.pointers.rows[index].root);
        let affine = &mut self.pointers.slots.affine;
        let ty = affine
            .context
            .value_type(affine.source, value, self.budget)?;
        Ok(root.filter(|_| match ty {
            Some(Type::Pointer(_)) => true,
            Some(Type::Slice(_)) => slices,
            _ => false,
        }))
    }
    fn operation(
        &mut self,
        value: ValueId,
    ) -> Result<Option<(&'owner Operation, FunctionOperationLocation)>> {
        let affine = &mut self.pointers.slots.affine;
        affine.context.operation(affine.source, value, self.budget)
    }
    fn valid_cast(&mut self, operation: &Operation, source: ValueId) -> Result<bool> {
        let affine = &mut self.pointers.slots.affine;
        let Some(ty) = affine
            .context
            .value_type(affine.source, source, self.budget)?
        else {
            return Ok(false);
        };
        Ok(
            crate::formal_memory_obligations::exact_origin_v18::address(operation, ty, self)?
                == Some(source),
        )
    }
    fn load(&mut self, value: ValueId) -> Result<Option<ValueId>> {
        let slots = &mut self.pointers.slots;
        slots.load_source(slots.affine.owner, slots.root_index, value, self.budget)
    }
    fn width(&mut self, value: ValueId) -> Result<Option<u64>> {
        let affine = &mut self.pointers.slots.affine;
        Ok(affine
            .context
            .value_type(affine.source, value, self.budget)?
            .and_then(pointer_byte_width))
    }
    fn affine(
        &mut self,
        value: ValueId,
    ) -> Result<crate::formal_memory_obligations::affine_engine_v2::Expression> {
        let slots = &mut self.pointers.slots;
        slots
            .affine
            .expression(slots.affine.owner, slots.root_index, value, self.budget)
    }
    fn allocation(
        &mut self,
        value: ValueId,
    ) -> Result<Option<CachedPointerDerivation<FormalAllocationIdentity>>> {
        Ok(self
            .position(value)?
            .and_then(|index| self.pointers.rows[index].allocation))
    }
    fn cache_allocation(
        &mut self,
        value: ValueId,
        result: CachedPointerDerivation<FormalAllocationIdentity>,
        replace: bool,
    ) -> Result<()> {
        let index = self.position(value)?.ok_or(ResourceError::Accounting)?;
        self.budget.charge_work(1)?;
        if replace || self.pointers.rows[index].allocation.is_none() {
            self.pointers.rows[index].allocation = Some(result);
        }
        Ok(())
    }
    fn expression(
        &mut self,
        value: ValueId,
    ) -> Result<Option<CachedPointerDerivation<PointerExpression>>> {
        Ok(self
            .position(value)?
            .and_then(|index| self.pointers.rows[index].expression))
    }
    fn cache_expression(
        &mut self,
        value: ValueId,
        result: CachedPointerDerivation<PointerExpression>,
    ) -> Result<()> {
        let index = self.position(value)?.ok_or(ResourceError::Accounting)?;
        self.budget.charge_work(1)?;
        self.pointers.rows[index].expression = Some(result);
        Ok(())
    }
}

#[must_use = "dropping pointer extraction without release retains its resource charge"]
pub(in crate::formal_memory_obligations) struct ActualOwnerPointersV18<
    'borrow,
    'affine,
    'owner,
    'work,
> {
    slots: &'borrow mut ActualOwnerPrivateSlotsV18<'affine, 'owner, 'work>,
    rows: Vec<PointerRow>,
    floor: usize,
    retained: usize,
    next_epoch: usize,
}

impl<'affine, 'owner, 'work> ActualOwnerPrivateSlotsV18<'affine, 'owner, 'work> {
    pub(in crate::formal_memory_obligations) fn pointers<'borrow>(
        &'borrow mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        budget: &mut Budget<'_>,
    ) -> Result<ActualOwnerPointersV18<'borrow, 'affine, 'owner, 'work>> {
        self.check(owner, root_index, budget)?;
        let floor = budget.storage();
        if let Err(error) = budget.reserve_storage(frame_bytes_v18()) {
            return self.affine.keep(Err(error.into()));
        }
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<_> {
            budget.charge_work(1)?;
            let definitions = self
                .affine
                .context
                .definition_rows_v2(self.affine.source, budget)?;
            let mut rows = allocate_vector_v2(definitions.len(), budget)?;
            budget.charge_work(definitions.len())?;
            rows.extend(definitions.iter().map(|row| PointerRow {
                value: ValueId(row.key),
                root: None,
                allocation: None,
                expression: None,
                active: [0; 4],
                touched: [0; 4],
            }));
            let source = self.affine.source;
            let body = source.body.as_ref().ok_or(ResourceError::Accounting)?;
            for (ordinal, (value, ty)) in body
                .parameters
                .iter()
                .zip(&source.signature.parameters)
                .enumerate()
            {
                budget.charge_work(1)?;
                if let Some(parameter) = formal_allocation_parameter(ordinal, *value, ty) {
                    let index =
                        verification_find_last_by_v1(&rows, 1, budget, |row| row.value.cmp(value))?
                            .ok_or(ResourceError::Accounting)?;
                    budget.charge_work(1)?;
                    rows[index].root = Some(parameter.identity);
                }
            }
            let retained = vector_bytes_v2(&rows)?
                .checked_add(frame_bytes_v18())
                .ok_or(ResourceError::Arithmetic)?;
            Ok((rows, retained))
        }));
        match result {
            Ok(Ok((rows, retained))) => Ok(ActualOwnerPointersV18 {
                slots: self,
                rows,
                floor,
                retained,
                next_epoch: 1,
            }),
            Ok(Err(error)) => {
                budget.rollback_storage(floor)?;
                self.affine.keep(Err(error))
            }
            Err(payload) => {
                budget.rollback_storage(floor)?;
                resume_unwind(payload)
            }
        }
    }
}

impl ActualOwnerPointersV18<'_, '_, '_, '_> {
    fn check(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        budget: &Budget<'_>,
    ) -> Result<()> {
        self.slots.check(owner, root_index, budget)?;
        let result = if budget.storage()
            < self
                .floor
                .checked_add(self.retained)
                .ok_or(ResourceError::Arithmetic)?
        {
            Err(ResourceError::Accounting.into())
        } else {
            Ok(())
        };
        self.slots.affine.keep(result)
    }
    fn query(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        value: ValueId,
        budget: &mut Budget<'_>,
        allocation_only: bool,
    ) -> Result<CachedPointerDerivation<QueryValue>> {
        self.check(owner, root_index, budget)?;
        let floor = budget.storage();
        if let Err(error) = budget.reserve_storage(query_frame_v18()) {
            return self.slots.affine.keep(Err(error.into()));
        }
        let query_owner = &mut *self;
        let query_budget = &mut *budget;
        let result = catch_unwind(AssertUnwindSafe(move || -> Result<_> {
            let budget = query_budget;
            budget.charge_work(1)?;
            let mut query = Query {
                pointers: query_owner,
                budget,
            };
            if query.position(value)?.is_none() {
                return Ok(Err(PointerDerivationFailure::AtAccess(value)));
            }
            if allocation_only {
                Ok(engine::allocation(value, &mut query)?.map(QueryValue::Allocation))
            } else {
                Ok(engine::expression(value, &mut query)?.map(QueryValue::Expression))
            }
        }));
        // All query-owned typed vectors and algorithm stacks have retired.
        budget.rollback_storage(floor)?;
        match result {
            Ok(result) => self.slots.affine.keep(result),
            Err(payload) => resume_unwind(payload),
        }
    }
    pub(in crate::formal_memory_obligations) fn allocation(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<CachedPointerDerivation<FormalAllocationIdentity>> {
        self.query(owner, root_index, value, budget, true)
            .map(|result| {
                result.map(|value| match value {
                    QueryValue::Allocation(value) => value,
                    QueryValue::Expression(_) => unreachable!(),
                })
            })
    }
    pub(in crate::formal_memory_obligations) fn expression(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<CachedPointerDerivation<PointerExpression>> {
        self.query(owner, root_index, value, budget, false)
            .map(|result| {
                result.map(|value| match value {
                    QueryValue::Expression(value) => value,
                    QueryValue::Allocation(_) => unreachable!(),
                })
            })
    }
    pub(in crate::formal_memory_obligations) fn release(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.slots.affine.identity(budget)?;
        if budget.storage()
            < self
                .floor
                .checked_add(self.retained)
                .ok_or(ResourceError::Arithmetic)?
        {
            return Err(ResourceError::Accounting.into());
        }
        let retained = self.retained;
        drop(self);
        Ok(budget.release_storage(retained)?)
    }
}

fn frame_bytes_v18() -> usize {
    size_of::<ActualOwnerPointersV18<'static, 'static, 'static, 'static>>()
        + size_of::<Result<ActualOwnerPointersV18<'static, 'static, 'static, 'static>>>()
        + size_of::<std::thread::Result<Result<(Vec<PointerRow>, usize)>>>()
        + size_of::<(
            &mut ActualOwnerPrivateSlotsV18<'static, 'static, 'static>,
            &mut Budget<'static>,
        )>()
}
fn query_frame_v18() -> usize {
    size_of::<Query<'static, 'static, 'static, 'static, 'static, 'static, 'static>>()
        + size_of::<meter::LiveGuardMeter<'static, 'static>>()
        + size_of::<std::thread::Result<Result<CachedPointerDerivation<QueryValue>>>>()
        + size_of::<Result<CachedPointerDerivation<QueryValue>>>()
        + size_of::<(
            &mut ActualOwnerPointersV18<'static, 'static, 'static, 'static>,
            ValueId,
            &mut Budget<'static>,
            bool,
        )>()
}

#[cfg(test)]
#[path = "pointer_source_bytes_v18_tests.rs"]
mod tests;
