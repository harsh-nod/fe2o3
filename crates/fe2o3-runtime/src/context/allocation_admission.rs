//! Context-request accounting; native backing residency is a separate adapter.

use super::*;
use crate::resource_credits::{
    RuntimeResourceCreditAccountV1, RuntimeResourceCreditErrorV1, RuntimeResourceCreditUsageV1,
    RuntimeResourceKindV1, RuntimeResourceVectorV1, RuntimeRetainedResourceCreditsV1,
    request_charge,
};

#[derive(Default)]
pub(super) struct ContextAllocationAdmissionV1 {
    accounts: HashMap<RuntimeDeviceIdV1, RuntimeResourceCreditAccountV1>,
    retained: HashMap<RuntimeAllocationIdV1, RuntimeRetainedResourceCreditsV1>,
    #[cfg(test)]
    reject_disposal: Option<RuntimeAllocationIdV1>,
}

impl ContextAllocationAdmissionV1 {
    pub(super) fn from_profile(
        devices: &[RuntimeDeviceV1],
        profile: RuntimeAllocationAdmissionProfileV1,
    ) -> Result<Self, RuntimeValidationErrorV1> {
        let RuntimeAllocationAdmissionProfileV1::Required(entries) = profile else {
            return Ok(Self::default());
        };
        if entries.len() != devices.len() || entries.len() > MAX_RUNTIME_DEVICES_V1 {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        let mut admission = Self::default();
        admission
            .accounts
            .try_reserve(entries.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        for entry in entries {
            let device = devices
                .iter()
                .find(|device| device.backend_device == entry.backend_device_v1())
                .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
            if admission.accounts.contains_key(&device.id)
                || admission.accounts.values().any(|account| {
                    account.composed_admission().is_some_and(|installed| {
                        installed.account().shares_account_with_v1(entry.account())
                    })
                })
                || !entry.is_live()
            {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
            }
            admission.accounts.insert(
                device.id,
                RuntimeResourceCreditAccountV1::composed(device.id, entry),
            );
        }
        Ok(admission)
    }

    pub(super) fn witness<'a>(
        &'a self,
        device: RuntimeDeviceIdV1,
        credits: Option<&'a RuntimeRetainedResourceCreditsV1>,
        bytes: u64,
    ) -> Result<Option<RuntimeAllocationRequestWitnessV1<'a>>, RuntimeValidationErrorV1> {
        let Some(account) = self.accounts.get(&device) else {
            return Ok(None);
        };
        let Some(admission) = account.composed_admission() else {
            return Ok(None);
        };
        let Some(credits) = credits else {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        };
        if !admission.is_live()
            || !account.matches_retained_charge_v1(device, credits, request_charge(bytes))
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        let RuntimeRetainedResourceCreditsV1::Composed(credit) = credits else {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        };
        Ok(Some(RuntimeAllocationRequestWitnessV1::new(
            device, admission, credit, bytes,
        )))
    }
    #[cfg(test)]
    pub(in crate::context) fn swap_retained_for_test_v1(
        &mut self,
        first: RuntimeAllocationIdV1,
        second: RuntimeAllocationIdV1,
    ) {
        let [Some(first), Some(second)] = self.retained.get_disjoint_mut([&first, &second]) else {
            panic!("test retained credits");
        };
        core::mem::swap(first, second);
    }

    #[cfg(test)]
    pub(in crate::context) fn retained_for_test_v1(
        &mut self,
        id: RuntimeAllocationIdV1,
    ) -> &mut RuntimeRetainedResourceCreditsV1 {
        self.retained.get_mut(&id).expect("test retained credit")
    }

    #[cfg(test)]
    pub(in crate::context) fn reject_disposal_for_test_v1(&mut self, id: RuntimeAllocationIdV1) {
        self.reject_disposal = Some(id);
    }

    pub(super) fn prepare_roster(
        &mut self,
        device: RuntimeDeviceIdV1,
        bytes: &[u64],
    ) -> Result<
        Option<crate::resource_credits::RuntimeResourceReservationsV1>,
        RuntimeResourceCreditErrorV1,
    > {
        let Some(account) = self.accounts.get(&device) else {
            return Ok(None);
        };
        self.retained
            .try_reserve(bytes.len())
            .map_err(|_| RuntimeResourceCreditErrorV1::AllocationFailed)?;
        account.reserve_requests(bytes).map(Some)
    }

