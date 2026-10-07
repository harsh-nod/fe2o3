//! Fixed host table payloads. Allocator overhead and the credit arena are excluded.

use super::{
    ResourceCreditAccountV1, ResourceCreditErrorV1, ResourceKindV1, ResourceVectorV1,
    RetainedResourceCreditsV1,
};
use core::ops::{Deref, DerefMut};

/// Checked requested payload, excluding allocator headers and allocator rounding.
pub fn host_metadata_table_payload_bytes_v1<T>(len: usize) -> Option<u64> {
    let bytes = len.checked_mul(core::mem::size_of::<T>())?;
    if bytes == 0 || bytes > isize::MAX as usize {
        return None;
    }
    u64::try_from(bytes).ok()
}

/// Fixed-length Rust metadata storage, optionally charged to a shared account.
///
/// This is not GPU-visible storage or native disposal authority. The table's
/// requested payload is charged as `ControlResidentBytes` before allocation or
/// initialization. A retained charge is refunded only after table destruction;
/// a panicking element destructor quarantines it. A leaked table keeps its debit.
/// Neither allocator overhead nor the account's own arena is covered here.
pub struct HostMetadataTableV1<T> {
    slots: Vec<T>,
    credits: Option<RetainedResourceCreditsV1>,
}

impl<T> HostMetadataTableV1<T> {
    pub fn try_new(
        len: usize,
        account: Option<&ResourceCreditAccountV1>,
        mut initialize: impl FnMut() -> T,
    ) -> Result<Self, ResourceCreditErrorV1> {
        Self::try_new_with_allocator(len, account, &mut initialize, |len| {
            let mut slots = Vec::new();
            slots
                .try_reserve_exact(len)
                .map_err(|_| ResourceCreditErrorV1::AllocationFailed)?;
            // Keep the charged Rust capacity exact; allocator-internal rounding
            // is outside this payload contract.
            if slots.capacity() != len {
                return Err(ResourceCreditErrorV1::AllocationFailed);
            }
            Ok(slots)
        })
    }

    fn try_new_with_allocator(
        len: usize,
        account: Option<&ResourceCreditAccountV1>,
        initialize: &mut impl FnMut() -> T,
        allocate: impl FnOnce(usize) -> Result<Vec<T>, ResourceCreditErrorV1>,
    ) -> Result<Self, ResourceCreditErrorV1> {
        let bytes = host_metadata_table_payload_bytes_v1::<T>(len)
            .ok_or(ResourceCreditErrorV1::AllocationFailed)?;
        let reservation = account
            .map(|account| {
                account.reserve(
                    ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
                )
            })
            .transpose()?;
        let mut slots = allocate(len)?;
        for _ in 0..len {
            slots.push(initialize());
        }
        Ok(Self {
            slots,
            credits: reservation.map(|reservation| reservation.retain()),
        })
    }

    /// Clones the exact ledger handle without duplicating the table or debit.
    /// Identity lives with the retained credit, not a second inline account copy.
    pub fn account(&self) -> Option<ResourceCreditAccountV1> {
        self.credits
            .as_ref()
            .map(RetainedResourceCreditsV1::account)
    }

    /// A copy is a distinct allocation and requires its own complete reservation.
    pub fn try_clone(&self) -> Result<Self, ResourceCreditErrorV1>
    where
        T: Clone,
    {
        let mut values = self.slots.iter();
        Self::try_new(self.slots.len(), self.account().as_ref(), || {
            values.next().expect("exact source table length").clone()
        })
    }
}

impl<T> Deref for HostMetadataTableV1<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.slots
    }
}

impl<T> DerefMut for HostMetadataTableV1<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.slots
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for HostMetadataTableV1<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HostMetadataTableV1")
            .field("slots", &self.slots)
            .field("accounted", &self.credits.is_some())
            .finish()
    }
}

impl<T: PartialEq> PartialEq for HostMetadataTableV1<T> {
    fn eq(&self, other: &Self) -> bool {
        self.slots == other.slots
            && match (self.account(), other.account()) {
                (None, None) => true,
                (Some(left), Some(right)) => left.shares_ledger_with(&right),
                _ => false,
            }
    }
}

impl<T: Eq> Eq for HostMetadataTableV1<T> {}

