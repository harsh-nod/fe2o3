//! Source-stage adapters debit the original prepared ledger before allocation.

use super::*;
use std::mem::{size_of, size_of_val};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

pub(super) fn resource(error: Resource) -> ProductionRankedProjectionErrorV1 {
    canonical_source_facts_v18::source_error(error.into())
}

pub(super) fn add(left: usize, right: usize) -> Result<usize, ProductionRankedProjectionErrorV1> {
    left.checked_add(right).ok_or_else(|| resource(Resource::Arithmetic))
}

pub(super) fn product(count: usize, width: usize) -> Result<usize, ProductionRankedProjectionErrorV1> {
    count.checked_mul(width).ok_or_else(|| resource(Resource::Arithmetic))
}

pub(super) struct SourceAssertionMeterV18<'b, 'w>(pub(super) &'b mut Budget<'w>);

impl neutral_assertion::SemanticAssertionMeterV1 for SourceAssertionMeterV18<'_, '_> {
    type Error = ProductionRankedProjectionErrorV1;

    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.charge_work(amount).map_err(resource)
    }

    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.0.reserve_storage(bytes).map_err(resource)
    }
}

impl fe2o3_mir_model::SemanticU32InductionBoundSnapshotMeterV1
    for SourceAssertionMeterV18<'_, '_>
{
    type Error = ProductionRankedProjectionErrorV1;

    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.charge_work(amount).map_err(resource)
    }

    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.0.reserve_storage(bytes).map_err(resource)
    }
}

pub(super) enum ProjectionAllocationV18<'a> {
    Legacy,
    Source(&'a mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<Error = ProductionRankedProjectionErrorV1>),
}

impl fe2o3_mir_model::SemanticAssertionMeterV1 for ProjectionAllocationV18<'_> {
    type Error=ProductionRankedProjectionErrorV1;
    fn charge_work(&mut self,amount:usize)->Result<(),Self::Error>{ self.charge(amount) }
    fn reserve_storage(&mut self,bytes:usize)->Result<(),Self::Error>{
        match self {Self::Legacy=>Ok(()),Self::Source(meter)=>meter.reserve_storage(bytes)}
    }
}

impl fe2o3_mir_model::SemanticU32InductionBoundSnapshotMeterV1 for ProjectionAllocationV18<'_> {
    type Error=ProductionRankedProjectionErrorV1;
    fn charge_work(&mut self,amount:usize)->Result<(),Self::Error>{self.charge(amount)}
    fn reserve_storage(&mut self,bytes:usize)->Result<(),Self::Error>{
        fe2o3_mir_model::SemanticAssertionMeterV1::reserve_storage(self,bytes)
    }
}

