//! Consuming registration/publication only. This module has no queue API.
use super::{Gfx950DebugColdOwnerV1, Gfx950DebugColdPreparationFactsV1, Result, explain};

/// Owns acknowledged debug-runtime registration and a published actual code object,
/// but NO queue, dispatch, stop, register sample, cleanup or retry capability.
///
/// Engineering-only, one-shot API for an isolated disposable process. The caller
/// must ensure no foreign KFD/ROCr runtime or queue exists or is created in the
/// process. The in-library exclusive gate cannot establish that foreign exclusion.
/// TMA is zero: this owner must never be extended to execution without a separately
/// validated trap/sampling-memory design. Registration is not trap execution proof.
///
/// All native resources remain retained until process exit, even after ordinary
/// Drop. There is no runtime-disable or trap-clear acknowledgment in this tranche.
/// This is intentionally not Send, Sync, Clone, or a general-purpose runtime.
pub struct Gfx950DebugMetadataNoQueueOwnerV1 {
    retained: Gfx950DebugColdOwnerV1,
}

impl Gfx950DebugColdOwnerV1 {
    /// Consume actual cold custody, install its exact trap, enable mode-3 debug
    /// metadata and publish the actual retained kernel. No queue is created.
    ///
    /// This can block while an attached debugger handles the KFD runtime event.
    /// An external disposable-process supervisor is required for timeout/termination.
    /// Error/unwind/Drop never frees possibly published storage or attempts rollback.
    /// Do not call in a process containing another runtime or foreign KFD queues.
    ///
    /// # Safety
    ///
    /// The caller must establish an isolated disposable process with no existing
    /// foreign KFD/ROCr runtime or queue, and must exclude all concurrent and future
    /// foreign runtime/queue creation until process exit, even after this owner is
    /// dropped. The in-library gate and observational /proc checks do not prove
    /// this process-wide condition. SET_TRAP_HANDLER precedes runtime enable's
    /// existing-queue rejection; TMA zero is not safe for trap sampling execution.
    /// The caller must also retain supervisor control over code injection and
    /// thread/runtime creation, and terminate/reap the process after observation.
    ///
    /// This control is exercised with `engineering-gfx950` enabled:
    ///
    /// ```compile_fail,E0133
    /// use fe2o3_kfd::Gfx950DebugColdOwnerV1;
    /// fn safe_call(owner: Gfx950DebugColdOwnerV1) {
    ///     let _ = owner.enable_debug_metadata_without_queue();
    /// }
    /// ```
    pub unsafe fn enable_debug_metadata_without_queue(
        mut self,
    ) -> Result<Gfx950DebugMetadataNoQueueOwnerV1> {
        // Re-run actual identity/readback/currentness checks before registration.
        // The existing retention arm and exposed process reservation stay owned.
        if self.resources.get_mut().finish()? != self.facts {
            return Err("cold debug preparation identity changed".into());
        }
        {
            let resource = self.resources.get_mut();
            let trap = resource.trap.as_ref().ok_or("missing retained trap")?;
            let metadata = resource
                .metadata
                .as_mut()
                .ok_or("missing retained metadata")?;
            metadata
                .activate_no_queue(trap, &mut resource.context)
                .map_err(explain)?;
        }
        Ok(Gfx950DebugMetadataNoQueueOwnerV1 { retained: self })
    }
}

impl Gfx950DebugMetadataNoQueueOwnerV1 {
    /// Earlier immutable preparation facts, not a currentness check or live sample.
    pub fn preparation_facts(&self) -> Gfx950DebugColdPreparationFactsV1 {
        self.retained.facts()
    }

    /// Version advertised during successful registration, not debugger acceptance.
    pub fn registered_metadata_version(&self) -> i32 {
        11
    }
}