    pub(super) fn retained_records(&self) -> usize {
        self.accounts
            .values()
            .map(|account| {
                let usage = account.usage();
                usage.reserved_records + usage.retained_records + usage.quarantined_records
            })
            .sum()
    }

    pub(super) fn has_expected_credit(
        &self,
        id: RuntimeAllocationIdV1,
        device: RuntimeDeviceIdV1,
        byte_len: u64,
    ) -> bool {
        match (self.accounts.get(&device), self.retained.get(&id)) {
            (None, None) => true,
            (Some(account), Some(credits)) => {
                account.matches_retained_charge_v1(device, credits, request_charge(byte_len))
            }
            _ => false,
        }
    }

    pub(super) fn prepare_registry(
        &mut self,
        device: RuntimeDeviceIdV1,
    ) -> Result<bool, RuntimeResourceCreditErrorV1> {
        let configured = self.accounts.contains_key(&device);
        if configured {
            self.retained
                .try_reserve(1)
                .map_err(|_| RuntimeResourceCreditErrorV1::AllocationFailed)?;
        }
        Ok(configured)
    }

    pub(super) fn reserve(
        &self,
        device: RuntimeDeviceIdV1,
        bytes: u64,
    ) -> Result<Option<RuntimeRetainedResourceCreditsV1>, RuntimeResourceCreditErrorV1> {
        self.accounts
            .get(&device)
            .map(|account| {
                account
                    .reserve(request_charge(bytes))
                    .map(|reservation| reservation.retain())
            })
            .transpose()
    }

    pub(super) fn attach(
        &mut self,
        id: RuntimeAllocationIdV1,
        credits: Option<RuntimeRetainedResourceCreditsV1>,
    ) {
        if let Some(credits) = credits {
            assert!(self.retained.insert(id, credits).is_none());
        }
    }

    pub(super) fn release_disposed(
        &mut self,
        id: RuntimeAllocationIdV1,
    ) -> Result<(), RuntimeResourceCreditErrorV1> {
        #[cfg(test)]
        if self.reject_disposal == Some(id) {
            self.reject_disposal = None;
            return Err(RuntimeResourceCreditErrorV1::Invariant);
        }
        if let Some(credits) = self.retained.remove(&id) {
            credits.release_after_disposal()?;
        }
        Ok(())
    }

    pub(in crate::context) fn quarantine(&mut self, id: RuntimeAllocationIdV1) {
        if let Some(credits) = self.retained.remove(&id) {
            credits.quarantine();
        }
    }
}

