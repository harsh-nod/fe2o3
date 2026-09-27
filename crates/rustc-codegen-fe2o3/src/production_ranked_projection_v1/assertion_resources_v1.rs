//! Private strict assertion helpers. No analyzer, source view or admission.
//! Accepted credits remain with the enclosing original-resource owner; no refund.
use super::bf16_nominal_preparation_resources_v1::{PreparationResourcesV1, resource};
use super::{
    MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1, ProductionRankedProjectionErrorV1 as Error,
    charge_statement_scan_equivalent_v1, insert_assertion_proof_cache_with_limit,
    project_loop_graph_charge_v1, same_semantic_operand_value_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCheckedBinaryRvalueV1, SemanticConstantValueV1, SemanticOperandV1, SemanticPlaceV1,
    SemanticProjectionV1,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::marker::PhantomData;
use std::mem::size_of;

type Result<T> = std::result::Result<T, Error>;

trait Meter {
    fn work(&mut self, amount: usize) -> Result<()>;
    fn storage(&mut self, bytes: usize) -> Result<()>;
    fn denied(&self) -> bool;
    fn original_ledger(&self) -> Option<(usize, CanonicalKernelIrWorkLedgerIdentityV1)>;
}
impl Meter for PreparationResourcesV1<'_, '_> {
    fn work(&mut self, amount: usize) -> Result<()> {
        self.work(amount)
    }
    fn storage(&mut self, bytes: usize) -> Result<()> {
        self.reserve_storage(bytes)
    }
    fn denied(&self) -> bool {
        self.has_denial()
    }
    fn original_ledger(&self) -> Option<(usize, CanonicalKernelIrWorkLedgerIdentityV1)> {
        self.original_ledger_v1()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LegacyReserve {
    Exact,
    Amortized,
}

/// Lexical custody only: addresses are not globally unique capabilities.
/// Containers retain 'a, the ORIGINAL exclusive adapter-borrow lifetime, even
/// after this wrapper drops. They cannot be used across adapter reconstruction.
/// Their invariant lifetime marker also prevents retagging by lifetime coercion.
#[derive(Clone, Copy, Eq, PartialEq)]
struct Owner {
    adapter_slot: usize,
    budget_slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
}
pub(super) struct AssertionResourcesV1<'a> {
    meter: Option<&'a mut dyn Meter>,
    poisoned: bool,
    owner: Option<Owner>,
    lifetime: PhantomData<fn(&'a ()) -> &'a ()>,
}
impl<'a> AssertionResourcesV1<'a> {
    pub(super) fn legacy() -> Self {
        Self {
            meter: None,
            poisoned: false,
            owner: None,
            lifetime: PhantomData,
        }
    }
    pub(super) fn strict(resources: &'a mut PreparationResourcesV1<'_, '_>) -> Result<Self> {
        if !resources.is_metered() || resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        let (budget_slot, ledger) = resources
            .original_ledger_v1()
            .ok_or_else(|| resource(Resource::Accounting))?;
        let owner = Owner {
            adapter_slot: resources as *const PreparationResourcesV1<'_, '_> as usize,
            budget_slot,
            ledger,
        };
        let bytes = 4096usize
            .checked_add(size_of::<Self>())
            .and_then(|n| {
                size_of::<Result<Self>>()
                    .checked_mul(2)
                    .and_then(|x| n.checked_add(x))
            })
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        resources.work(8)?;
        resources.reserve_storage(bytes)?;
        Ok(Self {
            meter: Some(resources),
            poisoned: false,
            owner: Some(owner),
            lifetime: PhantomData,
        })
    }
    pub(super) fn is_strict(&self) -> bool {
        self.meter.is_some()
    }
    pub(super) fn is_denied(&self) -> bool {
        self.poisoned || self.meter.as_ref().is_some_and(|meter| meter.denied())
    }
    fn available(&self) -> Result<()> {
        if self.is_strict()
            && (self.is_denied()
                || self
                    .meter
                    .as_ref()
                    .and_then(|meter| meter.original_ledger())
                    != self.owner.map(|owner| (owner.budget_slot, owner.ledger)))
        {
            Err(resource(Resource::Accounting))
        } else {
            Ok(())
        }
    }
    fn container_owner(&mut self, owner: Option<Owner>) -> Result<()> {
        self.available()?;
        if self.owner != owner {
            return Err(self.failure(resource(Resource::Accounting)));
        }
        Ok(())
    }
    fn failure(&mut self, error: Error) -> Error {
        if self.is_strict() {
            self.poisoned = true;
        }
        error
    }
    fn arithmetic(&mut self) -> Error {
        self.failure(resource(Resource::Arithmetic))
    }
    fn allocation(&mut self) -> Error {
        self.failure(resource(Resource::Allocation))
    }
    fn product(&mut self, count: usize, width: usize) -> Result<usize> {
        count.checked_mul(width).ok_or_else(|| self.arithmetic())
    }
    pub(super) fn extra_work(&mut self, amount: usize) -> Result<()> {
        self.available()?;
        let result = match &mut self.meter {
            Some(meter) => meter.work(amount),
            None => Ok(()),
        };
        result.map_err(|error| self.failure(error))
    }
    pub(super) fn reserve_storage(&mut self, bytes: usize) -> Result<()> {
        self.available()?;
        let result = match &mut self.meter {
            Some(meter) => meter.storage(bytes),
            None => Ok(()),
        };
        result.map_err(|error| self.failure(error))
    }
    pub(super) fn logical_work(&mut self, work: &mut usize, amount: usize) -> Result<()> {
        self.available()?;
        if !self.is_strict() {
            return project_loop_graph_charge_v1(work, amount);
        }
        let mut next = *work;
        if let Err(error) = project_loop_graph_charge_v1(&mut next, amount) {
            *work = next; // preserve the old exact local overflow/cap state
            return Err(self.failure(error));
        }
        self.extra_work(amount)?; // first external denial leaves caller state intact
        *work = next;
        Ok(())
    }
    pub(super) fn logical_statement_scan(&mut self, work: &mut usize, visits: usize) -> Result<()> {
        self.available()?;
        if !self.is_strict() {
            return charge_statement_scan_equivalent_v1(work, visits);
        }
        let mut next = *work;
        if let Err(error) = charge_statement_scan_equivalent_v1(&mut next, visits) {
            *work = next; // preserve the exact repeated-charge first failure
            return Err(self.failure(error));
        }
        self.extra_work(visits)?;
        *work = next;
        Ok(())
    }
    pub(super) fn reserve_frame<T>(&mut self, fixed_local_bytes: usize) -> Result<()> {
        self.available()?;
        if !self.is_strict() {
            return Ok(());
        }
        let bytes = 4096usize
            .checked_add(fixed_local_bytes)
            .and_then(|n| n.checked_add(size_of::<Self>()))
            .and_then(|n| size_of::<T>().checked_mul(2).and_then(|x| n.checked_add(x)))
            .and_then(|n| {
                size_of::<Result<T>>()
                    .checked_mul(2)
                    .and_then(|x| n.checked_add(x))
            })
            .ok_or_else(|| self.arithmetic())?;
        self.extra_work(16)?;
        self.reserve_storage(bytes)
    }
    // Raw vectors/payload values retain the enclosing analyzer's ownership.
    // This helper does not transfer a reservation or lend Budget; its caller
    // must drop all such values before that enclosing owner refunds credits.
    /// Exact preparation Vec policy through this existing retained handle.
    /// This entry's owner/sticky-denial admission is intentionally stricter
    /// than the original preparation wrapper's fitting-capacity early return.
    pub(super) fn preparation_reserve_v1<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<()> {
        let result =
            super::bf16_nominal_preparation_resources_v1::preparation_reserve_with_meter_v1(
                self, values, additional,
            );
        result.map_err(|error| self.failure(error))
    }
    pub(super) fn preparation_push_v1<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<()> {
        let result = super::bf16_nominal_preparation_resources_v1::preparation_push_with_meter_v1(
            self, values, value,
        );
        result.map_err(|error| self.failure(error))
    }

    pub(super) fn reserve_vec<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        legacy: LegacyReserve,
        old_error: &'static str,
    ) -> Result<()> {
        self.available()?;
        if !self.is_strict() {
            return match legacy {
                LegacyReserve::Exact => values.try_reserve_exact(additional),
                LegacyReserve::Amortized => values.try_reserve(additional),
            }
            .map_err(|_| Error::Unsupported(old_error));
        }
        self.extra_work(8)?;
        let requested = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| self.arithmetic())?;
        if requested <= values.capacity() {
            return Ok(());
        }
        let relocation = self.product(values.len(), size_of::<T>())?;
        self.extra_work(relocation)?;
        let bytes = self.product(requested, size_of::<T>())?;
        self.reserve_storage(bytes)?;
        values
            .try_reserve_exact(additional)
            .map_err(|_| self.allocation())?;
        if size_of::<T>() != 0 && values.capacity() != requested {
            return Err(self.allocation());
        }
        Ok(())
    }
    pub(super) fn push_vec<T>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
        old_error: &'static str,
    ) -> Result<()> {
        self.available()?;
        if self.is_strict() {
            // A by-value T remains live even when an existing Vec has capacity.
            self.reserve_frame::<T>(size_of::<Vec<T>>())?;
            let work = size_of::<T>()
                .checked_add(1)
                .ok_or_else(|| self.arithmetic())?;
            self.extra_work(work)?;
        }
        self.reserve_vec(values, 1, LegacyReserve::Amortized, old_error)?;
        values.push(value);
        Ok(())
    }
    pub(super) fn filled<T: Copy>(&mut self, count: usize, value: T) -> Result<Vec<T>> {
        self.available()?;
        if !self.is_strict() {
            return Ok(vec![value; count]);
        }
        // count == 0 still carries a complete by-value T through this call.
        self.reserve_frame::<T>(size_of::<Vec<T>>())?;
        let work = self.product(count, size_of::<T>())?;
        let work = work.checked_add(count).ok_or_else(|| self.arithmetic())?;
        self.extra_work(work)?;
        let mut values = Vec::new();
        self.reserve_vec(
            &mut values,
            count,
            LegacyReserve::Exact,
            "assertion array storage cannot be reserved",
        )?;
        // Copy directly, without invoking an arbitrary user Clone implementation.
        values.resize_with(count, || value);
        Ok(values)
    }
    pub(super) fn nested<T>(&mut self, count: usize) -> Result<Vec<Vec<T>>> {
        self.available()?;
        if !self.is_strict() {
            let mut values = Vec::with_capacity(count);
            values.resize_with(count, Vec::new);
            return Ok(values);
        }
        let work = self.product(count, size_of::<Vec<T>>())?;
        let work = work.checked_add(count).ok_or_else(|| self.arithmetic())?;
        self.extra_work(work)?;
        let mut values = Vec::new();
        self.reserve_vec(
            &mut values,
            count,
            LegacyReserve::Exact,
            "assertion nested table storage cannot be reserved",
        )?;
        values.resize_with(count, Vec::new);
        Ok(values)
    }

    fn place_payload_bytes(&mut self, place: &SemanticPlaceV1) -> Result<usize> {
        self.product(place.projections().len(), size_of::<SemanticProjectionV1>())
    }
    fn operand_payload_bytes(&mut self, operand: &SemanticOperandV1) -> Result<usize> {
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.place_payload_bytes(place)
            }
            SemanticOperandV1::Constant(constant) => match constant.value() {
                SemanticConstantValueV1::Bytes(bytes) => Ok(bytes.as_bytes().len()),
                _ => Ok(0),
            },
        }
    }
    fn prepay_copy<T>(&mut self, heap_bytes: usize) -> Result<()> {
        self.reserve_frame::<T>(0)?;
        let work = size_of::<T>()
            .checked_add(heap_bytes)
            .and_then(|n| n.checked_add(16))
            .ok_or_else(|| self.arithmetic())?;
        self.extra_work(work)?;
        self.reserve_storage(heap_bytes)
    }
    pub(super) fn clone_place(&mut self, place: &SemanticPlaceV1) -> Result<SemanticPlaceV1> {
        self.available()?;
        if self.is_strict() {
            let bytes = self.place_payload_bytes(place)?;
            self.prepay_copy::<SemanticPlaceV1>(bytes)?;
        }
        Ok(place.clone())
    }
    pub(super) fn clone_operand(
        &mut self,
        operand: &SemanticOperandV1,
    ) -> Result<SemanticOperandV1> {
        self.available()?;
        if self.is_strict() {
            let bytes = self.operand_payload_bytes(operand)?;
            self.prepay_copy::<SemanticOperandV1>(bytes)?;
        }
        Ok(operand.clone())
    }
    pub(super) fn clone_checked_binary(
        &mut self,
        value: &SemanticCheckedBinaryRvalueV1,
    ) -> Result<SemanticCheckedBinaryRvalueV1> {
        self.available()?;
        if self.is_strict() {
            let left = self.operand_payload_bytes(value.left())?;
            let right = self.operand_payload_bytes(value.right())?;
            let bytes = left.checked_add(right).ok_or_else(|| self.arithmetic())?;
            self.prepay_copy::<SemanticCheckedBinaryRvalueV1>(bytes)?;
        }
        Ok(value.clone())
    }
    pub(super) fn charge_operand_comparison(
        &mut self,
        left: &SemanticOperandV1,
        right: &SemanticOperandV1,
    ) -> Result<()> {
        self.available()?;
        if !self.is_strict() {
            return Ok(());
        }
        let left = self.operand_payload_bytes(left)?;
        let right = self.operand_payload_bytes(right)?;
        let work = left
            .checked_add(right)
            .and_then(|n| n.checked_add(2 * size_of::<SemanticOperandV1>()))
            .and_then(|n| n.checked_add(16))
            .ok_or_else(|| self.arithmetic())?;
        self.extra_work(work)
    }
    pub(super) fn same_operand_value(
        &mut self,
        left: &SemanticOperandV1,
        right: &SemanticOperandV1,
    ) -> Result<bool> {
        self.charge_operand_comparison(left, right)?;
        Ok(same_semantic_operand_value_v1(left, right))
    }
}

