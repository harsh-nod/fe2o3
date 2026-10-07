//! Exact runtime-device branding over the shared transactional credit engine.
//!
//! The shared account enforces only supplied vectors. This wrapper preserves
//! Context/device association without duplicating a ledger or exposing tokens as
//! native authority. General accounts retain the optional request profile;
//! composed accounts preserve a KFD-minted request leaf and its root custody.
//! Native backing costs are charged by sibling adapters, not by this wrapper.
//! Neither profile measures whole-process memory.

use crate::RuntimeDeviceIdV1;
use fe2o3_resource_accounting::{
    ResourceCreditAccountV1, ResourceReservationV1, RetainedResourceCreditsV1,
};

pub use fe2o3_resource_accounting::{
    MAX_RESOURCE_CREDIT_RECORDS_V1 as MAX_RUNTIME_RESOURCE_CREDIT_RECORDS_V1,
    ResourceCreditErrorV1 as RuntimeResourceCreditErrorV1, ResourceKindV1 as RuntimeResourceKindV1,
    ResourceVectorV1 as RuntimeResourceVectorV1,
};
pub(crate) enum RuntimeResourceReservationV1 {
    General(ResourceReservationV1),
    Composed(fe2o3_kfd::Gfx942RequestReservationV1),
}

impl RuntimeResourceReservationV1 {
    pub(crate) fn retain(self) -> RuntimeRetainedResourceCreditsV1 {
        match self {
            Self::General(credit) => RuntimeRetainedResourceCreditsV1::General(credit.retain()),
            Self::Composed(credit) => RuntimeRetainedResourceCreditsV1::Composed(credit.retain()),
        }
    }
}

pub(crate) enum RuntimeRetainedResourceCreditsV1 {
    General(RetainedResourceCreditsV1),
    Composed(fe2o3_kfd::Gfx942RetainedRequestV1),
}

impl RuntimeRetainedResourceCreditsV1 {
    pub(crate) fn release_after_rejection(self) -> Result<(), RuntimeResourceCreditErrorV1> {
        match self {
            Self::General(credit) => credit.release_after_rejection(),
            Self::Composed(credit) => credit.release_after_rejection(),
        }
    }

    pub(crate) fn release_after_disposal(self) -> Result<(), RuntimeResourceCreditErrorV1> {
        match self {
            Self::General(credit) => credit.release_after_disposal(),
            Self::Composed(credit) => credit.release_after_disposal(),
        }
    }

    pub(crate) fn quarantine(self) {
        match self {
            Self::General(credit) => credit.quarantine(),
            Self::Composed(credit) => credit.quarantine(),
        }
    }
}

pub(crate) enum RuntimeResourceReservationsV1 {
    General(std::vec::IntoIter<ResourceReservationV1>),
    Composed(fe2o3_kfd::Gfx942RequestReservationsV1),
}

impl Iterator for RuntimeResourceReservationsV1 {
    type Item = RuntimeResourceReservationV1;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::General(credits) => credits.next().map(RuntimeResourceReservationV1::General),
            Self::Composed(credits) => credits.next().map(RuntimeResourceReservationV1::Composed),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::General(credits) => credits.size_hint(),
            Self::Composed(credits) => credits.size_hint(),
        }
    }
}

impl ExactSizeIterator for RuntimeResourceReservationsV1 {}
impl std::iter::FusedIterator for RuntimeResourceReservationsV1 {}

pub(crate) fn request_charge(bytes: u64) -> RuntimeResourceVectorV1 {
    fe2o3_runtime_model::r67_requested_allocation_charge_v1(bytes)
}

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
    inner: RuntimeResourceCreditAccountInnerV1,
}

#[derive(Clone)]
enum RuntimeResourceCreditAccountInnerV1 {
    General(ResourceCreditAccountV1),
    Composed(Box<crate::RuntimeAllocationDeviceAdmissionV1>),
}

include!("retained_credit_dispatch_body.rs");

impl RuntimeResourceCreditAccountV1 {
    pub(crate) fn composed(
        device: RuntimeDeviceIdV1,
        admission: crate::RuntimeAllocationDeviceAdmissionV1,
    ) -> Self {
        Self {
            device,
            inner: RuntimeResourceCreditAccountInnerV1::Composed(Box::new(admission)),
        }
    }

