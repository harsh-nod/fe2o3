//! Exact runtime-device branding over the shared transactional credit engine.
//!
//! The shared account enforces only supplied vectors. This wrapper preserves
//! Context/device association without duplicating a ledger or exposing tokens as
//! native authority. Native cost extraction, parent/global ceilings and aggregate
//! quarantine remain separate work.

use crate::RuntimeDeviceIdV1;
use fe2o3_resource_accounting::ResourceCreditAccountV1;

pub use fe2o3_resource_accounting::{
    MAX_RESOURCE_CREDIT_RECORDS_V1 as MAX_RUNTIME_RESOURCE_CREDIT_RECORDS_V1,
    ResourceCreditErrorV1 as RuntimeResourceCreditErrorV1, ResourceKindV1 as RuntimeResourceKindV1,
    ResourceVectorV1 as RuntimeResourceVectorV1,
};
pub(crate) use fe2o3_resource_accounting::{
    ResourceReservationV1 as RuntimeResourceReservationV1,
    RetainedResourceCreditsV1 as RuntimeRetainedResourceCreditsV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeResourceCreditUsageV1 {
    pub device: RuntimeDeviceIdV1,
    pub capacity: RuntimeResourceVectorV1,
    pub used: RuntimeResourceVectorV1,
    pub reserved_records: usize,
    pub retained_records: usize,
    pub quarantined_records: usize,
    pub record_capacity: usize,
    pub poisoned: bool,
}

#[derive(Clone)]
pub(crate) struct RuntimeResourceCreditAccountV1 {
    device: RuntimeDeviceIdV1,
    inner: ResourceCreditAccountV1,
}

impl RuntimeResourceCreditAccountV1 {
    pub(crate) fn new(
        device: RuntimeDeviceIdV1,
        capacity: RuntimeResourceVectorV1,
        max_reservations: usize,
    ) -> Result<Self, RuntimeResourceCreditErrorV1> {
        Ok(Self {
            device,
            inner: ResourceCreditAccountV1::new(capacity, max_reservations)?,
        })
    }

    pub(crate) fn usage(&self) -> RuntimeResourceCreditUsageV1 {
        let usage = self.inner.usage();
        RuntimeResourceCreditUsageV1 {
            device: self.device,
            capacity: usage.capacity,
            used: usage.used,
            reserved_records: usage.reserved_records,
            retained_records: usage.retained_records,
            quarantined_records: usage.quarantined_records,
            record_capacity: usage.record_capacity,
            poisoned: usage.poisoned,
        }
    }

    pub(crate) fn reserve(
        &self,
        charge: RuntimeResourceVectorV1,
    ) -> Result<RuntimeResourceReservationV1, RuntimeResourceCreditErrorV1> {
        self.inner.reserve(charge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RuntimeResourceKindV1 as K;

    fn charge(bytes: u64) -> RuntimeResourceVectorV1 {
        RuntimeResourceVectorV1::ZERO
            .with(K::RequestedAllocationBytes, bytes)
            .with(K::AllocationRecords, 1)
    }

    #[test]
    fn wrapper_preserves_exact_device_brand_and_clones_share_one_ledger() {
        let device = crate::context::resource_credit_test_device_v1();
        let account = RuntimeResourceCreditAccountV1::new(device, charge(8), 1).unwrap();
        let cloned = account.clone();
        let reservation = cloned.reserve(charge(7)).unwrap();
        assert_eq!(
            account.usage(),
            RuntimeResourceCreditUsageV1 {
                device,
                capacity: charge(8),
                used: charge(7),
                reserved_records: 1,
                retained_records: 0,
                quarantined_records: 0,
                record_capacity: 1,
                poisoned: false,
            }
        );
        assert_eq!(cloned.usage(), account.usage());
        let retained = reservation.retain();
        assert_eq!(account.usage().device, device);
        assert_eq!(account.usage().retained_records, 1);
        retained.release_after_disposal().unwrap();
        assert_eq!(account.usage().used, RuntimeResourceVectorV1::ZERO);
        assert_eq!(cloned.usage(), account.usage());
    }

    #[test]
    fn equal_device_labels_do_not_merge_distinct_account_or_refund_identity() {
        let device = crate::context::resource_credit_test_device_v1();
        let first = RuntimeResourceCreditAccountV1::new(device, charge(8), 1).unwrap();
        let second = RuntimeResourceCreditAccountV1::new(device, charge(8), 1).unwrap();
        let a = first.reserve(charge(8)).unwrap().retain();
        let b = second.reserve(charge(8)).unwrap().retain();
        assert_eq!(first.usage(), second.usage());
        a.release_after_rejection().unwrap();
        assert_eq!(first.usage().used, RuntimeResourceVectorV1::ZERO);
        assert_eq!(second.usage().used, charge(8));
        assert_eq!(second.usage().retained_records, 1);
        b.release_after_disposal().unwrap();
        assert_eq!(first.usage(), second.usage());
    }

    #[test]
    fn public_aliases_and_error_text_remain_compatible() {
        let capacity: fe2o3_resource_accounting::ResourceVectorV1 = charge(8);
        let device = crate::context::resource_credit_test_device_v1();
        assert_eq!(
            MAX_RUNTIME_RESOURCE_CREDIT_RECORDS_V1,
            fe2o3_resource_accounting::MAX_RESOURCE_CREDIT_RECORDS_V1
        );
        let error = match RuntimeResourceCreditAccountV1::new(device, capacity, 0) {
            Ok(_) => panic!("zero record capacity must reject"),
            Err(error) => error,
        };
        assert_eq!(error, RuntimeResourceCreditErrorV1::InvalidRecordCapacity);
        assert_eq!(
            error.to_string(),
            "runtime resource credit error: InvalidRecordCapacity"
        );
        let shared: fe2o3_resource_accounting::ResourceCreditErrorV1 = error;
        assert_eq!(shared, error);
    }
}
