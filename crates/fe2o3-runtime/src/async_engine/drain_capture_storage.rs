//! Owned capture destinations and results, charged by their actual slice extent.
//!
//! This account covers `ReplyBytes` and one ledger owner record. Reply-cell
//! admission is separate. Caller allocation precedes admission; allocator
//! overhead, account bootstrap and aggregate process ceilings are not covered.
//! These owners grant no source-read, quiescence or native-disposal authority.

use fe2o3_resource_accounting::{
    ResourceCreditAccountV1, ResourceCreditErrorV1, ResourceKindV1, ResourceReservationV1,
    ResourceVectorV1, RetainedResourceCreditsV1,
};
use std::fmt;

pub(super) const MAX_CAPTURE_BYTES_V1: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct CaptureBudgetV1 {
    account: ResourceCreditAccountV1,
}

impl CaptureBudgetV1 {
    pub(super) fn new(capacity: usize) -> Result<Self, ResourceCreditErrorV1> {
        if capacity > MAX_CAPTURE_BYTES_V1 {
            return Err(ResourceCreditErrorV1::Capacity);
        }
        Ok(Self {
            account: ResourceCreditAccountV1::new(
                ResourceVectorV1::ZERO.with(ResourceKindV1::ReplyBytes, capacity as u64),
                1,
            )?,
        })
    }

    pub(super) fn used_bytes(&self) -> usize {
        // Admission bounds this coordinate by MAX_CAPTURE_BYTES_V1.
        self.account.usage().used.get(ResourceKindV1::ReplyBytes) as usize
    }

    pub(super) fn reserve(
        &self,
        destination: Box<[u8]>,
    ) -> Result<CaptureReservationV1, (ResourceCreditErrorV1, Box<[u8]>)> {
        let bytes = match u64::try_from(destination.len()) {
            Ok(bytes) => bytes,
            Err(_) => return Err((ResourceCreditErrorV1::Capacity, destination)),
        };
        let charge = ResourceVectorV1::ZERO.with(ResourceKindV1::ReplyBytes, bytes);
        match self.account.reserve(charge) {
            Ok(credit) => Ok(CaptureReservationV1 {
                destination: Some(destination),
                credit: Some(credit),
            }),
            Err(error) => Err((error, destination)),
        }
    }
}

/// A temporary reservation that has not crossed the capture admission cutoff.
pub(super) struct CaptureReservationV1 {
    destination: Option<Box<[u8]>>,
    credit: Option<ResourceReservationV1>,
}

impl CaptureReservationV1 {
    /// Used only when pre-cutoff registration is rejected, returning caller custody.
    pub(super) fn into_unadmitted_destination(mut self) -> Box<[u8]> {
        let destination = self.destination.take().expect("unadmitted destination");
        drop(self.credit.take());
        destination
    }

    /// Called under the admission lock before installing capture and closing cutoff.
    pub(super) fn retain(mut self) -> RuntimeAsyncCapturedBytesV1 {
        let credit = self.credit.take().expect("unadmitted credit").retain();
        RuntimeAsyncCapturedBytesV1 {
            destination: self.destination.take(),
            credit: Some(credit),
        }
    }
}

impl Drop for CaptureReservationV1 {
    fn drop(&mut self) {
        drop(self.destination.take());
        drop(self.credit.take());
    }
}

/// An owned host capture result whose slice remains charged until disposal.
///
/// Moving this result out of a reply or dropping its engine does not return its
/// byte credit. Borrowing its contents grants no GPU completion or native source
/// authority. The charge covers the slice extent, not allocator overhead.
///
/// ```compile_fail
/// use fe2o3_runtime::RuntimeAsyncCapturedBytesV1;
/// fn duplicate(result: RuntimeAsyncCapturedBytesV1) {
///     let second = result.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_runtime::RuntimeAsyncCapturedBytesV1;
/// fn shed_charge(result: RuntimeAsyncCapturedBytesV1) -> Box<[u8]> {
///     result.into_boxed_slice()
/// }
/// ```
#[must_use = "the capture result owns its storage and retained byte credit"]
pub struct RuntimeAsyncCapturedBytesV1 {
    destination: Option<Box<[u8]>>,
    credit: Option<RetainedResourceCreditsV1>,
}

impl RuntimeAsyncCapturedBytesV1 {
    pub fn as_bytes(&self) -> &[u8] {
        self.destination.as_deref().expect("owned capture storage")
    }

    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    pub(super) fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.destination
            .as_deref_mut()
            .expect("owned capture storage")
    }
}

