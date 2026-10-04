//! Original trace/account/slot checks before one-shot late custody operations.

use super::*;
use crate::{
    LateRetainedCustodyV2 as Holder, PreparedLateAttachmentV2 as Attachment,
    PreparedLateRetirementV2 as Retirement, ProtectedServiceCleanupServiceV2 as Cleanup,
    cleanup_bridge::LateRetainedPayloadV2 as Payload,
};

impl RootTaskTraceV2<'_> {
    /// Extend only the installed late payload using its trusted monotone builder.
    /// Every error, final accounting refusal and unwind retains acquired custody
    /// in the original slot. The result cannot expose any payload or owner.
    /// Shared trace access permits a same-trace scoped task observation as input;
    /// it still excludes consuming trace changes. The payload mutex exclusively
    /// serializes construction, attachment and retirement, without exporting it.
    ///
    /// # Safety
    /// Supply only the builder's validated operation and separately prepaid
    /// inputs. Preserve its complete storage maximum and monotone custody
    /// contract. This call establishes no lock, occurrence or publication authority.
    pub unsafe fn build_late_custody<T, Operation>(
        &self,
        holder: &Holder<T>,
        operation: Operation,
        b: &mut Budget<'_>,
    ) -> std::result::Result<(), <T as crate::cleanup_bridge::LateRetainedBuildV2<Operation>>::Error>
    where
        T: crate::cleanup_bridge::LateRetainedBuildV2<Operation>,
    {
        let floor = self
            .retained
            .checked_add(holder.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        let work = Holder::<T>::ATTACH_WORK
            .checked_add(T::BUILD_WORK)
            .ok_or(Resource::Arithmetic)?;
        let scratch = Holder::<T>::ATTACH_SCRATCH
            .checked_add(T::BUILD_SCRATCH)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, ENTRY, work, scratch, |b| {
            self.check_budget(b)?;
            self.check_thread()?;
            let (_, slot) = self
                .child
                .custody
                .0
                .as_ref()
                .ok_or(Error::State("root trace custody was retired or deferred"))?;
            slot.check_late(holder).map_err(Error::from)?;
            holder.build(operation, b)
        })
    }

    /// Install and fund an EMPTY late holder in this exact original cleanup slot.
    /// Call before acquiring the later lock. Immutable launch dependencies are
    /// untouched. The returned charge is FULL UNRESERVED foreground storage;
    /// keep it prepaid alongside the trace, independently of persistent funding.
    /// Only one holder can ever be installed in this slot's current lifetime.
    ///
    /// # Safety
    /// `storage` must cover the future payload and ALL transitive owned storage.
    /// Preserve the payload trait's bounded retirement contract. Do not acquire
    /// custody until prepare_late_attachment succeeds; then commit that custody
    /// before any fallible work or unwind can drop the new value in foreground.
    pub unsafe fn reserve_late_custody<T: Payload>(
        &mut self,
        cleanup: &mut Cleanup,
        storage: usize,
        b: &mut Budget<'_>,
    ) -> Result<(Holder<T>, Storage)> {
        b.with_prepaid_scope(self.retained, ENTRY, ENTRY, 0, |b| {
            self.check_budget(b)?;
            self.check_thread()?;
            let (_, slot) = self
                .child
                .custody
                .0
                .as_ref()
                .ok_or(Error::State("root trace custody was retired or deferred"))?;
            Ok(cleanup.reserve_late(slot, storage, b)?)
        })
    }

    /// Prepay and exclusively bind installation before later lock acquisition.
    /// Drop leaves the holder empty; commit performs only the prepaid move.
    /// Foreign holders, consumed slots, and repeated installation refuse.
    pub fn prepare_late_attachment<'slot, T: Payload>(
        &'slot self,
        holder: &'slot Holder<T>,
        b: &mut Budget<'_>,
    ) -> Result<Attachment<'slot, T>> {
        let floor = self
            .retained
            .checked_add(holder.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(
            floor,
            ENTRY,
            Holder::<T>::ATTACH_WORK,
            Holder::<T>::ATTACH_SCRATCH,
            |b| {
                self.check_budget(b)?;
                self.check_thread()?;
                let (_, slot) = self
                    .child
                    .custody
                    .0
                    .as_ref()
                    .ok_or(Error::State("root trace custody was retired or deferred"))?;
                slot.check_late(holder)?;
                Ok(holder.prepare_attachment()?)
            },
        )
    }

    /// Prepare release without removing the actual payload. `None` means that
    /// readiness deferred; no lock or custody has been released. A returned token
    /// retains custody on Drop and commits only after all accounting scope exits.
    /// The token borrows this trace and the holder, preventing slot invalidation.
    ///
    /// ```compile_fail
    /// use fe2o3_protected_service_spawn::{LateRetainedCustodyV2 as H,
    ///     native_spawn::RootTaskTraceV2 as Trace, cleanup_bridge::LateRetainedPayloadV2 as P};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn stale<T: P>(t: &mut Trace<'_>, h: &H<T>, b: &mut Budget<'_>) {
    ///     let p = unsafe { t.prepare_late_retirement(h, b) }.unwrap().unwrap();
    ///     t.cancel();
    ///     p.commit();
    /// }
    /// ```
    ///
    /// # Safety
    /// Independently establish that explicit release is permitted for this exact
    /// occurrence while the compiler/domain may still be live. Readiness alone
    /// is only mechanical exclusion, not retirement permission or currentness.
    pub unsafe fn prepare_late_retirement<'slot, T: Payload>(
        &'slot self,
        holder: &'slot Holder<T>,
        b: &mut Budget<'_>,
    ) -> Result<Option<Retirement<'slot, T>>> {
        let floor = self
            .retained
            .checked_add(holder.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(
            floor,
            ENTRY,
            holder.retirement_work(),
            Holder::<T>::retirement_scratch()?,
            |b| {
                self.check_budget(b)?;
                self.check_thread()?;
                let (_, slot) = self
                    .child
                    .custody
                    .0
                    .as_ref()
                    .ok_or(Error::State("root trace custody was retired or deferred"))?;
                slot.check_late(holder)?;
                Ok(holder.prepare_retirement()?)
            },
        )
    }
}

#[cfg(test)]
#[path = "native_root_late_tests.rs"]
mod tests;
