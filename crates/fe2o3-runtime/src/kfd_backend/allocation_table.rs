//! One handle namespace, with no ordinary byte shadow for generated storage.

use super::*;
use std::collections::hash_map::Entry;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedAllocationV1 {
    pub device: u64,
    pub kind: RuntimeMemoryKindV1,
    pub alignment: u64,
    pub byte_len: u64,
    pub adoption: u64,
    pub ordinal: usize,
}

#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "preserve inline ordinary records without a new allocation in preflighted commits"
)]
enum SlotV1 {
    Ordinary(AllocationRecordV1),
    Generated(GeneratedAllocationV1),
}

#[derive(Default, Debug)]
pub(super) struct AllocationTableV1 {
    records: HashMap<u64, SlotV1>,
}

impl AllocationTableV1 {
    pub(super) fn len(&self) -> usize {
        self.records.len()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    pub(super) fn contains_key(&self, id: &u64) -> bool {
        self.records.contains_key(id)
    }
    pub(super) fn try_reserve(
        &mut self,
        count: usize,
    ) -> Result<(), std::collections::TryReserveError> {
        self.records.try_reserve(count)
    }

    // Ordinary projections never fabricate bytes, mutate or consume a generated slot.
    pub(super) fn get(&self, id: &u64) -> Option<&AllocationRecordV1> {
        match self.records.get(id)? {
            SlotV1::Ordinary(value) => Some(value),
            SlotV1::Generated(_) => None,
        }
    }
    pub(super) fn get_mut(&mut self, id: &u64) -> Option<&mut AllocationRecordV1> {
        match self.records.get_mut(id)? {
            SlotV1::Ordinary(value) => Some(value),
            SlotV1::Generated(_) => None,
        }
    }
    pub(super) fn insert(&mut self, id: u64, value: AllocationRecordV1) {
        match self.records.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(SlotV1::Ordinary(value));
            }
            Entry::Occupied(_) => panic!("allocation handle already registered"),
        }
    }
    pub(super) fn remove(&mut self, id: &u64) -> Option<AllocationRecordV1> {
        let Entry::Occupied(entry) = self.records.entry(*id) else {
            return None;
        };
        if !matches!(entry.get(), SlotV1::Ordinary(_)) {
            return None;
        }
        let SlotV1::Ordinary(value) = entry.remove() else {
            unreachable!()
        };
        Some(value)
    }
    pub(super) fn ordinary_iter(&self) -> impl Iterator<Item = (&u64, &AllocationRecordV1)> {
        self.records.iter().filter_map(|(id, value)| match value {
            SlotV1::Ordinary(value) => Some((id, value)),
            SlotV1::Generated(_) => None,
        })
    }
    #[cfg(test)]
    pub(super) fn keys(&self) -> impl Iterator<Item = &u64> {
        self.records.keys()
    }
    #[cfg(test)]
    pub(super) fn values_mut(&mut self) -> impl Iterator<Item = &mut AllocationRecordV1> {
        self.records.values_mut().filter_map(|value| match value {
            SlotV1::Ordinary(value) => Some(value),
            SlotV1::Generated(_) => None,
        })
    }
    #[cfg(any(test, feature = "hardware-qualification"))]
    pub(super) fn has_generated(&self) -> bool {
        self.records
            .values()
            .any(|value| matches!(value, SlotV1::Generated(_)))
    }
    pub(super) fn generated_count_for_adoption(&self, adoption: u64) -> usize {
        self.records
            .values()
            .filter(|value| matches!(value, SlotV1::Generated(value) if value.adoption == adoption))
            .count()
    }
    pub(super) fn generated(&self, id: u64) -> Option<GeneratedAllocationV1> {
        match self.records.get(&id)? {
            SlotV1::Generated(value) => Some(*value),
            SlotV1::Ordinary(_) => None,
        }
    }
    pub(super) fn insert_generated(&mut self, id: u64, value: GeneratedAllocationV1) {
        match self.records.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(SlotV1::Generated(value));
            }
            Entry::Occupied(_) => panic!("generated allocation handle already registered"),
        }
    }
    pub(super) fn remove_generated(&mut self, id: u64, expected: GeneratedAllocationV1) -> bool {
        let Entry::Occupied(entry) = self.records.entry(id) else {
            return false;
        };
        if !matches!(entry.get(), SlotV1::Generated(value) if *value == expected) {
            return false;
        }
        entry.remove();
        true
    }
    pub(super) fn require_ordinary(
        &self,
        id: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match self.records.get(&id) {
            Some(SlotV1::Ordinary(_)) => Ok(()),
            Some(SlotV1::Generated(_)) => Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "allocation belongs to a generated operation",
            )),
            None => Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD allocation",
            )),
        }
    }
    pub(super) fn reject_generated(
        &self,
        id: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.generated(id).is_some() {
            self.require_ordinary(id)
        } else {
            Ok(())
        }
    }
}

impl std::ops::Index<&u64> for AllocationTableV1 {
    type Output = AllocationRecordV1;
    fn index(&self, id: &u64) -> &Self::Output {
        self.get(id).expect("admitted ordinary allocation")
    }
}