mod sealed {
    pub trait Key {}
    impl Key for usize {}
    impl Key for (usize, usize) {}
}
pub(super) trait AssertionKeyV1: sealed::Key + Copy + Eq + Hash {
    const WORDS: usize;
}
impl AssertionKeyV1 for usize {
    const WORDS: usize = 1;
}
impl AssertionKeyV1 for (usize, usize) {
    const WORDS: usize = 2;
}

enum SetStorage<K> {
    Legacy(HashSet<K>),
    Strict(Vec<K>),
}
pub(super) struct AssertionSetV1<'a, K: AssertionKeyV1> {
    storage: SetStorage<K>,
    owner: Option<Owner>,
    lifetime: PhantomData<fn(&'a ()) -> &'a ()>,
}
impl<'a, K: AssertionKeyV1> AssertionSetV1<'a, K> {
    pub(super) fn new(resources: &mut AssertionResourcesV1<'a>) -> Result<Self> {
        resources.reserve_frame::<Self>(0)?;
        Ok(Self {
            storage: if resources.is_strict() {
                SetStorage::Strict(Vec::new())
            } else {
                SetStorage::Legacy(HashSet::new())
            },
            owner: resources.owner,
            lifetime: PhantomData,
        })
    }
    fn mode(&self, resources: &mut AssertionResourcesV1<'a>) -> Result<()> {
        resources.container_owner(self.owner)
    }
    pub(super) fn len(&self) -> usize {
        match &self.storage {
            SetStorage::Legacy(v) => v.len(),
            SetStorage::Strict(v) => v.len(),
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(super) fn reserve(
        &mut self,
        additional: usize,
        resources: &mut AssertionResourcesV1<'a>,
        old_error: &'static str,
    ) -> Result<()> {
        self.mode(resources)?;
        match &mut self.storage {
            SetStorage::Legacy(values) => values
                .try_reserve(additional)
                .map_err(|_| Error::Unsupported(old_error)),
            SetStorage::Strict(values) => {
                resources.reserve_vec(values, additional, LegacyReserve::Amortized, old_error)
            }
        }
    }
    pub(super) fn contains(
        &self,
        key: &K,
        resources: &mut AssertionResourcesV1<'a>,
    ) -> Result<bool> {
        self.mode(resources)?;
        match &self.storage {
            SetStorage::Legacy(values) => Ok(values.contains(key)),
            SetStorage::Strict(values) => {
                let work = resources.product(values.len(), K::WORDS)?;
                let work = work.checked_add(4).ok_or_else(|| resources.arithmetic())?;
                resources.extra_work(work)?;
                Ok(values.contains(key))
            }
        }
    }
    pub(super) fn insert(
        &mut self,
        key: K,
        resources: &mut AssertionResourcesV1<'a>,
    ) -> Result<bool> {
        self.mode(resources)?;
        match &mut self.storage {
            SetStorage::Legacy(values) => Ok(values.insert(key)),
            SetStorage::Strict(values) => {
                let work = resources.product(values.len(), K::WORDS)?;
                let work = work.checked_add(4).ok_or_else(|| resources.arithmetic())?;
                resources.extra_work(work)?;
                if values.contains(&key) {
                    return Ok(false);
                }
                resources.push_vec(values, key, "assertion set storage cannot be reserved")?;
                Ok(true)
            }
        }
    }
    pub(super) fn remove(
        &mut self,
        key: &K,
        resources: &mut AssertionResourcesV1<'a>,
    ) -> Result<bool> {
        self.mode(resources)?;
        match &mut self.storage {
            SetStorage::Legacy(values) => Ok(values.remove(key)),
            SetStorage::Strict(values) => {
                let scan = resources.product(values.len(), K::WORDS)?;
                let work = scan
                    .checked_add(2 * size_of::<K>())
                    .and_then(|n| n.checked_add(4))
                    .ok_or_else(|| resources.arithmetic())?;
                resources.extra_work(work)?;
                let Some(index) = values.iter().position(|candidate| candidate == key) else {
                    return Ok(false);
                };
                values.swap_remove(index);
                Ok(true)
            }
        }
    }
}

enum CacheStorage {
    Legacy(HashMap<(usize, usize), bool>),
    Strict(Vec<((usize, usize), bool)>),
}
pub(super) struct AssertionCacheV1<'a> {
    storage: CacheStorage,
    owner: Option<Owner>,
    lifetime: PhantomData<fn(&'a ()) -> &'a ()>,
}
impl<'a> AssertionCacheV1<'a> {
    pub(super) fn new(resources: &mut AssertionResourcesV1<'a>) -> Result<Self> {
        resources.reserve_frame::<Self>(0)?;
        Ok(Self {
            storage: if resources.is_strict() {
                CacheStorage::Strict(Vec::new())
            } else {
                CacheStorage::Legacy(HashMap::new())
            },
            owner: resources.owner,
            lifetime: PhantomData,
        })
    }
    fn mode(&self, resources: &mut AssertionResourcesV1<'a>) -> Result<()> {
        resources.container_owner(self.owner)
    }
    pub(super) fn len(&self) -> usize {
        match &self.storage {
            CacheStorage::Legacy(v) => v.len(),
            CacheStorage::Strict(v) => v.len(),
        }
    }
    pub(super) fn get(
        &self,
        key: &(usize, usize),
        resources: &mut AssertionResourcesV1<'a>,
    ) -> Result<Option<bool>> {
        self.mode(resources)?;
        match &self.storage {
            CacheStorage::Legacy(values) => Ok(values.get(key).copied()),
            CacheStorage::Strict(values) => {
                let work = resources.product(values.len(), 2)?;
                let work = work.checked_add(4).ok_or_else(|| resources.arithmetic())?;
                resources.extra_work(work)?;
                Ok(values
                    .iter()
                    .find(|(candidate, _)| candidate == key)
                    .map(|(_, value)| *value))
            }
        }
    }
    pub(super) fn insert(
        &mut self,
        key: (usize, usize),
        value: bool,
        resources: &mut AssertionResourcesV1<'a>,
    ) -> Result<()> {
        self.insert_with_limit(
            key,
            value,
            MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1,
            resources,
        )
    }
    pub(super) fn insert_with_limit(
        &mut self,
        key: (usize, usize),
        value: bool,
        limit: usize,
        resources: &mut AssertionResourcesV1<'a>,
    ) -> Result<()> {
        self.mode(resources)?;
        match &mut self.storage {
            CacheStorage::Legacy(values) => {
                insert_assertion_proof_cache_with_limit(values, key, value, limit)
            }
            CacheStorage::Strict(values) => {
                let work = resources.product(values.len(), 2)?;
                let work = work.checked_add(8).ok_or_else(|| resources.arithmetic())?;
                resources.extra_work(work)?;
                if let Some((_, old)) = values.iter_mut().find(|(candidate, _)| *candidate == key) {
                    *old = value;
                    return Ok(());
                }
                if values.len() >= limit {
                    return Err(Error::Unsupported(
                        "assertion proof cache exceeds the bounded entry limit",
                    ));
                }
                resources.push_vec(
                    values,
                    (key, value),
                    "assertion proof cache storage cannot be reserved",
                )
            }
        }
    }
}

/// Drop-only storage: no source/resource lifetime, owner identity, or proof API.
/// Created only by consuming a cache; there is deliberately no reverse path.
#[allow(dead_code)]
pub(super) struct RetiredAssertionCacheV1 {
    storage: CacheStorage,
}
impl AssertionCacheV1<'_> {
    /// Header-only move, including after denial. The enclosing bridge must
    /// prepay retirement before constructing any live proof/cache payload.
    pub(super) fn retire_payload_v1(self) -> RetiredAssertionCacheV1 {
        let Self {
            storage,
            owner: _,
            lifetime: _,
        } = self;
        RetiredAssertionCacheV1 { storage }
    }
    /// Selected source transfers for the move above, computed before payloads.
    pub(super) fn retirement_frame_v1() -> Option<usize> {
        size_of::<(
            Self,
            CacheStorage,
            Option<Owner>,
            PhantomData<fn(&()) -> &()>,
        )>()
        .checked_add(size_of::<RetiredAssertionCacheV1>().checked_mul(2)?)
        .and_then(|n| n.checked_add(size_of::<Option<usize>>().checked_mul(2)?))
    }
}
#[cfg(test)]
impl RetiredAssertionCacheV1 {
    pub(super) fn snapshot_for_test(&self) -> (u8, usize, usize, usize) {
        match &self.storage {
            CacheStorage::Legacy(rows) => (0, 0, rows.len(), rows.capacity()),
            CacheStorage::Strict(rows) => (1, rows.as_ptr() as usize, rows.len(), rows.capacity()),
        }
    }
}
#[cfg(test)]
impl AssertionCacheV1<'_> {
    pub(super) fn snapshot_for_retirement_test(&self) -> (u8, usize, usize, usize) {
        match &self.storage {
            CacheStorage::Legacy(rows) => (0, 0, rows.len(), rows.capacity()),
            CacheStorage::Strict(rows) => (1, rows.as_ptr() as usize, rows.len(), rows.capacity()),
        }
    }
}