impl<T> Drop for HostMetadataTableV1<T> {
    fn drop(&mut self) {
        drop(core::mem::take(&mut self.slots));
        if let Some(credits) = self.credits.take() {
            let _ = credits.release_after_disposal();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn account(bytes: u64, records: usize) -> ResourceCreditAccountV1 {
        ResourceCreditAccountV1::new(
            ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
            records,
        )
        .unwrap()
    }

    #[test]
    fn checked_layout_rejects_empty_zst_and_overflow() {
        assert_eq!(
            host_metadata_table_payload_bytes_v1::<u64>(1024),
            Some(8192)
        );
        assert_eq!(host_metadata_table_payload_bytes_v1::<u64>(0), None);
        assert_eq!(host_metadata_table_payload_bytes_v1::<()>(1024), None);
        assert_eq!(
            host_metadata_table_payload_bytes_v1::<u64>(usize::MAX),
            None
        );
    }

    #[test]
    fn budget_rejection_precedes_allocation_and_initialization() {
        let account = account(8191, 4);
        let before = account.usage();
        let result = HostMetadataTableV1::<u64>::try_new_with_allocator(
            1024,
            Some(&account),
            &mut || panic!("rejected initialization"),
            |_| panic!("rejected allocation"),
        );
        assert!(matches!(result, Err(ResourceCreditErrorV1::Capacity)));
        assert_eq!(account.usage(), before);
    }

    #[test]
    fn allocation_failure_and_initializer_unwind_refund_reservation() {
        let account = account(8192, 4);
        let before = account.usage();
        let result = HostMetadataTableV1::<u64>::try_new_with_allocator(
            1024,
            Some(&account),
            &mut || panic!("allocation failed before initialization"),
            |_| Err(ResourceCreditErrorV1::AllocationFailed),
        );
        assert!(matches!(
            result,
            Err(ResourceCreditErrorV1::AllocationFailed)
        ));
        assert_eq!(account.usage(), before);
        let calls = AtomicUsize::new(0);
        assert!(
            std::panic::catch_unwind(|| {
                let _ = HostMetadataTableV1::try_new(1024, Some(&account), || {
                    assert_ne!(calls.fetch_add(1, Ordering::SeqCst), 7);
                    0_u64
                });
            })
            .is_err()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 8);
        assert_eq!(account.usage(), before);
    }

    #[test]
    fn independent_clones_are_charged_until_after_disposal() {
        let account = account(16384, 2);
        let table = HostMetadataTableV1::try_new(1024, Some(&account), || 7_u64).unwrap();
        assert_eq!(
            account
                .usage()
                .used
                .get(ResourceKindV1::ControlResidentBytes),
            8192
        );
        assert_eq!(account.usage().retained_records, 1);
        let copy = table.try_clone().unwrap();
        assert_eq!(
            account
                .usage()
                .used
                .get(ResourceKindV1::ControlResidentBytes),
            16384
        );
        assert!(matches!(
            table.try_clone(),
            Err(ResourceCreditErrorV1::Capacity)
        ));
        drop(table);
        assert_eq!(copy[1023], 7);
        assert_eq!(
            account
                .usage()
                .used
                .get(ResourceKindV1::ControlResidentBytes),
            8192
        );
        drop(copy);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(account.usage().retained_records, 0);
    }

    #[test]
    fn record_exhaustion_rejects_without_initialization() {
        let account = account(16384, 1);
        let table = HostMetadataTableV1::try_new(1024, Some(&account), || 0_u64).unwrap();
        let before = account.usage();
        assert!(matches!(
            HostMetadataTableV1::try_new(1024, Some(&account), || -> u64 {
                panic!("record rejection must precede initialization")
            }),
            Err(ResourceCreditErrorV1::RecordCapacity)
        ));
        assert_eq!(account.usage(), before);
        drop(table);
    }

    #[test]
    fn destructor_observes_live_charge_and_panic_quarantines_it() {
        struct Slot(ResourceCreditAccountV1);
        impl Drop for Slot {
            fn drop(&mut self) {
                assert_eq!(self.0.usage().retained_records, 1);
                panic!("slot destructor failed");
            }
        }
        let bytes = host_metadata_table_payload_bytes_v1::<Slot>(1).unwrap();
        let account = account(bytes, 1);
        let table =
            HostMetadataTableV1::try_new(1, Some(&account), || Slot(account.clone())).unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(table))).is_err());
        assert_eq!(
            account
                .usage()
                .used
                .get(ResourceKindV1::ControlResidentBytes),
            bytes
        );
        assert_eq!(account.usage().quarantined_records, 1);
    }
}