impl<'a> ProjectionAllocationV18<'a> {
    pub(super) fn from_optional_facts(
        facts: &'a mut Option<&mut dyn ProjectedAssertionFactsV1>,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        match facts.as_deref_mut() {
            Some(facts) => Self::from_facts(facts),
            None => Ok(Self::Legacy),
        }
    }

    pub(super) fn from_multi(
        multi: &'a mut Option<&mut multi_entry_induction_v1::Context<'_, '_>>,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        match multi.as_deref_mut() {
            Some(context)=>Self::from_facts(context.facts),
            None=>Ok(Self::Legacy),
        }
    }

    pub(super) fn from_facts(
        facts: &'a mut dyn ProjectedAssertionFactsV1,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        match facts.projection_meter_v18() {
            None => Ok(Self::Legacy),
            Some(meter) => {
                meter.reserve_storage(size_of::<Self>())?;
                Ok(Self::Source(meter))
            }
        }
    }

    pub(super) fn charge(&mut self, amount: usize) -> Result<(), ProductionRankedProjectionErrorV1> {
        match self { Self::Legacy => Ok(()), Self::Source(meter) => meter.charge_work(amount) }
    }

    pub(super) fn header<T>(&mut self) -> Result<(), ProductionRankedProjectionErrorV1> {
        match self { Self::Legacy => Ok(()), Self::Source(meter) => meter.reserve_storage(size_of::<T>()) }
    }

    pub(super) fn capacity<T>(&mut self, count: usize) -> Result<Vec<T>, ProductionRankedProjectionErrorV1> {
        if matches!(self, Self::Legacy) { return Ok(Vec::with_capacity(count)); }
        self.header::<Vec<T>>()?;
        let mut rows = Vec::new();
        self.reserve(&mut rows, count, true, "ranked projection storage")?;
        Ok(rows)
    }

    pub(super) fn filled<T: Copy>(&mut self, count: usize, value: T) -> Result<Vec<T>, ProductionRankedProjectionErrorV1> {
        if matches!(self, Self::Legacy) { return Ok(vec![value; count]); }
        self.charge(count)?;
        let mut rows = self.capacity(count)?;
        rows.resize(count,value);
        Ok(rows)
    }

    pub(super) fn nested<T: Clone>(&mut self, count: usize) -> Result<Vec<Vec<T>>, ProductionRankedProjectionErrorV1> {
        if matches!(self,Self::Legacy) { return Ok(vec![Vec::new();count]); }
        self.charge(count)?;
        let mut rows=self.capacity(count)?;
        rows.resize_with(count,Vec::new);
        Ok(rows)
    }

    pub(super) fn optional<T:Clone>(&mut self,count:usize)->Result<Vec<Option<T>>,ProductionRankedProjectionErrorV1>{
        if matches!(self,Self::Legacy) {return Ok(vec![None;count]);}
        self.charge(count)?;
        let mut rows=self.capacity(count)?;
        rows.resize_with(count,||None);
        Ok(rows)
    }

    pub(super) fn consume_optional<T: Clone>(
        &mut self,
        slots: &mut [Option<T>],
        index: usize,
    ) -> Result<Option<T>, ProductionRankedProjectionErrorV1> {
        match self {
            Self::Legacy => Ok(slots.get(index).cloned().flatten()),
            Self::Source(_) => {
                self.charge(1)?;
                Ok(slots.get_mut(index).and_then(Option::take))
            }
        }
    }

    // Callers supply fixed-width structural keys, never owned names or trees.
    pub(super) fn sort_fixed_key<T, K: Ord>(
        &mut self, rows: &mut [T], key: impl Fn(&T) -> K,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if matches!(self, Self::Legacy) { rows.sort_unstable_by_key(key); return Ok(()); }
        self.header::<(usize, usize, usize, K, K)>()?;
        self.header::<Result<(), ProductionRankedProjectionErrorV1>>()?;
        if let Self::Source(meter) = self { meter.reserve_storage(size_of_val(&key))?; }
        fn sift<T, K: Ord>(allocation: &mut ProjectionAllocationV18<'_>, rows: &mut [T],
            key: &impl Fn(&T) -> K, mut parent: usize, end: usize,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            while parent < end / 2 {
                let mut child = parent * 2 + 1;
                if child + 1 < end {
                    allocation.charge(1)?;
                    if key(&rows[child]) < key(&rows[child + 1]) { child += 1; }
                }
                allocation.charge(1)?;
                if key(&rows[parent]) >= key(&rows[child]) { break; }
                allocation.charge(1)?;
                rows.swap(parent, child);
                #[cfg(test)]
                super::capability_refusal_phase_v18::mutated();
                parent = child;
            }
            Ok(())
        }
        let len = rows.len();
        for parent in (0..len / 2).rev() { sift(self, rows, &key, parent, len)?; }
        for end in (1..len).rev() {
            self.charge(1)?;
            rows.swap(0, end);
            #[cfg(test)]
            super::capability_refusal_phase_v18::mutated();
            sift(self, rows, &key, 0, end)?;
        }
        Ok(())
    }

    pub(super) fn find_fixed_key<T, K: Ord>(
        &mut self, rows: &[T], wanted: &K, key: impl Fn(&T) -> K,
    ) -> Result<Option<usize>, ProductionRankedProjectionErrorV1> {
        if matches!(self, Self::Legacy) { return Ok(rows.binary_search_by_key(wanted, key).ok()); }
        self.header::<(usize, usize, usize, K)>()?;
        self.header::<Result<Option<usize>, ProductionRankedProjectionErrorV1>>()?;
        if let Self::Source(meter) = self { meter.reserve_storage(size_of_val(&key))?; }
        let (mut first, mut end) = (0, rows.len());
        while first < end {
            let middle = first + (end - first) / 2;
            self.charge(1)?;
            match key(&rows[middle]).cmp(wanted) {
                std::cmp::Ordering::Less => first = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => return Ok(Some(middle)),
            }
        }
        Ok(None)
    }

    pub(super) fn one<T>(&mut self,value:T)->Result<Vec<T>,ProductionRankedProjectionErrorV1>{
        if matches!(self,Self::Legacy) {return Ok(vec![value]);}
        let mut rows=self.capacity(1)?;
        self.push(&mut rows,value)?;
        Ok(rows)
    }

    pub(super) fn empty<T>(&mut self) -> Result<Vec<T>, ProductionRankedProjectionErrorV1> {
        self.header::<Vec<T>>()?;
        Ok(Vec::new())
    }

    pub(super) fn queue<T>(
        &mut self, count: usize, legacy_error: &'static str,
    ) -> Result<VecDeque<T>, ProductionRankedProjectionErrorV1> {
        if matches!(self, Self::Legacy) {
            let mut rows = VecDeque::new();
            rows.try_reserve(count)
                .map_err(|_| ProductionRankedProjectionErrorV1::Unsupported(legacy_error))?;
            return Ok(rows);
        }
        self.header::<VecDeque<T>>()?;
        let Self::Source(meter) = self else { unreachable!() };
        meter.reserve_storage(product(count, size_of::<T>())?)?;
        let mut rows = VecDeque::new();
        rows.try_reserve_exact(count).map_err(|_| resource(Resource::Allocation))?;
        meter.reserve_storage(product(rows.capacity().checked_sub(count)
            .ok_or_else(|| resource(Resource::Accounting))?, size_of::<T>())?)?;
        Ok(rows)
    }

    pub(super) fn reserve<T>(
        &mut self,
        rows: &mut Vec<T>,
        additional: usize,
        exact: bool,
        legacy_error: &'static str,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        match self {
            Self::Legacy => {
                if exact { rows.try_reserve_exact(additional) } else { rows.try_reserve(additional) }
                    .map_err(|_| ProductionRankedProjectionErrorV1::Unsupported(legacy_error))
            }
            Self::Source(meter) => {
                let required = add(rows.len(),additional)?;
                if required <= rows.capacity() { return Ok(()); }
                // Dynamic appends retain amortized linear copying, while the
                // exact backing request is paid before the allocator runs.
                let count=if exact {required} else {
                    rows.capacity().checked_mul(2).unwrap_or(required).max(required)
                };
                meter.charge_work(rows.len())?;
                meter.reserve_storage(product(count,size_of::<T>())?)?;
                rows.try_reserve_exact(count.checked_sub(rows.len())
                    .ok_or_else(||resource(Resource::Accounting))?)
                    .map_err(|_| resource(Resource::Allocation))?;
                meter.reserve_storage(product(rows.capacity().checked_sub(count)
                    .ok_or_else(|| resource(Resource::Accounting))?,size_of::<T>())?)
            }
        }
    }

    pub(super) fn queue_push<T>(&mut self, rows: &mut VecDeque<T>, value: T)
        -> Result<(), ProductionRankedProjectionErrorV1>
    {
        if matches!(self, Self::Legacy) { rows.push_back(value); return Ok(()); }
        self.header::<Result<(), ProductionRankedProjectionErrorV1>>()?;
        self.charge(1)?;
        if rows.len() == rows.capacity() {
            let required = add(rows.len(), 1)?;
            let count = rows.capacity().checked_mul(2).unwrap_or(required).max(required);
            self.charge(rows.len())?;
            let Self::Source(meter) = self else { unreachable!() };
            meter.reserve_storage(product(count, size_of::<T>())?)?;
            rows.try_reserve_exact(count.checked_sub(rows.len())
                .ok_or_else(|| resource(Resource::Accounting))?)
                .map_err(|_| resource(Resource::Allocation))?;
            meter.reserve_storage(product(rows.capacity().checked_sub(count)
                .ok_or_else(|| resource(Resource::Accounting))?, size_of::<T>())?)?;
        }
        rows.push_back(value);
        Ok(())
    }

    pub(super) fn queue_pop<T>(&mut self, rows: &mut VecDeque<T>)
        -> Result<Option<T>, ProductionRankedProjectionErrorV1>
    {
        self.header::<Result<Option<T>, ProductionRankedProjectionErrorV1>>()?;
        if !rows.is_empty() { self.charge(1)?; }
        Ok(rows.pop_front())
    }

    pub(super) fn push<T>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), ProductionRankedProjectionErrorV1> {
        if !matches!(self,Self::Legacy) {
            self.charge(1)?;
            self.reserve(rows,1,false,"ranked projection storage")?;
        }
        rows.push(value);
        Ok(())
    }

    pub(super) fn copy_slice<T: Copy>(&mut self, source: &[T]) -> Result<Vec<T>, ProductionRankedProjectionErrorV1> {
        if matches!(self, Self::Legacy) { return Ok(source.to_vec()); }
        self.charge(source.len())?;
        let mut rows = self.capacity(source.len())?;
        rows.extend_from_slice(source);
        Ok(rows)
    }

    pub(super) fn boxed<T>(&mut self, rows: Vec<T>) -> Result<Box<[T]>, ProductionRankedProjectionErrorV1> {
        if matches!(self,Self::Legacy) || rows.capacity()==rows.len() { return Ok(rows.into_boxed_slice()); }
        let count=rows.len();
        let mut exact=self.capacity(count)?;
        if exact.capacity()!=count { return Err(resource(Resource::Accounting)); }
        self.charge(count)?;
        exact.extend(rows);
        Ok(exact.into_boxed_slice())
    }
}

