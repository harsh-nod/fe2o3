//! Dedicated-process creator lifetime mechanics, not root or deployment admission.
//!
//! A closed entrypoint must independently establish its main-thread/root contract
//! and an external service-manager custodian for the entire service cgroup. This
//! scope only prevents return/unwind with its original cleanup pool still open.

use crate::{
    ProtectedServiceCleanupErrorV2 as Failure, ProtectedServiceCleanupServiceV2 as Cleanup,
};
use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account;
use std::{marker::PhantomData, rc::Rc};

/// Owns one original pool on the creating thread until successful empty shutdown.
///
/// Dropping an open scope immediately exits the entire dedicated process with
/// status 125, including during unwinding. No further destructor, panic hook, allocation,
/// additional cleanup attempt, or replacement ledger runs on that failure path.
/// This does not prove termination of descendants; the external custodian must
/// retain whole-cgroup cleanup responsibility after this process exits.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::creator_scope::DedicatedCreatorScopeV1;
/// fn send<T: Send>() {} send::<DedicatedCreatorScopeV1>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::creator_scope::DedicatedCreatorScopeV1;
/// fn sync<T: Sync>() {} sync::<DedicatedCreatorScopeV1>();
/// ```
#[must_use]
pub struct DedicatedCreatorScopeV1 {
    cleanup: Cleanup,
    open: bool,
    thread_affine: PhantomData<Rc<()>>,
}

impl DedicatedCreatorScopeV1 {
    /// Prepay once for scalar scope control and the single fail-stop exit call.
    /// Pool operations retain their existing independent cleanup-account charges.
    pub const CONTROL_WORK: usize = 256 + 1024 + 64;

    /// Takes the original controller before the first child can be cloned.
    ///
    /// # Safety
    /// The current process must be dedicated to this operation: exiting it must
    /// not kill unrelated application work. The caller must retain this scope
    /// without leaking it through all launches and cleanup on this thread. For
    /// native root launches, independently validate the dedicated main thread,
    /// real root, and the actual external whole-cgroup cleanup custodian. This
    /// function authenticates none of them. Prepay CONTROL_WORK before entry.
    #[allow(unsafe_code)]
    pub unsafe fn enter(cleanup: Cleanup) -> Self {
        Self {
            cleanup,
            open: true,
            thread_affine: PhantomData,
        }
    }

    /// Borrows the same pool for the closed launch and guard-installation path.
    ///
    /// # Safety
    /// Do not replace, move out, or independently shut down this controller. No
    /// callback may escape the closed creator-thread lifetime. All ordinary
    /// native spawn and deployment obligations remain the caller's responsibility.
    #[allow(unsafe_code)]
    pub unsafe fn cleanup_for_launch(&mut self) -> &mut Cleanup {
        assert!(self.open, "creator scope already shut down");
        &mut self.cleanup
    }

    /// Performs one funded scan on the original pool without releasing the scope.
    pub fn pump(&mut self, count: usize) -> Result<(), Failure> {
        self.cleanup.pump(count)?;
        Ok(())
    }

    /// Charges the original controller for a finite all-empty phase checkpoint.
    /// The scope remains armed and retains the same account and deployment guard;
    /// only final `shutdown` disarms it. This is not permanent retirement.
    pub fn checkpoint_quiescent_phase(&mut self) -> Result<(), Failure> {
        self.cleanup.checkpoint_quiescent_phase()
    }

    /// Releases the scope only after the original pool's successful empty shutdown.
    /// Busy, quarantine, accounting failure, and exhaustion leave it armed.
    pub fn shutdown(&mut self) -> Result<Account, Failure> {
        let original_account = self.cleanup.shutdown()?;
        self.open = false;
        Ok(original_account)
    }
}

impl Drop for DedicatedCreatorScopeV1 {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if self.open {
            // SAFETY: enter's dedicated-process contract permits fail-stop. Linux
            // _exit terminates all threads without Rust unwinding or atexit hooks.
            unsafe { libc::_exit(125) }
        }
    }
}

#[cfg(test)]
#[path = "creator_scope_tests.rs"]
mod tests;
