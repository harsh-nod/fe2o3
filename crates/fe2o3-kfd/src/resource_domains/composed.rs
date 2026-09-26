//! Request/N1/N2 accounting minting and native intake; Context witnesses are separate.

use super::*;
use crate::Gfx942DeviceBackingBudgetV1;
use fe2o3_resource_accounting::{ResourceReservationV1, RetainedResourceCreditsV1};

/// Requested bytes and live request records, independent of padded native backing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942AllocationRequestBudgetV1 {
    bytes: u64,
    records: usize,
}

impl Gfx942AllocationRequestBudgetV1 {
    pub const fn new(bytes: u64, records: usize) -> Option<Self> {
        if bytes == 0 || records == 0 || records > MAX_RESOURCE_CREDIT_RECORDS_V1 {
            return None;
        }
        Some(Self { bytes, records })
    }

    pub const fn max_requested_bytes(self) -> u64 {
        self.bytes
    }

    pub const fn max_requests(self) -> usize {
        self.records
    }

    fn capacity(self) -> ResourceVectorV1 {
        request_charge(self.bytes).with(ResourceKindV1::AllocationRecords, self.records as u64)
    }
}

/// Inclusive ceilings across the request, host and device classes of all sessions.
/// The combined count includes logical requests and cached/bootstrap native records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ComposedBackingDeviceBudgetV1 {
    requested_bytes: u64,
    host_bytes: u64,
    device_bytes: u64,
    combined_records: usize,
}

impl Gfx942ComposedBackingDeviceBudgetV1 {
    pub const fn new(
        requested_bytes: u64,
        host_bytes: u64,
        device_bytes: u64,
        combined_records: usize,
    ) -> Option<Self> {
        if requested_bytes == 0
            || host_bytes == 0
            || device_bytes == 0
            || combined_records == 0
            || combined_records > MAX_RESOURCE_CREDIT_RECORDS_V1
        {
            return None;
        }
        Some(Self {
            requested_bytes,
            host_bytes,
            device_bytes,
            combined_records,
        })
    }

    pub const fn max_requested_bytes(self) -> u64 {
        self.requested_bytes
    }
    pub const fn max_host_bytes(self) -> u64 {
        self.host_bytes
    }
    pub const fn max_device_bytes(self) -> u64 {
        self.device_bytes
    }
    pub const fn max_combined_records(self) -> usize {
        self.combined_records
    }

    fn capacity(self) -> ResourceVectorV1 {
        ResourceVectorV1::ZERO
            .with(
                ResourceKindV1::RequestedAllocationBytes,
                self.requested_bytes,
            )
            .with(ResourceKindV1::ResidentHostAllocationBytes, self.host_bytes)
            .with(
                ResourceKindV1::ResidentDeviceAllocationBytes,
                self.device_bytes,
            )
            .with(
                ResourceKindV1::AllocationRecords,
                self.combined_records as u64,
            )
    }
}

/// Three independent class ceilings and one explicit combined session ceiling.
/// This does not reinterpret the existing native-only session record limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ComposedBackingSessionBudgetV1 {
    request: Gfx942AllocationRequestBudgetV1,
    host: Gfx942HostVisibleBackingBudgetV1,
    device: Gfx942DeviceBackingBudgetV1,
    combined_records: usize,
}

impl Gfx942ComposedBackingSessionBudgetV1 {
    pub const fn new(
        request: Gfx942AllocationRequestBudgetV1,
        host: Gfx942HostVisibleBackingBudgetV1,
        device: Gfx942DeviceBackingBudgetV1,
        combined_records: usize,
    ) -> Option<Self> {
        if combined_records == 0 || combined_records > MAX_RESOURCE_CREDIT_RECORDS_V1 {
            return None;
        }
        Some(Self {
            request,
            host,
            device,
            combined_records,
        })
    }