// The containing analysis/source scope settles these credits only after every
// corresponding result has dropped. This allocator never authorizes a refund.
pub(super) fn rows<T>(
    count: usize,
    budget: &mut Budget<'_>,
) -> Result<Vec<T>, ProductionRankedProjectionErrorV1> {
    let bytes = add(std::mem::size_of::<Vec<T>>(), product(count, std::mem::size_of::<T>())?)?;
    budget.charge_work(count).map_err(resource)?;
    budget.reserve_storage(bytes).map_err(resource)?;
    let mut result = Vec::new();
    result.try_reserve_exact(count).map_err(|_| resource(Resource::Allocation))?;
    budget.reserve_storage(product(result.capacity().checked_sub(count)
        .ok_or_else(|| resource(Resource::Accounting))?, std::mem::size_of::<T>())?)
        .map_err(resource)?;
    Ok(result)
}

#[cfg(test)]
mod consuming_projection_tests_v18 {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn growing_fifo_pays_duplicate_pending_entries_copying_and_each_mutation() {
        let storage = size_of::<VecDeque<u32>>() + 7 * size_of::<u32>()
            + 3 * size_of::<Result<(), ProductionRankedProjectionErrorV1>>()
            + 4 * size_of::<Result<Option<u32>, ProductionRankedProjectionErrorV1>>();
        for (work, bytes) in [(9, storage), (8, storage), (9, storage - 1)] {
            let mut ledger = Work::new(work);
            let mut budget = Budget::new(&mut ledger, 17 + bytes);
            budget.reserve_storage(17).unwrap();
            let mut queue = ProjectionAllocationV18::Source(&mut SourceAssertionMeterV18(&mut budget))
                .queue(0, "unused legacy error").unwrap();
            let result = (|| {
                let mut meter = SourceAssertionMeterV18(&mut budget);
                let mut allocation = ProjectionAllocationV18::Source(&mut meter);
                for _ in 0..3 { allocation.queue_push(&mut queue, 7_u32)?; }
                assert_eq!(queue.capacity(), 4);
                for _ in 0..3 { assert_eq!(allocation.queue_pop(&mut queue)?, Some(7)); }
                assert_eq!(allocation.queue_pop(&mut queue)?, None);
                Ok::<(), ProductionRankedProjectionErrorV1>(())
            })();
            if work == 9 && bytes == storage {
                result.unwrap();
                assert!(queue.is_empty());
                assert_eq!((budget.work(), budget.storage()), (9, 17 + storage));
                drop(queue);
                budget.release_storage(storage).unwrap();
                assert_eq!(budget.storage(), 17);
            } else {
                assert!(result.is_err());
                if bytes < storage { assert_eq!(budget.failed_storage(), Some(17 + storage)); }
                else {
                    assert_eq!(queue.iter().copied().collect::<Vec<_>>(), [7]);
                    drop(budget);
                    assert_eq!(ledger.failed_work(), Some(9));
                }
            }
        }
    }

