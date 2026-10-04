//! One-shot late storage; the original slot alone controls terminal retirement.

use crate::ProtectedServiceCleanupErrorV2 as Error;
use crate::cleanup_bridge::LateRetainedPayloadV2 as Payload;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::{
    alloc::Layout,
    marker::PhantomData,
    mem::size_of,
    rc::Rc,
    sync::{Arc, Mutex, MutexGuard, TryLockError, atomic::AtomicUsize},
};

const ENTRY: usize = 8;

/// Checked complete quotas for a single late holder; none reserve resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LateRetainedQuotaV2 {
    pub(crate) work: usize,
    request_work: usize,
    pub(crate) scratch: usize,
    pub(crate) persistent: usize,
    pub(crate) retained: usize,
    pub(crate) retirement_work: usize,
}

impl LateRetainedQuotaV2 {
    /// Persistent service work before installing the empty holder.
    pub const fn work(self) -> usize {
        self.work
    }
    /// Request work including the original trace's outer account check.
    pub const fn request_work(self) -> usize {
        self.request_work
    }
    /// Request scratch, including the full newly returned owner overlap.
    pub const fn scratch(self) -> usize {
        self.scratch
    }
    /// Full persistent pool growth, including prepaid retirement scratch.
    pub const fn persistent_storage(self) -> usize {
        self.persistent
    }
    /// FULL UNRESERVED foreground charge, including the future payload maximum.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
    /// Fixed work for each readiness attempt over this complete declaration,
    /// whether ready or deferred. The per-byte term is frozen in this quote.
    pub const fn retirement_work(self) -> usize {
        self.retirement_work
    }
}

struct Value<T> {
    installed: bool,
    payload: Option<T>,
}
struct Backing<T> {
    value: Mutex<Value<T>>,
}

/// Move-only identity of one late holder in the ORIGINAL cleanup slot.
/// No payload view, extraction, replacement, cloning or standalone installation
/// is exposed. Keep its full charge prepaid until Drop, even after retirement.
/// Dropping this handle does not release the slot's payload. Allocation identity
/// remains unique while this handle exists, including after slot reuse.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::LateRetainedCustodyV2;
/// use fe2o3_protected_service_spawn::cleanup_bridge::LateRetainedPayloadV2;
/// fn extract<T: LateRetainedPayloadV2>(v: LateRetainedCustodyV2<T>) { v.into_inner(); }
/// ```
pub struct LateRetainedCustodyV2<T: Payload> {
    owner: Arc<Backing<T>>,
    quota: LateRetainedQuotaV2,
    local: PhantomData<Rc<()>>,
}

