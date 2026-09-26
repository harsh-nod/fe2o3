//! Compound N1/N2 admission with distinct class and aggregate session ceilings.

use super::*;
use crate::Gfx942DeviceBackingBudgetV1;

/// Aggregate limits for one canonical device across participating native sessions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942NativeBackingDeviceBudgetV1 {
    host_bytes: u64,
    device_bytes: u64,
    max_allocations: usize,
}

impl Gfx942NativeBackingDeviceBudgetV1 {
    pub const fn new(host_bytes: u64, device_bytes: u64, max_allocations: usize) -> Option<Self> {
        if host_bytes == 0
            || device_bytes == 0
            || max_allocations == 0
            || max_allocations > MAX_RESOURCE_CREDIT_RECORDS_V1
        {
            return None;
        }
        Some(Self {
            host_bytes,
            device_bytes,
            max_allocations,
        })
    }

    pub const fn max_host_bytes(self) -> u64 {
        self.host_bytes
    }
    pub const fn max_device_bytes(self) -> u64 {
        self.device_bytes
    }
    pub const fn max_allocations(self) -> usize {
        self.max_allocations
    }

    fn capacity(self) -> ResourceVectorV1 {
        ResourceVectorV1::ZERO
            .with(ResourceKindV1::ResidentHostAllocationBytes, self.host_bytes)
            .with(
                ResourceKindV1::ResidentDeviceAllocationBytes,
                self.device_bytes,
            )
            .with(
                ResourceKindV1::AllocationRecords,
                self.max_allocations as u64,
            )
    }
}

/// Both class budgets remain authoritative, in addition to the combined record limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942NativeBackingSessionBudgetV1 {
    host: Gfx942HostVisibleBackingBudgetV1,
    device: Gfx942DeviceBackingBudgetV1,
    max_allocations: usize,
}

impl Gfx942NativeBackingSessionBudgetV1 {
    pub const fn new(
        host: Gfx942HostVisibleBackingBudgetV1,
        device: Gfx942DeviceBackingBudgetV1,
        max_allocations: usize,
    ) -> Option<Self> {
        if max_allocations == 0 || max_allocations > MAX_RESOURCE_CREDIT_RECORDS_V1 {
            return None;
        }
        Some(Self {
            host,
            device,
            max_allocations,
        })
    }

    pub const fn host_budget(self) -> Gfx942HostVisibleBackingBudgetV1 {
        self.host
    }
    pub const fn device_budget(self) -> Gfx942DeviceBackingBudgetV1 {
        self.device
    }
    pub const fn max_allocations(self) -> usize {
        self.max_allocations
    }

    fn capacity(self) -> ResourceVectorV1 {
        ResourceVectorV1::ZERO
            .with(
                ResourceKindV1::ResidentHostAllocationBytes,
                self.host.max_backing_bytes(),
            )
            .with(
                ResourceKindV1::ResidentDeviceAllocationBytes,
                self.device.max_backing_bytes(),
            )
            .with(
                ResourceKindV1::AllocationRecords,
                self.max_allocations as u64,
            )
    }
}

/// A shared canonical registry for N1/N2 root, device, session and class accounts.
/// Legacy N1-only roots are a separate profile; no existing parent is upgraded.
#[derive(Clone)]
pub struct Gfx942NativeBackingRootV1(Gfx942HostBackingRootV1);

impl Gfx942NativeBackingRootV1 {
    /// Includes all generic arena and registry slot payloads, not Arc/allocator overhead.
    pub fn bootstrap_bytes_v1(
        max_devices: usize,
        max_domains: usize,
        max_records: usize,
    ) -> Result<u64, ResourceCreditErrorV1> {
        if max_devices
            .checked_add(4)
            .is_none_or(|minimum| max_domains < minimum)
        {
            return Err(ResourceCreditErrorV1::InvalidDomainCapacity);
        }
        Gfx942HostBackingRootV1::bootstrap_bytes_v1(max_devices, max_domains, max_records)
    }

    /// Domain capacity includes every device, session and both class leaves.
    /// Registry metadata consumes one generic record, not an AllocationRecords unit.
    pub fn new(
        capacity: ResourceVectorV1,
        max_devices: usize,
        max_domains: usize,
        max_records: usize,
    ) -> Result<Self, ResourceCreditErrorV1> {
        Self::bootstrap_bytes_v1(max_devices, max_domains, max_records)?;
        Gfx942HostBackingRootV1::new_with_profile(
            capacity,
            max_devices,
            max_domains,
            max_records,
            true,
        )
        .map(Self)
    }

