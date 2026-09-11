//! One session's ordinary HostVisibleCoherent non-userptr backing credits.
//!
//! Exact native profile/layout extraction and successful disposal are the
//! parent's obligations. This adapter binds their private identity and retains
//! one backing charge across views, mapping and ownership loans. Other GTT
//! profiles, virtual-address capacity, metadata and aggregate budgets are not
//! included. Ordinary coherent bootstrap allocations use the same charge path.

use super::{
    DeviceKeyV1, HostVisibleCoherentGttV1, MAX_SHARED_GTT_ALLOCATIONS_V1,
    SharedGttAllocationLayoutV1, VmKeyV1, profile_layout,
};
use fe2o3_resource_accounting::{
    ResourceCreditAccountV1, ResourceCreditErrorV1, ResourceKindV1, ResourceReservationV1,
    ResourceVectorV1, RetainedResourceCreditsV1,
};
use fe2o3_runtime_model::r72_host_visible_backing_charge_v1;
use std::sync::Arc;

// A numeric profile ceiling matching the existing session envelope, not VA accounting.
const MAX_HOST_VISIBLE_BACKING_BUDGET_BYTES_V1: u64 = 8 * 1024 * 1024 * 1024;

/// Limits for ordinary coherent host backing owned by one native session.
///
/// Includes ordinary coherent bootstrap allocations, not just payload buffers.
/// Other GTT profiles, VA reservations, metadata and other sessions are excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942HostVisibleBackingBudgetV1 {
    max_backing_bytes: u64,
    max_allocations: usize,
}

impl Gfx942HostVisibleBackingBudgetV1 {
    /// Both limits must be positive, at most 8 GiB and 256 allocation records.
    pub const fn new(max_backing_bytes: u64, max_allocations: usize) -> Option<Self> {
        if max_backing_bytes == 0
            || max_backing_bytes > MAX_HOST_VISIBLE_BACKING_BUDGET_BYTES_V1
            || max_allocations == 0
            || max_allocations > MAX_SHARED_GTT_ALLOCATIONS_V1
        {
            None
        } else {
            Some(Self {
                max_backing_bytes,
                max_allocations,
            })
        }
    }

    pub const fn max_backing_bytes(self) -> u64 {
        self.max_backing_bytes
    }

    pub const fn max_allocations(self) -> usize {
        self.max_allocations
    }
}

/// Inert usage, including all reserved, retained and quarantined charges.
/// A usage snapshot is neither a native receipt nor disposal authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942HostVisibleBackingUsageV1 {
    pub budget: Gfx942HostVisibleBackingBudgetV1,
    pub used_backing_bytes: u64,
    pub used_allocation_records: u64,
    pub reserved_records: usize,
    pub retained_records: usize,
    pub quarantined_records: usize,
    pub poisoned: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HostBackingAccountingErrorV1 {
    InvalidBudget,
    InvalidDomain,
    InvalidAllocation,
    Credits(ResourceCreditErrorV1),
}

impl From<ResourceCreditErrorV1> for HostBackingAccountingErrorV1 {
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
    layout: SharedGttAllocationLayoutV1,
}

impl AllocationV1 {
    fn matches(&self, id: u64, generation: u64, layout: SharedGttAllocationLayoutV1) -> bool {
        self.id == id && self.generation == generation && self.layout == layout
    }
}

pub(super) struct HostBackingAccountV1 {
    domain: Arc<DomainV1>,
    budget: Gfx942HostVisibleBackingBudgetV1,
    credits: ResourceCreditAccountV1,
}

