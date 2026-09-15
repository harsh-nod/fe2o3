//! Dense type-coordinate memo; source traversal and fact rules are unchanged.
use super::*;

const TYPES_PER_WORD: usize = 32;
const WORD_CELLS: usize = std::mem::size_of::<u64>().div_ceil(std::mem::size_of::<usize>());

struct Memo<'a> {
    types: &'a [SemanticTypeDeclV1],
    words: Vec<u64>,
    failed: bool,
}

impl<'a> Memo<'a> {
    fn new(
        types: &'a [SemanticTypeDeclV1],
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        budget.charge(std::mem::size_of::<Self>().div_ceil(std::mem::size_of::<usize>()))?;
        let mut words = Vec::new();
        for _ in 0..types.len().div_ceil(TYPES_PER_WORD) {
            // The existing helper accounts retained capacity, initialization,
            // simultaneous old/new storage, relocation and allocator surplus.
            push(&mut words, 0u64, budget)?;
        }
        Ok(Self {
            types,
            words,
            failed: false,
        })
    }

    fn begin(
        &self,
        types: &[SemanticTypeDeclV1],
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        budget.charge(1)?;
        if self.failed || !std::ptr::eq(types, self.types) {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        Ok(())
    }

    // 00 unseen, 01 pending, 10 completed false, 11 completed true.
    // Nested Option deliberately matches the original tree memo's four states.
    fn get(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<Option<Option<bool>>, ProductionSemanticSsaErrorV1> {
        budget.charge(WORD_CELLS)?;
        let index = ty.index() as usize;
        if index >= self.types.len() {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        let word = self
            .words
            .get(index / TYPES_PER_WORD)
            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
        Ok(match (word >> (2 * (index % TYPES_PER_WORD))) & 3 {
            0 => None,
            1 => Some(None),
            2 => Some(Some(false)),
            _ => Some(Some(true)),
        })
    }

    fn insert(
        &mut self,
        ty: SemanticTypeIdV1,
        value: Option<bool>,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        // Read-modify-write preserves all neighboring type coordinates.
        budget.charge(2 * WORD_CELLS)?;
        let index = ty.index() as usize;
        if index >= self.types.len() {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        let word = self
            .words
            .get_mut(index / TYPES_PER_WORD)
            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
        let shift = 2 * (index % TYPES_PER_WORD);
        let state = match value {
            None => 1,
            Some(false) => 2,
            Some(true) => 3,
        };
        *word = (*word & !(3u64 << shift)) | (state << shift);
        Ok(())
    }
}

pub(super) struct Shapes<'a> {
    pub(super) types: &'a [SemanticTypeDeclV1],
    pairs: BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    barriers: BTreeSet<SemanticTypeIdV1>,
    memo: Memo<'a>,
    shared_memo: Memo<'a>,
}

impl<'a> Shapes<'a> {
    pub(super) fn new(
        types: &'a [SemanticTypeDeclV1],
        facts: &[GlobalBf16BorrowV1],
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let mut result = Self {
            memo: Memo::new(types, budget)?,
            shared_memo: Memo::new(types, budget)?,
            types,
            pairs: BTreeMap::new(),
            barriers: BTreeSet::new(),
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

    #[cfg(test)]
    pub(super) fn contains(
        &mut self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        let result = self
            .memo
            .begin(self.types, budget)
            .and_then(|()| self.contains_indexed(ty, budget));
        if result.is_err() {
            self.memo.failed = true;
        }
        result
    }

    #[cfg(test)]
    pub(super) fn for_each_declaration(
        &mut self,
        declarations: &[SemanticLocalDeclV1],
        budget: &mut Budget,
        visit: impl FnMut(
            usize,
            &SemanticLocalDeclV1,
            bool,
            &mut Budget,
        ) -> Result<(), ProductionSemanticSsaErrorV1>,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.for_each_selected_declaration(declarations, false, budget, visit)
    }

    pub(super) fn for_each_transport_declaration(
        &mut self,
        declarations: &[SemanticLocalDeclV1],
        budget: &mut Budget,
        visit: impl FnMut(
            usize,
            &SemanticLocalDeclV1,
            bool,
            &mut Budget,
        ) -> Result<(), ProductionSemanticSsaErrorV1>,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.for_each_selected_declaration(declarations, true, budget, visit)
    }

    fn for_each_selected_declaration(
        &mut self,
        declarations: &[SemanticLocalDeclV1],
        shared_carriers: bool,
        budget: &mut Budget,
        mut visit: impl FnMut(
            usize,
            &SemanticLocalDeclV1,
            bool,
            &mut Budget,
        ) -> Result<(), ProductionSemanticSsaErrorV1>,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        let result = (|| {
            // Fixed batch entry; no retained handle, census or allocation.
            budget.charge(1)?;
            self.memo.begin(self.types, budget)?;
            // This call owns the full loop and the exclusive Shapes borrow.
            // The callback cannot change the owner or resume after an error.
            for (local, declaration) in declarations.iter().enumerate() {
                budget.charge(1)?;
                let selected = self.contains_indexed(declaration.ty(), budget)?
                    || (shared_carriers
                        && self
                            .shared_carrier_indexed(declaration.ty(), budget)?
                            .is_some());
                visit(local, declaration, selected, budget)?;
            }
            Ok(())
        })();
        if result.is_err() {
            self.memo.failed = true;
        }
        result
    }

    pub(super) fn transport_contains(
        &mut self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        let result = (|| {
            self.memo.begin(self.types, budget)?;
            Ok(self.contains_indexed(ty, budget)?
                || self.shared_carrier_indexed(ty, budget)?.is_some())
        })();
        if result.is_err() {
            self.memo.failed = true;
        }
        result
    }

    pub(super) fn shared_carrier_pointee(
        &mut self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<Option<SemanticTypeIdV1>, ProductionSemanticSsaErrorV1> {
        let result = self
            .memo
            .begin(self.types, budget)
            .and_then(|()| self.shared_carrier_indexed(ty, budget));
        if result.is_err() {
            self.memo.failed = true;
        }
        result
    }

    fn shared_carrier_indexed(
        &mut self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<Option<SemanticTypeIdV1>, ProductionSemanticSsaErrorV1> {
        budget.charge(1)?;
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = self
            .types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Ok(None);
        };
        match self.shared_memo.get(ty, budget)? {
            Some(Some(false)) => return Ok(None),
            Some(Some(true)) => {
                budget.charge(1)?;
                return Ok(Some(pointer.pointee()));
            }
            Some(None) => return Err(ProductionSemanticSsaErrorV1::ReplayMismatch),
            None => {}
        }
        // Shared eligibility never enters the raw by-value memo: otherwise a
        // warmed wrapper could turn a later aggregate query into pointer traversal.
        let result = self.shared_carrier_uncached(ty, budget)?;
        self.shared_memo
            .insert(ty, Some(result.is_some()), budget)?;
        Ok(result)
    }

    fn shared_carrier_uncached(
        &mut self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<Option<SemanticTypeIdV1>, ProductionSemanticSsaErrorV1> {
        budget.charge(13)?;
        let Some(pointee) = super::super::matrix_access_borrow::shared_pointee(self.types, ty)
        else {
            return Ok(None);
        };
        let layout = self.types[ty.index() as usize].layout();
        if layout.size_bytes() != Some(8)
            || layout.alignment_bytes() != 8
            || layout.is_uninhabited()
            || !matches!(
                layout.details(),
                fe2o3_mir_model::semantic_mir_v1::SemanticTypeLayoutDetailsV1::None
            )
        {
            return Ok(None);
        }
        if self.fields(pointee).is_none() {
            return Ok(None);
        }
        // The raw by-value query does not follow pointers. This admits exactly
        // one shared edge, never a recursive pointer chain or a matrix wrapper.
        Ok(self.contains_indexed(pointee, budget)?.then_some(pointee))
    }

    fn contains_indexed(
        &mut self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        if let Some(value) = self.memo.get(ty, budget)? {
            return value.ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        let mut pending = Vec::new();
        push(&mut pending, (ty, false), budget)?;
        while let Some((current, finish)) = pending.pop() {
            let value = self.memo.get(current, budget)?;
            if finish {
                let mut selected = false;
                for field in self.fields(current).unwrap_or_default() {
                    selected |= self
                        .memo
                        .get(*field, budget)?
                        .flatten()
                        .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
                }
                self.memo.insert(current, Some(selected), budget)?;
                continue;
            }
            if let Some(value) = value {
                value.ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
                continue;
            }
            budget.charge(4 + map_work(self.pairs.len()) + map_work(self.barriers.len()))?;
            if self.types.get(current.index() as usize).is_none() {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            if self.pairs.contains_key(&current) {
                self.memo.insert(current, Some(true), budget)?;
            } else if self.barriers.contains(&current) || self.fields(current).is_none() {
                self.memo.insert(current, Some(false), budget)?;
            } else {
                self.memo.insert(current, None, budget)?;
                push(&mut pending, (current, true), budget)?;
                for &field in self.fields(current).unwrap().iter().rev() {
                    push(&mut pending, (field, false), budget)?;
                }
            }
        }
        self.memo
            .get(ty, budget)?
            .flatten()
            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)
    }
}

#[cfg(test)]
mod tests;