    pub const fn request_budget(self) -> Gfx942AllocationRequestBudgetV1 {
        self.request
    }
    pub const fn host_budget(self) -> Gfx942HostVisibleBackingBudgetV1 {
        self.host
    }
    pub const fn device_budget(self) -> Gfx942DeviceBackingBudgetV1 {
        self.device
    }
    pub const fn max_combined_records(self) -> usize {
        self.combined_records
    }

    fn capacity(self) -> ResourceVectorV1 {
        Gfx942ComposedBackingDeviceBudgetV1 {
            requested_bytes: self.request.bytes,
            host_bytes: self.host.max_backing_bytes(),
            device_bytes: self.device.max_backing_bytes(),
            combined_records: self.combined_records,
        }
        .capacity()
    }
}

/// Canonical accounting root for sibling request/N1/N2 leaves.
///
/// Native intake retains all three classes; mandatory Context witnesses are separate.
/// Admissions cannot be downgraded into native-only admissions or raw accounts.
/// Existing native-only and independent-account defaults are unchanged.
#[derive(Clone)]
pub struct Gfx942ComposedBackingRootV1(Gfx942HostBackingRootV1);

impl Gfx942ComposedBackingRootV1 {
    pub fn bootstrap_bytes_v1(
        max_devices: usize,
        max_domains: usize,
        max_records: usize,
    ) -> Result<u64, ResourceCreditErrorV1> {
        if max_devices
            .checked_add(5)
            .is_none_or(|minimum| max_domains < minimum)
        {
            return Err(ResourceCreditErrorV1::InvalidDomainCapacity);
        }
        Gfx942HostBackingRootV1::bootstrap_bytes_v1(max_devices, max_domains, max_records)
    }