#[cfg(test)]
mod retirement_cache_controls {
    use super::*;
    #[test]
    fn cache_retirement_frame_is_the_selected_typed_move_and_metadata_return() {
        type C = AssertionCacheV1<'static>;
        let expected = size_of::<(C, CacheStorage, Option<Owner>, PhantomData<fn(&()) -> &()>)>()
            + 2 * size_of::<RetiredAssertionCacheV1>()
            + 2 * size_of::<Option<usize>>();
        assert_eq!(C::retirement_frame_v1(), Some(expected));
    }
    #[test]
    fn strict_cache_payload_move_keeps_exact_backing_allocation_after_lifetime_end() {
        // Synthetic owned cache storage only: does not authenticate a strict
        // owner or create a usable resource/proof capability.
        let retired;
        let expected;
        {
            let rows = vec![((3usize, 5usize), true), ((7, 11), false)];
            let cache = AssertionCacheV1 {
                storage: CacheStorage::Strict(rows),
                owner: None,
                lifetime: PhantomData,
            };
            expected = cache.snapshot_for_retirement_test();
            retired = cache.retire_payload_v1();
        }
        assert_eq!(retired.snapshot_for_test(), expected);
    }
    #[test]
    fn legacy_cache_payload_can_only_retire_not_rehydrate_as_strict() {
        let retired = {
            let mut resources = AssertionResourcesV1::legacy();
            let mut cache = AssertionCacheV1::new(&mut resources).unwrap();
            cache.insert((13, 17), true, &mut resources).unwrap();
            cache.retire_payload_v1()
        };
        let state = retired.snapshot_for_test();
        assert_eq!(state.0, 0);
        assert_eq!(state.2, 1);
    }
}