    struct NoSourceClone(Vec<u8>);
    impl Clone for NoSourceClone {
        fn clone(&self) -> Self { panic!("source projection cloned an owned descendant") }
    }

    #[test]
    fn consumed_source_slot_moves_its_backing_and_checks_work_before_mutation() {
        for limit in [0, 1] {
            let value = NoSourceClone(vec![1, 2, 3]);
            let pointer = value.0.as_ptr();
            let mut slots = [Some(value)];
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 17);
            budget.reserve_storage(17).unwrap();
            let result = ProjectionAllocationV18::Source(&mut SourceAssertionMeterV18(&mut budget))
                .consume_optional(&mut slots, 0);
            if limit == 0 {
                assert!(result.is_err());
                assert_eq!(slots[0].as_ref().unwrap().0.as_ptr(), pointer);
            } else {
                let value = result.unwrap().unwrap();
                assert_eq!(value.0.as_ptr(), pointer);
                assert!(slots[0].is_none());
            }
            assert_eq!((budget.storage(), budget.peak_storage()), (17, 17));
            drop(budget);
            assert_eq!(work.failed_work(), (limit == 0).then_some(1));
        }
    }

    #[test]
    fn legacy_optional_slot_keeps_its_original_clone_and_lookup_semantics() {
        let mut slots = [Some(vec![1, 2, 3]), None];
        let original = slots[0].as_ref().unwrap().as_ptr();
        let mut allocation = ProjectionAllocationV18::Legacy;
        let copy = allocation.consume_optional(&mut slots, 0).unwrap().unwrap();
        assert_eq!(copy, [1, 2, 3]);
        assert_ne!(copy.as_ptr(), original);
        assert_eq!(slots[0].as_ref().unwrap().as_ptr(), original);
        assert!(allocation.consume_optional(&mut slots, 1).unwrap().is_none());
        assert!(allocation.consume_optional(&mut slots, 2).unwrap().is_none());
    }

    #[test]
    fn fixed_key_sort_has_independent_two_element_oracles() {
        let storage = size_of::<(usize, usize, usize, u32, u32)>()
            + size_of::<Result<(), ProductionRankedProjectionErrorV1>>();
        // A two-element max-heap needs one comparison and one final swap.
        for (work_limit, storage_limit) in [(2, storage), (1, storage), (2, storage - 1)] {
            let mut rows = [2_u32, 1];
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, 7 + storage_limit);
            budget.reserve_storage(7).unwrap();
            let result = ProjectionAllocationV18::Source(&mut SourceAssertionMeterV18(&mut budget))
                .sort_fixed_key(&mut rows, |row| *row);
            if work_limit < 2 {
                assert!(result.is_err());
                assert_eq!(rows, [2, 1]);
                assert_eq!(budget.storage(), 7 + storage);
                drop(budget);
                assert_eq!(work.failed_work(), Some(2));
            } else if storage_limit < storage {
                assert!(result.is_err());
                assert_eq!(rows, [2, 1]);
                assert_eq!(budget.work(), 0);
                assert_eq!(budget.failed_storage(), Some(7 + storage));
            } else {
                result.unwrap();
                assert_eq!(rows, [1, 2]);
                assert_eq!((budget.work(), budget.storage()), (2, 7 + storage));
            }
        }
    }

    #[test]
    fn fixed_key_sort_and_search_preserve_sparse_and_duplicate_keys() {
        for mut rows in [vec![], vec![u64::MAX], vec![3, 1, u64::MAX, 0, 3, 7], vec![7, 6, 5, 4, 3, 2, 1]] {
            let mut expected = rows.clone();
            expected.sort_unstable();
            let mut work = Work::new(1000);
            let mut budget = Budget::new(&mut work, 100_000);
            let mut meter = SourceAssertionMeterV18(&mut budget);
            let mut allocation = ProjectionAllocationV18::Source(&mut meter);
            allocation.sort_fixed_key(&mut rows, |row| *row).unwrap();
            assert_eq!(rows, expected);
            for wanted in [0, 2, 3, 5, 9, u64::MAX] {
                let found = allocation.find_fixed_key(&rows, &wanted, |row| *row).unwrap();
                assert_eq!(found.map(|index| rows[index]), expected.binary_search(&wanted).ok().map(|index| expected[index]));
            }
        }
    }
}

