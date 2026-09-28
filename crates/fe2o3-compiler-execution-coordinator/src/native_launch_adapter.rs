//! One consuming schedule over genuine V2/V3 preparations; no provider interface.
macro_rules! launch {
    ($Prepared:ident, $Managed:ident, $version:literal, $other:literal) => {
        use crate::native_launch::{self as launch, Channels, Result, LOCAL_WORK, sum,
            CompilerExecutionLaunchErrorV2 as Error, CompilerExecutionLaunchQuotaV2 as Quota,
            CompilerExecutionLaunchStorageV2 as Storage};
        use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
        use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV2 as Inputs;
        use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrVerificationResourceErrorV1 as Resource};
        use fe2o3_protected_service_profile::{ProtectedServiceNamespaceSetV2 as Namespaces, observations};
        use fe2o3_protected_service_spawn::{ProtectedServiceCleanupServiceV2 as Cleanup,
            ProtectedServiceDescriptorBindingV1 as Binding, RetainedResourcesV2 as Resources,
            cleanup_bridge::CleanupPollV1 as CleanupPoll, launch_io,
            native_spawn::{StagedProtectedServiceExecV2 as Stage,
                RootOwnedProtectedServiceChildV2 as PlainChild,
                RootOwnedRetainedServiceChildV2 as RetainedChild}};
        use fe2o3_protected_static_executable::{ProtectedStaticExecutableV2 as Image,
            ProtectedStaticExecutableOperationV2 as ImageOperation};
        use std::{fmt, mem::size_of, os::fd::AsFd, time::{Duration, Instant}};

        type Child = RetainedChild<$Prepared>;
        const DESTINATIONS: [i32; 11] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 220];
        const _: () = {
            use fe2o3_compiler_execution_supervisor::*;
            use fe2o3_compiler_closure_capability::COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1;
            assert!(DESTINATIONS[0] == COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1);
            assert!(DESTINATIONS[1] == COMPILER_EXECUTION_SUPERVISOR_ROOT_FD_V1);
            assert!(DESTINATIONS[2] == COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_FD_V1);
            assert!(DESTINATIONS[3] == COMPILER_EXECUTION_SUPERVISOR_ISSUER_FD_V1);
            assert!(DESTINATIONS[4] == COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1);
            assert!(DESTINATIONS[5] == COMPILER_EXECUTION_SUPERVISOR_SIGNING_KEY_FD_V1);
            assert!(DESTINATIONS[6] == COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PEER_FD_V1);
            assert!(DESTINATIONS[7] == COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1);
            assert!(DESTINATIONS[8] == COMPILER_EXECUTION_SUPERVISOR_BOOTSTRAP_FD_V1);
            assert!(DESTINATIONS[9] == COMPILER_EXECUTION_SUPERVISOR_LIFECYCLE_FD_V1);
            assert!(DESTINATIONS[10] == COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1);
            assert!(READY_BYTES == launch_io::MAX_READY_BYTES);
        };
        const BINDINGS_STORAGE: usize = size_of::<[Binding<'static>; 11]>();
        const FRAME: usize = 4 * size_of::<($Managed, Storage)>() + 4 * size_of::<Stage>()
            + 8 * size_of::<Error>() + 4 * size_of::<rustix::fs::Stat>() + 16384;

        fn decode_ready(bytes: &[u8], pid: rustix::process::Pid, deployment: &Deployment,
            b: &mut Budget<'_>) -> Result<(Ready, usize)> {
            let (ready, charge) = Ready::decode(bytes, launch::pid_u32(pid)?, deployment, b)?;
            Ok((ready, charge.additional_storage()))
        }

        impl $Prepared {
            fn transfer_source_storage(&self) -> Result<usize> {
                Self::transfer_source_storage_for(self.programs.each_ref().map(|p| p.measurement().byte_len()))
            }

            fn transfer_source_storage_for(n: [u64; 3]) -> Result<usize> {
                sum(&[Image::file_storage_for_length(n[0])?,
                    Image::file_storage_for_length(n[1])?,
                    Image::file_storage_for_length(n[2])?, Inputs::PAIR_STORAGE,
                    PolicyCap::FILE_STORAGE, Key::FILE_STORAGE, DeploymentCap::FILE_STORAGE,
                    AnchorTransfer::STORAGE, Lease::FILE_STORAGE, 4 * launch::FILE_STORAGE,
                    BINDINGS_STORAGE])
            }

            fn staging_quota(&self) -> Result<Quota> {
                Self::staging_quota_for(self.programs.each_ref().map(|p| p.measurement().byte_len()),
                    self.revalidation_quota()?, self.anchor.supervisor_transfer_quota()?,
                    self.anchor.supervisor_transfer_validation_quota()?)
            }

            fn staging_quota_for(n: [u64; 3], validation: crate::CompilerExecutionPreparationQuotaV2,
                anchor: fe2o3_external_anchor_coordinator::ExternalAnchorSupervisorTransferQuotaV2,
                anchor_check: fe2o3_external_anchor_coordinator::ExternalAnchorSupervisorTransferQuotaV2)
                -> Result<Quota> {
                let image = [Image::quota_for_length(n[0], ImageOperation::Transfer)?,
                    Image::quota_for_length(n[1], ImageOperation::Transfer)?,
                    Image::quota_for_length(n[2], ImageOperation::Transfer)?];
                let source = Self::transfer_source_storage_for(n)?;
                let stage = Stage::storage_for_sources(source)?;
                let transfer = sum(&[image[0].work(), image[1].work(), image[2].work(),
                    Inputs::WORK, PolicyCap::IO_WORK, Key::IO_WORK, DeploymentCap::IO_WORK,
                    Lease::TRANSFER_WORK])?;
                Ok(Quota {
                    work: sum(&[LOCAL_WORK, validation.work(), validation.work(), transfer, transfer,
                        anchor.work(), anchor_check.work(), AnchorTransfer::INTO_DESCRIPTORS_WORK,
                        Stage::STAGING_WORK])?,
                    scratch: sum(&[FRAME, source, stage, stage, Stage::STAGING_SCRATCH,
                        validation.scratch(), image[0].scratch(), image[1].scratch(), image[2].scratch(),
                        Inputs::SCRATCH, PolicyCap::IO_STORAGE, Key::IO_STORAGE, DeploymentCap::IO_STORAGE,
                        Lease::IO_STORAGE, anchor.scratch(), anchor_check.scratch(),
                        AnchorTransfer::INTO_DESCRIPTORS_SCRATCH])?,
                })
            }

            fn process_quota(&self) -> Result<Quota> {
                Self::process_quota_for(self.revalidation_quota()?)
            }

            fn process_quota_for(validation: crate::CompilerExecutionPreparationQuotaV2) -> Result<Quota> {
                Ok(Quota {
                    work: sum(&[validation.work(), Namespaces::REVALIDATE_PROCESS_WORK,
                        observations::PROCESS_VALIDATE_WORK])?,
                    scratch: sum(&[validation.scratch(), Namespaces::REVALIDATE_PROCESS_SCRATCH,
                        observations::PROCESS_VALIDATE_SCRATCH])?,
                })
            }

            fn managed_quota(&self) -> Result<Quota> {
                Self::managed_quota_for(self.process_quota()?)
            }

            fn managed_quota_for(process: Quota) -> Result<Quota> {
                Ok(Quota {
                    work: sum(&[LOCAL_WORK, Resources::<Self>::ACCESS_WORK, process.work,
                        READY_WORK, PlainChild::OPERATION_WORK])?,
                    scratch: sum(&[FRAME, Resources::<Self>::ACCESS_SCRATCH, process.scratch,
                        READY_SCRATCH, PlainChild::OPERATION_SCRATCH])?,
                })
            }

            /// Complete successful request envelope, including finite readiness retries.
            /// Keep retained_storage prepaid. The original cleanup account separately
            /// funds Cleanup::retained_launch_work and Resources::payload_storage for
            /// this entire preparation; successful funding is not a deadline promise.
            pub fn launch_quota(&self) -> Result<Quota> {
                Self::launch_quota_for(self.retained, self.transfer_source_storage()?,
                    self.staging_quota()?, self.cleanup_guard_quota()?, self.process_quota()?)
            }

            /// Conservative complete request envelope before native preparation exists.
            /// Uses the same calculation at fixed image ceilings and retained-owner bounds.
            /// Persistent cleanup funding is still separate; this query grants no authority.
            pub fn maximum_launch_quota() -> Result<Quota> {
                let n = [super::MAX_SUPERVISOR, super::MAX_LAUNCHER, super::MAX_LAUNCHER];
                Self::launch_quota_for(Self::maximum_retained_storage()?,
                    Self::transfer_source_storage_for(n)?,
                    Self::staging_quota_for(n, Self::maximum_revalidation_quota()?,
                        super::Anchor::maximum_supervisor_transfer_quota()?,
                        super::Anchor::maximum_supervisor_transfer_validation_quota()?)?,
                    Self::maximum_cleanup_guard_quota()?,
                    Self::process_quota_for(Self::maximum_revalidation_quota()?)?)
            }

            fn launch_quota_for(retained: usize, source: usize, staging: Quota,
                guard: crate::CompilerExecutionPreparationQuotaV2, process: Quota) -> Result<Quota> {
                let polling = launch_io::MAX_LIVENESS_CHECKS.checked_mul(PlainChild::OPERATION_WORK)
                    .and_then(|n| n.checked_add(launch_io::MAX_WORK)).ok_or(Resource::Arithmetic)?;
                let child_growth = Child::storage_for(retained)?.checked_sub(retained)
                    .ok_or(Resource::Accounting)?;
                Ok(Quota {
                    work: sum(&[LOCAL_WORK, guard.work(), staging.work, process.work, process.work,
                        3 * Resources::<Self>::ACCESS_WORK, 2 * READY_WORK,
                        Stage::spawn_work_for(DESTINATIONS.len(), 63)?,
                        Cleanup::retained_launch_work::<Self>(retained)?, polling,
                        2 * PlainChild::OPERATION_WORK])?,
                    scratch: sum(&[FRAME, Channels::STORAGE, guard.scratch(), staging.scratch,
                        Stage::storage_for_sources(source)?,
                        Stage::spawn_retaining_scratch::<Self>(retained)?, child_growth,
                        Resources::<Self>::ACCESS_SCRATCH, process.scratch, READY_BYTES,
                        READY_SCRATCH, READY_OWNER_STORAGE, launch_io::ATTEMPT_SCRATCH,
                        PlainChild::OPERATION_SCRATCH, $Managed::ENVELOPE])?,
                })
            }

            #[allow(unsafe_code)]
            fn stage_transfers(&self, channels: &Channels, b: &mut Budget<'_>) -> Result<(Stage, usize)> {
                let floor = sum(&[self.retained, Channels::STORAGE])?;
                b.with_prepaid_scope(floor, 8, LOCAL_WORK, FRAME, |b| {
                    self.revalidate(b)?;
                    let (supervisor, c) = self.programs[0].try_clone_for_exec(b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (launcher, c) = self.programs[1].try_clone_for_exec(b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (issuer, c) = self.programs[2].try_clone_for_exec(b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let ((listener, root), c) = self.service_inputs.try_clone_ordered_for_spawn(b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (policy, c) = self.trust.policy().try_clone_for_transfer(b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (key, c) = self.trust.key_template().try_clone_for_transfer(b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (deployment, c) = self.trust.deployment().try_clone_for_transfer(b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (lease, c) = self.supervisor_lifecycle.try_clone_for_transfer(b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (anchor, c) = self.anchor.try_clone_for_supervisor(
                        self.trust.deployment(), self.trust.policy(), b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (peer, pidfd) = anchor.into_ordered_descriptors(b)?;
                    b.reserve_storage(BINDINGS_STORAGE)?;
                    let sources = [listener.as_fd(), root.as_fd(), launcher.as_fd(), issuer.as_fd(),
                        policy.as_fd(), key.as_fd(), peer.as_fd(), pidfd.as_fd(), channels.child.as_fd(),
                        lease.as_fd(), deployment.as_fd()];
                    let mut bindings = [Binding::new(sources[0], DESTINATIONS[0])
                        .map_err(|_| Error::Invalid("invalid supervisor binding"))?; 11];
                    for ((binding, source), destination) in bindings.iter_mut().zip(sources).zip(DESTINATIONS) {
                        *binding = Binding::new(source, destination)
                            .map_err(|_| Error::Invalid("invalid supervisor binding"))?;
                    }
                    // SAFETY: this closed table charges every complete native transfer
                    // image and channel. Their original owners remain live; every final
                    // staged File is validated below before the stage can leave this scope.
                    let (stage, c) = unsafe { Stage::stage(&supervisor, &bindings,
                        channels.profile_writer.as_fd(), channels.gate_reader.as_fd(),
                        channels.child.as_fd(), self.transfer_source_storage()?, b) }?;
                    b.reserve_storage(c.additional_storage())?;
                    self.validate_staged(&stage, channels, b)?;
                    Ok((stage, c.additional_storage()))
                })
            }

            fn validate_staged(&self, stage: &Stage, channels: &Channels, b: &mut Budget<'_>) -> Result<()> {
                self.revalidate(b)?;
                let file = |fd| stage.binding(fd).ok_or(Error::Invalid("missing supervisor staged input"));
                self.programs[0].revalidate_exec_clone(stage.executable(), b)?;
                self.programs[1].revalidate_exec_clone(file(5)?, b)?;
                self.programs[2].revalidate_exec_clone(file(6)?, b)?;
                self.service_inputs.validate_transfer(file(3)?.as_fd(), file(4)?, b)?;
                self.trust.policy().validate_transfer(file(7)?, b)?;
                self.trust.key_template().validate_transfer(file(8)?, self.trust.policy().policy(), b)?;
                self.anchor.validate_supervisor_transfer(file(9)?.as_fd(), file(10)?.as_fd(),
                    self.trust.deployment(), self.trust.policy(), b)?;
                self.supervisor_lifecycle.validate_transfer(file(12)?, b)?;
                self.trust.deployment().validate_transfer(file(220)?, b)?;
                let original = rustix::fs::fstat(&channels.child).map_err(|e| launch::io("inspect supervisor bootstrap", e))?;
                let staged = rustix::fs::fstat(file(11)?).map_err(|e| launch::io("inspect staged supervisor bootstrap", e))?;
                if (original.st_dev, original.st_ino, original.st_mode) != (staged.st_dev, staged.st_ino, staged.st_mode)
                    || !rustix::io::fcntl_getfd(file(11)?).map_err(|e| launch::io("inspect supervisor bootstrap flags", e))?
                        .contains(rustix::io::FdFlags::CLOEXEC) {
                    return Err(Error::Invalid("supervisor bootstrap object changed"));
                }
                Ok(())
            }

            fn validate_process(&self, pid: rustix::process::Pid, b: &mut Budget<'_>) -> Result<()> {
                self.revalidate(b)?;
                self.namespaces.revalidate_process(pid, b)?;
                b.with_prepaid_scope(self.retained, 0, observations::PROCESS_VALIDATE_WORK,
                    observations::PROCESS_VALIDATE_SCRATCH, |_| -> Result<()> {
                        Ok(observations::validate_process(self.credentials, pid)?)
                    })
            }

            /// Consumes all prepared resources into ordered native supervisor custody.
            /// The cleanup guard must already have been installed before anchor launch.
            /// Refusal consumes preparation without retiring its request reservation;
            /// pending/quarantined supervisor cleanup retains preparation and the anchor.
            /// Reserve returned growth before retaining the managed result. Inherited
            /// root startup/provisioning provenance remain separate deployment obligations.
            ///
            /// ```
            #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Prepared), " as Prepared, ", stringify!($Managed), " as Managed, CompilerExecutionLaunchErrorV2 as Error, CompilerExecutionLaunchStorageV2 as Storage};")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// use fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2 as Cleanup;
            /// fn launch(p: Prepared, c: &mut Cleanup, b: &mut Budget<'_>) -> Result<(Managed, Storage), Error> {
            ///     p.launch(std::time::Duration::from_secs(30), c, b)
            /// }
            /// ```
            #[allow(unsafe_code)]
            pub fn launch(self, timeout: Duration, cleanup: &mut Cleanup, b: &mut Budget<'_>)
                -> Result<($Managed, Storage)> {
                let input = self.retained;
                b.with_prepaid_scope(input, 8, LOCAL_WORK, FRAME + launch_io::ATTEMPT_SCRATCH, |b| {
                    let deadline = launch_io::bounded_deadline(timeout)?;
                    self.validate_cleanup_guard(cleanup, b)?;
                    let continuity = self.managed_quota()?;
                    let credentials = self.credentials;
                    b.reserve_storage(Channels::STORAGE)?;
                    let channels = Channels::new()?;
                    let (stage, stage_charge) = self.stage_transfers(&channels, b)?;
                    b.reserve_storage(stage_charge)?;
                    if Instant::now() >= deadline { return Err(launch_io::Failure::Timeout("supervisor staging").into()); }
                    // SAFETY: actual final staged Files passed all native context checks;
                    // this complete prepared owner and its separately prepaid anchor
                    // cancellation enter the reserved slot BEFORE clone. Only this child
                    // owner/pool consumes waits. No preparation or raw transfer escapes.
                    let (mut child, c) = unsafe { stage.spawn_retaining(credentials, self, input, cleanup, b) }?;
                    b.reserve_storage(c.additional_storage())?;
                    drop(stage);
                    b.release_storage(stage_charge)?;
                    let Channels { root, child: bootstrap_child, profile_reader, profile_writer,
                        gate_reader, gate_writer } = channels;
                    drop(bootstrap_child); drop(profile_writer); drop(gate_reader);
                    launch_io::await_profile_ready(profile_reader.as_fd(), root.as_fd(),
                        &mut launch::Observer { child: &child, budget: b }, deadline)?;
                    child.with_resources(b, |p, b| p.validate_process(child.pid(), b))?;
                    launch_io::release_child(gate_writer.as_fd(),
                        &mut launch::Observer { child: &child, budget: b }, deadline)?;
                    drop(gate_writer);
                    let (readiness, ready_charge) = {
                        b.reserve_storage(READY_BYTES)?;
                        let (bytes, fd) = launch_io::receive_ready::<READY_BYTES, false, _>(root.as_fd(),
                            &mut launch::Observer { child: &child, budget: b }, deadline)?;
                        if fd.is_some() { return Err(Error::Invalid("unexpected supervisor ready descriptor")); }
                        child.with_resources(b, |p, b| decode_ready(&bytes, child.pid(), p.trust.deployment().deployment(), b))?
                    };
                    b.release_storage(READY_BYTES)?;
                    b.reserve_storage(ready_charge)?;
                    launch_io::await_exec_eof(root.as_fd(),
                        &mut launch::Observer { child: &child, budget: b }, deadline)?;
                    child.with_resources(b, |p, b| -> Result<()> {
                        p.validate_process(child.pid(), b)?;
                        if !readiness.matches_deployment(launch::pid_u32(child.pid())?, p.trust.deployment().deployment(), b)? {
                            return Err(Error::Invalid("supervisor readiness context changed"));
                        }
                        Ok(())
                    })?;
                    if !child.is_live(b)? { return Err(launch_io::Failure::ChildExited("supervisor readiness").into()); }
                    if Instant::now() >= deadline { return Err(launch_io::Failure::Timeout("supervisor final validation").into()); }
                    // SAFETY: private exact-family ready binds this actual PID/deployment;
                    // CLOEXEC EOF follows its measured exec. Final profile, context and
                    // pidfd liveness checks passed; all bootstrap aliases are closed here.
                    unsafe { child.confirm_exec(b) }?;
                    let retained = sum(&[child.retained_storage(), ready_charge, $Managed::ENVELOPE])?;
                    let growth = retained.checked_sub(input).ok_or(Resource::Accounting)?;
                    b.reserve_storage($Managed::ENVELOPE)?;
                    Ok(($Managed { child, readiness, retained, continuity }, Storage(growth)))
                })
            }
        }

        /// Root-managed native supervisor with ordered complete preparation custody.
        /// The shared pool retains preparation through deferred/quarantined cleanup;
        /// exact supervisor termination permits nested anchor cancellation, which may
        /// itself remain deferred. No publication, compiler result or GPU authority
        /// is granted. Retire full retained_storage only after this owner is dropped.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Managed), " as Managed;")]
        /// fn cloned<T: Clone>() {} cloned::<Managed>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Managed), " as Managed;")]
        /// fn fd<T: std::os::fd::AsFd>() {} fd::<Managed>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Managed), " as Managed, RootManagedCompilerExecutionServiceV", $other, " as Other};")]
        /// fn mix(m: Managed) -> Other { m.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Managed), " as Managed, RootManagedCompilerExecutionServiceV1 as Old};")]
        /// fn upgrade(m: Old) -> Managed { m.into() }
        /// ```
        pub struct $Managed {
            child: Child,
            readiness: Ready,
            retained: usize,
            continuity: Quota,
        }
        impl $Managed {
            const ENVELOPE: usize = size_of::<(Self, Storage)>() - size_of::<Child>() - size_of::<Ready>();
            /// Inert conservative charge for the complete managed owner at image ceilings.
            pub fn maximum_retained_storage() -> Result<usize> {
                sum(&[Child::storage_for($Prepared::maximum_retained_storage()?)?,
                    READY_OWNER_STORAGE, Self::ENVELOPE])
            }
            /// Same continuity calculation as launch, without a running child or authority.
            pub fn maximum_continuity_quota() -> Result<Quota> {
                $Prepared::managed_quota_for($Prepared::process_quota_for(
                    $Prepared::maximum_revalidation_quota()?)?)
            }
            /// Scalar child PID; not standalone signal or launch authority.
            pub fn pid(&self) -> rustix::process::Pid { self.child.pid() }
            /// Exact same-family private-bootstrap record, not independent admission.
            pub const fn readiness(&self) -> &Ready { &self.readiness }
            /// Full original-request charge, excluding independent persistent cleanup funding.
            pub const fn retained_storage(&self) -> usize { self.retained }
            /// Immutable continuity envelope derived from the retained preparation.
            pub const fn continuity_quota(&self) -> Quota { self.continuity }
            /// Rechecks root, all retained inputs, namespaces/profile, readiness and liveness.
            pub fn validate_continuity(&self, b: &mut Budget<'_>) -> Result<()> {
                b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
                    self.child.with_resources(b, |p, b| -> Result<()> {
                        p.validate_process(self.child.pid(), b)?;
                        if !self.readiness.matches_deployment(launch::pid_u32(self.child.pid())?,
                            p.trust.deployment().deployment(), b)? {
                            return Err(Error::Invalid("managed supervisor readiness context changed"));
                        }
                        Ok(())
                    })?;
                    if !self.child.is_live(b)? { return Err(launch_io::Failure::ChildExited("managed supervisor").into()); }
                    Ok(())
                })
            }
            /// Consumes foreground custody with one prepaid supervisor cancellation step.
            /// Even Reaped describes only the supervisor; nested anchor cleanup may defer.
            pub fn cancel(mut self) -> CleanupPoll { self.child.cancel() }
        }
        impl fmt::Debug for $Managed {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Managed)).field("authority", &"native-service-custody-only")
                    .field("pid", &self.pid()).field("readiness", &self.readiness.identity()).finish_non_exhaustive()
            }
        }
    };
}
pub(crate) use launch;