    pub(crate) fn composed_admission(&self) -> Option<&crate::RuntimeAllocationDeviceAdmissionV1> {
        match &self.inner {
            RuntimeResourceCreditAccountInnerV1::Composed(admission) => Some(admission),
            RuntimeResourceCreditAccountInnerV1::General(_) => None,
        }
    }

    pub(crate) fn matches_retained_charge_v1(
        &self,
        device: RuntimeDeviceIdV1,
        credits: &RuntimeRetainedResourceCreditsV1,
        expected: RuntimeResourceVectorV1,
    ) -> bool {
        runtime_retained_credit_dispatch_body_v1!(self, device, credits, expected)
    }

    pub(crate) fn in_domain(
        device: RuntimeDeviceIdV1,
        parent: &ResourceCreditAccountV1,
        capacity: RuntimeResourceVectorV1,
        max_reservations: usize,
    ) -> Result<Self, RuntimeResourceCreditErrorV1> {
        Ok(Self {
            device,
            inner: RuntimeResourceCreditAccountInnerV1::General(
                parent.new_child(capacity, max_reservations)?,
            ),
        })
    }

    pub(crate) fn is_domain(&self) -> bool {
        match &self.inner {
            RuntimeResourceCreditAccountInnerV1::General(account) => account.root_usage().is_some(),
            RuntimeResourceCreditAccountInnerV1::Composed(_) => true,
        }
    }

    pub(crate) fn new(
        device: RuntimeDeviceIdV1,
        capacity: RuntimeResourceVectorV1,
        max_reservations: usize,
    ) -> Result<Self, RuntimeResourceCreditErrorV1> {
        Ok(Self {
            device,
            inner: RuntimeResourceCreditAccountInnerV1::General(ResourceCreditAccountV1::new(
                capacity,
                max_reservations,
            )?),
        })
    }

    pub(crate) fn usage(&self) -> RuntimeResourceCreditUsageV1 {
        let usage = match &self.inner {
            RuntimeResourceCreditAccountInnerV1::General(account) => account.usage(),
            RuntimeResourceCreditAccountInnerV1::Composed(admission) => {
                admission.account().usage_v1()
            }
        };
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
        match &self.inner {
            RuntimeResourceCreditAccountInnerV1::General(account) => account
                .reserve(charge)
                .map(RuntimeResourceReservationV1::General),
            RuntimeResourceCreditAccountInnerV1::Composed(admission) => {
                let bytes = charge.get(RuntimeResourceKindV1::RequestedAllocationBytes);
                if charge != request_charge(bytes) || !admission.is_live() {
                    return Err(RuntimeResourceCreditErrorV1::Invariant);
                }
                admission
                    .account()
                    .reserve_v1(bytes)
                    .map(RuntimeResourceReservationV1::Composed)
            }
        }
    }