    pub fn admit_session_v1(
        &self,
        device: &CheckedGfx942XnackMinusDevice,
        device_budget: Gfx942NativeBackingDeviceBudgetV1,
        session_budget: Gfx942NativeBackingSessionBudgetV1,
    ) -> Result<Gfx942NativeBackingAdmissionV1, ResourceCreditErrorV1> {
        self.admit(
            Identity::of(device),
            device.model_admission().model_key(),
            device_budget,
            session_budget,
        )
    }

    fn admit(
        &self,
        identity: Identity,
        generation: DeviceKeyV1,
        device_budget: Gfx942NativeBackingDeviceBudgetV1,
        budget: Gfx942NativeBackingSessionBudgetV1,
    ) -> Result<Gfx942NativeBackingAdmissionV1, ResourceCreditErrorV1> {
        self.0.with_device_account(
            identity,
            device_budget.capacity(),
            device_budget.max_allocations(),
            |parent| {
                let session = parent.new_child(budget.capacity(), budget.max_allocations())?;
                let host =
                    session.new_child(capacity(budget.host), budget.host.max_allocations())?;
                let device = session.new_child(
                    ResourceVectorV1::ZERO
                        .with(
                            ResourceKindV1::ResidentDeviceAllocationBytes,
                            budget.device.max_backing_bytes(),
                        )
                        .with(
                            ResourceKindV1::AllocationRecords,
                            budget.device.max_allocations() as u64,
                        ),
                    budget.device.max_allocations(),
                )?;
                Ok(Gfx942NativeBackingAdmissionV1 {
                    host: Gfx942HostBackingAdmissionV1 {
                        root: self.0.clone(),
                        identity,
                        generation,
                        budget: budget.host,
                        account: host,
                        session: Some(session.clone()),
                    },
                    device: DeviceBackingAdmissionV1 {
                        generation,
                        budget: budget.device,
                        account: device,
                        binding: BackingRootBindingV1 {
                            root: self.0.clone(),
                            session: Some(session),
                        },
                    },
                    budget,
                })
            },
        )
    }

    pub fn usage_v1(&self) -> ResourceCreditUsageV1 {
        self.0.usage_v1()
    }

    pub fn device_usage_v1(
        &self,
        unique_id: u64,
    ) -> Result<Option<ResourceCreditUsageV1>, ResourceCreditErrorV1> {
        self.0.device_usage_v1(unique_id)
    }
}

/// Move-only compound binding. Neither class account can be extracted publicly.
#[must_use = "move into one native session; unused admission releases all three child domains"]
pub struct Gfx942NativeBackingAdmissionV1 {
    host: Gfx942HostBackingAdmissionV1,
    device: DeviceBackingAdmissionV1,
    budget: Gfx942NativeBackingSessionBudgetV1,
}

impl Gfx942NativeBackingAdmissionV1 {
    pub const fn budget_v1(&self) -> Gfx942NativeBackingSessionBudgetV1 {
        self.budget
    }

    pub fn matches_device_v1(&self, device: &CheckedGfx942XnackMinusDevice) -> bool {
        self.coherent()
            && self.host.matches_device_v1(device)
            && self.device.generation == device.model_admission().model_key()
    }

    fn coherent(&self) -> bool {
        self.host.generation == self.device.generation
            && self.host.budget == self.budget.host
            && self.device.budget == self.budget.device
            && Arc::ptr_eq(&self.host.root.0, &self.device.binding.root.0)
            && matches!((&self.host.session, &self.device.binding.session),
                (Some(host), Some(device)) if host.shares_ledger_with(device)
                    && host.usage().capacity == self.budget.capacity()
                    && self.host.account.shares_root_with(host)
                    && self.device.account.shares_root_with(device))
    }

    pub(crate) fn into_parts(
        self,
    ) -> Result<(Gfx942HostBackingAdmissionV1, DeviceBackingAdmissionV1), ResourceCreditErrorV1>
    {
        if !self.coherent() {
            return Err(ResourceCreditErrorV1::Invariant);
        }
        Ok((self.host, self.device))
    }
}

pub(crate) struct DeviceBackingAdmissionV1 {
    pub(super) generation: DeviceKeyV1,
    pub(super) budget: Gfx942DeviceBackingBudgetV1,
    pub(super) account: ResourceCreditAccountV1,
    pub(super) binding: BackingRootBindingV1,
}

impl DeviceBackingAdmissionV1 {
    pub(crate) const fn budget_v1(&self) -> Gfx942DeviceBackingBudgetV1 {
        self.budget
    }

    pub(crate) fn into_account(
        self,
        device: DeviceKeyV1,
        budget: Gfx942DeviceBackingBudgetV1,
    ) -> Result<(ResourceCreditAccountV1, BackingRootBindingV1), ResourceCreditErrorV1> {
        if self.generation != device || self.budget != budget {
            return Err(ResourceCreditErrorV1::Invariant);
        }
        Ok((self.account, self.binding))
    }
}

#[cfg(test)]
pub(crate) mod tests;