    /// Includes root, canonical device slots, and four domains per live session.
    /// Registry metadata consumes one generic record but zero AllocationRecords.
    /// Charges fixed arena/registry payloads, not wrapper/Arc/allocator overhead.
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
        device_budget: Gfx942ComposedBackingDeviceBudgetV1,
        session_budget: Gfx942ComposedBackingSessionBudgetV1,
    ) -> Result<Gfx942ComposedBackingAdmissionV1, ResourceCreditErrorV1> {
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
        device_budget: Gfx942ComposedBackingDeviceBudgetV1,
        budget: Gfx942ComposedBackingSessionBudgetV1,
    ) -> Result<Gfx942ComposedBackingAdmissionV1, ResourceCreditErrorV1> {
        self.0.with_device_account(
            identity,
            device_budget.capacity(),
            device_budget.combined_records,
            |parent| {
                let session = parent.new_child(budget.capacity(), budget.combined_records)?;
                let request =
                    session.new_child(budget.request.capacity(), budget.request.records)?;
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
                // Construct every sibling before publishing the canonical device entry.
                Ok(Gfx942ComposedBackingAdmissionV1 {
                    request: Gfx942RequestAccountV1(Arc::new(RequestInner {
                        root: self.0.clone(),
                        identity,
                        generation,
                        account: request,
                        session: session.clone(),
                    })),
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

/// Move-only bundle. Only the typed request handle can be borrowed publicly.
/// Native-only extraction is intentionally unavailable.
#[must_use = "unused admission releases its session and all three class leaves"]
pub struct Gfx942ComposedBackingAdmissionV1 {
    request: Gfx942RequestAccountV1,
    host: Gfx942HostBackingAdmissionV1,
    device: DeviceBackingAdmissionV1,
    budget: Gfx942ComposedBackingSessionBudgetV1,
}

impl Gfx942ComposedBackingAdmissionV1 {
    pub const fn budget_v1(&self) -> Gfx942ComposedBackingSessionBudgetV1 {
        self.budget
    }

    pub fn request_account_v1(&self) -> &Gfx942RequestAccountV1 {
        &self.request
    }

    pub fn matches_device_v1(&self, device: &CheckedGfx942XnackMinusDevice) -> bool {
        self.coherent()
            && self.request.matches_device_v1(device)
            && self.host.matches_device_v1(device)
            && self.device.generation == device.model_admission().model_key()
    }

    pub(crate) fn is_live_v1(&self) -> bool {
        self.request.is_session_live_v1()
    }

    pub(crate) fn into_parts(
        self,
        device: DeviceKeyV1,
    ) -> Result<
        (
            Gfx942RequestAccountV1,
            Gfx942HostBackingAdmissionV1,
            DeviceBackingAdmissionV1,
        ),
        ResourceCreditErrorV1,
    > {
        if !self.coherent() || self.request.0.generation != device || !self.is_live_v1() {
            return Err(ResourceCreditErrorV1::Invariant);
        }
        Ok((self.request, self.host, self.device))
    }

    fn coherent(&self) -> bool {
        let request = &self.request.0;
        self.host.identity == request.identity
            && self.host.generation == request.generation
            && self.device.generation == request.generation
            && self.host.budget == self.budget.host
            && self.device.budget == self.budget.device
            && Arc::ptr_eq(&request.root.0, &self.host.root.0)
            && Arc::ptr_eq(&request.root.0, &self.device.binding.root.0)
            && request.account.usage().capacity == self.budget.request.capacity()
            && request.session.usage().capacity == self.budget.capacity()
            && request.account.shares_root_with(&request.session)
            && matches!((&self.host.session, &self.device.binding.session),
                (Some(host), Some(device)) if request.session.shares_ledger_with(host)
                    && request.session.shares_ledger_with(device)
                    && self.host.account.shares_root_with(host)
                    && self.device.account.shares_root_with(device))
    }
}

struct RequestInner {
    root: Gfx942HostBackingRootV1,
    identity: Identity,
    generation: DeviceKeyV1,
    account: ResourceCreditAccountV1,
    session: ResourceCreditAccountV1,
}

impl Drop for RequestInner {
    fn drop(&mut self) {
        let usage = self.account.usage();
        let session = self.session.usage();
        if usage.poisoned
            || usage.reserved_records != 0
            || usage.retained_records != 0
            || usage.quarantined_records != 0
            || usage.used != ResourceVectorV1::ZERO
            || session.poisoned
            || session.quarantined_records != 0
        {
            self.root.retain_quarantine();
        }
    }
}

/// Exact request leaf and typed canonical registry custody, without raw extraction.
/// Clones and every outstanding reservation/credit preserve the same binding.
#[derive(Clone)]
pub struct Gfx942RequestAccountV1(Arc<RequestInner>);

impl Gfx942RequestAccountV1 {
    pub(crate) fn is_session_live_v1(&self) -> bool {
        let usage = self.session_usage_v1();
        !usage.poisoned && usage.quarantined_records == 0
    }

    pub fn matches_device_v1(&self, device: &CheckedGfx942XnackMinusDevice) -> bool {
        self.0.identity == Identity::of(device)
            && self.0.generation == device.model_admission().model_key()
    }

    pub fn usage_v1(&self) -> ResourceCreditUsageV1 {
        self.0.account.usage()
    }

    pub fn session_usage_v1(&self) -> ResourceCreditUsageV1 {
        self.0.session.usage()
    }

    /// Zero bytes still consumes one accounting record, not a native allocation.
    pub fn reserve_v1(
        &self,
        bytes: u64,
    ) -> Result<Gfx942RequestReservationV1, ResourceCreditErrorV1> {
        Ok(Gfx942RequestReservationV1 {
            inner: self.0.account.reserve(request_charge(bytes))?,
            account: self.clone(),
        })
    }

    /// Atomic whole-roster admission through all ancestors. Each member is one request.
    pub fn reserve_batch_v1(
        &self,
        bytes: &[u64],
    ) -> Result<Gfx942RequestReservationsV1, ResourceCreditErrorV1> {
        if bytes.is_empty()
            || bytes.len() > fe2o3_resource_accounting::MAX_RESOURCE_CREDIT_BATCH_MEMBERS_V1
        {
            return Err(ResourceCreditErrorV1::InvalidRecordCapacity);
        }
        let mut charges = Vec::new();
        charges
            .try_reserve_exact(bytes.len())
            .map_err(|_| ResourceCreditErrorV1::AllocationFailed)?;
        charges.extend(bytes.iter().copied().map(request_charge));
        Ok(Gfx942RequestReservationsV1 {
            inner: self
                .0
                .account
                .reserve_batch(&charges)?
                .into_vec()
                .into_iter(),
            account: self.clone(),
        })
    }

    /// Borrowed accounting observation, not native allocation/disposal authority.
    pub fn matches_retained_charge_v1(&self, credit: &Gfx942RetainedRequestV1, bytes: u64) -> bool {
        Arc::ptr_eq(&self.0, &credit.account.0)
            && self
                .0
                .account
                .matches_retained_charge_v1(&credit.inner, request_charge(bytes))
    }
}

fn request_charge(bytes: u64) -> ResourceVectorV1 {
    ResourceVectorV1::ZERO
        .with(ResourceKindV1::RequestedAllocationBytes, bytes)
        .with(ResourceKindV1::AllocationRecords, 1)
}

/// Owns an atomic request roster without allocating another token array.
/// Unconsumed reservations cancel before the typed registry owner is dropped.
#[must_use = "consume reservations or drop to cancel the remaining roster"]
pub struct Gfx942RequestReservationsV1 {
    inner: std::vec::IntoIter<ResourceReservationV1>,
    account: Gfx942RequestAccountV1,
}

impl Iterator for Gfx942RequestReservationsV1 {
    type Item = Gfx942RequestReservationV1;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|inner| Gfx942RequestReservationV1 {
            inner,
            account: self.account.clone(),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for Gfx942RequestReservationsV1 {}
impl std::iter::FusedIterator for Gfx942RequestReservationsV1 {}

/// Move-only request reservation. Drop cancels before releasing typed root custody.
#[must_use = "retain for allocation or drop to cancel before effects"]
pub struct Gfx942RequestReservationV1 {
    inner: ResourceReservationV1,
    account: Gfx942RequestAccountV1,
}

impl Gfx942RequestReservationV1 {
    pub fn retain(self) -> Gfx942RetainedRequestV1 {
        let Self { inner, account } = self;
        Gfx942RetainedRequestV1 {
            inner: inner.retain(),
            account,
        }
    }
}

/// Retained request and canonical registry owner. Drop quarantines, never refunds.
/// The generic credit drops before the account so cold quarantine retains the registry.
///
/// ```compile_fail
/// fn duplicate(credit: fe2o3_kfd::Gfx942RetainedRequestV1) {
///     let _copy = credit.clone();
/// }
/// ```
/// ```compile_fail
/// fn refund_twice(credit: fe2o3_kfd::Gfx942RetainedRequestV1) {
///     credit.release_after_disposal().unwrap();
///     credit.release_after_disposal().unwrap();
/// }
/// ```
/// ```compile_fail
/// fn extract(credit: fe2o3_kfd::Gfx942RetainedRequestV1) {
///     let _raw = credit.inner;
/// }
/// ```
#[must_use = "release only after definite rejection/disposal; otherwise quarantine"]
pub struct Gfx942RetainedRequestV1 {
    inner: RetainedResourceCreditsV1,
    account: Gfx942RequestAccountV1,
}

impl Gfx942RetainedRequestV1 {
    pub fn release_after_rejection(self) -> Result<(), ResourceCreditErrorV1> {
        let Self {
            inner,
            account: _account,
        } = self;
        inner.release_after_rejection()
    }

    pub fn release_after_disposal(self) -> Result<(), ResourceCreditErrorV1> {
        let Self {
            inner,
            account: _account,
        } = self;
        inner.release_after_disposal()
    }

    pub fn quarantine(self) {
        let Self {
            inner,
            account: _account,
        } = self;
        inner.quarantine();
    }
}

#[cfg(test)]
pub(crate) mod tests;
