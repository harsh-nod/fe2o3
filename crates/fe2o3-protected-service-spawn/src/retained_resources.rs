//! Shared ownership for resources retained by the existing cleanup slot.

use std::alloc::Layout;
use std::any::Any;
use std::error::Error;
use std::fmt;
use std::mem::size_of;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, LockResult, Mutex, MutexGuard};

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

const ENTRY: usize = 8;

/// Fixed refusal for scoped access to retained resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedResourceAccessErrorV2 {
    /// A previous panic while holding the owner lock prevents further access.
    Poisoned,
}

impl fmt::Display for RetainedResourceAccessErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Poisoned => formatter.write_str("retained resource owner is poisoned"),
        }
    }
}

impl Error for RetainedResourceAccessErrorV2 {}

/// Move-only handle providing scoped shared access under an owner mutex.
///
/// The typed handle may outlive terminal cleanup; the last owner drops `T`.
/// This handle neither admits resources nor supplies accounting or execution
/// authority; its charge depends on the trusted caller's complete declaration
/// of owned storage.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::RetainedResourcesV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<RetainedResourcesV2<String>>();
/// ```
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::RetainedResourcesV2;
/// fn require_from<T: From<String>>() {}
/// require_from::<RetainedResourcesV2<String>>();
/// ```
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::RetainedResourcesV2;
/// fn extract(value: RetainedResourcesV2<String>) -> String { value.into_inner() }
/// ```
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::RetainedResourcesV2;
/// fn unlocked(value: &RetainedResourcesV2<String>) -> &String { value.get() }
/// ```
pub struct RetainedResourcesV2<T: Send + 'static> {
    owner: Arc<Mutex<T>>,
    charge: usize,
}