    pub(crate) fn reserve_requests(
        &self,
        bytes: &[u64],
    ) -> Result<RuntimeResourceReservationsV1, RuntimeResourceCreditErrorV1> {
        match &self.inner {
            RuntimeResourceCreditAccountInnerV1::General(account) => {
                let mut charges = Vec::new();
                charges
                    .try_reserve_exact(bytes.len())
                    .map_err(|_| RuntimeResourceCreditErrorV1::AllocationFailed)?;
                charges.extend(bytes.iter().copied().map(request_charge));
                account.reserve_batch(&charges).map(|credits| {
                    RuntimeResourceReservationsV1::General(credits.into_vec().into_iter())
                })
            }
            RuntimeResourceCreditAccountInnerV1::Composed(admission) => {
                if !admission.is_live() {
                    return Err(RuntimeResourceCreditErrorV1::Invariant);
                }
                admission
                    .account()
                    .reserve_batch_v1(bytes)
                    .map(RuntimeResourceReservationsV1::Composed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RuntimeResourceKindV1 as K;

    #[test]
    fn request_account_variants_keep_legacy_metadata_compact() {
        assert!(
            core::mem::size_of::<RuntimeResourceCreditAccountInnerV1>()
                <= core::mem::size_of::<ResourceCreditAccountV1>()
                    + core::mem::align_of::<ResourceCreditAccountV1>()
        );
    }

    fn charge(bytes: u64) -> RuntimeResourceVectorV1 {
        RuntimeResourceVectorV1::ZERO
            .with(K::RequestedAllocationBytes, bytes)
            .with(K::AllocationRecords, 1)
    }

    #[test]
    fn request_profile_wrapper_and_live_record_reject_every_contaminated_coordinate() {
        let kinds = [
            K::LogicalPayloadBytes,
            K::RequestedAllocationBytes,
            K::ResidentHostAllocationBytes,
            K::ResidentDeviceAllocationBytes,
            K::ExecutableHostImageBytes,
            K::ExecutableDeviceBytes,
            K::ControlResidentBytes,
            K::QueueResidentBytes,
            K::SignalResidentBytes,
            K::KernargResidentBytes,
            K::QueueSlots,
            K::SignalSlots,
            K::KernargSlots,
            K::OperationSlots,
            K::ReplyBytes,
            K::ReplyCells,
            K::TerminalRecordBytes,
            K::QuarantineBookkeepingBytes,
            K::AllocationRecords,
        ];
        let device = crate::context::resource_credit_test_device_v1();
        for bytes in [0, 1, u64::MAX] {
            assert_eq!(request_charge(bytes), charge(bytes));
            let account = RuntimeResourceCreditAccountV1::new(device, charge(bytes), 1).unwrap();
            let retained = account.reserve(request_charge(bytes)).unwrap().retain();
            assert!(account.matches_retained_charge_v1(device, &retained, charge(bytes)));
            for kind in kinds {
                let contaminated =
                    charge(bytes).with(kind, charge(bytes).get(kind).wrapping_add(1));
                assert!(!account.matches_retained_charge_v1(device, &retained, contaminated));
            }
            retained.release_after_disposal().unwrap();
            assert_eq!(account.usage().used, RuntimeResourceVectorV1::ZERO);
        }
    }

    #[test]
    fn retained_charge_runtime_wrapper_preserves_device_and_exact_account() {
        let device = crate::context::resource_credit_test_device_v1();
        let first = RuntimeResourceCreditAccountV1::new(device, charge(8), 1).unwrap();
        let second = RuntimeResourceCreditAccountV1::new(device, charge(8), 1).unwrap();
        let credit = first.reserve(charge(8)).unwrap().retain();
        assert!(first.matches_retained_charge_v1(device, &credit, charge(8)));
        assert!(
            first
                .clone()
                .matches_retained_charge_v1(device, &credit, charge(8))
        );
        assert!(!second.matches_retained_charge_v1(device, &credit, charge(8)));
        assert!(!first.matches_retained_charge_v1(device, &credit, charge(7)));
        credit.release_after_disposal().unwrap();
    }

    #[test]
    fn retained_dispatch_general_independent_and_domain_preserve_arbitrary_vectors() {
        let device = crate::context::resource_credit_test_device_v1();
        for domain in [false, true] {
            for bytes in [0, 1, u64::MAX] {
                let expected = charge(bytes).with(RuntimeResourceKindV1::ReplyCells, 2);
                let root = domain.then(|| {
                    ResourceCreditAccountV1::new_root(
                        expected.with(RuntimeResourceKindV1::ControlResidentBytes, 1 << 20),
                        4,
                        4,
                    )
                    .unwrap()
                });
                let account = match &root {
                    Some(root) => {
                        RuntimeResourceCreditAccountV1::in_domain(device, root, expected, 1)
                            .unwrap()
                    }
                    None => RuntimeResourceCreditAccountV1::new(device, expected, 1).unwrap(),
                };
                let retained = account.reserve(expected).unwrap().retain();
                let before = account.usage();
                let root_before = root.as_ref().map(ResourceCreditAccountV1::usage);
                for _ in 0..3 {
                    assert!(account.matches_retained_charge_v1(device, &retained, expected));
                    assert!(!account.matches_retained_charge_v1(device, &retained, charge(bytes)));
                    assert!(!account.matches_retained_charge_v1(
                        device,
                        &retained,
                        expected.with(RuntimeResourceKindV1::ReplyCells, 3)
                    ));
                }
                assert_eq!(account.usage(), before);
                assert_eq!(
                    root.as_ref().map(ResourceCreditAccountV1::usage),
                    root_before
                );
                retained.release_after_disposal().unwrap();
                assert_eq!(account.usage().used, RuntimeResourceVectorV1::ZERO);
            }
        }
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
