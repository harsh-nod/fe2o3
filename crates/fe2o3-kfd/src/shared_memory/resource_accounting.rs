//! Session-local device-backing credits, not a root or whole-runtime budget.
//!
//! The native owner supplies its actual private layout and domain. This adapter
//! checks their correspondence and uses the shared R67 ledger, but does not
//! establish native allocation or disposal facts. Tokens never leave this module's
//! parent. Session bootstrap, GTT, queue backing and aggregate quarantine are not
//! covered. A dropped retained attempt preserves its complete charge.

use super::{
    DeviceKeyV1, Gfx942DeviceMemoryLayoutV1, KfdAllocMemoryFlags,
    MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1, MAX_GFX942_DEVICE_MEMORY_BYTES_V1, VmKeyV1,
    device_memory_layout,
};
use fe2o3_resource_accounting::{
    ResourceCreditAccountV1, ResourceCreditErrorV1, ResourceKindV1, ResourceReservationV1,
    ResourceVectorV1, RetainedResourceCreditsV1,
};
use fe2o3_runtime_model::r68_device_backing_charge_v1;
use std::sync::Arc;

/// Limits for one native session's device-local backing allocations only.
///
/// These limits do not include session bootstrap, GTT, queue/control storage,
/// metadata bytes or other sessions. Requested logical bytes are not this cost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942DeviceBackingBudgetV1 {
    max_backing_bytes: u64,
    max_allocations: usize,
}

impl Gfx942DeviceBackingBudgetV1 {
    /// Both limits must be positive and within the existing native profile.
    pub const fn new(max_backing_bytes: u64, max_allocations: usize) -> Option<Self> {
        if max_backing_bytes == 0
            || max_backing_bytes > MAX_GFX942_DEVICE_MEMORY_BYTES_V1
            || max_allocations == 0
            || max_allocations > MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1
        {
            return None;
        }
        Some(Self {
            max_backing_bytes,
            max_allocations,
        })
    }

    pub const fn max_backing_bytes(self) -> u64 {
        self.max_backing_bytes
    }

    pub const fn max_allocations(self) -> usize {
        self.max_allocations
    }
}

/// Inert usage of a configured session-local backing account.
///
/// Reserved, retained and quarantined records remain included in both used
/// quantities. This snapshot is neither a native receipt nor disposal authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942DeviceBackingUsageV1 {
    pub budget: Gfx942DeviceBackingBudgetV1,
    pub used_backing_bytes: u64,
    pub used_allocation_records: u64,
    pub reserved_records: usize,
    pub retained_records: usize,
    pub quarantined_records: usize,
    pub poisoned: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DeviceBackingAccountingErrorV1 {
    InvalidBudget,
    InvalidDomain,
    InvalidAllocation,
    Credits(ResourceCreditErrorV1),
}

impl From<ResourceCreditErrorV1> for DeviceBackingAccountingErrorV1 {
    fn from(error: ResourceCreditErrorV1) -> Self {
        Self::Credits(error)
    }
}

struct DomainV1 {
    session_id: u64,
    device: DeviceKeyV1,
    vm: VmKeyV1,
}

impl DomainV1 {
    fn matches(&self, session_id: u64, device: DeviceKeyV1, vm: VmKeyV1) -> bool {
        self.session_id == session_id && self.device == device && self.vm == vm
    }
}

#[derive(Clone, Copy)]
struct AllocationV1 {
    id: u64,
    generation: u64,
    layout: Gfx942DeviceMemoryLayoutV1,
}

impl AllocationV1 {
    fn matches(&self, id: u64, generation: u64, layout: Gfx942DeviceMemoryLayoutV1) -> bool {
        self.id == id && self.generation == generation && self.layout == layout
    }
}

pub(super) struct DeviceBackingAccountV1 {
    domain: Arc<DomainV1>,
    budget: Gfx942DeviceBackingBudgetV1,
    credits: ResourceCreditAccountV1,
}

impl DeviceBackingAccountV1 {
    pub(super) fn new(
        session_id: u64,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        budget: Gfx942DeviceBackingBudgetV1,
    ) -> Result<Self, DeviceBackingAccountingErrorV1> {
        if Gfx942DeviceBackingBudgetV1::new(budget.max_backing_bytes, budget.max_allocations)
            != Some(budget)
        {
            return Err(DeviceBackingAccountingErrorV1::InvalidBudget);
        }
        if session_id == 0 || device.generation.0 == 0 || vm.id.0 == 0 || vm.device != device {
            return Err(DeviceBackingAccountingErrorV1::InvalidDomain);
        }
        let capacity = ResourceVectorV1::ZERO
            .with(
                ResourceKindV1::ResidentDeviceAllocationBytes,
                budget.max_backing_bytes,
            )
            .with(
                ResourceKindV1::AllocationRecords,
                budget.max_allocations as u64,
            );
        let credits = ResourceCreditAccountV1::new(capacity, budget.max_allocations)?;
        Ok(Self {
            domain: Arc::new(DomainV1 {
                session_id,
                device,
                vm,
            }),
            budget,
            credits,
        })
    }

