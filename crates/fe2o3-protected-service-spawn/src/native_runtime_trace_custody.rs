//! Original slot/publication forwarding without exposing root-only resume/Drop.
use super::*;
use crate::{
    LateRetainedCustodyV2 as Holder, PreparedLateAttachmentV2 as Attachment,
    PreparedLateRetirementV2 as Retirement, ProtectedServiceCleanupServiceV2 as Cleanup,
    cleanup_bridge::{LateRetainedBuildV2 as Build, LateRetainedPayloadV2 as Payload},
    native_spawn::ProtectedServiceSpawnStorageV2 as Storage,
};

impl RootRuntimeTraceV1<'_> {
    /// Same original root identity, never a new task constructor or wait authority.
    pub fn pid(&self) -> Pid {
        self.root().pid()
    }

    /// # Safety
    /// Follow the original trace's exact-occurrence monotone builder contract.
    /// No execution or publication authority is established by this forwarding.
    pub unsafe fn build_late_custody<P: Build<Operation>, Operation>(
        &self,
        holder: &Holder<P>,
        operation: Operation,
        b: &mut Budget<'_>,
    ) -> std::result::Result<(), <P as Build<Operation>>::Error> {
        self.check_budget(b)?;
        // SAFETY: the caller preserves the same original slot/builder contract.
        unsafe { self.root().build_late_custody(holder, operation, b) }
    }

    /// # Safety
    /// Prepay the future payload's complete transitive storage and preserve the
    /// original reserve-before-acquire/install-before-fallible-work contract.
    pub unsafe fn reserve_late_custody<P: Payload>(
        &mut self,
        cleanup: &mut Cleanup,
        storage: usize,
        b: &mut Budget<'_>,
    ) -> Result<(Holder<P>, Storage)> {
        self.check_budget(b)?;
        // SAFETY: only the same original slot is forwarded; no trace escapes.
        unsafe { self.root_mut().reserve_late_custody(cleanup, storage, b) }
    }

    pub fn prepare_late_attachment<'slot, P: Payload>(
        &'slot self,
        holder: &'slot Holder<P>,
        b: &mut Budget<'_>,
    ) -> Result<Attachment<'slot, P>> {
        self.check_budget(b)?;
        self.root().prepare_late_attachment(holder, b)
    }

    /// # Safety
    /// Independently authorize release of this exact occurrence. Mechanical
    /// readiness is not authority; unresolved trace retirement grants none.
    pub unsafe fn prepare_late_retirement<'slot, P: Payload>(
        &'slot self,
        holder: &'slot Holder<P>,
        b: &mut Budget<'_>,
    ) -> Result<Option<Retirement<'slot, P>>> {
        self.check_budget(b)?;
        // SAFETY: caller retains the original exact-occurrence release contract.
        unsafe { self.root().prepare_late_retirement(holder, b) }
    }

    /// Release only the original spawn lease at its held root exec boundary.
    ///
    /// # Safety
    /// Authenticate actual native exec and closure of EVERY inherited artifact
    /// alias. The selected exec observation is not image or mapping admission.
    pub unsafe fn confirm_exec(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.check(b, true)?;
        let (index, task) = self.selected()?;
        if index != 0 || task.stop != Stop::Exec {
            return Err(Error::State("runtime trace lacks its held root exec"));
        }
        // SAFETY: caller provides the unchanged exact-child/alias closure proof.
        unsafe { self.root_mut().confirm_exec(b) }
    }
}
