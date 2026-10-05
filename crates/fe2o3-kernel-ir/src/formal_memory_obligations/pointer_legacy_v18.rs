use super::*;
use std::convert::Infallible;

pub(super) struct Legacy<'source, 'view> {
    pub(super) definitions: &'view Definitions<'source>,
    pub(super) value_types: &'view BTreeMap<ValueId, Type>,
    pub(super) allocation_by_value: &'view BTreeMap<ValueId, FormalAllocationIdentity>,
    pub(super) private_load_sources: &'view BTreeMap<ValueId, ValueId>,
    pub(super) cache: &'view mut PointerDerivationCache,
}
impl<'source> engine::State<'source> for Legacy<'source, '_> {
    type Error = Infallible;
    type Set = BTreeSet<ValueId>;
    fn step(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
    fn empty<T: Copy>(&mut self) -> Result<Vec<T>, Infallible> {
        Ok(Vec::new())
    }
    fn push<T: Copy>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), Infallible> {
        rows.push(value);
        Ok(())
    }
    fn sort<T: Copy>(
        &mut self,
        rows: &mut [T],
        key: impl Fn(&T) -> u32 + Copy,
    ) -> Result<(), Infallible> {
        rows.sort_by_key(key);
        Ok(())
    }
    fn find<T>(
        &mut self,
        rows: &[T],
        compare: impl Fn(&T) -> std::cmp::Ordering,
    ) -> Result<Option<usize>, Infallible> {
        let end = rows.partition_point(|row| compare(row) != std::cmp::Ordering::Greater);
        Ok(end
            .checked_sub(1)
            .filter(|index| compare(&rows[*index]) == std::cmp::Ordering::Equal))
    }
    fn set(&mut self, _: engine::SetRole) -> Result<Self::Set, Infallible> {
        Ok(BTreeSet::new())
    }
    fn insert(&mut self, set: &mut Self::Set, value: ValueId) -> Result<bool, Infallible> {
        Ok(set.insert(value))
    }
    fn remove(&mut self, set: &mut Self::Set, value: ValueId) -> Result<(), Infallible> {
        set.remove(&value);
        Ok(())
    }
    fn members(&mut self, set: &Self::Set) -> Result<Vec<ValueId>, Infallible> {
        Ok(set.iter().copied().collect())
    }
    fn phi_count(&mut self, value: ValueId) -> Result<Option<usize>, Infallible> {
        Ok(self
            .definitions
            .block_parameter_inputs
            .get(&value)
            .map(Vec::len))
    }
    fn phi_input(&mut self, value: ValueId, ordinal: usize) -> Result<ValueId, Infallible> {
        Ok(self.definitions.block_parameter_inputs[&value][ordinal])
    }
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>, Infallible> {
        Ok(self.definitions.unique_ssa_origin(value))
    }
    fn root(
        &mut self,
        value: ValueId,
        slices: bool,
    ) -> Result<Option<FormalAllocationIdentity>, Infallible> {
        Ok(self.allocation_by_value.get(&value).copied().filter(|_| {
            match self.value_types.get(&value) {
                Some(Type::Pointer(_)) => true,
                Some(Type::Slice(_)) => slices,
                _ => false,
            }
        }))
    }
    fn operation(
        &mut self,
        value: ValueId,
    ) -> Result<Option<(&'source Operation, FunctionOperationLocation)>, Infallible> {
        Ok(self.definitions.operations.get(&value).copied())
    }
    fn valid_cast(&mut self, op: &Operation, source: ValueId) -> Result<bool, Infallible> {
        Ok(self
            .value_types
            .get(&source)
            .and_then(|ty| checked_address_cast_source_v18(op, ty))
            == Some(source))
    }
    fn load(&mut self, value: ValueId) -> Result<Option<ValueId>, Infallible> {
        Ok(self.private_load_sources.get(&value).copied())
    }
    fn width(&mut self, value: ValueId) -> Result<Option<u64>, Infallible> {
        Ok(self.value_types.get(&value).and_then(pointer_byte_width))
    }
    fn affine(
        &mut self,
        value: ValueId,
    ) -> Result<Result<AffineExpression, IndexExpressionError>, Infallible> {
        Ok(derive_affine_index(value, self.definitions))
    }
    fn allocation(
        &mut self,
        value: ValueId,
    ) -> Result<Option<CachedPointerDerivation<FormalAllocationIdentity>>, Infallible> {
        Ok(self.cache.allocations.get(&value).copied())
    }
    fn cache_allocation(
        &mut self,
        value: ValueId,
        result: CachedPointerDerivation<FormalAllocationIdentity>,
        replace: bool,
    ) -> Result<(), Infallible> {
        if replace {
            self.cache.allocations.insert(value, result);
        } else {
            self.cache.allocations.entry(value).or_insert(result);
        }
        Ok(())
    }
    fn expression(
        &mut self,
        value: ValueId,
    ) -> Result<Option<CachedPointerDerivation<PointerExpression>>, Infallible> {
        Ok(self.cache.expressions.get(&value).copied())
    }
    fn cache_expression(
        &mut self,
        value: ValueId,
        result: CachedPointerDerivation<PointerExpression>,
    ) -> Result<(), Infallible> {
        self.cache.expressions.insert(value, result);
        Ok(())
    }
}