impl fmt::Debug for RuntimeAsyncCapturedBytesV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeAsyncCapturedBytesV1")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

fn dispose_then_refund<T>(storage: T, credit: RetainedResourceCreditsV1) {
    drop(storage);
    // A failed accounting transition retains/quarantines the debit in the core.
    let _ = credit.release_after_disposal();
}

impl Drop for RuntimeAsyncCapturedBytesV1 {
    fn drop(&mut self) {
        if let Some(credit) = self.credit.take() {
            dispose_then_refund(self.destination.take(), credit);
        } else {
            drop(self.destination.take());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_engine::{owned::Reply, reply_budget::ReplyBudgetV1};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::{Context, Poll, Waker};

    fn destination(bytes: usize) -> Box<[u8]> {
        vec![0; bytes].into_boxed_slice()
    }

    #[test]
    fn capture_capacity_and_record_arena_are_bounded() {
        let budget = CaptureBudgetV1::new(MAX_CAPTURE_BYTES_V1).unwrap();
        let usage = budget.account.usage();
        assert_eq!(usage.record_capacity, 1);
        assert_eq!(
            usage.capacity,
            ResourceVectorV1::ZERO.with(ResourceKindV1::ReplyBytes, MAX_CAPTURE_BYTES_V1 as u64)
        );
        assert!(matches!(
            CaptureBudgetV1::new(MAX_CAPTURE_BYTES_V1 + 1),
            Err(ResourceCreditErrorV1::Capacity)
        ));
    }

    #[test]
    fn actual_box_extent_is_charged_only_as_reply_bytes() {
        let budget = CaptureBudgetV1::new(16).unwrap();
        let mut bytes = Vec::with_capacity(64);
        bytes.resize(7, 4);
        let reservation = budget.reserve(bytes.into_boxed_slice()).unwrap();
        let usage = budget.account.usage();
        assert_eq!(
            usage.used,
            ResourceVectorV1::ZERO.with(ResourceKindV1::ReplyBytes, 7)
        );
        assert_eq!(usage.reserved_records, 1);
        assert_eq!(usage.retained_records, 0);
        assert_eq!(budget.used_bytes(), 7);
        drop(reservation);
        assert_eq!(budget.used_bytes(), 0);
    }

    #[test]
    fn capacity_failure_returns_exact_destination_without_changing_usage() {
        let budget = CaptureBudgetV1::new(8).unwrap();
        let bytes = vec![5; 9].into_boxed_slice();
        let address = bytes.as_ptr();
        let before = budget.account.usage();
        let (error, returned) = budget.reserve(bytes).err().unwrap();
        assert_eq!(error, ResourceCreditErrorV1::Capacity);
        assert_eq!(returned.as_ptr(), address);
        assert_eq!(&*returned, &[5; 9]);
        assert_eq!(budget.account.usage(), before);
    }

    #[test]
    fn record_exhaustion_is_shared_by_clones_and_failure_atomic() {
        let budget = CaptureBudgetV1::new(8).unwrap();
        let clone = budget.clone();
        let reservation = budget.reserve(destination(4)).unwrap();
        let before = budget.account.usage();
        let (error, _) = clone.reserve(destination(4)).err().unwrap();
        assert_eq!(error, ResourceCreditErrorV1::RecordCapacity);
        assert_eq!(clone.account.usage(), before);
        drop(reservation);
        drop(clone.reserve(destination(8)).unwrap());
        assert_eq!(budget.used_bytes(), 0);
    }

    #[test]
    fn internal_empty_destination_still_owns_the_only_record() {
        let budget = CaptureBudgetV1::new(0).unwrap();
        let reservation = budget.reserve(destination(0)).unwrap();
        assert_eq!(budget.used_bytes(), 0);
        assert_eq!(budget.account.usage().reserved_records, 1);
        let (error, _) = budget.reserve(destination(0)).err().unwrap();
        assert_eq!(error, ResourceCreditErrorV1::RecordCapacity);
        let result = reservation.retain();
        assert!(result.is_empty());
        drop(result);
        assert_eq!(budget.account.usage().retained_records, 0);
    }

    #[test]
    fn pre_cutoff_rollback_returns_caller_storage_and_cancels_reservation() {
        let budget = CaptureBudgetV1::new(8).unwrap();
        let bytes = vec![3; 8].into_boxed_slice();
        let address = bytes.as_ptr();
        let reservation = budget.reserve(bytes).unwrap();
        let returned = reservation.into_unadmitted_destination();
        assert_eq!(returned.as_ptr(), address);
        assert_eq!(&*returned, &[3; 8]);
        assert_eq!(budget.used_bytes(), 0);
        assert_eq!(budget.account.usage().reserved_records, 0);
        assert_eq!(budget.account.usage().quarantined_records, 0);
    }

    #[test]
    fn retained_result_supports_capture_mutation_but_redacts_debug() {
        let budget = CaptureBudgetV1::new(8).unwrap();
        let mut result = budget.reserve(destination(8)).unwrap().retain();
        assert_eq!(budget.account.usage().reserved_records, 0);
        assert_eq!(budget.account.usage().retained_records, 1);
        result.as_bytes_mut().copy_from_slice(b"redacted");
        assert_eq!(result.as_bytes(), b"redacted");
        assert_eq!(result.len(), 8);
        assert!(!result.is_empty());
        assert_eq!(
            format!("{result:?}"),
            "RuntimeAsyncCapturedBytesV1 { len: 8, .. }"
        );
        drop(result);
        assert_eq!(budget.used_bytes(), 0);
    }

    #[test]
    fn reply_extraction_and_owner_drop_keep_result_bytes_charged() {
        let budget = CaptureBudgetV1::new(8).unwrap();
        let usage = budget.clone();
        let cells = ReplyBudgetV1::new(1);
        let (mut reply, mut future) = Reply::budgeted_pair(&cells).unwrap();
        reply.complete(Ok(budget.reserve(destination(8)).unwrap().retain()));
        let mut context = Context::from_waker(Waker::noop());
        let Poll::Ready(Ok(result)) = Pin::new(&mut future).poll(&mut context) else {
            panic!("capture reply must contain its owned result");
        };
        drop(future);
        drop(reply);
        drop(budget);
        assert_eq!(cells.used(), 0);
        assert_eq!(usage.used_bytes(), 8);
        assert_eq!(usage.account.usage().retained_records, 1);
        drop(result);
        assert_eq!(usage.used_bytes(), 0);
    }

    #[test]
    fn observer_drop_does_not_release_result_still_owned_by_reply() {
        let budget = CaptureBudgetV1::new(8).unwrap();
        let cells = ReplyBudgetV1::new(1);
        let (mut reply, future) = Reply::budgeted_pair(&cells).unwrap();
        reply.complete(Ok(budget.reserve(destination(8)).unwrap().retain()));
        drop(future);
        assert_eq!(budget.used_bytes(), 8);
        assert_eq!(cells.used(), 1);
        drop(reply);
        assert_eq!(budget.used_bytes(), 0);
        assert_eq!(cells.used(), 0);
    }

    #[test]
    fn result_move_to_another_thread_keeps_exact_account_charge() {
        let budget = CaptureBudgetV1::new(8).unwrap();
        let usage = budget.clone();
        let result = budget.reserve(destination(8)).unwrap().retain();
        drop(budget);
        let result = std::thread::spawn(move || result).join().unwrap();
        assert_eq!(usage.used_bytes(), 8);
        drop(result);
        assert_eq!(usage.used_bytes(), 0);
    }

    #[test]
    fn storage_destructor_runs_before_refund_transition() {
        struct DisposalProbe<'a> {
            budget: &'a CaptureBudgetV1,
            disposed: &'a AtomicBool,
        }
        impl Drop for DisposalProbe<'_> {
            fn drop(&mut self) {
                assert_eq!(self.budget.used_bytes(), 8);
                assert_eq!(self.budget.account.usage().retained_records, 1);
                self.disposed.store(true, Ordering::Release);
            }
        }

        let budget = CaptureBudgetV1::new(8).unwrap();
        let credit = budget
            .account
            .reserve(ResourceVectorV1::ZERO.with(ResourceKindV1::ReplyBytes, 8))
            .unwrap()
            .retain();
        let disposed = AtomicBool::new(false);
        dispose_then_refund(
            DisposalProbe {
                budget: &budget,
                disposed: &disposed,
            },
            credit,
        );
        assert!(disposed.load(Ordering::Acquire));
        assert_eq!(budget.used_bytes(), 0);
        assert_eq!(budget.account.usage().retained_records, 0);
    }

    #[test]
    fn equal_capacity_accounts_do_not_exchange_result_credits() {
        let first = CaptureBudgetV1::new(8).unwrap();
        let second = CaptureBudgetV1::new(8).unwrap();
        let a = first.reserve(destination(8)).unwrap().retain();
        let b = second.reserve(destination(8)).unwrap().retain();
        drop(a);
        assert_eq!(first.used_bytes(), 0);
        assert_eq!(second.used_bytes(), 8);
        drop(b);
        assert_eq!(second.used_bytes(), 0);
    }
}