pub(super) fn push<T>(
    rows: &mut Vec<T>,
    value: T,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if rows.len() == rows.capacity() { return Err(resource(Resource::Accounting)); }
    rows.push(value);
    Ok(())
}

pub(super) struct SourceBindingAllocationV18<'b, 'w> {
    budget: Option<&'b mut Budget<'w>>,
}

impl<'b, 'w> SourceBindingAllocationV18<'b, 'w> {
    pub(super) fn new(mut budget: Option<&'b mut Budget<'w>>) -> Result<Self, ProductionRankedProjectionErrorV1> {
        if let Some(budget) = budget.as_deref_mut() {
            budget.charge_work(size_of::<Self>()).map_err(resource)?;
            budget.reserve_storage(size_of::<Self>()).map_err(resource)?;
        }
        Ok(Self { budget })
    }

    pub(super) fn charge(&mut self, amount: usize) -> Result<(), ProductionRankedProjectionErrorV1> {
        match self.budget.as_deref_mut() {
            Some(budget) => budget.charge_work(amount).map_err(resource),
            None => Ok(()),
        }
    }

    pub(super) fn rows<T>(&mut self, count: usize) -> Result<Vec<T>, ProductionRankedProjectionErrorV1> {
        match self.budget.as_deref_mut() {
            Some(budget) => rows(count, budget),
            None => Ok(Vec::with_capacity(count)),
        }
    }

