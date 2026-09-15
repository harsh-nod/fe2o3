use super::*;

const COMPLETED_SLOTS: usize = 64;
type CompletedSlot = Option<(SemanticTypeIdV1, bool)>;
const SLOT_CELLS: usize =
    std::mem::size_of::<CompletedSlot>().div_ceil(std::mem::size_of::<usize>());

// Only completed contains results from this Shapes instance enter the front
// table. Collisions evict a verdict, never a traversal or cycle-check entry.
struct Completed<'a> {
    types: &'a [SemanticTypeDeclV1],
    slots: [CompletedSlot; COMPLETED_SLOTS],
    failed: bool,
}

impl<'a> Completed<'a> {
    fn new(
        types: &'a [SemanticTypeDeclV1],
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        // Fixed inline storage and initialization, including owner and failure
        // state. No allocation, resizing, relocation or allocator surplus.
        budget.charge(2 * std::mem::size_of::<Self>().div_ceil(std::mem::size_of::<usize>()))?;
        Ok(Self {
            types,
            slots: [None; COMPLETED_SLOTS],
            failed: false,
        })
    }
}

pub(super) struct Shapes<'a> {
    pub(super) types: &'a [SemanticTypeDeclV1],
    pairs: BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    barriers: BTreeSet<SemanticTypeIdV1>,
    cache: BTreeMap<SemanticTypeIdV1, Option<bool>>,
    completed: Completed<'a>,
}

impl<'a> Shapes<'a> {
    pub(super) fn new(
        types: &'a [SemanticTypeDeclV1],
        facts: &[GlobalBf16BorrowV1],
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let mut result = Self {
            completed: Completed::new(types, budget)?,
            types,
            pairs: BTreeMap::new(),
            barriers: BTreeSet::new(),
            cache: BTreeMap::new(),
        };
        for fact in facts {
            let pairs = fact.pairs();
            budget.charge(6 + map_work(result.pairs.len()) + map_work(result.barriers.len()))?;
            if result
                .pairs
                .insert(pairs[2].0, pairs[2].1)
                .is_some_and(|old| old != pairs[2].1)
            {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            // A matrix's Global field is consumed only by the existing exact
            // constructor matcher, never by ordinary carrier transparency.
            result.barriers.insert(pairs[0].1);
        }
        Ok(result)
    }

    pub(super) fn pointee(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<Option<SemanticTypeIdV1>, ProductionSemanticSsaErrorV1> {
        budget.charge(map_work(self.pairs.len()))?;
        Ok(self.pairs.get(&ty).copied())
    }

    pub(super) fn fields(&self, ty: SemanticTypeIdV1) -> Option<&'a [SemanticTypeIdV1]> {
        match self.types.get(ty.index() as usize)?.shape() {
            SemanticTypeShapeV1::Aggregate(aggregate) | SemanticTypeShapeV1::Tuple(aggregate) => {
                Some(aggregate.fields())
            }
            _ => None,
        }
    }

    pub(super) fn contains(
        &mut self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        let result = (|| {
            budget.charge(1)?;
            if self.completed.failed || !std::ptr::eq(self.types, self.completed.types) {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            budget.charge(SLOT_CELLS)?;
            let slot = ty.index() as usize % COMPLETED_SLOTS;
            if let Some((key, value)) = self.completed.slots[slot]
                && key == ty
            {
                return Ok(value);
            }
            let value = self.contains_cold(ty, budget)?;
            budget.charge(SLOT_CELLS)?;
            self.completed.slots[slot] = Some((ty, value));
            Ok(value)
        })();
        if result.is_err() {
            self.completed.failed = true;
        }
        result
    }

    // The original tree memo and exhaustive iterative traversal remain the
    // cold path, including pending-state cycles, barriers and malformed IDs.
    fn contains_cold(
        &mut self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        budget.charge(map_work(self.cache.len()))?;
        if let Some(value) = self.cache.get(&ty) {
            return value.ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        let mut pending = Vec::new();
        push(&mut pending, (ty, false), budget)?;
        while let Some((current, finish)) = pending.pop() {
            budget.charge(map_work(self.cache.len()))?;
            if finish {
                let mut selected = false;
                for field in self.fields(current).unwrap_or_default() {
                    budget.charge(map_work(self.cache.len()))?;
                    selected |= self
                        .cache
                        .get(field)
                        .copied()
                        .flatten()
                        .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
                }
                budget.charge(map_work(self.cache.len()))?;
                self.cache.insert(current, Some(selected));
                continue;
            }
            if let Some(value) = self.cache.get(&current) {
                value.ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
                continue;
            }
            budget.charge(
                4 + map_work(self.pairs.len())
                    + map_work(self.barriers.len())
                    + map_work(self.cache.len()),
            )?;
            if self.types.get(current.index() as usize).is_none() {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            if self.pairs.contains_key(&current) {
                self.cache.insert(current, Some(true));
            } else if self.barriers.contains(&current) || self.fields(current).is_none() {
                self.cache.insert(current, Some(false));
            } else {
                self.cache.insert(current, None);
                push(&mut pending, (current, true), budget)?;
                for &field in self.fields(current).unwrap().iter().rev() {
                    push(&mut pending, (field, false), budget)?;
                }
            }
        }
        budget.charge(map_work(self.cache.len()))?;
        self.cache
            .get(&ty)
            .copied()
            .flatten()
            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)
    }
}

#[cfg(test)]
mod tests;
