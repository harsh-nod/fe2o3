//! One root composition over two closed native authority families.
macro_rules! inherited {
    ($Inherited:ident, $version:literal, $other:literal) => {
        use crate::{native_inherited::{self as root, Result, sum, length,
            CompilerExecutionRootStorageV2 as Storage}, native_root_source as source,
            CompilerExecutionSupervisorProgramSourcesV1 as Sources};
        use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
        use fe2o3_compiler_execution_supervisor::{IssuerServiceCredentialProfileV1 as Credentials,
            ProvisionedProtectedIssuerServiceInputsV2 as Inputs};
        use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrVerificationResourceErrorV1 as Resource};
        use fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2 as Cleanup;
        use fe2o3_protected_static_executable::ProtectedStaticExecutableV2 as Image;
        use std::{fmt, mem::size_of, time::Duration};

        /// Move-only native root composition from the fixed fourteen-descriptor ABI.
        /// Reads actual root-provisioned canonical records and zeroizing seeds, then
        /// constructs fresh native capabilities. No V1 admitted owner is converted.
        /// Filesystem provenance is not proof of compiler or GPU semantics.
        /// The installed system-manager entrypoint is a separate integration gate.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Inherited), " as Deployment;")]
        /// fn cloned<T: Clone>() {} cloned::<Deployment>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Inherited), " as Deployment;")]
        /// fn fd<T: std::os::fd::AsFd>() {} fd::<Deployment>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Inherited), " as Deployment, InheritedCompilerExecutionDeploymentV", $other, " as Other};")]
        /// fn mix(d: Deployment) -> Other { d.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Inherited), " as Deployment, InheritedCompilerExecutionDeploymentV1 as Old};")]
        /// fn upgrade(d: Old) -> Deployment { d.into() }
        /// ```
        pub struct $Inherited {
            programs: Sources,
            trust: Trust,
            inputs: Inputs,
            anchor: Anchor,
            supervisor_lifecycle: Lease,
            lifecycle: Lease,
            retained: usize,
        }
        impl $Inherited {
            const ENVELOPE: usize = size_of::<(Self, Storage)>() - size_of::<Sources>()
                - size_of::<Trust>() - size_of::<Inputs>() - size_of::<Anchor>() - 2 * size_of::<Lease>();
            const FRAME: usize = 4 * size_of::<(Self, Storage)>() + 16384;
            const SOURCE_LIMITS: [usize; 11] = [MAX_SUPERVISOR as usize, MAX_LAUNCHER as usize,
                MAX_LAUNCHER as usize, MAX_HELPER as usize, MAX_ANCHOR as usize,
                SUPERVISOR_BYTES, POLICY_BYTES, ANCHOR_BYTES, PROVISIONING_BYTES, 32, 32];

            /// Conservative raw-input reservation made before descriptor adoption.
            /// Includes five complete maximum-size images and every fixed record/seed.
            /// Nested native operations additionally meter their work and peak storage;
            /// this is not a complete successful-admission scratch or work quota.
            pub const SOURCE_STORAGE: usize = 14 * root::FILE_STORAGE + POLICY_BYTES
                + SUPERVISOR_BYTES + ANCHOR_BYTES + PROVISIONING_BYTES + 64
                + MAX_SUPERVISOR as usize + 2 * MAX_LAUNCHER as usize
                + MAX_ANCHOR as usize + MAX_HELPER as usize;

            /// Complete conservative admission envelope at the fixed image ceilings.
            /// Requires no descriptor access, seed or admitted authority. This query
            /// includes raw input reservations and every nested operation; it does
            /// not fund consuming launch, monitoring or persistent child cleanup.
            /// Successful funding cannot make invalid inputs or failed I/O succeed.
            pub fn admission_quota() -> Result<root::CompilerExecutionRootAdmissionQuotaV2> {
                let records = [
                    root::record_quota::<POLICY_BYTES>(POLICY_WORK, POLICY_SCRATCH,
                        PolicyCap::IO_WORK, PolicyCap::IO_STORAGE)?,
                    root::record_quota::<SUPERVISOR_BYTES>(SUPERVISOR_WORK, SUPERVISOR_SCRATCH,
                        SupervisorCap::IO_WORK, SupervisorCap::IO_STORAGE)?,
                    root::record_quota::<ANCHOR_BYTES>(ANCHOR_WORK, ANCHOR_SCRATCH,
                        AnchorCap::IO_WORK, AnchorCap::IO_STORAGE)?,
                    root::record_quota::<PROVISIONING_BYTES>(PROVISIONING_WORK, PROVISIONING_SCRATCH,
                        ProvisioningCap::IO_WORK, ProvisioningCap::IO_STORAGE)?,
                ];
                let anchor = Anchor::maximum_preparation_quota()?;
                Ok(root::CompilerExecutionRootAdmissionQuotaV2 {
                    work: sum(&[3 * root::LOCAL_WORK, 3 * Lease::ADMISSION_WORK,
                        2 * Lease::ROOT_BINDING_WORK, records[0].work(), records[1].work(),
                        records[2].work(), records[3].work(), Inputs::WORK,
                        2 * Inputs::LIFECYCLE_WORK, 5 * source::VALIDATE_WORK,
                        2 * source::SEED_WORK, Key::ADMISSION_WORK, AnchorKey::ADMISSION_WORK,
                        Trust::BIND_WORK, anchor.work()])?,
                    // Each fixed I/O envelope includes its output owner. Keeping a
                    // second envelope funds that owner's later outer reservation.
                    // Summing transient peaks deliberately overestimates overlap.
                    scratch: sum(&[Self::FRAME, Self::SOURCE_STORAGE, root::INTAKE_SCRATCH,
                        6 * Lease::IO_STORAGE, 2 * Lease::ROOT_BINDING_SCRATCH,
                        records[0].scratch(), records[1].scratch(), records[2].scratch(),
                        records[3].scratch(), root::LISTENER_SCRATCH, root::LISTENER_GROWTH,
                        Inputs::SCRATCH, Inputs::PAIR_STORAGE, Inputs::OWNER_GROWTH,
                        2 * Inputs::LIFECYCLE_SCRATCH, source::VALIDATE_SCRATCH,
                        2 * source::SEED_SCRATCH, 2 * source::SEED_STORAGE,
                        2 * Key::IO_STORAGE, 2 * AnchorKey::IO_STORAGE,
                        Trust::SCRATCH, Trust::GROWTH_STORAGE, anchor.scratch(),
                        Anchor::GROWTH_STORAGE, Self::ENVELOPE])?,
                })
            }

            /// Inert conservative bound for this full admitted owner at fixed image ceilings.
            pub fn maximum_retained_storage() -> Result<usize> {
                let image = |n| Image::file_storage_for_length(n)
                    .map_err(crate::CompilerExecutionPreparationErrorV2::from);
                sum(&[image(MAX_SUPERVISOR)?, image(MAX_LAUNCHER)?, image(MAX_LAUNCHER)?,
                    Trust::maximum_retained_storage()?, Inputs::PAIR_STORAGE, Inputs::OWNER_GROWTH,
                    Anchor::maximum_retained_storage()?, 2 * Lease::IO_STORAGE, Self::ENVELOPE])
            }

            /// Funds one complete startup and at most the specified positive numbers
            /// of monitoring/cleanup turns. Each monitoring turn may wait once, check
            /// continuity once and scan the entire cleanup pool once. Each cleanup turn
            /// may wait once, scan once and attempt shutdown once; the first shutdown
            /// attempt is separately prepaid at cleanup admission. Cancellation is
            /// already prepaid by launch. No account may be renewed between phases.
            ///
            /// Includes private activation mechanics but does not install a runner,
            /// authenticate provisioning, guarantee successful I/O or eventual reaping.
            /// Bounds cover root-coordinator work, not the executed programs' accounts.
            /// Quarantined/nonterminal custody must remain charged after the turn bound.
            pub fn startup_quota(monitor_ticks: usize, cleanup_turns: usize)
                -> Result<root::CompilerExecutionStartupQuotaV2> {
                use crate::native_activation as activation;
                use fe2o3_protected_service_spawn::{RetainedResourcesV2 as Resources,
                    MAX_PROTECTED_SERVICE_PROCESSES_V2 as CAPACITY};
                if monitor_ticks == 0 || cleanup_turns == 0 {
                    return Err(root::invalid("startup", "monitoring and cleanup require finite positive turns"));
                }
                let admission = Self::admission_quota()?;
                let guard = Anchor::maximum_cleanup_guard_quota(true)?;
                let anchor = Anchor::maximum_launch_quota()?;
                let preparation = Prepared::maximum_preparation_quota()?;
                let compiler = Prepared::maximum_launch_quota()?;
                let continuity = Managed::maximum_continuity_quota()?;
                let prepared_storage = Prepared::maximum_retained_storage()?;
                let launch_work = sum(&[root::LOCAL_WORK, Trust::REVALIDATION_WORK,
                    2 * Inputs::LIFECYCLE_WORK, 3 * source::VALIDATE_WORK, guard.work(),
                    anchor.work(), preparation.work(), compiler.work()])?;
                let launch_peak = sum(&[Self::maximum_retained_storage()?, Self::FRAME,
                    Trust::SCRATCH, Inputs::LIFECYCLE_SCRATCH, source::VALIDATE_SCRATCH,
                    guard.scratch(), anchor.scratch(), ManagedAnchor::maximum_retained_storage()?,
                    preparation.scratch(), prepared_storage, compiler.scratch(),
                    Managed::maximum_retained_storage()?])?;
                let lifetime_peak = sum(&[Managed::maximum_retained_storage()?,
                    crate::native::maximum(&[continuity.scratch(), activation::WAIT_SCRATCH,
                        activation::RESTORE_SCRATCH, activation::PUBLISH_SCRATCH])])?;
                let scans = sum(&[monitor_ticks, cleanup_turns])?;
                Ok(root::CompilerExecutionStartupQuotaV2 {
                    request_work: sum(&[2 * root::LOCAL_WORK, activation::CAPTURE_WORK,
                        activation::INSTALL_WORK, activation::PUBLISH_WORK, activation::RESTORE_WORK,
                        admission.work(), launch_work,
                        root::repeated(monitor_ticks, sum(&[root::TURN_WORK,
                            activation::WAIT_WORK, continuity.work()])?)?,
                        root::repeated(cleanup_turns, root::TURN_WORK + activation::WAIT_WORK)?])?,
                    request_storage: sum(&[Self::FRAME.max(crate::native_entrypoint::FRAME), activation::ACTIVATION_STORAGE,
                        activation::SIGNALS_STORAGE, crate::native::maximum(&[
                            activation::CAPTURE_SCRATCH, activation::INSTALL_SCRATCH,
                            activation::PUBLISH_SCRATCH, admission.scratch(), launch_peak, lifetime_peak])])?,
                    cleanup_work: sum(&[Cleanup::ADMISSION_WORK, Cleanup::GUARD_WORK,
                        2 * Cleanup::GUARD_CLONE_WORK,
                        Cleanup::retained_launch_work::<Prepared>(prepared_storage)?,
                        root::repeated(scans, Cleanup::pump_work(CAPACITY)?)?,
                        root::repeated(cleanup_turns, Cleanup::shutdown_work())?])?,
                    cleanup_storage: sum(&[Cleanup::STORAGE,
                        Resources::<Prepared>::payload_storage(prepared_storage)?])?,
                })
            }

            /// Admits one complete inherited native deployment, returning its FULL
            /// unreserved owner charge. All source bytes and nested native owners are
            /// funded on the supplied original ledger before access/retention. Entry
            /// storage is restored on every exit; work and first-denial history remain.
            /// No child is created. On refusal, adopted descriptors close exactly once.
            ///
            /// # Safety
            /// Only the single-threaded root activation entrypoint may call this.
            /// It must transfer unique ownership of all FDs 3..=16, with no Rust owner,
            /// signal handler, or foreign code able to close/reuse them during intake.
            /// A successful intake consumes these slots; repeating it is forbidden.
            ///
            /// ```compile_fail
            #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Inherited), " as Deployment;")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// fn missing_ownership_contract(b: &mut Budget<'_>) { let _ = Deployment::admit(b); }
            /// ```
            #[allow(unsafe_code)]
            pub unsafe fn admit(b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                b.with_prepaid_scope(0, 8, root::LOCAL_WORK, Self::FRAME, |b| {
                    crate::native::require_root()?;
                    b.reserve_storage(Self::SOURCE_STORAGE)?;
                    // SAFETY: inherited ownership is transferred by this API's caller.
                    let [runtime_root, supervisor_root, anchor_root, supervisor, launcher, issuer,
                        helper, daemon, supervisor_file, policy_file, anchor_file, provisioning_file,
                        issuer_seed, anchor_seed] = unsafe { root::take(&Self::SOURCE_LIMITS, b) }?;

                    // Three independent OFDs, joined to the same canonical parent,
                    // before either secret is read or any child can exist.
                    let (lifecycle, c) = Lease::open(&supervisor_root, b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (supervisor_lifecycle, c) = Lease::open(&supervisor_root, b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (anchor_lifecycle, c) = Lease::open(&anchor_root, b)?;
                    b.reserve_storage(c.additional_storage())?;
                    anchor_lifecycle.revalidate_for_root(&supervisor_root, b)?;
                    lifecycle.revalidate_for_root(&anchor_root, b)?;

                    let (policy, c) = root::record::<POLICY_BYTES, _>(policy_file, b, |bytes, b| {
                        let (record, c) = Policy::decode(bytes, b)?;
                        b.reserve_storage(c.additional_storage())?;
                        let (cap, c) = PolicyCap::create(record, b)?;
                        b.reserve_storage(c.additional_storage())?;
                        let retained = cap.retained_storage();
                        Ok((cap, retained))
                    })?;
                    b.reserve_storage(c)?;
                    let (deployment, c) = root::record::<SUPERVISOR_BYTES, _>(supervisor_file, b, |bytes, b| {
                        let (record, c) = Supervisor::decode(bytes, policy.policy(), b)?;
                        b.reserve_storage(c.additional_storage())?;
                        let (cap, c) = SupervisorCap::create(record, b)?;
                        b.reserve_storage(c.additional_storage())?;
                        let retained = cap.retained_storage();
                        Ok((cap, retained))
                    })?;
                    b.reserve_storage(c)?;
                    let (anchor_deployment, c) = root::record::<ANCHOR_BYTES, _>(anchor_file, b, |bytes, b| {
                        let (record, c) = AnchorDeployment::decode(bytes, deployment.deployment(), policy.policy(), b)?;
                        b.reserve_storage(c.additional_storage())?;
                        let (cap, c) = AnchorCap::create(record, b)?;
                        b.reserve_storage(c.additional_storage())?;
                        let retained = cap.retained_storage();
                        Ok((cap, retained))
                    })?;
                    b.reserve_storage(c)?;
                    let (provisioning, c) = root::record::<PROVISIONING_BYTES, _>(provisioning_file, b, |bytes, b| {
                        let (record, c) = Provisioning::decode(bytes, anchor_deployment.deployment(), b)?;
                        b.reserve_storage(c.additional_storage())?;
                        let (cap, c) = ProvisioningCap::create(record, b)?;
                        b.reserve_storage(c.additional_storage())?;
                        let retained = cap.retained_storage();
                        Ok((cap, retained))
                    })?;
                    b.reserve_storage(c)?;

                    let credentials = Credentials::new(deployment.deployment().service_uid(),
                        deployment.deployment().service_gid())?;
                    let (mut listener, c) = root::listener(runtime_root.into(), credentials.gid(), b)?;
                    b.reserve_storage(c)?;
                    let (inputs, c) = Inputs::admit(listener.take_descriptor()?, supervisor_root, credentials, b)?;
                    b.reserve_storage(c.additional_storage())?;
                    inputs.validate_lifecycle(&lifecycle, b)?;
                    inputs.validate_lifecycle(&supervisor_lifecycle, b)?;

                    // Bound declared lengths as well as the preflighted actual files,
                    // especially the policy's issuer, before reading either seed.
                    let measurements = [
                        crate::native::measurement(deployment.deployment().executable(), MAX_SUPERVISOR)?,
                        crate::native::measurement(deployment.deployment().launcher(), MAX_LAUNCHER)?,
                        crate::native::measurement(policy.policy().executable(), MAX_LAUNCHER)?,
                        crate::native::measurement(provisioning.provisioning().helper(), MAX_HELPER)?,
                        crate::native::measurement(anchor_deployment.deployment().executable(), MAX_ANCHOR)?,
                    ];
                    let source_storage = sum(&[
                        Image::file_storage(measurements[0]).map_err(crate::CompilerExecutionPreparationErrorV2::from)?,
                        Image::file_storage(measurements[1]).map_err(crate::CompilerExecutionPreparationErrorV2::from)?,
                        Image::file_storage(measurements[2]).map_err(crate::CompilerExecutionPreparationErrorV2::from)?,
                    ])?;
                    for (file, measurement) in [&supervisor, &launcher, &issuer, &helper, &daemon].into_iter().zip(measurements) {
                        source::validate_executable(file, length(measurement.byte_len())?, b)?;
                    }
                    let (mut seed, c) = source::read_seed(&issuer_seed, b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (key, kc) = Key::create_and_zeroize(&mut seed, policy.policy(), b)?;
                    b.reserve_storage(kc.additional_storage())?;
                    drop(seed);
                    b.release_storage(c.additional_storage())?;
                    let (mut seed, c) = source::read_seed(&anchor_seed, b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (anchor_key, kc) = AnchorKey::create_and_zeroize(&mut seed, anchor_deployment.deployment(), b)?;
                    b.reserve_storage(kc.additional_storage())?;
                    drop(seed);
                    b.release_storage(c.additional_storage())?;
                    drop(issuer_seed);
                    drop(anchor_seed);

                    let (trust, c) = Trust::new(deployment, policy, key, b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (anchor, c) = Anchor::prepare(helper, daemon, anchor_root, anchor_lifecycle,
                        anchor_deployment, provisioning, anchor_key, trust.deployment(), trust.policy(), b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let retained = sum(&[source_storage, trust.retained_storage(), inputs.retained_storage(),
                        anchor.retained_storage(), supervisor_lifecycle.retained_storage(), lifecycle.retained_storage(), Self::ENVELOPE])?;
                    b.reserve_storage(Self::ENVELOPE)?;
                    let value = Self { programs: Sources::new(supervisor, launcher, issuer), trust,
                        inputs, anchor, supervisor_lifecycle, lifecycle, retained };
                    // Only complete admission may leave the fixed listener pathname.
                    listener.disarm_cleanup();
                    Ok((value, Storage(retained)))
                })
            }

            /// Full retained reservation, excluding independent cleanup-pool funding.
            pub const fn retained_storage(&self) -> usize { self.retained }

            /// Installs the canonical guard before launching the anchor, then consumes
            /// all resources through native supervisor preparation and retained launch.
            /// Keep this complete input reservation and reserve returned growth. Work,
            /// nested scratch and persistent cleanup funding may still refuse; a finite
            /// account does not promise successful startup or eventual child reaping.
            pub fn launch(self, timeout: Duration, cleanup: &mut Cleanup, b: &mut Budget<'_>)
                -> Result<(Managed, Storage)> {
                let input = self.retained;
                b.with_prepaid_scope(input, 8, root::LOCAL_WORK, Self::FRAME, |b| {
                    crate::native::require_root()?;
                    self.trust.revalidate(b)?;
                    self.inputs.validate_lifecycle(&self.lifecycle, b)?;
                    self.inputs.validate_lifecycle(&self.supervisor_lifecycle, b)?;
                    let Sources { supervisor, launcher, issuer } = self.programs;
                    for (file, measurement) in [(&supervisor, self.trust.deployment().deployment().executable()),
                        (&launcher, self.trust.deployment().deployment().launcher()), (&issuer, self.trust.policy().policy().executable())] {
                        source::validate_executable(file, length(measurement.byte_len())?, b)?;
                    }
                    self.anchor.retain_cleanup_guard(self.trust.deployment(), self.trust.policy(), cleanup, b)?;
                    let (anchor, c) = self.anchor.launch(self.trust.deployment(), self.trust.policy(), timeout, cleanup, b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (prepared, c) = Prepared::prepare(Sources::new(supervisor, launcher, issuer), self.trust,
                        self.inputs, self.supervisor_lifecycle, self.lifecycle, anchor, b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (managed, c) = prepared.launch(timeout, cleanup, b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let growth = managed.retained_storage().checked_sub(input).ok_or(Resource::Accounting)?;
                    Ok((managed, Storage(growth)))
                })
            }
        }
        impl fmt::Debug for $Inherited {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Inherited)).field("authority", &"native-root-composition-only").finish_non_exhaustive()
            }
        }
    };
}
pub(crate) use inherited;
