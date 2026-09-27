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