    pub(super) fn names(
        &mut self,
        left: &str,
        right: &str,
    ) -> Result<std::cmp::Ordering, ProductionRankedProjectionErrorV1> {
        self.charge(add(add(left.len(), right.len())?, 1)?)?;
        Ok(left.cmp(right))
    }

    pub(super) fn sort_roots(
        &mut self,
        roots: &mut [(&str, usize)],
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        // In-place heapsort: the fallible shared ledger must run before each
        // comparison, not inside an infallible sort closure or a second pass.
        fn sift(
            allocation: &mut SourceBindingAllocationV18<'_, '_>,
            roots: &mut [(&str, usize)],
            mut parent: usize,
            end: usize,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            while parent < end / 2 {
                let mut child = add(product(parent, 2)?, 1)?;
                if child + 1 < end && allocation.names(roots[child].0, roots[child + 1].0)?.is_lt() {
                    child += 1;
                }
                if !allocation.names(roots[parent].0, roots[child].0)?.is_lt() { break; }
                allocation.charge(1)?;
                roots.swap(parent, child);
                parent = child;
            }
            Ok(())
        }
        let length = roots.len();
        for parent in (0..length / 2).rev() { sift(self, roots, parent, length)?; }
        for end in (1..roots.len()).rev() {
            self.charge(1)?;
            roots.swap(0, end);
            sift(self, roots, 0, end)?;
        }
        Ok(())
    }