impl HostBackingAccountV1 {
    pub(super) fn new(
        session_id: u64,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        budget: Gfx942HostVisibleBackingBudgetV1,
    ) -> Result<Self, HostBackingAccountingErrorV1> {
        if Gfx942HostVisibleBackingBudgetV1::new(budget.max_backing_bytes, budget.max_allocations)
            != Some(budget)
        {
            return Err(HostBackingAccountingErrorV1::InvalidBudget);
        }
        if session_id == 0 || device.generation.0 == 0 || vm.id.0 == 0 || vm.device != device {
            return Err(HostBackingAccountingErrorV1::InvalidDomain);
        }
        let capacity = ResourceVectorV1::ZERO
            .with(
                ResourceKindV1::ResidentHostAllocationBytes,
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

    pub(super) fn domain(&self) -> (DeviceKeyV1, VmKeyV1) {
        (self.domain.device, self.domain.vm)
    }

    pub(super) fn matches_domain(&self, session_id: u64, device: DeviceKeyV1, vm: VmKeyV1) -> bool {
        self.domain.matches(session_id, device, vm)
    }

    pub(super) fn usage(&self) -> Gfx942HostVisibleBackingUsageV1 {
        let usage = self.credits.usage();
        Gfx942HostVisibleBackingUsageV1 {
            budget: self.budget,
            used_backing_bytes: usage.used.get(ResourceKindV1::ResidentHostAllocationBytes),
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
        layout: SharedGttAllocationLayoutV1,
    ) -> Result<HostBackingReservationV1, HostBackingAccountingErrorV1> {
        if !self.matches_domain(session_id, device, vm) {
            return Err(HostBackingAccountingErrorV1::InvalidDomain);
        }
        if allocation_id == 0 || generation == 0 {
            return Err(HostBackingAccountingErrorV1::InvalidAllocation);
        }
        let credits = self.credits.reserve(host_backing_cost_v1(layout)?)?;
        Ok(HostBackingReservationV1 {
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

fn host_backing_cost_v1(
    layout: SharedGttAllocationLayoutV1,
) -> Result<ResourceVectorV1, HostBackingAccountingErrorV1> {
    // Full profile/flag equality matters: the enum alone does not exclude userptr.
    if profile_layout::<HostVisibleCoherentGttV1>(layout.requested_bytes()).ok() != Some(layout) {
        return Err(HostBackingAccountingErrorV1::InvalidAllocation);
    }
    let requested = u64::try_from(layout.requested_bytes())
        .map_err(|_| HostBackingAccountingErrorV1::InvalidAllocation)?;
    let backing = u64::try_from(layout.cpu_mapping_bytes())
        .map_err(|_| HostBackingAccountingErrorV1::InvalidAllocation)?;
    r72_host_visible_backing_charge_v1(requested, backing, layout.gpu_va_bytes())
        .ok_or(HostBackingAccountingErrorV1::InvalidAllocation)
}

/// Cancels on Drop only while the native attempt is still unissued.
#[must_use = "retain immediately before the first native effect"]
pub(super) struct HostBackingReservationV1 {
    domain: Arc<DomainV1>,
    allocation: AllocationV1,
    credits: ResourceReservationV1,
}

impl HostBackingReservationV1 {
    pub(super) fn retain(self) -> HostBackingChargeV1 {
        HostBackingChargeV1 {
            domain: self.domain,
            allocation: self.allocation,
            credits: self.credits.retain(),
        }
    }
}

/// Native records hold this move-only charge. Drop quarantines, never refunds.
#[must_use = "only exact confirmed native disposal permits a refund"]
pub(super) struct HostBackingChargeV1 {
    domain: Arc<DomainV1>,
    allocation: AllocationV1,
    credits: RetainedResourceCreditsV1,
}

impl HostBackingChargeV1 {
    pub(super) fn quarantine(self) {
        self.credits.quarantine();
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn matches(
        &self,
        account: &HostBackingAccountV1,
        session_id: u64,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        allocation_id: u64,
        generation: u64,
        layout: SharedGttAllocationLayoutV1,
    ) -> bool {
        Arc::ptr_eq(&self.domain, &account.domain)
            && self.domain.matches(session_id, device, vm)
            && self.allocation.matches(allocation_id, generation, layout)
    }

    /// Accounting transition after the native owner has confirmed full disposal.
    /// Exact identity is necessary but cannot itself establish a native outcome.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn release_after_disposal(
        self,
        account: &HostBackingAccountV1,
        session_id: u64,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        allocation_id: u64,
        generation: u64,
        layout: SharedGttAllocationLayoutV1,
    ) -> Result<(), HostBackingAccountingErrorV1> {
        if !Arc::ptr_eq(&self.domain, &account.domain)
            || !self.domain.matches(session_id, device, vm)
        {
            return Err(HostBackingAccountingErrorV1::InvalidDomain);
        }
        if !self.allocation.matches(allocation_id, generation, layout) {
            return Err(HostBackingAccountingErrorV1::InvalidAllocation);
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

    fn account(bytes: u64, records: usize) -> HostBackingAccountV1 {
        let (session, device, vm) = domain();
        HostBackingAccountV1::new(
            session,
            device,
            vm,
            Gfx942HostVisibleBackingBudgetV1::new(bytes, records).unwrap(),
        )
        .unwrap()
    }

    fn layout(bytes: usize) -> SharedGttAllocationLayoutV1 {
        profile_layout::<HostVisibleCoherentGttV1>(bytes).unwrap()
    }

    fn reserve(account: &HostBackingAccountV1, id: u64, bytes: usize) -> HostBackingReservationV1 {
        let (session, device, vm) = domain();
        account
            .reserve(session, device, vm, id, 1, layout(bytes))
            .unwrap()
    }

    fn release(account: &HostBackingAccountV1, charge: HostBackingChargeV1, id: u64, bytes: usize) {
        let (session, device, vm) = domain();
        charge
            .release_after_disposal(account, session, device, vm, id, 1, layout(bytes))
            .unwrap();
    }

    #[test]
    fn host_backing_budget_has_positive_numeric_byte_and_record_bounds() {
        for (bytes, records) in [
            (0, 1),
            (1, 0),
            (MAX_HOST_VISIBLE_BACKING_BUDGET_BYTES_V1 + 1, 1),
            (1, 257),
        ] {
            assert!(Gfx942HostVisibleBackingBudgetV1::new(bytes, records).is_none());
        }
        let budget =
            Gfx942HostVisibleBackingBudgetV1::new(MAX_HOST_VISIBLE_BACKING_BUDGET_BYTES_V1, 256)
                .unwrap();
        assert_eq!(
            budget.max_backing_bytes(),
            MAX_HOST_VISIBLE_BACKING_BUDGET_BYTES_V1
        );
        assert_eq!(budget.max_allocations(), 256);
        let (session, device, vm) = domain();
        assert!(matches!(
            HostBackingAccountV1::new(
                session,
                device,
                vm,
                Gfx942HostVisibleBackingBudgetV1 {
                    max_backing_bytes: 0,
                    max_allocations: 1
                }
            ),
            Err(HostBackingAccountingErrorV1::InvalidBudget)
        ));
    }

    #[test]
    fn host_backing_account_rejects_invalid_domains() {
        let (session, device, vm) = domain();
        let budget = Gfx942HostVisibleBackingBudgetV1::new(8192, 2).unwrap();
        for (session, device, vm) in [
            (0, device, vm),
            (
                session,
                DeviceKeyV1 {
                    generation: DeviceGenerationV1(0),
                    ..device
                },
                vm,
            ),
            (
                session,
                device,
                VmKeyV1 {
                    id: VmIdV1(0),
                    ..vm
                },
            ),
            (
                session,
                device,
                VmKeyV1 {
                    device: DeviceKeyV1 {
                        physical: PhysicalDeviceIdV1(10),
                        ..device
                    },
                    ..vm
                },
            ),
        ] {
            assert!(matches!(
                HostBackingAccountV1::new(session, device, vm, budget),
                Err(HostBackingAccountingErrorV1::InvalidDomain)
            ));
        }
        assert_eq!(account(8192, 2).domain(), (device, vm));
    }

    #[test]
    fn host_backing_reserves_padded_bytes_before_retain_and_cancels_unissued() {
        let account = account(8192, 2);
        let before = account.usage();
        let reservation = reserve(&account, 1, 4100);
        let reserved = account.usage();
        assert_eq!(
            (
                reserved.used_backing_bytes,
                reserved.used_allocation_records
            ),
            (8192, 1)
        );
        assert_eq!(
            (reserved.reserved_records, reserved.retained_records),
            (1, 0)
        );
        drop(reservation);
        assert_eq!(account.usage(), before);
    }

    #[test]
    fn host_backing_byte_and_record_failures_are_atomic() {
        let (session, device, vm) = domain();
        for (bytes, records) in [(8191, 2), (8192, 1)] {
            let account = account(bytes, records);
            let first = reserve(&account, 1, 1).retain();
            let before = account.usage();
            assert!(matches!(
                account.reserve(session, device, vm, 2, 1, layout(1)),
                Err(HostBackingAccountingErrorV1::Credits(_))
            ));
            assert_eq!(account.usage(), before);
            release(&account, first, 1, 1);
            assert_eq!(account.usage().used_backing_bytes, 0);
        }
    }

    #[test]
    fn host_backing_only_full_canonical_ordinary_profile_is_accepted() {
        let account = account(16384, 4);
        let (session, device, vm) = domain();
        let good = layout(4100);
        let mut invalid = [good; 6];
        invalid[0].requested_bytes = 0;
        invalid[1].cpu_mapping_bytes = 4100;
        invalid[2].gpu_va_bytes *= 2;
        invalid[3].uapi_flags ^= 1;
        invalid[4].profile = super::super::SharedGttProfileV1::Executable;
        invalid[5].requested_bytes = 8193;
        let before = account.usage();
        for bad in invalid {
            assert!(matches!(
                account.reserve(session, device, vm, 1, 1, bad),
                Err(HostBackingAccountingErrorV1::InvalidAllocation)
            ));
            assert_eq!(account.usage(), before);
        }
        for other in [
            profile_layout::<super::super::KernargGttV1>(4096).unwrap(),
            profile_layout::<super::super::ExecutableGttV1>(4096).unwrap(),
            profile_layout::<super::super::UserptrAqlControlGttV1>(4096).unwrap(),
        ] {
            assert!(matches!(
                account.reserve(session, device, vm, 1, 1, other),
                Err(HostBackingAccountingErrorV1::InvalidAllocation)
            ));
            assert_eq!(account.usage(), before);
        }
    }

    #[test]
    fn host_backing_allocation_and_domain_substitutions_reject_without_debit() {
        let account = account(8192, 2);
        let (session, device, vm) = domain();
        let before = account.usage();
        for (session, device, vm, id, generation) in [
            (session + 1, device, vm, 1, 1),
            (
                session,
                DeviceKeyV1 {
                    generation: DeviceGenerationV1(2),
                    ..device
                },
                vm,
                1,
                1,
            ),
            (
                session,
                device,
                VmKeyV1 {
                    id: VmIdV1(12),
                    ..vm
                },
                1,
                1,
            ),
            (session, device, vm, 0, 1),
            (session, device, vm, 1, 0),
        ] {
            assert!(
                account
                    .reserve(session, device, vm, id, generation, layout(1))
                    .is_err()
            );
            assert_eq!(account.usage(), before);
        }
    }

    #[test]
    fn host_backing_exact_charge_match_is_inert_and_disposal_preserves_sibling() {
        let account = account(16384, 3);
        let other = self::account(16384, 3);
        let (session, device, vm) = domain();
        let first = reserve(&account, 1, 4100).retain();
        let second = reserve(&account, 2, 1).retain();
        let before = account.usage();
        assert!(first.matches(&account, session, device, vm, 1, 1, layout(4100)));
        assert!(!first.matches(&other, session, device, vm, 1, 1, layout(4100)));
        for (session, device, vm, id, generation, layout) in [
            (session + 1, device, vm, 1, 1, layout(4100)),
            (
                session,
                device,
                VmKeyV1 {
                    id: VmIdV1(12),
                    ..vm
                },
                1,
                1,
                layout(4100),
            ),
            (session, device, vm, 2, 1, layout(4100)),
            (session, device, vm, 1, 2, layout(4100)),
            (session, device, vm, 1, 1, layout(4101)),
        ] {
            assert!(!first.matches(&account, session, device, vm, id, generation, layout));
        }
        assert_eq!(account.usage(), before);
        release(&account, first, 1, 4100);
        assert_eq!(
            (
                account.usage().used_backing_bytes,
                account.usage().used_allocation_records
            ),
            (4096, 1)
        );
        release(&account, second, 2, 1);
        assert_eq!(account.usage().used_backing_bytes, 0);
    }

    #[test]
    fn host_backing_abandoned_retained_attempt_keeps_credit_after_account_owner_drop() {
        let account = account(8192, 2);
        let observer = account.credits.clone();
        let charge = reserve(&account, 1, 4100).retain();
        drop(account);
        assert_eq!(
            observer
                .usage()
                .used
                .get(ResourceKindV1::ResidentHostAllocationBytes),
            8192
        );
        drop(charge);
        let usage = observer.usage();
        assert_eq!(
            usage.used.get(ResourceKindV1::ResidentHostAllocationBytes),
            8192
        );
        assert_eq!(usage.used.get(ResourceKindV1::AllocationRecords), 1);
        assert_eq!(usage.quarantined_records, 1);
    }

    #[test]
    fn host_backing_invalid_refund_quarantines_instead_of_releasing() {
        let (session, device, vm) = domain();
        for mutation in 0..5 {
            let account = account(8192, 2);
            let other = self::account(8192, 2);
            let charge = reserve(&account, 1, 4100).retain();
            let target = if mutation == 0 { &other } else { &account };
            let result = charge.release_after_disposal(
                target,
                if mutation == 1 { session + 1 } else { session },
                device,
                vm,
                if mutation == 2 { 2 } else { 1 },
                if mutation == 3 { 2 } else { 1 },
                if mutation == 4 {
                    layout(4101)
                } else {
                    layout(4100)
                },
            );
            assert!(result.is_err());
            assert_eq!(account.usage().used_backing_bytes, 8192);
            assert_eq!(account.usage().used_allocation_records, 1);
            assert_eq!(account.usage().quarantined_records, 1);
            assert_eq!(other.usage().used_backing_bytes, 0);
        }
    }
}