impl<T: Send + 'static> RetainedResourcesV2<T> {
    /// Fixed lock/unlock allowance on the original request ledger; excludes callback work.
    pub const ACCESS_WORK: usize = ENTRY + 2 * 1088;
    /// Fixed guard/result metadata and control frame; excludes callback scratch and output.
    pub const ACCESS_SCRATCH: usize = size_of::<MutexGuard<'static, T>>()
        + size_of::<LockResult<MutexGuard<'static, T>>>()
        + size_of::<Result<(), RetainedResourceAccessErrorV2>>()
        + 1024;

    /// Calls a metered operation with `&T` while exclusively holding the owner mutex.
    ///
    /// The full request charge must remain prepaid. The callback receives the
    /// ORIGINAL budget and funds its own work, scratch and any returned owner.
    /// Entry storage is restored on success, error and unwind; work, peak and
    /// denial history remain. A panic poisons the owner and later access refuses
    /// without invoking the callback. Mutex waiting time is not bounded by quota.
    ///
    /// Never recursively access this same owner, including through another
    /// reference: the mutex is not reentrant. No mutable reference, guard or
    /// borrowed view may escape the callback; `T` may have interior mutability.
    ///
    /// ```compile_fail
    /// use fe2o3_protected_service_spawn::{RetainedResourcesV2, RetainedResourceAccessErrorV2 as Access};
    /// use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    ///     CanonicalKernelIrVerificationResourceErrorV1 as Resource};
    /// fn escape<'a, E>(owner: &'a RetainedResourcesV2<String>, b: &mut Budget<'_>)
    ///     -> Result<&'a String, E> where E: From<Resource> + From<Access> {
    ///     owner.with(b, |value, _| Ok(value))
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_protected_service_spawn::{RetainedResourcesV2, RetainedResourceAccessErrorV2 as Access};
    /// use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    ///     CanonicalKernelIrVerificationResourceErrorV1 as Resource};
    /// fn mutate<E>(owner: &RetainedResourcesV2<String>, b: &mut Budget<'_>)
    ///     -> Result<(), E> where E: From<Resource> + From<Access> {
    ///     owner.with(b, |value, _| { value.clear(); Ok(()) })
    /// }
    /// ```
    pub fn with<R, E>(
        &self,
        b: &mut Budget<'_>,
        operation: impl FnOnce(&T, &mut Budget<'_>) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<Resource> + From<RetainedResourceAccessErrorV2>,
    {
        b.with_prepaid_scope(
            self.charge,
            ENTRY,
            Self::ACCESS_WORK,
            Self::ACCESS_SCRATCH,
            |b| {
                let owner = self
                    .owner
                    .lock()
                    .map_err(|_| RetainedResourceAccessErrorV2::Poisoned)?;
                operation(&owner, b)
            },
        )
    }

    /// Full request charge: payload storage plus `size_of::<(Self, usize)>()`.
    pub const fn retained_storage(&self) -> usize {
        self.charge
    }

    /// Inert checked payload quota, including Mutex/Arc overhead and alignment padding.
    ///
    /// `retained_storage` must cover `T` and ALL transitively owned storage. A
    /// declaration below `size_of::<T>()` returns `Resource::Accounting`; layout
    /// or arithmetic overflow returns `Resource::Arithmetic`. This validates no
    /// provenance and reserves nothing. The persistent cleanup pool funds this
    /// payload quota; the request uses [`Self::storage_for`]. The erased owner
    /// record itself belongs to the separately prepaid cleanup-slot storage.
    pub fn payload_storage(retained_storage: usize) -> Result<usize, Resource> {
        if retained_storage < size_of::<T>() {
            return Err(Resource::Accounting);
        }
        // The allocation contains two reference counts and Mutex<T>. Subtract
        // only T, so the mutex and all prefix/tail padding remain fully charged.
        let (allocation, _) = Layout::new::<[AtomicUsize; 2]>()
            .extend(Layout::new::<Mutex<T>>())
            .map_err(|_| Resource::Arithmetic)?;
        allocation
            .pad_to_align()
            .size()
            .checked_sub(size_of::<T>())
            .and_then(|overhead| retained_storage.checked_add(overhead))
            .ok_or(Resource::Arithmetic)
    }

    /// Inert checked full request quota, matching [`Self::retained_storage`].
    ///
    /// Includes the full payload charge and `size_of::<(Self, usize)>()`, since
    /// the typed handle can outlive its cleanup slot. The caller preserves the
    /// original `retained_storage` input reservation and returns only the growth
    /// `storage_for(retained_storage) - retained_storage` for further reservation.
    /// Prepay full output/persistent overlap before construction; this arithmetic
    /// validates no declaration, reserves nothing, and grants no authority.
    pub fn storage_for(retained_storage: usize) -> Result<usize, Resource> {
        Self::payload_storage(retained_storage)?
            .checked_add(size_of::<(Self, usize)>())
            .ok_or(Resource::Arithmetic)
    }

    /// Creates the typed handle and erased cleanup-slot owner after prepayment.
    ///
    /// The trusted unsafe `cleanup_bridge::reserve_launch_retaining` boundary
    /// must ensure complete storage charges, prepay the full request
    /// output/persistent cleanup overlap and bounded allocation/retirement work,
    /// and preserve the original input reservation. Every possible final
    /// `T::drop` must be bounded and nonpanicking, including independently prepaid
    /// nested child cancellation. Unresolved child dependencies must remain held
    /// by their slot. Install this payload in the existing cleanup slot BEFORE
    /// process clone and retain it through deferred cleanup or quarantine.
    ///
    /// All validation precedes allocation. On refusal, consuming `value` drops
    /// the original `T` under those same caller obligations. After allocating,
    /// construction cannot return an error that unexpectedly releases `T`.
    pub(crate) fn pair(
        value: T,
        retained_storage: usize,
    ) -> Result<(Self, RetainedPayload), Resource> {
        let charge = Self::storage_for(retained_storage)?;
        let payload_charge = Self::payload_storage(retained_storage)?;
        let owner = Arc::new(Mutex::new(value));
        let erased: Arc<dyn Any + Send + Sync> = Arc::<Mutex<T>>::clone(&owner);
        Ok((
            Self { owner, charge },
            RetainedPayload {
                owner: erased,
                charge: payload_charge,
            },
        ))
    }
}

impl<T: Send + 'static> fmt::Debug for RetainedResourcesV2<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedResourcesV2")
            .field("retained_storage", &self.charge)
            .finish_non_exhaustive()
    }
}

/// Erased owner retained in the existing cleanup slot, never in child custody.
pub(crate) struct RetainedPayload {
    // Retained solely for Drop; no downcast, data access, or extraction API.
    #[allow(dead_code)]
    owner: Arc<dyn Any + Send + Sync>,
    charge: usize,
}

impl RetainedPayload {
    /// Full payload charge, matching the typed handle's inert payload quota.
    pub(crate) const fn storage(&self) -> usize {
        self.charge
    }
}

#[cfg(test)]
#[path = "retained_resources_tests.rs"]
mod tests;