    pub(super) fn find_root(
        &mut self,
        roots: &[(&str, usize)],
        name: &str,
    ) -> Result<Option<usize>, ProductionRankedProjectionErrorV1> {
        let (mut first, mut end) = (0, roots.len());
        while first < end {
            let middle = first + (end - first) / 2;
            match self.names(roots[middle].0, name)? {
                std::cmp::Ordering::Less => first = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => return Ok(Some(roots[middle].1)),
            }
        }
        Ok(None)
    }

    pub(super) fn binding(
        &mut self,
        original: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1,
    ) -> Result<crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1, ProductionRankedProjectionErrorV1> {
        use crate::reference_effect_v1::{binding_clone_envelope_v18, BindingCloneEnvelopeErrorV18};
        let Some(budget) = self.budget.as_deref_mut() else { return Ok(original.clone()); };
        let bytes = binding_clone_envelope_v18(original, budget).map_err(|error| match error {
            BindingCloneEnvelopeErrorV18::Resource(error) => resource(error),
            BindingCloneEnvelopeErrorV18::Structure(error) =>
                ProductionRankedProjectionErrorV1::ReferenceEffectJoin(
                    crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2::Construction(error.to_string())),
        })?;
        budget.charge_work(bytes).map_err(resource)?;
        budget.reserve_storage(bytes).map_err(resource)?;
        let copied = original.clone();
        // All other variable-size members are Boxes. Account the actual two
        // String capacities, without assuming the allocator returned len bytes.
        let extra = add(copied.registration_path.capacity().checked_sub(copied.registration_path.len())
            .ok_or_else(|| resource(Resource::Accounting))?,
            copied.logical_kernel_name.capacity().checked_sub(copied.logical_kernel_name.len())
                .ok_or_else(|| resource(Resource::Accounting))?)?;
        budget.reserve_storage(extra).map_err(resource)?;
        Ok(copied)
    }

    pub(super) fn box_storage<T>(&mut self, count: usize) -> Result<(), ProductionRankedProjectionErrorV1> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.charge_work(count).map_err(resource)?;
            budget.reserve_storage(add(size_of::<Box<[T]>>(), product(count, size_of::<T>())?)?).map_err(resource)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod projection_allocation_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn dynamic_source_rows_use_amortized_growth_with_exact_resource_oracles() {
        let count=1024;
        let work=2*count-1;
        let bytes=size_of::<Vec<u64>>()+(2*count-1)*size_of::<u64>();
        for (work_limit,storage_limit) in [(work,bytes),(work-1,bytes),(work,bytes-1)] {
            let mut ledger=Work::new(work_limit);
            let mut budget=Budget::new(&mut ledger,storage_limit);
            let result=(|| {
                let mut meter=SourceAssertionMeterV18(&mut budget);
                let mut allocation=ProjectionAllocationV18::Source(&mut meter);
                let mut rows=allocation.empty()?;
                for index in 0..count {allocation.push(&mut rows,index as u64)?;}
                Ok::<_,ProductionRankedProjectionErrorV1>(rows)
            })();
            if (work_limit,storage_limit)==(work,bytes) {
                let rows=result.unwrap();
                assert_eq!(rows.len(),count);
                assert_eq!(rows.capacity(),count);
                assert!(rows.iter().enumerate().all(|(index,value)|*value==index as u64));
                assert_eq!((budget.work(),budget.storage()),(work,bytes));
            } else {
                assert!(result.is_err());
                if storage_limit<bytes {assert_eq!(budget.failed_storage(),Some(bytes));}
            }
        }
    }

    #[test]
    fn source_row_byte_overflow_refuses_before_allocator_or_resource_reservation() {
        let mut work=Work::new(0);
        let mut budget=Budget::new(&mut work,0);
        let mut rows=Vec::<u64>::new();
        let error=ProjectionAllocationV18::Source(&mut SourceAssertionMeterV18(&mut budget))
            .reserve(&mut rows,usize::MAX,true,"legacy").unwrap_err();
        assert!(matches!(error,ProductionRankedProjectionErrorV1::CanonicalAssertions(_)));
        assert_eq!((budget.work(),budget.storage(),budget.failed_storage()),(0,0,None));
        assert_eq!((rows.len(),rows.capacity()),(0,0));
    }
}