#[cfg(test)]
mod tests;

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    /// Limits allocation requests on one exact Context device before backend entry.
    ///
    /// Both requested bytes and allocation records are reserved atomically. A
    /// successful allocation retains its charge through completion and every
    /// failed release, until the backend confirms actual disposal. An uncertain
    /// allocation attempt retains quarantined credits even without a returned
    /// handle. Definite pre-mutation rejection or an explicit allocation-specific
    /// settled no-owner outcome refunds that attempt; generic quiescence does not.
    ///
    /// This opt-in profile does not account padded native backing, cached pools,
    /// executable/control residency, allocator overhead or arbitrary engine
    /// payloads. Unintegrated resource-vector fields are not residency readings.
    /// The default Context behavior is unchanged. Limits may be installed or
    /// replaced only with no live allocations or retained credits on that device.
    pub fn configure_allocation_admission_v1(
        &mut self,
        device: RuntimeDeviceIdV1,
        requested_bytes: u64,
        allocation_records: usize,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        self.device(device)?;
        if self
            .allocations
            .values()
            .any(|record| record.device == device)
        {
            return Err(RuntimeValidationErrorV1::ContextReserved.into());
        }
        if let Some(account) = self.allocation_admission.accounts.get(&device) {
            let usage = account.usage();
            if account.is_domain()
                || usage.poisoned
                || usage.used != RuntimeResourceVectorV1::ZERO
                || usage.reserved_records != 0
                || usage.retained_records != 0
                || usage.quarantined_records != 0
            {
                return Err(RuntimeValidationErrorV1::ContextReserved.into());
            }
        }
        let records =
            u64::try_from(allocation_records).map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let capacity = RuntimeResourceVectorV1::ZERO
            .with(
                RuntimeResourceKindV1::RequestedAllocationBytes,
                requested_bytes,
            )
            .with(RuntimeResourceKindV1::AllocationRecords, records);
        let account = RuntimeResourceCreditAccountV1::new(device, capacity, allocation_records)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        self.allocation_admission.accounts.insert(device, account);
        Ok(())
    }

    /// Attaches request admission to one immutable child of a shared accounting
    /// domain. All participating Contexts consume the same ancestor byte/record
    /// limits before backend entry, including charges quarantined by destroyed
    /// Contexts. This attachment cannot subsequently be replaced by local limits
    /// or another root, even after all allocations have been disposed.
    ///
    /// The parent must come from `ResourceCreditAccountV1::new_root` or its child.
    /// It is an accounting domain, not physical-device identity or native
    /// authority. This accounts requested bytes/records only, not earlier Context
    /// bootstrap, native backing, registry/output-box storage or all process
    /// memory. Other Context devices remain unconfigured unless explicitly attached.
    pub fn configure_allocation_admission_in_domain_v1(
        &mut self,
        device: RuntimeDeviceIdV1,
        parent: &fe2o3_resource_accounting::ResourceCreditAccountV1,
        requested_bytes: u64,
        allocation_records: usize,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        self.device(device)?;
        if self.allocation_admission.accounts.contains_key(&device)
            || self
                .allocations
                .values()
                .any(|record| record.device == device)
        {
            return Err(RuntimeValidationErrorV1::ContextReserved.into());
        }
        self.allocation_admission
            .accounts
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let records =
            u64::try_from(allocation_records).map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let capacity = RuntimeResourceVectorV1::ZERO
            .with(
                RuntimeResourceKindV1::RequestedAllocationBytes,
                requested_bytes,
            )
            .with(RuntimeResourceKindV1::AllocationRecords, records);
        let account =
            RuntimeResourceCreditAccountV1::in_domain(device, parent, capacity, allocation_records)
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        self.allocation_admission.accounts.insert(device, account);
        Ok(())
    }

    /// Returns retained request credits, including quarantine, even after terminal
    /// failure. This inspection grants no release or execution authority.
    pub fn allocation_admission_usage_v1(
        &self,
        device: RuntimeDeviceIdV1,
    ) -> Result<Option<RuntimeResourceCreditUsageV1>, RuntimeValidationErrorV1> {
        self.device(device)?;
        Ok(self
            .allocation_admission
            .accounts
            .get(&device)
            .map(RuntimeResourceCreditAccountV1::usage))
    }

    pub(super) fn dispose_allocation_credits_v1(&mut self, id: RuntimeAllocationIdV1) {
        if let Err(error) = self.allocation_admission.release_disposed(id) {
            self.quarantine_submission_writers_v1();
            panic!("allocation credit owner invariant failed after disposal: {error:?}");
        }
    }

    pub(super) fn release_admitted_allocation_backend_v1(
        &mut self,
        id: RuntimeAllocationIdV1,
        backend_allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<B::Error>> {
        if !self.allocation_admission.retained.contains_key(&id) && self.versions.is_none() {
            return self.backend.release_allocation_v1(backend_allocation);
        }
        match catch_unwind(AssertUnwindSafe(|| {
            self.backend.release_allocation_v1(backend_allocation)
        })) {
            Ok(result) => {
                if matches!(&result, Err(RuntimeBackendFailureV1::Terminal(_))) {
                    self.quarantine_submission_writers_v1();
                    self.allocation_admission.quarantine(id);
                }
                result
            }
            Err(payload) => {
                self.quarantine_submission_writers_v1();
                self.allocation_admission.quarantine(id);
                std::panic::resume_unwind(payload);
            }
        }
    }
}
