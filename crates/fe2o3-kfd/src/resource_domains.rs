//! Canonical checked-device parents for ordinary coherent GTT backing.

use crate::{CheckedGfx942XnackMinusDevice, Gfx942HostVisibleBackingBudgetV1};
use fe2o3_resource_accounting::{
    HostMetadataTableV1, MAX_RESOURCE_CREDIT_RECORDS_V1, ResourceCreditAccountV1,
    ResourceCreditErrorV1, ResourceCreditUsageV1, ResourceKindV1, ResourceVectorV1,
};
use fe2o3_runtime_model::{DeviceKeyV1, PciAddressV1};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    unique_id: u64,
    pci: PciAddressV1,
}

impl Identity {
    fn of(device: &CheckedGfx942XnackMinusDevice) -> Self {
        let pci = device.observation().pci();
        Self {
            unique_id: device.observation().unique_id(),
            pci: PciAddressV1 {
                domain: pci.domain(),
                bus: pci.bus(),
                device: pci.device(),
                function: pci.function(),
            },
        }
    }
}

struct DeviceEntry {
    identity: Identity,
    capacity: ResourceVectorV1,
    record_limit: usize,
    account: ResourceCreditAccountV1,
}

struct Inner {
    account: ResourceCreditAccountV1,
    devices: Mutex<HostMetadataTableV1<Option<DeviceEntry>>>,
    quarantine_anchor: Mutex<Option<Arc<Inner>>>,
}

/// Persistent accounting root for participating ordinary coherent GTT sessions.
///
/// Clones share one bounded canonical-device registry. Native consumers accept
/// only root-issued admissions bound to the exact checked device generation.
/// Root capacity includes fixed accounting arenas and registry slot payloads.
/// The registry's one metadata record counts against `max_records` but does not
/// charge `AllocationRecords`. Root usage therefore includes that metadata row.
/// Arc/allocator overhead, this wrapper, VM bootstrap, other GTT profiles, N2 and
/// unrelated roots remain outside this profile. This is not process-global.
#[derive(Clone)]
pub struct Gfx942HostBackingRootV1(Arc<Inner>);

impl Gfx942HostBackingRootV1 {
    /// Exact charged Rust arena/registry payload, excluding wrapper/allocator
    /// overhead. The complete amount is checked before constructing any arena.
    pub fn bootstrap_bytes_v1(
        max_devices: usize,
        max_domains: usize,
        max_records: usize,
    ) -> Result<u64, ResourceCreditErrorV1> {
        if max_devices == 0
            || max_records < 2
            || max_domains > MAX_RESOURCE_CREDIT_RECORDS_V1
            || max_devices.checked_add(2).is_none_or(|n| max_domains < n)
        {
            return Err(ResourceCreditErrorV1::InvalidDomainCapacity);
        }
        fe2o3_resource_accounting::resource_domain_bootstrap_bytes_v1(max_domains, max_records)?
            .checked_add(
                fe2o3_resource_accounting::host_metadata_table_payload_bytes_v1::<
                    Option<DeviceEntry>,
                >(max_devices)
                .ok_or(ResourceCreditErrorV1::AllocationFailed)?,
            )
            .ok_or(ResourceCreditErrorV1::AllocationFailed)
    }

    /// `max_domains` includes root, permanently registered device parents and
    /// live/retained session leaves. It must accommodate every device plus one
    /// session. Unused device-slot capacity is not a separate session ceiling.
    pub fn new(
        capacity: ResourceVectorV1,
        max_devices: usize,
        max_domains: usize,
        max_records: usize,
    ) -> Result<Self, ResourceCreditErrorV1> {
        Self::new_with_profile(capacity, max_devices, max_domains, max_records, false)
    }

    fn new_with_profile(
        capacity: ResourceVectorV1,
        max_devices: usize,
        max_domains: usize,
        max_records: usize,
        class_domains: bool,
    ) -> Result<Self, ResourceCreditErrorV1> {
        if Self::bootstrap_bytes_v1(max_devices, max_domains, max_records)?
            > capacity.get(ResourceKindV1::ControlResidentBytes)
        {
            return Err(ResourceCreditErrorV1::Capacity);
        }
        let account = if class_domains {
            ResourceCreditAccountV1::new_root_with_class_domains_v1(
                capacity,
                max_domains,
                max_records,
            )?
        } else {
            ResourceCreditAccountV1::new_root(capacity, max_domains, max_records)?
        };
        let devices = HostMetadataTableV1::try_new(max_devices, Some(&account), || None)?;
        Ok(Self(Arc::new(Inner {
            account,
            devices: Mutex::new(devices),
            quarantine_anchor: Mutex::new(None),
        })))
    }

    /// Issues a dedicated N1 leaf from a canonical physical-device parent.
    /// Existing device limits and UID/PCI association are immutable. Admission
    /// does not allocate native memory or bypass checked-device currentness.
    pub fn admit_session_v1(
        &self,
        device: &CheckedGfx942XnackMinusDevice,
        device_budget: Gfx942HostVisibleBackingBudgetV1,
        session_budget: Gfx942HostVisibleBackingBudgetV1,
    ) -> Result<Gfx942HostBackingAdmissionV1, ResourceCreditErrorV1> {
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
        device_budget: Gfx942HostVisibleBackingBudgetV1,
        budget: Gfx942HostVisibleBackingBudgetV1,
    ) -> Result<Gfx942HostBackingAdmissionV1, ResourceCreditErrorV1> {
        self.with_device_account(
            identity,
            capacity(device_budget),
            device_budget.max_allocations(),
            |parent| {
                let account = parent.new_child(capacity(budget), budget.max_allocations())?;
                Ok(Gfx942HostBackingAdmissionV1 {
                    root: self.clone(),
                    identity,
                    generation,
                    budget,
                    account,
                    session: None,
                })
            },
        )
    }

