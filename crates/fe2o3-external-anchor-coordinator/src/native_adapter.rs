//! One closed implementation; family modules supply only nominal native owners.
macro_rules! preparation {
    ($Prepared:ident, $version:literal, $other:literal) => {
        use crate::{StateRootSnapshotV1, native::{
            self, ENTRY_WORK, LOCAL_WORK, Result, measurement, sum, maximum,
            ExternalAnchorPreparationErrorV2 as Error,
            ExternalAnchorPreparationFailureV2 as Failure,
            ExternalAnchorPreparationQuotaV2 as Quota,
            ExternalAnchorPreparationStorageV2 as Storage,
        }};
        use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 as CapabilityError;
        use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
        use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrVerificationResourceErrorV1 as Resource};
        use fe2o3_protected_static_executable::{ProtectedStaticExecutableV2 as Image,
            ProtectedStaticExecutableOwnerV1 as ImageOwner,
            ProtectedStaticExecutableOperationV2 as ImageOperation,
            ProtectedStaticExecutableStorageV2 as ImageStorage,
            ProtectedStaticExecutableErrorV2 as ImageError};
        use std::{fmt, fs::File, mem::size_of};

        const CONTEXT_WORK: usize = PolicyCap::IO_WORK + SupervisorCap::IO_WORK
            + DeploymentCap::IO_WORK + ProvisioningCap::IO_WORK + DEPLOYMENT_WORK
            + PROVISIONING_WORK + Key::IO_WORK + Lease::ROOT_BINDING_WORK;
        const CONTEXT_STORAGE: usize = maximum(&[PolicyCap::IO_STORAGE,
            SupervisorCap::IO_STORAGE, DeploymentCap::IO_STORAGE, ProvisioningCap::IO_STORAGE,
            DEPLOYMENT_STORAGE, PROVISIONING_STORAGE, Key::IO_STORAGE, Lease::ROOT_BINDING_SCRATCH]);
        const IMAGE_GROWTH: usize = size_of::<(Image, ImageStorage)>()
            - size_of::<(File, ImageStorage)>();

        /// Move-only root-prepared native anchor inputs, NOT a running service or launch authority.
        /// Owns native deployment/provisioning/key/lease/images and a pinned state root.
        /// Actual same-family policy and supervisor capabilities are borrowed afresh on
        /// each operation; identities alone are insufficient. No V1 owner upgrade,
        /// raw descriptor/key accessor or generic provider exists. Launch requires
        /// actual same-family contexts, finite cleanup funding and the original ledger.
        /// Root-owned configuration does not authenticate provisioning provenance.
        ///
        /// Prepay prepare_input_storage(), including BOTH full source images and all
        /// borrowed/consumed owners. Operations restore entry storage on every exit;
        /// work, peak and first-denial history stay on the original ledger. On success
        /// reserve returned GROWTH_STORAGE before retention, keeping consumed charges.
        /// On consuming failure all owned inputs close, but their reservations remain
        /// for caller cleanup. After Drop retire FULL retained_storage(); borrowed
        /// contexts retain their own reservations. Preparation performs no persistence,
        /// spawn or signing; the separate consuming launch enters the measured helper.
        ///
        /// ```
        #[doc = concat!("use fe2o3_external_anchor_coordinator::{", stringify!($Prepared), " as Prepared, ExternalAnchorPreparationErrorV2 as Error};")]
        #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Supervisor};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn check(p: &Prepared, s: &Supervisor, policy: &Policy, b: &mut Budget<'_>)
        ///     -> Result<(), Error> { p.revalidate(s, policy, b) }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Prepared), " as Prepared;")]
        /// fn duplicate(p: Prepared) { let _ = p.clone(); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Prepared), " as Prepared;")]
        /// fn descriptor<T: std::os::fd::AsFd>() {} descriptor::<Prepared>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Prepared), " as Prepared;")]
        /// fn raw<T: std::os::fd::FromRawFd>() {} raw::<Prepared>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::{", stringify!($Prepared), " as Prepared, PreparedExternalAnchorOccurrenceV1 as Old};")]
        /// fn upgrade(p: Old) -> Prepared { p.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::{", stringify!($Prepared), " as Prepared, PreparedExternalAnchorOccurrenceV", $other, " as Other};")]
        /// fn mix(p: Prepared) -> Other { p.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Prepared), " as Prepared;")]
        #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSupervisorDeploymentCapabilityV", $other, " as Supervisor};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(p: &Prepared, s: &Supervisor, policy: &Policy, b: &mut Budget<'_>) {
        ///     let _ = p.revalidate(s, policy, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Prepared), " as Prepared;")]
        #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionPolicyCapabilityV", $other, " as Policy, CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Supervisor};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(p: &Prepared, s: &Supervisor, policy: &Policy, b: &mut Budget<'_>) {
        ///     let _ = p.revalidate(s, policy, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Prepared), " as Prepared;")]
        /// fn escape(p: Prepared) { let _ = p.key_template; let _ = p.root; }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Prepared), " as Prepared;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn context_free(p: &Prepared, b: &mut Budget<'_>) { let _ = p.revalidate(b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Prepared), " as Prepared;")]
        /// fn launch(p: Prepared) { let _ = p.launch(std::time::Duration::from_secs(1)); }
        /// ```
        pub struct $Prepared {
            helper: Image,
            daemon: Image,
            root: File,
            lifecycle: Lease,
            deployment: DeploymentCap,
            provisioning: ProvisioningCap,
            key_template: Key,
            root_snapshot: StateRootSnapshotV1,
            prepared_by: rustix::process::Pid,
            retained: usize,
        }
        impl $Prepared {
            /// Full incoming root File charge; root is never duplicated here.
            pub const ROOT_STORAGE: usize = size_of::<(File, Storage)>();
            // Charge each embedded owner at its native full size, plus only the
            // enclosing snapshot/PID/accounting/padding not covered by those owners.
            const ENVELOPE: usize = size_of::<(Self, Storage)>() - 2 * size_of::<Image>()
                - size_of::<File>() - size_of::<Lease>() - size_of::<DeploymentCap>()
                - size_of::<ProvisioningCap>() - size_of::<Key>();
            /// Exact growth above all consumed Files and native owners.
            pub const GROWTH_STORAGE: usize = 2 * IMAGE_GROWTH + Self::ENVELOPE;
            /// Fixed local control/owner/error staging, excluding nested scratch.
            pub const FRAME_STORAGE: usize = 4 * size_of::<(Self, Storage)>()
                + 8 * size_of::<Error>() + 4 * size_of::<rustix::fs::Stat>() + 8192;
            /// Local precharge, excluding nested native work.
            pub const LOCAL_WORK: usize = native::LOCAL_WORK;

            /// Constant-time checked full floor, including the two BORROWED contexts.
            pub fn prepare_input_storage(lifecycle: &Lease, deployment: &DeploymentCap,
                provisioning: &ProvisioningCap, key: &Key, supervisor: &SupervisorCap,
                policy: &PolicyCap) -> Result<usize> {
                sum(&[Self::ROOT_STORAGE, lifecycle.retained_storage(),
                    deployment.retained_storage(), provisioning.retained_storage(), key.retained_storage(),
                    Image::file_storage(measurement(provisioning.provisioning().helper())?)?,
                    Image::file_storage(measurement(deployment.deployment().executable())?)?,
                    supervisor.retained_storage(), policy.retained_storage()])
            }

            /// Complete successful-path quota; queries do not perform I/O or grant authority.
            pub fn preparation_quota(deployment: &DeploymentCap, provisioning: &ProvisioningCap)
                -> Result<Quota> {
                let helper = measurement(provisioning.provisioning().helper())?;
                let daemon = measurement(deployment.deployment().executable())?;
                let h = Image::quota(helper, ImageOperation::Admit)?;
                let d = Image::quota(daemon, ImageOperation::Admit)?;
                let hr = Image::quota(helper, ImageOperation::Revalidate)?;
                let dr = Image::quota(daemon, ImageOperation::Revalidate)?;
                Ok(Quota {
                    work: sum(&[LOCAL_WORK, CONTEXT_WORK, h.work(), d.work(),
                        CONTEXT_WORK, hr.work(), dr.work()])?,
                    scratch: sum(&[Self::FRAME_STORAGE, maximum(&[
                        CONTEXT_STORAGE, h.scratch(), sum(&[IMAGE_GROWTH, d.scratch()])?,
                        sum(&[Self::GROWTH_STORAGE,
                            maximum(&[CONTEXT_STORAGE, hr.scratch(), dr.scratch()])])?])])?,
                })
            }

            /// Consumes prepaid inputs under exact root credentials and returns only growth.
            /// The helper and daemon are freshly sealed for the deployment's exact service.
            #[allow(clippy::too_many_arguments)]
            pub fn prepare(helper_source: File, daemon_source: File, root: File,
                lifecycle: Lease, deployment: DeploymentCap, provisioning: ProvisioningCap,
                key_template: Key, supervisor: &SupervisorCap, policy: &PolicyCap,
                b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Self::prepare_inner::<true>(helper_source, daemon_source, root, lifecycle,
                    deployment, provisioning, key_template, supervisor, policy, b)
            }

            #[allow(clippy::too_many_arguments)]
            fn prepare_inner<const ROOT: bool>(helper_source: File, daemon_source: File, root: File,
                lifecycle: Lease, deployment: DeploymentCap, provisioning: ProvisioningCap,
                key_template: Key, supervisor: &SupervisorCap, policy: &PolicyCap,
                b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                b.charge_work(ENTRY_WORK)?;
                let floor = Self::prepare_input_storage(&lifecycle, &deployment, &provisioning,
                    &key_template, supervisor, policy)?;
                let retained = floor.checked_sub(sum(&[supervisor.retained_storage(),
                    policy.retained_storage()])?).and_then(|v| v.checked_add(Self::GROWTH_STORAGE))
                    .ok_or(Resource::Arithmetic)?;
                b.with_prepaid_scope(floor, 0, LOCAL_WORK - ENTRY_WORK, Self::FRAME_STORAGE, |b| {
                    native::require_root::<ROOT>()?;
                    check_context(&deployment, &provisioning, &key_template, supervisor, policy, b)?;
                    let root_snapshot = native::state_root(&root, deployment.deployment().service())?;
                    lifecycle.revalidate_for_root(&root, b)?;
                    let owner = ImageOwner::new(deployment.deployment().service().uid(),
                        deployment.deployment().service().gid()).map_err(ImageError::from)?;
                    let (helper, growth) = Image::seal_source_for_owner(helper_source,
                        measurement(provisioning.provisioning().helper())?, owner,
                        "native external-anchor helper", b)?;
                    if growth.additional_storage() != IMAGE_GROWTH { return Err(Resource::Accounting.into()); }
                    b.reserve_storage(growth.additional_storage())?;
                    let (daemon, growth) = Image::seal_source_for_owner(daemon_source,
                        measurement(deployment.deployment().executable())?, owner,
                        "native external-anchor daemon", b)?;
                    if growth.additional_storage() != IMAGE_GROWTH { return Err(Resource::Accounting.into()); }
                    b.reserve_storage(growth.additional_storage())?;
                    b.reserve_storage(Self::ENVELOPE)?;
                    let prepared = Self { helper, daemon, root, lifecycle, deployment, provisioning,
                        key_template, root_snapshot, prepared_by: rustix::process::getpid(), retained };
                    prepared.check::<ROOT>(supervisor, policy, b)?;
                    Ok((prepared, Storage(Self::GROWTH_STORAGE)))
                })
            }

            /// Complete revalidation work and scratch above this owner AND actual contexts.
            pub fn revalidation_quota(&self) -> Result<Quota> {
                let h = Image::quota(self.helper.measurement(), ImageOperation::Revalidate)?;
                let d = Image::quota(self.daemon.measurement(), ImageOperation::Revalidate)?;
                Ok(Quota { work: sum(&[LOCAL_WORK, CONTEXT_WORK, h.work(), d.work()])?,
                    scratch: sum(&[Self::FRAME_STORAGE,
                        maximum(&[CONTEXT_STORAGE, h.scratch(), d.scratch()])])? })
            }
            /// Rechecks actual context, exact root/key/lease/images and preparing PID.
            pub fn revalidate(&self, supervisor: &SupervisorCap, policy: &PolicyCap,
                b: &mut Budget<'_>) -> Result<()> {
                self.revalidate_inner::<true>(supervisor, policy, b)
            }
            fn revalidate_inner<const ROOT: bool>(&self, supervisor: &SupervisorCap,
                policy: &PolicyCap, b: &mut Budget<'_>) -> Result<()> {
                b.charge_work(ENTRY_WORK)?;
                let floor = sum(&[self.retained, supervisor.retained_storage(), policy.retained_storage()])?;
                b.with_prepaid_scope(floor, 0, LOCAL_WORK - ENTRY_WORK, Self::FRAME_STORAGE,
                    |b| self.check::<ROOT>(supervisor, policy, b))
            }
            fn check<const ROOT: bool>(&self, supervisor: &SupervisorCap, policy: &PolicyCap,
                b: &mut Budget<'_>) -> Result<()> {
                native::require_root::<ROOT>()?;
                if rustix::process::getpid() != self.prepared_by {
                    return Err(Error::Invalid(Failure::CoordinatorChanged));
                }
                check_context(&self.deployment, &self.provisioning, &self.key_template,
                    supervisor, policy, b)?;
                if native::state_root(&self.root, self.deployment.deployment().service())? != self.root_snapshot {
                    return Err(Error::Invalid(Failure::StateRootChanged));
                }
                self.lifecycle.revalidate_for_root(&self.root, b)?;
                self.helper.revalidate(b)?;
                self.daemon.revalidate(b)?;
                native::require_root::<ROOT>()
            }
            /// Full retained charge, excluding the borrowed supervisor and policy.
            pub const fn retained_storage(&self) -> usize { self.retained }
        }

        fn check_context(deployment: &DeploymentCap, provisioning: &ProvisioningCap,
            key: &Key, supervisor: &SupervisorCap, policy: &PolicyCap,
            b: &mut Budget<'_>) -> Result<()> {
            policy.revalidate(b)?;
            supervisor.revalidate(b)?;
            let d = deployment.deployment();
            if !d.matches_supervisor_and_policy(supervisor.deployment(), policy.policy(), b)
                .map_err(CapabilityError::from)? {
                return Err(Error::Invalid(Failure::ContextMismatch));
            }
            deployment.revalidate(b)?;
            if !provisioning.provisioning().matches_deployment(d, b).map_err(CapabilityError::from)? {
                return Err(Error::Invalid(Failure::ProvisioningMismatch));
            }
            provisioning.revalidate(b)?;
            // Secret validation requires the CURRENT owner; the public path checked
            // every root ID before reaching here, hence a root-owned template.
            key.revalidate(d, b)?;
            Ok(())
        }
        impl fmt::Debug for $Prepared {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Prepared)).field("authority", &"preparation-only")
                    .field("deployment", &self.deployment.deployment().identity())
                    .field("provisioning", &self.provisioning.provisioning().identity())
                    .finish_non_exhaustive()
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            type Prepared = $Prepared;
            include!("native_cases_tests.rs");
        }
    };
}
pub(crate) use preparation;
