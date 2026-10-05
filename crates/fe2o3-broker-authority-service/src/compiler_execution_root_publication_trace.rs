//! Closed forwarding over original and advanced ownership of the SAME trace.
use super::*;
use fe2o3_protected_service_spawn::{
    PreparedLateAttachmentV2 as Attachment,
    native_spawn::{ProtectedServiceSpawnStorageV2 as Storage, RootRuntimeTraceV1 as Runtime},
};

// Private implementations only; no caller-defined observation or custody source.
pub(super) trait PublicationTrace<'work> {
    fn retained_storage(&self) -> usize;
    fn with_task_observation<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(
            &RootObservation<'_, 'work>,
            &mut Budget<'budget>,
        ) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource> + From<SpawnError>;
    unsafe fn reserve_late_custody<P: Payload>(
        &mut self,
        cleanup: &mut Cleanup,
        storage: usize,
        b: &mut Budget<'_>,
    ) -> std::result::Result<(Holder<P>, Storage), SpawnError>;
    fn prepare_late_attachment<'slot, P: Payload>(
        &'slot self,
        holder: &'slot Holder<P>,
        b: &mut Budget<'_>,
    ) -> std::result::Result<Attachment<'slot, P>, SpawnError>;
    unsafe fn build_late_custody<P: Build<Operation>, Operation>(
        &self,
        holder: &Holder<P>,
        operation: Operation,
        b: &mut Budget<'_>,
    ) -> std::result::Result<(), <P as Build<Operation>>::Error>;
}

macro_rules! forward_original_custody {
    ($trace:ty) => {
        fn retained_storage(&self) -> usize {
            <$trace>::retained_storage(self)
        }
        fn with_task_observation<'budget, R, E>(
            &self,
            b: &mut Budget<'budget>,
            operation: impl FnOnce(
                &RootObservation<'_, 'work>,
                &mut Budget<'budget>,
            ) -> std::result::Result<R, E>,
        ) -> std::result::Result<R, E>
        where
            E: From<Resource> + From<SpawnError>,
        {
            <$trace>::with_task_observation(self, b, operation)
        }
        unsafe fn reserve_late_custody<P: Payload>(
            &mut self,
            cleanup: &mut Cleanup,
            storage: usize,
            b: &mut Budget<'_>,
        ) -> std::result::Result<(Holder<P>, Storage), SpawnError> {
            // SAFETY: the sole caller prepays the exact artifact quote and
            // installs its payload before any owner acquisition, for both modes.
            unsafe { <$trace>::reserve_late_custody(self, cleanup, storage, b) }
        }
        fn prepare_late_attachment<'slot, P: Payload>(
            &'slot self,
            holder: &'slot Holder<P>,
            b: &mut Budget<'_>,
        ) -> std::result::Result<Attachment<'slot, P>, SpawnError> {
            <$trace>::prepare_late_attachment(self, holder, b)
        }
        unsafe fn build_late_custody<P: Build<Operation>, Operation>(
            &self,
            holder: &Holder<P>,
            operation: Operation,
            b: &mut Budget<'_>,
        ) -> std::result::Result<(), <P as Build<Operation>>::Error> {
            // SAFETY: only the concrete Acquire/Validate builders are passed by
            // the common publication implementation; no payload owner escapes.
            unsafe { <$trace>::build_late_custody(self, holder, operation, b) }
        }
    };
}

impl<'work, T: Send + 'static> PublicationTrace<'work> for Trace<'work, T> {
    forward_original_custody!(Trace<'work, T>);
}
impl<'work> PublicationTrace<'work> for Runtime<'work> {
    forward_original_custody!(Runtime<'work>);
}
