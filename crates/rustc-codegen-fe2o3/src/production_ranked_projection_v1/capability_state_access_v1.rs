//! Private shared state access, separating legacy HashMap behavior from a
//! bounded dense caller-state view. No producer, phase or admission authority.
use super::tensor_capability_read_v1::CapabilityStateReadV1;
use super::{ProjectedCapabilityStateV1, ProjectedCapabilityValueV1};
use fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1 as Error;
use std::cell::Cell;

/// Methods deliberately mirror the existing transfer implementation. Legacy
/// HashMap behavior is unchanged. Nominal callers prepay transfers and the full
/// dense local scan before using precharged_values_v1; no HashMap capacity is
/// represented as an exact nominal storage receipt.
pub(super) trait CapabilityStateAccessV1: CapabilityStateReadV1 {
    fn get(&self, local: &usize) -> Option<&ProjectedCapabilityValueV1>;
    fn insert(
        &mut self,
        local: usize,
        value: ProjectedCapabilityValueV1,
    ) -> Option<ProjectedCapabilityValueV1>;
    fn remove(&mut self, local: &usize) -> Option<ProjectedCapabilityValueV1>;
    fn contains_key(&self, local: &usize) -> bool;
    fn precharged_values_v1(&self) -> impl Iterator<Item = &ProjectedCapabilityValueV1>;
}
impl CapabilityStateAccessV1 for ProjectedCapabilityStateV1 {
    fn get(&self, local: &usize) -> Option<&ProjectedCapabilityValueV1> {
        ProjectedCapabilityStateV1::get(self, local)
    }
    fn insert(
        &mut self,
        local: usize,
        value: ProjectedCapabilityValueV1,
    ) -> Option<ProjectedCapabilityValueV1> {
        ProjectedCapabilityStateV1::insert(self, local, value)
    }
    fn remove(&mut self, local: &usize) -> Option<ProjectedCapabilityValueV1> {
        ProjectedCapabilityStateV1::remove(self, local)
    }
    fn contains_key(&self, local: &usize) -> bool {
        ProjectedCapabilityStateV1::contains_key(self, local)
    }
    fn precharged_values_v1(&self) -> impl Iterator<Item = &ProjectedCapabilityValueV1> {
        self.values()
    }
}

/// Borrows an exactly prepaid, actual-local-sized slice. No allocation occurs.
/// An out-of-range local is a sticky STRUCTURAL fault, not a missing capability.
/// The nominal driver checks it after each shared transfer and before a query;
/// no errored view or partially updated state may be committed to a successor.
pub(super) struct DenseCapabilityStateV1<'a> {
    slots: &'a mut [Option<ProjectedCapabilityValueV1>],
    out_of_range: Cell<bool>,
}
impl<'a> DenseCapabilityStateV1<'a> {
    pub(super) fn new(slots: &'a mut [Option<ProjectedCapabilityValueV1>]) -> Self {
        Self {
            slots,
            out_of_range: Cell::new(false),
        }
    }
    pub(super) fn check_bounds_v1(&self) -> Result<(), Error> {
        if self.out_of_range.get() {
            Err(Error::Unavailable(
                "nominal capability local is outside its dense source table",
            ))
        } else {
            Ok(())
        }
    }
    fn checked_slot(&self, local: usize) -> Option<&Option<ProjectedCapabilityValueV1>> {
        match self.slots.get(local) {
            Some(value) => Some(value),
            None => {
                self.out_of_range.set(true);
                None
            }
        }
    }
}
impl CapabilityStateReadV1 for DenseCapabilityStateV1<'_> {
    fn capability_value_v1(&self, local: usize) -> Option<ProjectedCapabilityValueV1> {
        self.checked_slot(local).copied().flatten()
    }
}
impl CapabilityStateAccessV1 for DenseCapabilityStateV1<'_> {
    fn get(&self, local: &usize) -> Option<&ProjectedCapabilityValueV1> {
        self.checked_slot(*local).and_then(Option::as_ref)
    }
    fn insert(
        &mut self,
        local: usize,
        value: ProjectedCapabilityValueV1,
    ) -> Option<ProjectedCapabilityValueV1> {
        match self.slots.get_mut(local) {
            Some(slot) => slot.replace(value),
            None => {
                self.out_of_range.set(true);
                None
            }
        }
    }
    fn remove(&mut self, local: &usize) -> Option<ProjectedCapabilityValueV1> {
        match self.slots.get_mut(*local) {
            Some(slot) => slot.take(),
            None => {
                self.out_of_range.set(true);
                None
            }
        }
    }
    fn contains_key(&self, local: &usize) -> bool {
        self.checked_slot(*local).is_some_and(Option::is_some)
    }
    fn precharged_values_v1(&self) -> impl Iterator<Item = &ProjectedCapabilityValueV1> {
        self.slots.iter().filter_map(Option::as_ref)
    }
}
