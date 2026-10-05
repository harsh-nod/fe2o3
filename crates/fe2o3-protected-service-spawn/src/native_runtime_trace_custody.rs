//! Original slot/publication forwarding without exposing root-only resume/Drop.
use super::*;
use crate::{
    LateRetainedCustodyV2 as Holder, PreparedLateAttachmentV2 as Attachment,
    PreparedLateRetirementV2 as Retirement, ProtectedServiceCleanupServiceV2 as Cleanup,
    cleanup_bridge::{LateRetainedBuildV2 as Build, LateRetainedPayloadV2 as Payload},
    native_spawn::ProtectedServiceSpawnStorageV2 as Storage,
};

impl RootRuntimeTraceV1<'_> {
    /// Fixed account/thread validation and original allocation comparison work.
    pub const IDENTITY_COMPARISON_WORK: usize = ENTRY + 4 * 1088;

    /// Compare the original move-stable root allocation, including after actual
    /// trace retirement. This inert equality establishes no liveness, execution
    /// or publication authority and never reconstructs custody from a PID.
    pub fn matches_original_identity(
        &self,
        identity: &super::super::RootTaskIdentityV2,
        b: &mut Budget<'_>,
    ) -> Result<bool> {
        original_identity_scope(self.retained, identity.retained_storage(), b, |b| {
            self.check_budget(b)?;
            Ok(std::rc::Rc::ptr_eq(
                &self.root().identity,
                &identity.allocation,
            ))
        })
    }

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

fn original_identity_scope<R>(
    owner_storage: usize,
    identity_storage: usize,
    b: &mut Budget<'_>,
    operation: impl FnOnce(&mut Budget<'_>) -> Result<R>,
) -> Result<R> {
    let floor = owner_storage
        .checked_add(identity_storage)
        .ok_or(Resource::Arithmetic)?;
    b.with_prepaid_scope(
        floor,
        ENTRY,
        RootRuntimeTraceV1::IDENTITY_COMPARISON_WORK,
        0,
        operation,
    )
}

#[cfg(test)]
mod identity_budget_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn original_identity_comparison_prepays_exact_work_and_full_storage() {
        let quote = RootRuntimeTraceV1::IDENTITY_COMPARISON_WORK;
        for case in 0..3 {
            let mut work = Work::new(quote - usize::from(case == 1));
            let mut b = Budget::new(&mut work, 30);
            b.reserve_storage(30 - usize::from(case == 2)).unwrap();
            let mut invoked = false;
            let result = original_identity_scope(10, 20, &mut b, |_| {
                invoked = true;
                Ok(17)
            });
            match case {
                0 => {
                    assert_eq!(result.unwrap(), 17);
                    assert_eq!(b.work(), quote);
                    assert_eq!(b.storage(), 30);
                    assert!(invoked);
                }
                1 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    assert!(!invoked);
                }
                2 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert!(!invoked);
                }
                _ => unreachable!(),
            }
        }
        let mut work = Work::new(quote);
        let mut b = Budget::new(&mut work, 30);
        assert!(matches!(
            original_identity_scope(usize::MAX, 1, &mut b, |_| Ok(())),
            Err(Error::Resource(Resource::Arithmetic))
        ));
        assert_eq!(b.work(), 0);
    }
}
