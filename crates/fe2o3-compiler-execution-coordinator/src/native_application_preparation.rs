//! Same-account application root sources and genuine native preparation.
use crate::{
    InheritedCompilerExecutionDeploymentV3 as Inherited,
    PreparedCompilerExecutionSupervisorV3 as Prepared,
    native_inherited::{self as root, CompilerExecutionRootStorageV2 as Storage},
    native_launch,
};
use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Deployment;
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as StorageAccount,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2 as Cleanup;
use std::{mem::size_of, time::Duration};

/// Actual fixed-path native inputs bound to the original root installation and
/// cumulative Work/storage account. No bare admitted owner or raw input escapes.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::NativeApplicationRootSourcesV3 as S;
/// fn clone<T: Clone>() {} clone::<S<'static, 'static>>();
/// ```
pub struct NativeApplicationRootSourcesV3<'root, 'work> {
    inner: Inherited,
    installation: &'root Deployment<'work>,
    ledger: Ledger,
    account: Option<StorageAccount>,
    thread: i32,
    retained: usize,
}

/// Real PreparedV3, signing key template, live original anchor, lifecycle and
/// measured images retained under one original root Work/account. Only the
/// closed application supervisor and consuming currentness routes use it.
/// This owner is not constructed from configuration identities or a readiness record.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::{NativeApplicationRootPreparationV3 as A,
///     PreparedCompilerExecutionSupervisorV3 as P};
/// fn downgrade(a: A<'_, '_>) -> P { a.into() }
/// ```
pub struct NativeApplicationRootPreparationV3<'root, 'work> {
    pub(crate) inner: Prepared,
    pub(crate) installation: &'root Deployment<'work>,
    pub(crate) ledger: Ledger,
    pub(crate) account: Option<StorageAccount>,
    pub(crate) thread: i32,
    pub(crate) retained: usize,
}

impl<'root, 'work> NativeApplicationRootSourcesV3<'root, 'work> {
    const ENVELOPE: usize = size_of::<(Self, Storage)>() - size_of::<Inherited>();

    /// Opens the closed native source roster and performs actual source, key,
    /// image, lifecycle and anchor-preparation admission. Returns a FULL
    /// unreserved owner charge, above the independently retained installation.
    pub fn open(
        installation: &'root Deployment<'work>,
        b: &mut Budget<'work>,
    ) -> root::Result<(Self, Storage)> {
        let floor = installation.retained_storage()?;
        b.with_prepaid_scope(floor, 8, 4096, Self::ENVELOPE, |b| {
            let (inner, charge) = Inherited::admit_native_application(installation, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let retained = root::sum(&[inner.retained_storage(), Self::ENVELOPE])?;
            Ok((
                Self {
                    inner,
                    installation,
                    ledger: b.work_ledger_identity_v1(),
                    account: b.storage_account_identity_v1(),
                    thread: rustix::thread::gettid().as_raw_pid(),
                    retained,
                },
                Storage(retained),
            ))
        })
    }

    /// Full original-account charge; opening or preparing never refunds work.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Launches the actual anchor under the original cleanup account and builds
    /// real native Prepared custody. Keep the consumed charge and add returned
    /// growth. Refusal may leave an original child in the same cleanup pool.
    pub fn prepare(
        self,
        timeout: Duration,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> root::Result<(NativeApplicationRootPreparationV3<'root, 'work>, Storage)> {
        self.prepare_with_guard(timeout, cleanup, true, b)
    }

    pub(crate) fn prepare_after_compiler(
        self,
        installation: &Deployment<'work>,
        _retired: &crate::native_entrypoint::fixed_phase::OriginalCompilerPhaseRetired,
        timeout: Duration,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> root::Result<(NativeApplicationRootPreparationV3<'root, 'work>, Storage)> {
        if !std::ptr::eq(self.installation, installation) {
            return Err(Resource::Accounting.into());
        }
        self.prepare_with_guard(timeout, cleanup, false, b)
    }

    fn prepare_with_guard(
        self,
        timeout: Duration,
        cleanup: &mut Cleanup,
        install_guard: bool,
        b: &mut Budget<'work>,
    ) -> root::Result<(NativeApplicationRootPreparationV3<'root, 'work>, Storage)> {
        if self.ledger != b.work_ledger_identity_v1()
            || self.account != b.storage_account_identity_v1()
            || self.thread != rustix::thread::gettid().as_raw_pid()
        {
            return Err(Resource::Accounting.into());
        }
        let input = self.retained;
        let floor = root::sum(&[input, self.installation.retained_storage()?])?;
        b.with_prepaid_scope(floor, 8, 4096, Self::ENVELOPE, |b| {
            let (inner, charge) = if install_guard {
                self.inner.prepare_native_application_root(
                    self.installation,
                    timeout,
                    cleanup,
                    b,
                )?
            } else {
                self.inner.prepare_native_application_after_compiler(
                    self.installation,
                    timeout,
                    cleanup,
                    b,
                )?
            };
            b.reserve_storage(charge.additional_storage())?;
            let retained = root::sum(&[
                inner.retained_storage(),
                NativeApplicationRootPreparationV3::ENVELOPE,
            ])?
            .max(input);
            let growth = retained.checked_sub(input).ok_or(Resource::Accounting)?;
            b.reserve_storage(growth)?;
            Ok((
                NativeApplicationRootPreparationV3 {
                    inner,
                    installation: self.installation,
                    ledger: self.ledger,
                    account: self.account,
                    thread: self.thread,
                    retained,
                },
                Storage(growth),
            ))
        })
    }
}

impl<'work> NativeApplicationRootPreparationV3<'_, 'work> {
    const ENVELOPE: usize = size_of::<(Self, Storage)>() - size_of::<Prepared>();

    /// Full monotone reservation retained from actual source admission.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    pub(crate) fn revalidate(&self, b: &mut Budget<'work>) -> native_launch::Result<()> {
        b.charge_work(8)?;
        if self.ledger != b.work_ledger_identity_v1()
            || self.account != b.storage_account_identity_v1()
            || self.thread != rustix::thread::gettid().as_raw_pid()
            || b.storage()
                < root::sum(&[self.retained, self.installation.retained_storage()?]).map_err(
                    |_| {
                        native_launch::CompilerExecutionLaunchErrorV2::Resource(
                            Resource::Arithmetic,
                        )
                    },
                )?
        {
            return Err(Resource::Accounting.into());
        }
        self.installation.revalidate(b)?;
        self.inner.revalidate(b)?;
        Ok(())
    }
}