enum QueueStorage {
    Legacy(VecDeque<usize>),
    Strict { values: Vec<usize>, head: usize },
}
pub(super) struct AssertionQueueV1<'a> {
    storage: QueueStorage,
    owner: Option<Owner>,
    lifetime: PhantomData<fn(&'a ()) -> &'a ()>,
}
impl<'a> AssertionQueueV1<'a> {
    pub(super) fn new(resources: &mut AssertionResourcesV1<'a>) -> Result<Self> {
        resources.reserve_frame::<Self>(0)?;
        Ok(Self {
            storage: if resources.is_strict() {
                QueueStorage::Strict {
                    values: Vec::new(),
                    head: 0,
                }
            } else {
                QueueStorage::Legacy(VecDeque::new())
            },
            owner: resources.owner,
            lifetime: PhantomData,
        })
    }
    fn mode(&self, resources: &mut AssertionResourcesV1<'a>) -> Result<()> {
        resources.container_owner(self.owner)
    }
    pub(super) fn len(&self) -> usize {
        match &self.storage {
            QueueStorage::Legacy(v) => v.len(),
            QueueStorage::Strict { values, head } => values.len() - head,
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(super) fn reserve(
        &mut self,
        additional: usize,
        resources: &mut AssertionResourcesV1<'a>,
        old_error: &'static str,
    ) -> Result<()> {
        self.mode(resources)?;
        match &mut self.storage {
            QueueStorage::Legacy(values) => values
                .try_reserve(additional)
                .map_err(|_| Error::Unsupported(old_error)),
            QueueStorage::Strict { values, .. } => {
                resources.reserve_vec(values, additional, LegacyReserve::Amortized, old_error)
            }
        }
    }
    pub(super) fn push_back(
        &mut self,
        value: usize,
        resources: &mut AssertionResourcesV1<'a>,
    ) -> Result<()> {
        self.mode(resources)?;
        match &mut self.storage {
            QueueStorage::Legacy(values) => {
                values.push_back(value);
                Ok(())
            }
            QueueStorage::Strict { values, .. } => {
                resources.push_vec(values, value, "assertion FIFO storage cannot be reserved")
            }
        }
    }
    pub(super) fn pop_front(
        &mut self,
        resources: &mut AssertionResourcesV1<'a>,
    ) -> Result<Option<usize>> {
        self.mode(resources)?;
        resources.extra_work(4)?;
        Ok(match &mut self.storage {
            QueueStorage::Legacy(values) => values.pop_front(),
            QueueStorage::Strict { values, head } => {
                let value = values.get(*head).copied();
                if value.is_some() {
                    *head += 1;
                }
                value
            }
        })
    }
    pub(super) fn clear(&mut self, resources: &mut AssertionResourcesV1<'a>) -> Result<()> {
        self.mode(resources)?;
        match &mut self.storage {
            QueueStorage::Legacy(values) => values.clear(),
            QueueStorage::Strict { values, head } => {
                let work = values
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| resources.arithmetic())?;
                resources.extra_work(work)?;
                values.clear();
                *head = 0;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "assertion_resources_v1_tests.rs"]
mod tests;

impl super::bf16_nominal_preparation_resources_v1::PreparationPolicyMeterV1
    for AssertionResourcesV1<'_>
{
    fn preparation_admit_v1(&self) -> Result<()> {
        self.available()
    }
    fn preparation_is_metered_v1(&self) -> bool {
        self.is_strict()
    }
    fn preparation_work_v1(&mut self, amount: usize) -> Result<()> {
        self.extra_work(amount)
    }
    fn preparation_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.reserve_storage(amount)
    }
}

#[cfg(test)]
#[path = "preparation_reserve_policy_v1_tests.rs"]
mod preparation_policy_tests;