    fn with_device_account<T>(
        &self,
        identity: Identity,
        capacity: ResourceVectorV1,
        record_limit: usize,
        create: impl FnOnce(&ResourceCreditAccountV1) -> Result<T, ResourceCreditErrorV1>,
    ) -> Result<T, ResourceCreditErrorV1> {
        if identity.unique_id == 0 {
            return Err(ResourceCreditErrorV1::Invariant);
        }
        let mut devices = self
            .0
            .devices
            .lock()
            .map_err(|_| ResourceCreditErrorV1::Invariant)?;
        if let Some(entry) = devices.iter().flatten().find(|entry| {
            entry.identity.unique_id == identity.unique_id || entry.identity.pci == identity.pci
        }) {
            if entry.identity != identity
                || entry.capacity != capacity
                || entry.record_limit != record_limit
            {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            return create(&entry.account);
        }
        let slot = devices
            .iter()
            .position(Option::is_none)
            .ok_or(ResourceCreditErrorV1::DomainCapacity)?;
        let parent = self.0.account.new_child(capacity, record_limit)?;
        let result = create(&parent)?;
        devices[slot] = Some(DeviceEntry {
            identity,
            capacity,
            record_limit,
            account: parent,
        });
        Ok(result)
    }

    pub fn usage_v1(&self) -> ResourceCreditUsageV1 {
        let mut usage = self.0.account.usage();
        usage.poisoned |= self.0.devices.is_poisoned();
        usage
    }

    pub(crate) fn retain_quarantine(&self) {
        let mut anchor = self
            .0
            .quarantine_anchor
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        anchor.get_or_insert_with(|| Arc::clone(&self.0));
    }

    /// Inert inclusive device usage. A missing registration is not a disposal receipt.
    pub fn device_usage_v1(
        &self,
        unique_id: u64,
    ) -> Result<Option<ResourceCreditUsageV1>, ResourceCreditErrorV1> {
        let devices = self
            .0
            .devices
            .lock()
            .map_err(|_| ResourceCreditErrorV1::Invariant)?;
        Ok(devices
            .iter()
            .flatten()
            .find(|entry| entry.identity.unique_id == unique_id)
            .map(|entry| entry.account.usage()))
    }
}

fn capacity(budget: Gfx942HostVisibleBackingBudgetV1) -> ResourceVectorV1 {
    ResourceVectorV1::ZERO
        .with(
            ResourceKindV1::ResidentHostAllocationBytes,
            budget.max_backing_bytes(),
        )
        .with(
            ResourceKindV1::AllocationRecords,
            budget.max_allocations() as u64,
        )
}

/// Move-only N1 session account, minted only from a checked-device root binding.
/// It conveys accounting identity, never permission to launch or dispose memory.
#[must_use = "move into the native session; dropping an unused admission releases its leaf"]
pub struct Gfx942HostBackingAdmissionV1 {
    root: Gfx942HostBackingRootV1,
    identity: Identity,
    generation: DeviceKeyV1,
    budget: Gfx942HostVisibleBackingBudgetV1,
    account: ResourceCreditAccountV1,
    session: Option<ResourceCreditAccountV1>,
}

impl Gfx942HostBackingAdmissionV1 {
    pub const fn budget_v1(&self) -> Gfx942HostVisibleBackingBudgetV1 {
        self.budget
    }

    pub fn matches_device_v1(&self, device: &CheckedGfx942XnackMinusDevice) -> bool {
        self.matches_identity(Identity::of(device), device.model_admission().model_key())
    }

    fn matches_identity(&self, identity: Identity, generation: DeviceKeyV1) -> bool {
        self.identity == identity && self.generation == generation
    }

    pub(crate) fn into_account(
        self,
        device: DeviceKeyV1,
    ) -> Result<(ResourceCreditAccountV1, BackingRootBindingV1), ResourceCreditErrorV1> {
        if self.generation != device {
            return Err(ResourceCreditErrorV1::Invariant);
        }
        Ok((
            self.account,
            BackingRootBindingV1 {
                root: self.root,
                session: self.session,
            },
        ))
    }
}

pub(crate) struct BackingRootBindingV1 {
    root: Gfx942HostBackingRootV1,
    session: Option<ResourceCreditAccountV1>,
}

impl BackingRootBindingV1 {
    pub(crate) fn retain_quarantine(&self) {
        self.root.retain_quarantine();
    }

    pub(crate) fn session_usage(&self) -> Option<ResourceCreditUsageV1> {
        self.session.as_ref().map(ResourceCreditAccountV1::usage)
    }
}

pub(crate) enum HostBackingAdmission {
    Local(Option<Gfx942HostVisibleBackingBudgetV1>),
    Rooted(Gfx942HostBackingAdmissionV1),
    Native(Gfx942NativeBackingAdmissionV1),
}

mod native;
pub(crate) use native::DeviceBackingAdmissionV1;
pub use native::{
    Gfx942NativeBackingAdmissionV1, Gfx942NativeBackingDeviceBudgetV1, Gfx942NativeBackingRootV1,
    Gfx942NativeBackingSessionBudgetV1,
};

#[cfg(test)]
pub(crate) use native::tests as native_tests;

impl From<Option<Gfx942HostVisibleBackingBudgetV1>> for HostBackingAdmission {
    fn from(budget: Option<Gfx942HostVisibleBackingBudgetV1>) -> Self {
        Self::Local(budget)
    }
}

#[cfg(test)]
pub(crate) mod tests;
