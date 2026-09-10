//! Context-request accounting; native backing residency is a separate adapter.

use super::*;
use crate::resource_credits::{
    RuntimeResourceCreditAccountV1, RuntimeResourceCreditErrorV1, RuntimeResourceCreditUsageV1,
    RuntimeResourceKindV1, RuntimeResourceVectorV1, RuntimeRetainedResourceCreditsV1,
};

#[derive(Default)]
pub(super) struct ContextAllocationAdmissionV1 {
    accounts: HashMap<RuntimeDeviceIdV1, RuntimeResourceCreditAccountV1>,
    retained: HashMap<RuntimeAllocationIdV1, RuntimeRetainedResourceCreditsV1>,
}

impl ContextAllocationAdmissionV1 {
    pub(super) fn retained_records(&self) -> usize {
        self.accounts
            .values()
            .map(|account| {
                let usage = account.usage();
                usage.reserved_records + usage.retained_records + usage.quarantined_records
            })
            .sum()
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
                    .reserve(
                        RuntimeResourceVectorV1::ZERO
                            .with(RuntimeResourceKindV1::RequestedAllocationBytes, bytes)
                            .with(RuntimeResourceKindV1::AllocationRecords, 1),
                    )
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
        if let Some(credits) = self.retained.remove(&id) {
            credits.release_after_disposal()?;
        }
        Ok(())
    }

    fn quarantine(&mut self, id: RuntimeAllocationIdV1) {
        if let Some(credits) = self.retained.remove(&id) {
            credits.quarantine();
        }
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    /// Limits allocation requests on one exact Context device before backend entry.
    ///
    /// Both requested bytes and allocation records are reserved atomically. A
    /// successful allocation retains its charge through completion and every
    /// failed release, until the backend confirms actual disposal. An uncertain
    /// allocation attempt retains quarantined credits even without a returned
    /// handle. Only definite pre-mutation rejection refunds that attempt.
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
            if usage.poisoned
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
            self.terminal = true;
            panic!("allocation credit owner invariant failed after disposal: {error:?}");
        }
    }

    pub(super) fn release_admitted_allocation_backend_v1(
        &mut self,
        id: RuntimeAllocationIdV1,
        backend_allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<B::Error>> {
        if !self.allocation_admission.retained.contains_key(&id) {
            return self.backend.release_allocation_v1(backend_allocation);
        }
        match catch_unwind(AssertUnwindSafe(|| {
            self.backend.release_allocation_v1(backend_allocation)
        })) {
            Ok(result) => {
                if matches!(&result, Err(RuntimeBackendFailureV1::Terminal(_))) {
                    self.terminal = true;
                    self.allocation_admission.quarantine(id);
                }
                result
            }
            Err(payload) => {
                self.terminal = true;
                self.allocation_admission.quarantine(id);
                std::panic::resume_unwind(payload);
            }
        }
    }
}
