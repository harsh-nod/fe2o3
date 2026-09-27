//! One preparation schedule over the two closed native authority families.
macro_rules! preparation {
    ($Prepared:ident, $version:literal, $other:literal) => {
        use crate::{CompilerExecutionSupervisorProgramSourcesV1 as Sources, native::{
            self, Result, ENTRY, LOCAL_WORK, IMAGE_GROWTH, sum, maximum,
            CompilerExecutionPreparationErrorV2 as Error,
            CompilerExecutionPreparationQuotaV2 as Quota,
            CompilerExecutionPreparationStorageV2 as Storage,
        }};
        use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
        use fe2o3_compiler_execution_protocol::{
            MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1 as MAX_SUPERVISOR,
            MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1 as MAX_LAUNCHER,
        };
        use fe2o3_compiler_execution_supervisor::{
            IssuerServiceCredentialProfileV1 as Credentials,
            ProvisionedProtectedIssuerServiceInputsV2 as ServiceInputs,
        };
        use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrVerificationResourceErrorV1 as Resource};
        use fe2o3_protected_service_profile::{ProtectedServiceNamespaceSetV2 as Namespaces,
            ProtectedServiceProfileStorageV2 as NamespaceStorage};
        use fe2o3_protected_static_executable::{ProtectedStaticExecutableV2 as Image,
            ProtectedStaticExecutableMeasurementV1 as Measurement,
            ProtectedStaticExecutableOwnerV1 as Owner,
            ProtectedStaticExecutableOperationV2 as Operation,
            ProtectedStaticExecutableErrorV2 as ImageError};
        use std::{fmt, mem::size_of};

        const NAMESPACE_STORAGE: usize = size_of::<(Namespaces, NamespaceStorage)>();

        fn measurements(trust: &Trust) -> Result<[Measurement; 3]> {
            let deployment = trust.deployment().deployment();
            Ok([
                native::measurement(deployment.executable(), MAX_SUPERVISOR)?,
                native::measurement(deployment.launcher(), MAX_LAUNCHER)?,
                native::measurement(trust.policy().policy().executable(), MAX_LAUNCHER)?,
            ])
        }

        fn context_quota(anchor: &Anchor) -> Result<Quota> {
            let anchor = anchor.continuity_quota()?;
            Ok(Quota {
                work: sum(&[Trust::REVALIDATION_WORK, ServiceInputs::LIFECYCLE_WORK,
                    ServiceInputs::LIFECYCLE_WORK, anchor.work()])?,
                scratch: maximum(&[Trust::SCRATCH, ServiceInputs::LIFECYCLE_SCRATCH,
                    anchor.scratch()]),
            })
        }

        fn check_context(trust: &Trust, inputs: &ServiceInputs, supervisor_lease: &Lease,
            root_lease: &Lease, anchor: &Anchor, b: &mut Budget<'_>) -> Result<Credentials> {
            native::require_root()?;
            trust.revalidate(b)?;
            let deployment = trust.deployment().deployment();
            let credentials = Credentials::new(deployment.service_uid(), deployment.service_gid())?;
            if inputs.credentials() != credentials { return Err(Error::ServiceIdentityMismatch); }
            inputs.validate_lifecycle(supervisor_lease, b)?;
            inputs.validate_lifecycle(root_lease, b)?;
            anchor.validate_continuity(trust.deployment(), trust.policy(), b)?;
            Ok(credentials)
        }

        /// Move-only native preparation of the protected compiler supervisor.
        ///
        /// Owns the complete native trust context, freshly sealed supervisor/launcher/
        /// issuer, exact listener/root inputs, root and supervisor lifecycle leases,
        /// and a genuine live native anchor. No V1 admitted owner is upgraded. The V1
        /// program-source bundle is only three untrusted Files, not legacy authority.
        /// Preparation grants neither provisioning provenance nor permission to launch
        /// a GPU. Inherited production composition and consuming supervisor launch are
        /// separate integration steps; this owner itself performs no process creation.
        ///
        /// Prepay prepare_input_storage(), including all three FULL source images.
        /// Reserve returned growth before retaining the result; keep consumed charges.
        /// Entry storage is restored on every exit without refunding work or history.
        /// Consuming refusal drops all owned inputs before caller retirement; anchor
        /// cleanup uses its separately funded shared controller and may be deferred.
        /// Retire FULL retained_storage only after Drop. No clone, raw descriptor,
        /// signing operation, provider interface or cross-family conversion is exposed.
        ///
        /// ```
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Prepared), " as Prepared, CompilerExecutionPreparationErrorV2 as Error};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn check(p: &Prepared, b: &mut Budget<'_>) -> Result<(), Error> { p.revalidate(b) }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Prepared), " as Prepared;")]
        /// fn duplicate<T: Clone>() {} duplicate::<Prepared>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Prepared), " as Prepared;")]
        /// fn descriptor<T: std::os::fd::AsFd>() {} descriptor::<Prepared>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Prepared), " as Prepared, PreparedCompilerExecutionSupervisorV1 as Old};")]
        /// fn upgrade(old: Old) -> Prepared { old.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Prepared), " as Prepared, PreparedCompilerExecutionSupervisorV", $other, " as Other};")]
        /// fn mix(other: Other) -> Prepared { other.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Prepared), " as Prepared;")]
        /// fn escape(p: Prepared) { let _ = p.trust; let _ = p.programs; }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Prepared), " as Prepared;")]
        /// fn unmetered(p: &Prepared) { let _ = p.revalidate(); }
        /// ```
        pub struct $Prepared {
            programs: [Image; 3],
            trust: Trust,
            service_inputs: ServiceInputs,
            supervisor_lifecycle: Lease,
            anchor: Anchor,
            namespaces: Namespaces,
            credentials: Credentials,
            prepared_by: rustix::process::Pid,
            retained: usize,
            // Close root provisioning custody last; child cleanup may retain its own leases.
            lifecycle: Lease,
        }
        impl $Prepared {
            const ENVELOPE: usize = size_of::<(Self, Storage)>() - size_of::<[Image; 3]>()
                - size_of::<Trust>() - size_of::<ServiceInputs>() - 2 * size_of::<Lease>()
                - size_of::<Anchor>() - size_of::<Namespaces>();
            /// Fixed growth above all consumed Files and native owners.
            pub const GROWTH_STORAGE: usize = 3 * IMAGE_GROWTH + NAMESPACE_STORAGE + Self::ENVELOPE;
            /// Local logical frame, excluding nested owners and native scratch.
            pub const FRAME_STORAGE: usize = 4 * size_of::<(Self, Storage)>()
                + 8 * size_of::<Error>() + 4 * size_of::<Sources>() + 8192;

            /// Checked complete input floor, including all three full source images.
            pub fn prepare_input_storage(trust: &Trust, inputs: &ServiceInputs,
                supervisor_lifecycle: &Lease, lifecycle: &Lease, anchor: &Anchor) -> Result<usize> {
                let m = measurements(trust)?;
                sum(&[trust.retained_storage(), inputs.retained_storage(),
                    supervisor_lifecycle.retained_storage(), lifecycle.retained_storage(),
                    anchor.retained_storage(), Image::file_storage(m[0])?,
                    Image::file_storage(m[1])?, Image::file_storage(m[2])?])
            }

            /// Complete successful preparation envelope, including overlapping image growth.
            /// This query observes inert measurements, not descriptors or authority.
            pub fn preparation_quota(trust: &Trust, anchor: &Anchor) -> Result<Quota> {
                let m = measurements(trust)?;
                let a = [Image::quota(m[0], Operation::Admit)?, Image::quota(m[1], Operation::Admit)?,
                    Image::quota(m[2], Operation::Admit)?];
                let r = [Image::quota(m[0], Operation::Revalidate)?, Image::quota(m[1], Operation::Revalidate)?,
                    Image::quota(m[2], Operation::Revalidate)?];
                let c = context_quota(anchor)?;
                Ok(Quota {
                    work: sum(&[LOCAL_WORK, c.work(), a[0].work(), a[1].work(), a[2].work(),
                        Namespaces::CAPTURE_WORK, Namespaces::REVALIDATE_SELF_WORK, c.work(),
                        r[0].work(), r[1].work(), r[2].work()])?,
                    scratch: sum(&[Self::FRAME_STORAGE, maximum(&[
                        c.scratch(), a[0].scratch(), sum(&[IMAGE_GROWTH, a[1].scratch()])?,
                        sum(&[2 * IMAGE_GROWTH, a[2].scratch()])?,
                        sum(&[3 * IMAGE_GROWTH, Namespaces::CAPTURE_SCRATCH])?,
                        sum(&[Self::GROWTH_STORAGE, maximum(&[c.scratch(),
                            Namespaces::REVALIDATE_SELF_SCRATCH,
                            r[0].scratch(), r[1].scratch(), r[2].scratch()])])?,
                    ])])?,
                })
            }

            /// Consumes genuine native custody under exact root credentials.
            ///
            /// ```
            #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Prepared), " as Prepared, CompilerExecutionSupervisorTrustV", $version, " as Trust, CompilerExecutionSupervisorProgramSourcesV1 as Sources, CompilerExecutionPreparationErrorV2 as Error, CompilerExecutionPreparationStorageV2 as Storage};")]
            #[doc = concat!("use fe2o3_external_anchor_coordinator::RootManagedExternalAnchorV", $version, " as Anchor;")]
            /// use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV2 as Inputs;
            /// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// fn prepare(s: Sources, t: Trust, i: Inputs, service: Lease, root: Lease,
            ///     a: Anchor, b: &mut Budget<'_>) -> Result<(Prepared, Storage), Error> {
            ///     Prepared::prepare(s, t, i, service, root, a, b)
            /// }
            /// ```
            /// ```compile_fail
            #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Prepared), " as Prepared, CompilerExecutionSupervisorTrustV", $other, " as Trust, CompilerExecutionSupervisorProgramSourcesV1 as Sources};")]
            #[doc = concat!("use fe2o3_external_anchor_coordinator::RootManagedExternalAnchorV", $version, " as Anchor;")]
            /// use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV2 as Inputs;
            /// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// fn wrong(s: Sources, t: Trust, i: Inputs, l: Lease, r: Lease, a: Anchor, b: &mut Budget<'_>) {
            ///     let _ = Prepared::prepare(s, t, i, l, r, a, b);
            /// }
            /// ```
            /// ```compile_fail
            #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Prepared), " as Prepared, CompilerExecutionSupervisorTrustV", $version, " as Trust, CompilerExecutionSupervisorProgramSourcesV1 as Sources};")]
            #[doc = concat!("use fe2o3_external_anchor_coordinator::RootManagedExternalAnchorV", $other, " as Anchor;")]
            /// use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV2 as Inputs;
            /// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// fn wrong(s: Sources, t: Trust, i: Inputs, l: Lease, r: Lease, a: Anchor, b: &mut Budget<'_>) {
            ///     let _ = Prepared::prepare(s, t, i, l, r, a, b);
            /// }
            /// ```
            #[allow(clippy::too_many_arguments)]
            pub fn prepare(programs: Sources, trust: Trust, service_inputs: ServiceInputs,
                supervisor_lifecycle: Lease, lifecycle: Lease, anchor: Anchor,
                budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                budget.charge_work(ENTRY)?;
                let floor = Self::prepare_input_storage(&trust, &service_inputs,
                    &supervisor_lifecycle, &lifecycle, &anchor)?;
                let retained = sum(&[floor, Self::GROWTH_STORAGE])?;
                budget.with_prepaid_scope(floor, 0, LOCAL_WORK - ENTRY, Self::FRAME_STORAGE, |b| {
                    let credentials = check_context(&trust, &service_inputs,
                        &supervisor_lifecycle, &lifecycle, &anchor, b)?;
                    let owner = Owner::new(credentials.uid(), credentials.gid()).map_err(ImageError::from)?;
                    let programs = native::seal_programs(programs, measurements(&trust)?, owner, b)?;
                    let (namespaces, charge) = Namespaces::capture_self(b)?;
                    if charge.additional_storage() != NAMESPACE_STORAGE { return Err(Resource::Accounting.into()); }
                    b.reserve_storage(charge.additional_storage())?;
                    b.reserve_storage(Self::ENVELOPE)?;
                    let prepared = Self { programs, trust, service_inputs, supervisor_lifecycle,
                        anchor, namespaces, credentials, prepared_by: rustix::process::getpid(),
                        retained, lifecycle };
                    prepared.check(b)?;
                    Ok((prepared, Storage(Self::GROWTH_STORAGE)))
                })
            }

            /// Full owned reservation, excluding the independently funded cleanup pool.
            pub const fn retained_storage(&self) -> usize { self.retained }

            /// Complete revalidation work and additional peak above this owner.
            pub fn revalidation_quota(&self) -> Result<Quota> {
                let c = context_quota(&self.anchor)?;
                let r = [Image::quota(self.programs[0].measurement(), Operation::Revalidate)?,
                    Image::quota(self.programs[1].measurement(), Operation::Revalidate)?,
                    Image::quota(self.programs[2].measurement(), Operation::Revalidate)?];
                Ok(Quota {
                    work: sum(&[LOCAL_WORK, Namespaces::REVALIDATE_SELF_WORK, c.work(),
                        r[0].work(), r[1].work(), r[2].work()])?,
                    scratch: sum(&[Self::FRAME_STORAGE, maximum(&[c.scratch(),
                        Namespaces::REVALIDATE_SELF_SCRATCH, r[0].scratch(), r[1].scratch(), r[2].scratch()])])?,
                })
            }

            /// Rechecks exact native inputs, full context, images, root identity and namespaces.
            pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
                budget.with_prepaid_scope(self.retained, ENTRY, LOCAL_WORK, Self::FRAME_STORAGE,
                    |b| self.check(b))
            }

            /// Complete request work and extra peak above FULL retained_storage for
            /// validate_cleanup_guard. The original cleanup account separately pays
            /// GUARD_CLONE_WORK and keeps its existing pool/guard reservation.
            /// This query grants no launch authority or launch/protected validation credit.
            pub fn cleanup_guard_quota(&self) -> Result<Quota> {
                native::cleanup_guard_quota(self.revalidation_quota()?, Self::FRAME_STORAGE)
            }

            /// Revalidates this actual Prepared, then joins the existing cleanup guard
            /// to this owner's root-bound lifecycle on the original request budget.
            /// Keep FULL retained_storage prepaid. The anchor installed the guard
            /// before launch; this owner already holds a live anchor and cannot install
            /// or replace that guard. An independently valid unrelated lease is refused.
            /// The temporary alias is fully charged and dropped before retirement;
            /// all exits restore entry storage without refunding work or denial history.
            /// Success grants no launch authority or launch/protected validation credit.
            pub fn validate_cleanup_guard(&self,
                cleanup: &mut fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2,
                b: &mut Budget<'_>) -> Result<()> {
                native::with_cleanup_guard(self.retained, Self::FRAME_STORAGE, cleanup, b,
                    |b| self.revalidate(b),
                    |alias, b| Ok(self.lifecycle.validate_transfer(alias, b)?))
            }

            fn check(&self, b: &mut Budget<'_>) -> Result<()> {
                native::require_root()?;
                if rustix::process::getpid() != self.prepared_by { return Err(Error::CoordinatorChanged); }
                self.namespaces.revalidate_self(b)?;
                let credentials = check_context(&self.trust, &self.service_inputs,
                    &self.supervisor_lifecycle, &self.lifecycle, &self.anchor, b)?;
                if self.credentials != credentials { return Err(Error::ServiceIdentityMismatch); }
                native::check_programs(&self.programs, measurements(&self.trust)?, credentials, b)?;
                native::require_root()
            }
        }
        impl fmt::Debug for $Prepared {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Prepared)).field("authority", &"native-preparation-only")
                    .field("deployment", &self.trust.deployment().deployment().identity())
                    .field("credentials", &self.credentials).finish_non_exhaustive()
            }
        }
        const _: () = assert!($Prepared::GROWTH_STORAGE >= 3 * IMAGE_GROWTH + NAMESPACE_STORAGE);
    };
}
pub(crate) use preparation;