impl<T: Payload> LateRetainedCustodyV2<T> {
    /// Bounded slot comparison, mutex attempt, payload moves and cancellation.
    pub const ATTACH_WORK: usize = ENTRY + 4 * 1088 + 4 * size_of::<T>();
    /// Attachment guard/control and inline move overlap; transitive storage is retained.
    pub const ATTACH_SCRATCH: usize =
        4 * size_of::<(T, PreparedLateAttachmentV2<'static, T>)>() + 1024;

    /// Checks the complete payload declaration and both storage overlaps before
    /// allocation. Actual transitive size and callbacks remain unsafe obligations.
    /// Retirement work is ATTACH_WORK plus the payload's fixed base and checked
    /// per-byte work over this entire immutable storage declaration.
    pub fn quota(payload_storage: usize) -> Result<LateRetainedQuotaV2, Resource> {
        if payload_storage < size_of::<T>() || T::RETIRE_SCRATCH < size_of::<T::Prepared>() {
            return Err(Resource::Accounting);
        }
        let (allocation, _) = Layout::new::<[AtomicUsize; 2]>()
            .extend(Layout::new::<Backing<T>>())
            .map_err(|_| Resource::Arithmetic)?;
        let backing = allocation
            .pad_to_align()
            .size()
            .checked_sub(size_of::<T>())
            .and_then(|n| n.checked_add(payload_storage))
            .ok_or(Resource::Arithmetic)?;
        let variable_retirement_work = payload_storage
            .checked_mul(T::RETIRE_WORK_PER_BYTE)
            .ok_or(Resource::Arithmetic)?;
        let retirement_work = Self::ATTACH_WORK
            .checked_add(T::RETIRE_WORK)
            .and_then(|n| n.checked_add(variable_retirement_work))
            .ok_or(Resource::Arithmetic)?;
        let retirement_scratch = Self::retirement_scratch()?;
        let persistent = backing
            .checked_add(retirement_scratch)
            .ok_or(Resource::Arithmetic)?;
        let retained = persistent
            .checked_add(size_of::<(Self, usize)>())
            .ok_or(Resource::Arithmetic)?;
        let work = persistent
            .checked_mul(64)
            .and_then(|n| n.checked_add(retirement_work))
            .ok_or(Resource::Arithmetic)?;
        let request_work = work.checked_add(ENTRY).ok_or(Resource::Arithmetic)?;
        let scratch = retained
            .checked_add(Self::ATTACH_SCRATCH)
            .ok_or(Resource::Arithmetic)?;
        Ok(LateRetainedQuotaV2 {
            work,
            request_work,
            scratch,
            persistent,
            retained,
            retirement_work,
        })
    }

    /// Additional request scratch for preparation; also prepaid in the pool.
    pub fn retirement_scratch() -> Result<usize, Resource> {
        T::RETIRE_SCRATCH
            .checked_add(4 * size_of::<(T, PreparedLateRetirementV2<'static, T>)>() + 1024)
            .ok_or(Resource::Arithmetic)
    }

    /// Full foreground maximum, even while empty or already retired.
    pub const fn retained_storage(&self) -> usize {
        self.quota.retained
    }
    pub(crate) const fn retirement_work(&self) -> usize {
        self.quota.retirement_work
    }

    pub(crate) fn pair(quota: LateRetainedQuotaV2) -> (Self, LatePayload) {
        let owner = Arc::new(Backing {
            value: Mutex::new(Value {
                installed: false,
                payload: None,
            }),
        });
        let erased: Arc<dyn ErasedRetirement> = owner.clone();
        (
            Self {
                owner,
                quota,
                local: PhantomData,
            },
            LatePayload {
                owner: erased,
                work: quota.retirement_work,
            },
        )
    }

    pub(crate) fn matches(&self, payload: &LatePayload) -> bool {
        std::ptr::eq(
            Arc::as_ptr(&self.owner).cast::<()>(),
            Arc::as_ptr(&payload.owner).cast::<()>(),
        )
    }

    fn lock(&self) -> Result<MutexGuard<'_, Value<T>>, Error> {
        self.owner.value.try_lock().map_err(|_| Error::Busy)
    }

    pub(crate) fn prepare_attachment(&self) -> Result<PreparedLateAttachmentV2<'_, T>, Error> {
        let value = self.lock()?;
        if value.installed || value.payload.is_some() {
            return Err(Error::State);
        }
        Ok(PreparedLateAttachmentV2 { value })
    }

    pub(crate) fn prepare_retirement(
        &self,
    ) -> Result<Option<PreparedLateRetirementV2<'_, T>>, Error> {
        let value = self.lock()?;
        let payload = value.payload.as_ref().ok_or(Error::State)?;
        let Some(prepared) = payload.try_prepare_retirement() else {
            return Ok(None);
        };
        Ok(Some(PreparedLateRetirementV2 { value, prepared }))
    }