    pub(super) fn matches_domain(&self, session_id: u64, device: DeviceKeyV1, vm: VmKeyV1) -> bool {
        self.domain.matches(session_id, device, vm)
    }

    pub(super) fn usage(&self) -> Gfx942DeviceBackingUsageV1 {
        let usage = self.credits.usage();
        Gfx942DeviceBackingUsageV1 {
            budget: self.budget,
            used_backing_bytes: usage
                .used
                .get(ResourceKindV1::ResidentDeviceAllocationBytes),
            used_allocation_records: usage.used.get(ResourceKindV1::AllocationRecords),
            reserved_records: usage.reserved_records,
            retained_records: usage.retained_records,
            quarantined_records: usage.quarantined_records,
            poisoned: usage.poisoned,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn reserve(
        &self,
        session_id: u64,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        allocation_id: u64,
        generation: u64,
        layout: Gfx942DeviceMemoryLayoutV1,
    ) -> Result<DeviceBackingReservationV1, DeviceBackingAccountingErrorV1> {
        if !self.matches_domain(session_id, device, vm) {
            return Err(DeviceBackingAccountingErrorV1::InvalidDomain);
        }
        if allocation_id == 0 || generation == 0 {
            return Err(DeviceBackingAccountingErrorV1::InvalidAllocation);
        }
        let charge = device_backing_cost_v1(layout)?;
        let credits = self.credits.reserve(charge)?;
        Ok(DeviceBackingReservationV1 {
            domain: Arc::clone(&self.domain),
            allocation: AllocationV1 {
                id: allocation_id,
                generation,
                layout,
            },
            credits,
        })
    }
}

fn device_backing_cost_v1(
    layout: Gfx942DeviceMemoryLayoutV1,
) -> Result<ResourceVectorV1, DeviceBackingAccountingErrorV1> {
    let flags = if layout.uapi_flags() == KfdAllocMemoryFlags::DEVICE_LOCAL.bits() {
        KfdAllocMemoryFlags::DEVICE_LOCAL
    } else if layout.uapi_flags() == KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC.bits() {
        KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC
    } else {
        return Err(DeviceBackingAccountingErrorV1::InvalidAllocation);
    };
    if device_memory_layout(layout.requested_bytes(), layout.alignment(), flags).ok()
        != Some(layout)
    {
        return Err(DeviceBackingAccountingErrorV1::InvalidAllocation);
    }
    r68_device_backing_charge_v1(layout.backing_bytes())
        .ok_or(DeviceBackingAccountingErrorV1::InvalidAllocation)
}

/// The native caller must retain immediately before its first effectful call.
/// Dropping this reservation cancels only the still-unissued accounting attempt.
#[must_use = "retain before native entry; an unissued reservation cancels on Drop"]
pub(super) struct DeviceBackingReservationV1 {
    domain: Arc<DomainV1>,
    allocation: AllocationV1,
    credits: ResourceReservationV1,
}

impl DeviceBackingReservationV1 {
    pub(super) fn retain(self) -> DeviceBackingChargeV1 {
        DeviceBackingChargeV1 {
            domain: self.domain,
            allocation: self.allocation,
            credits: self.credits.retain(),
        }
    }
}

/// Held inside the native record. Drop quarantines the shared credit record.
#[must_use = "only exact successful native disposal permits a refund"]
pub(super) struct DeviceBackingChargeV1 {
    domain: Arc<DomainV1>,
    allocation: AllocationV1,
    credits: RetainedResourceCreditsV1,
}

impl DeviceBackingChargeV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn matches(
        &self,
        account: &DeviceBackingAccountV1,
        session_id: u64,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        allocation_id: u64,
        generation: u64,
        layout: Gfx942DeviceMemoryLayoutV1,
    ) -> bool {
        Arc::ptr_eq(&self.domain, &account.domain)
            && self.domain.matches(session_id, device, vm)
            && self.allocation.matches(allocation_id, generation, layout)
    }

    /// Private adapter contract: full native disposal has already succeeded.
    /// Matching identity is necessary but does not itself prove disposal.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn release_after_disposal(
        self,
        account: &DeviceBackingAccountV1,
        session_id: u64,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        allocation_id: u64,
        generation: u64,
        layout: Gfx942DeviceMemoryLayoutV1,
    ) -> Result<(), DeviceBackingAccountingErrorV1> {
        if !Arc::ptr_eq(&self.domain, &account.domain)
            || !self.domain.matches(session_id, device, vm)
        {
            return Err(DeviceBackingAccountingErrorV1::InvalidDomain);
        }
        if !self.allocation.matches(allocation_id, generation, layout) {
            return Err(DeviceBackingAccountingErrorV1::InvalidAllocation);
        }
        self.credits.release_after_disposal().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_runtime_model::{DeviceGenerationV1, PhysicalDeviceIdV1, VmIdV1};

    fn domain() -> (u64, DeviceKeyV1, VmKeyV1) {
        let device = DeviceKeyV1 {
            physical: PhysicalDeviceIdV1(9),
            generation: DeviceGenerationV1(1),
        };
        (
            7,
            device,
            VmKeyV1 {
                device,
                id: VmIdV1(11),
            },
        )
    }

    fn account(bytes: u64, records: usize) -> DeviceBackingAccountV1 {
        let (session, device, vm) = domain();
        DeviceBackingAccountV1::new(
            session,
            device,
            vm,
            Gfx942DeviceBackingBudgetV1::new(bytes, records).unwrap(),
        )
        .unwrap()
    }

    fn layout(bytes: u64) -> Gfx942DeviceMemoryLayoutV1 {
        device_memory_layout(bytes, 256, KfdAllocMemoryFlags::DEVICE_LOCAL).unwrap()
    }

    fn reserve(
        account: &DeviceBackingAccountV1,
        id: u64,
        bytes: u64,
    ) -> Result<DeviceBackingReservationV1, DeviceBackingAccountingErrorV1> {
        let (session, device, vm) = domain();
        account.reserve(session, device, vm, id, 1, layout(bytes))
    }

    #[test]
    fn budget_bounds_match_the_native_profile_and_model() {
        assert_eq!(
            MAX_GFX942_DEVICE_MEMORY_BYTES_V1,
            fe2o3_runtime_model::R68_MAX_DEVICE_BACKING_BYTES_V1
        );
        assert_eq!(MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1, 128);
        for (bytes, records) in [
            (0, 1),
            (1, 0),
            (MAX_GFX942_DEVICE_MEMORY_BYTES_V1 + 1, 1),
            (1, 129),
        ] {
            assert_eq!(Gfx942DeviceBackingBudgetV1::new(bytes, records), None);
        }
        let budget =
            Gfx942DeviceBackingBudgetV1::new(MAX_GFX942_DEVICE_MEMORY_BYTES_V1, 128).unwrap();
        assert_eq!(
            budget.max_backing_bytes(),
            MAX_GFX942_DEVICE_MEMORY_BYTES_V1
        );
        assert_eq!(budget.max_allocations(), 128);
    }

    #[test]
    fn cost_uses_canonical_padded_layout_and_only_the_two_native_dimensions() {
        for flags in [
            KfdAllocMemoryFlags::DEVICE_LOCAL,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        ] {
            for requested in [1, 4096, 4097, MAX_GFX942_DEVICE_MEMORY_BYTES_V1] {
                let layout = device_memory_layout(requested, 256, flags).unwrap();
                let cost = device_backing_cost_v1(layout).unwrap();
                for (index, value) in cost.counts().iter().copied().enumerate() {
                    let expected =
                        if index == ResourceKindV1::ResidentDeviceAllocationBytes as usize {
                            layout.backing_bytes()
                        } else if index == ResourceKindV1::AllocationRecords as usize {
                            1
                        } else {
                            0
                        };
                    assert_eq!(value, expected);
                }
            }
        }
        let canonical = layout(4097);
        let mut mutations = [canonical; 4];
        mutations[0].backing_bytes = canonical.requested_bytes();
        mutations[1].alignment = 3;
        mutations[2].requested_bytes = 0;
        mutations[3].uapi_flags = 0;
        for altered in mutations {
            assert_eq!(
                device_backing_cost_v1(altered),
                Err(DeviceBackingAccountingErrorV1::InvalidAllocation)
            );
        }
    }

    #[test]
    fn both_byte_and_allocation_saturation_reject_atomically() {
        let bytes = account(4096, 2);
        let before = bytes.usage();
        assert!(matches!(
            reserve(&bytes, 1, 4097),
            Err(DeviceBackingAccountingErrorV1::Credits(
                ResourceCreditErrorV1::Capacity
            ))
        ));
        assert_eq!(bytes.usage(), before);
        let records = account(8192, 1);
        let first = reserve(&records, 1, 1).unwrap();
        let before = records.usage();
        assert!(matches!(
            reserve(&records, 2, 1),
            Err(DeviceBackingAccountingErrorV1::Credits(
                ResourceCreditErrorV1::Capacity
            ))
        ));
        assert_eq!(records.usage(), before);
        drop(first);
        assert_eq!(records.usage().used_backing_bytes, 0);
        assert_eq!(records.usage().used_allocation_records, 0);
    }

    #[test]
    fn unissued_cancellation_and_exact_disposal_are_distinct_from_retained_drop() {
        let account = account(8192, 2);
        let (session, device, vm) = domain();
        drop(reserve(&account, 1, 4097).unwrap());
        assert_eq!(account.usage().used_backing_bytes, 0);
        let charge = reserve(&account, 2, 4097).unwrap().retain();
        assert_eq!(account.usage().retained_records, 1);
        assert_eq!(account.usage().used_backing_bytes, 8192);
        assert!(charge.matches(&account, session, device, vm, 2, 1, layout(4097)));
        charge
            .release_after_disposal(&account, session, device, vm, 2, 1, layout(4097))
            .unwrap();
        assert_eq!(account.usage().used_backing_bytes, 0);
        drop(reserve(&account, 3, 4097).unwrap().retain());
        assert_eq!(account.usage().used_backing_bytes, 8192);
        assert_eq!(account.usage().quarantined_records, 1);
        assert!(!account.usage().poisoned);
    }

    #[test]
    fn domain_and_allocation_substitutions_reject_before_admission() {
        let account = account(8192, 2);
        let (session, device, vm) = domain();
        let before = account.usage();
        let foreign_device = DeviceKeyV1 {
            generation: DeviceGenerationV1(2),
            ..device
        };
        let foreign_vm = VmKeyV1 {
            id: VmIdV1(12),
            ..vm
        };
        for (session, device, vm) in [
            (session + 1, device, vm),
            (session, foreign_device, vm),
            (session, device, foreign_vm),
        ] {
            assert!(!account.matches_domain(session, device, vm));
            assert!(matches!(
                account.reserve(session, device, vm, 1, 1, layout(1)),
                Err(DeviceBackingAccountingErrorV1::InvalidDomain)
            ));
            assert_eq!(account.usage(), before);
        }
        for (id, generation) in [(0, 1), (1, 0)] {
            assert!(matches!(
                account.reserve(session, device, vm, id, generation, layout(1)),
                Err(DeviceBackingAccountingErrorV1::InvalidAllocation)
            ));
            assert_eq!(account.usage(), before);
        }
    }

    #[test]
    fn exact_account_identity_cannot_be_replaced_by_equal_domain_labels() {
        let first = account(8192, 2);
        let second = account(8192, 2);
        let (session, device, vm) = domain();
        let charge = reserve(&first, 1, 1).unwrap().retain();
        assert!(!charge.matches(&second, session, device, vm, 1, 1, layout(1)));
        assert_eq!(
            charge.release_after_disposal(&second, session, device, vm, 1, 1, layout(1)),
            Err(DeviceBackingAccountingErrorV1::InvalidDomain)
        );
        assert_eq!(first.usage().used_backing_bytes, 4096);
        assert_eq!(first.usage().quarantined_records, 1);
        assert_eq!(second.usage().used_backing_bytes, 0);
    }

    #[test]
    fn every_retained_binding_coordinate_is_checked_without_refund() {
        let account = account(8192, 2);
        let (session, device, vm) = domain();
        let charge = reserve(&account, 1, 4097).unwrap().retain();
        let before = account.usage();
        let changed_device = DeviceKeyV1 {
            physical: PhysicalDeviceIdV1(10),
            ..device
        };
        let changed_vm = VmKeyV1 {
            id: VmIdV1(12),
            ..vm
        };
        for (session, device, vm, id, generation, layout) in [
            (session + 1, device, vm, 1, 1, layout(4097)),
            (session, changed_device, vm, 1, 1, layout(4097)),
            (session, device, changed_vm, 1, 1, layout(4097)),
            (session, device, vm, 2, 1, layout(4097)),
            (session, device, vm, 1, 2, layout(4097)),
            (session, device, vm, 1, 1, layout(4098)),
        ] {
            assert!(!charge.matches(&account, session, device, vm, id, generation, layout));
            assert_eq!(account.usage(), before);
        }
        assert_eq!(
            charge.release_after_disposal(&account, session, device, vm, 1, 2, layout(4097)),
            Err(DeviceBackingAccountingErrorV1::InvalidAllocation)
        );
        assert_eq!(account.usage().used_backing_bytes, 8192);
        assert_eq!(account.usage().quarantined_records, 1);
    }

    #[test]
    fn post_entry_unwind_retains_the_backing_charge_without_poison_reclassification() {
        let account = account(8192, 2);
        let failure = std::panic::catch_unwind(|| {
            let _charge = reserve(&account, 1, 4097).unwrap().retain();
            panic!("synthetic native-entry failure");
        });
        assert!(failure.is_err());
        let usage = account.usage();
        assert_eq!(usage.used_backing_bytes, 8192);
        assert_eq!(usage.used_allocation_records, 1);
        assert_eq!(usage.quarantined_records, 1);
        assert!(!usage.poisoned);
    }
}