    pub(crate) fn build<Operation>(
        &self,
        operation: Operation,
        b: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), <T as crate::cleanup_bridge::LateRetainedBuildV2<Operation>>::Error>
    where
        T: crate::cleanup_bridge::LateRetainedBuildV2<Operation>,
    {
        use crate::native_spawn::ProtectedServiceSpawnErrorV2 as SpawnError;
        let mut value = self.lock().map_err(SpawnError::from)?;
        let payload = value
            .payload
            .as_mut()
            .ok_or(SpawnError::State("late payload is absent"))?;
        payload.build(operation, b)
    }
}

/// Exclusive one-shot installation in an already funded ORIGINAL trace slot.
/// Acquire the later lock only after receiving this token. Drop leaves the
/// holder empty. The trace borrow prevents cancel, replacement or slot reuse.
#[must_use]
pub struct PreparedLateAttachmentV2<'slot, T: Payload> {
    value: MutexGuard<'slot, Value<T>>,
}
impl<T: Payload> PreparedLateAttachmentV2<'_, T> {
    /// Moves the completely prepaid payload into its original slot, infallibly.
    pub fn commit(mut self, payload: T) {
        self.value.payload = Some(payload);
        self.value.installed = true;
    }
}

/// Exclusive prepared release; dropping it retains the actual payload in-place.
/// The independently owned readiness value cannot borrow the slot. This token
/// cannot be sent, cloned, retargeted, or survive mutation of its borrowed trace.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::{PreparedLateRetirementV2 as P,
///     cleanup_bridge::LateRetainedPayloadV2 as T};
/// fn transfer<X: T>(p: P<'_, X>) { fn send<V: Send>(_: V) {} send(p); }
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::{PreparedLateRetirementV2 as P,
///     cleanup_bridge::LateRetainedPayloadV2 as T};
/// fn clone<X: T>(p: P<'_, X>) { let _ = p.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::{PreparedLateRetirementV2 as P,
///     cleanup_bridge::LateRetainedPayloadV2 as T};
/// fn twice<X: T>(p: P<'_, X>) { p.commit(); p.commit(); }
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::{PreparedLateRetirementV2 as P,
///     cleanup_bridge::LateRetainedPayloadV2 as T};
/// fn retarget<X: T>(p: P<'_, X>, other: &mut Option<X>) { p.commit(other); }
/// ```
#[must_use]
pub struct PreparedLateRetirementV2<'slot, T: Payload> {
    value: MutexGuard<'slot, Value<T>>,
    prepared: T::Prepared,
}
impl<T: Payload> PreparedLateRetirementV2<'_, T> {
    /// Consuming release after all fallible accounting and validation completed.
    /// The unsafe payload contract makes retirement bounded and infallible.
    pub fn commit(mut self) {
        let payload = self.value.payload.take();
        drop(self.value);
        if let Some(payload) = payload {
            payload.retire(self.prepared);
        }
    }
}

trait ErasedRetirement: Send + Sync {
    fn try_retire(&self) -> bool;
}
impl<T: Payload> ErasedRetirement for Backing<T> {
    fn try_retire(&self) -> bool {
        let mut value = match self.value.try_lock() {
            Ok(value) => value,
            Err(TryLockError::WouldBlock) => return false,
            // A builder unwind preserves its complete partial custody. Only
            // terminal cleanup may recover this lock, solely for final release.
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
        };
        let Some(payload) = value.payload.as_ref() else {
            return true;
        };
        let Some(prepared) = payload.try_prepare_retirement() else {
            return false;
        };
        let payload = value.payload.take();
        drop(value);
        if let Some(payload) = payload {
            payload.retire(prepared);
        }
        true
    }
}

pub(crate) struct LatePayload {
    owner: Arc<dyn ErasedRetirement>,
    pub(crate) work: usize,
}
impl LatePayload {
    #[cfg(test)]
    pub(crate) fn strong_count_for_test(&self) -> usize {
        Arc::strong_count(&self.owner)
    }

    pub(crate) fn pending_copy(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            work: self.work,
        }
    }
    pub(crate) fn try_retire(&self) -> bool {
        self.owner.try_retire()
    }
}
