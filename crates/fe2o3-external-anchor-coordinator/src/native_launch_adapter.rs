//! Closed family integration: actual native owners, one spawn/readiness engine.
macro_rules! launch {
    ($Prepared:ident, $Managed:ident, $version:literal, $other:literal) => {
        use crate::{launch_io, native_launch::{self as launch, Channels,
            ExternalAnchorLaunchErrorV2 as LaunchError, ExternalAnchorLaunchStorageV2 as LaunchStorage,
            ExternalAnchorLaunchQuotaV2 as LaunchQuota, Result as LaunchResult}};
        use fe2o3_broker_authority_service::{ProtectedExternalAnchorServiceAdmissionV2 as Admission,
            ProtectedExternalAnchorServiceStorageV2 as AdmissionStorage};
        use fe2o3_protected_service_profile::{ProtectedServiceNamespaceSetV2 as Namespaces,
            ProtectedServiceProfileStorageV2 as ProfileStorage,
            ProtectedServiceCredentialProfileV1 as Credentials, observations};
        use fe2o3_protected_service_spawn::{ProtectedServiceCleanupServiceV2 as Cleanup,
            cleanup_bridge::CleanupPollV1 as CleanupPoll,
            ProtectedServiceDescriptorBindingV1 as Binding,
            native_spawn::{StagedProtectedServiceExecV2 as Stage, RootOwnedProtectedServiceChildV2 as Child}};
        use std::{os::fd::AsFd, time::Duration};

        const DESTINATIONS: [i32; 9] = [3, 4, 5, 6, 202, 220, 221, 222, 223];
        const _: () = {
            use fe2o3_external_anchor_provisioner::*;
            use fe2o3_compiler_closure_capability::*;
            assert!(DESTINATIONS[0] == EXTERNAL_ANCHOR_HELPER_BOOTSTRAP_FD_V1);
            assert!(DESTINATIONS[1] == EXTERNAL_ANCHOR_HELPER_ROOT_FD_V1);
            assert!(DESTINATIONS[2] == EXTERNAL_ANCHOR_HELPER_DAEMON_EXECUTABLE_FD_V1);
            assert!(DESTINATIONS[3] == EXTERNAL_ANCHOR_HELPER_LIFECYCLE_FD_V1);
            assert!(DESTINATIONS[4] == COMPILER_EXECUTION_POLICY_CHILD_FD_V1);
            assert!(DESTINATIONS[5] == COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1);
            assert!(DESTINATIONS[6] == COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1);
            assert!(DESTINATIONS[7] == COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1);
            assert!(DESTINATIONS[8] == COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1);
        };
        const NAMESPACE_STORAGE: usize = size_of::<(Namespaces, ProfileStorage)>();
        const ADMISSION_STORAGE: usize = size_of::<(Admission, AdmissionStorage)>();
        const BINDINGS_STORAGE: usize = size_of::<[Binding<'static>; 9]>();
        const LAUNCH_FRAME: usize = 4 * size_of::<($Managed, LaunchStorage)>()
            + 4 * size_of::<Stage>() + 8 * size_of::<LaunchError>()
            + 4 * size_of::<rustix::fs::Stat>() + 16384;
        #[cfg(test)]
        type ManagedFixture = $Managed;

        impl $Prepared {
            /// Full prepared-owner AND borrowed-context floor for launch and continuity.
            pub fn launch_input_storage(&self, supervisor: &SupervisorCap, policy: &PolicyCap)
                -> LaunchResult<usize> {
                launch::sum(&[self.retained, supervisor.retained_storage(), policy.retained_storage()])
            }

            /// Complete request work/extra peak for installing or validating the guard.
            /// The cleanup account independently pays GUARD_WORK or GUARD_CLONE_WORK.
            pub fn cleanup_guard_quota(&self, install: bool) -> LaunchResult<LaunchQuota> {
                Self::cleanup_guard_quota_for(self.revalidation_quota()?, install)
            }

            /// Request-ledger guard envelope at the fixed image ceilings. Full owner
            /// and borrowed contexts remain prepaid separately; this does not fund the
            /// independent cleanup account or install/validate an actual guard.
            pub fn maximum_cleanup_guard_quota(install: bool) -> LaunchResult<LaunchQuota> {
                Self::cleanup_guard_quota_for(Self::maximum_revalidation_quota()?, install)
            }

            fn cleanup_guard_quota_for(q: Quota, install: bool) -> LaunchResult<LaunchQuota> {
                Ok(LaunchQuota {
                    work: launch::sum(&[launch::LOCAL_WORK, q.work(), Lease::TRANSFER_WORK,
                        if install { Cleanup::GUARD_WORK } else { Cleanup::GUARD_CLONE_WORK }])?,
                    scratch: launch::sum(&[LAUNCH_FRAME, q.scratch(), Lease::IO_STORAGE,
                        Cleanup::GUARD_CLONE_SCRATCH, 2 * Cleanup::GUARD_FILE_STORAGE])?,
                })
            }

            /// Installs an actual root-bound lifecycle alias before the first child.
            /// Requires an empty, admission-open cleanup pool with no guard. Keep the
            /// full prepared/context floor charged on the original request ledger.
            /// No lease, identity or fresh budget is substituted after installation.
            /// Startup refusal or unwind cannot detach this persistent pool custody.
            pub fn retain_cleanup_guard(&self, supervisor: &SupervisorCap, policy: &PolicyCap,
                cleanup: &mut Cleanup, b: &mut Budget<'_>) -> LaunchResult<()> {
                self.cleanup_guard::<true>(supervisor, policy, cleanup, true, b)
            }

            fn cleanup_guard<const ROOT: bool>(&self, supervisor: &SupervisorCap, policy: &PolicyCap,
                cleanup: &mut Cleanup, install: bool, b: &mut Budget<'_>) -> LaunchResult<()> {
                let floor = self.launch_input_storage(supervisor, policy)?;
                b.with_prepaid_scope(floor, ENTRY_WORK, launch::LOCAL_WORK, LAUNCH_FRAME, |b| {
                    self.revalidate_inner::<ROOT>(supervisor, policy, b)?;
                    if install {
                        let (guard, c) = self.lifecycle.try_clone_for_transfer(b).map_err(Error::from)?;
                        b.reserve_storage(c.additional_storage())?;
                        cleanup.retain_deployment_guard(guard, b)?;
                        b.release_storage(c.additional_storage())?;
                    } else {
                        let (guard, c) = cleanup.try_clone_deployment_guard(b)?;
                        b.reserve_storage(c.additional_storage())?;
                        self.lifecycle.validate_transfer(&guard, b).map_err(Error::from)?;
                        drop(guard);
                        b.release_storage(c.additional_storage())?;
                    }
                    Ok(())
                })
            }

            fn transfer_source_storage(&self) -> LaunchResult<usize> {
                Self::transfer_source_storage_for_lengths(self.helper.measurement().byte_len(),
                    self.daemon.measurement().byte_len())
            }

            fn transfer_source_storage_for_lengths(helper: u64, daemon: u64) -> LaunchResult<usize> {
                launch::sum(&[Image::file_storage_for_length(helper).map_err(Error::from)?,
                    Image::file_storage_for_length(daemon).map_err(Error::from)?,
                    Lease::FILE_STORAGE, PolicyCap::FILE_STORAGE, SupervisorCap::FILE_STORAGE,
                    DeploymentCap::FILE_STORAGE, ProvisioningCap::FILE_STORAGE, Key::FILE_STORAGE,
                    Self::ROOT_STORAGE, 4 * launch::FILE_STORAGE, BINDINGS_STORAGE])
            }

            fn transfer_work_for_lengths(helper: u64, daemon: u64) -> LaunchResult<usize> {
                launch::sum(&[PolicyCap::IO_WORK, SupervisorCap::IO_WORK, DeploymentCap::IO_WORK,
                    ProvisioningCap::IO_WORK, Key::IO_WORK, Lease::TRANSFER_WORK,
                    Image::quota_for_length(helper, ImageOperation::Transfer).map_err(Error::from)?.work(),
                    Image::quota_for_length(daemon, ImageOperation::Transfer).map_err(Error::from)?.work()])
            }

            fn staging_quota(&self) -> LaunchResult<LaunchQuota> {
                Self::staging_quota_for_lengths(self.helper.measurement().byte_len(),
                    self.daemon.measurement().byte_len())
            }

            fn staging_quota_for_lengths(helper: u64, daemon: u64) -> LaunchResult<LaunchQuota> {
                let source = Self::transfer_source_storage_for_lengths(helper, daemon)?;
                let retained = Stage::storage_for_sources(source)?;
                let revalidation = Self::revalidation_quota_for_lengths(helper, daemon)?;
                let transfer = Self::transfer_work_for_lengths(helper, daemon)?;
                Ok(LaunchQuota {
                    work: launch::sum(&[launch::LOCAL_WORK, revalidation.work(), revalidation.work(),
                        transfer, transfer, Stage::STAGING_WORK])?,
                    scratch: launch::sum(&[LAUNCH_FRAME, source, retained, Stage::STAGING_SCRATCH,
                        retained, revalidation.scratch(), CONTEXT_STORAGE, Lease::IO_STORAGE,
                        Image::quota_for_length(helper, ImageOperation::Transfer).map_err(Error::from)?.scratch(),
                        Image::quota_for_length(daemon, ImageOperation::Transfer).map_err(Error::from)?.scratch()])?,
                })
            }

            /// Conservative successful-path envelope, including every bounded readiness attempt.
            /// Queries do not admit inputs or promise successful launch within a deadline.
            pub fn launch_quota(&self) -> LaunchResult<LaunchQuota> {
                Self::launch_quota_for(self.staging_quota()?, self.revalidation_quota()?,
                    self.transfer_source_storage()?)
            }

            /// Complete launch envelope at both protocol image ceilings, requiring
            /// no prepared owner. Prepared/context floors, prior guard installation,
            /// and persistent cleanup-pool funding are separate. Does not spawn a child.
            pub fn maximum_launch_quota() -> LaunchResult<LaunchQuota> {
                Self::launch_quota_for(Self::staging_quota_for_lengths(MAX_HELPER, MAX_DAEMON)?,
                    Self::maximum_revalidation_quota()?,
                    Self::transfer_source_storage_for_lengths(MAX_HELPER, MAX_DAEMON)?)
            }

            fn launch_quota_for(staging: LaunchQuota, validation: Quota, source: usize)
                -> LaunchResult<LaunchQuota> {
                let guard = Self::cleanup_guard_quota_for(validation, false)?;
                let polling = launch_io::MAX_LIVENESS_CHECKS.checked_mul(Child::OPERATION_WORK)
                    .and_then(|n| n.checked_add(launch_io::MAX_WORK)).ok_or(Resource::Arithmetic)?;
                Ok(LaunchQuota {
                    work: launch::sum(&[launch::LOCAL_WORK, staging.work(), validation.work(), guard.work(),
                        Namespaces::CAPTURE_WORK, Namespaces::REVALIDATE_SELF_WORK,
                        Namespaces::REVALIDATE_PROCESS_WORK, observations::PROCESS_VALIDATE_WORK,
                        Stage::spawn_work_for(DESTINATIONS.len(), 63)?, polling, 3 * Child::OPERATION_WORK,
                        Admission::ADMISSION_WORK, Admission::REVALIDATION_WORK])?,
                    scratch: launch::sum(&[LAUNCH_FRAME, Channels::STORAGE, NAMESPACE_STORAGE, guard.scratch(),
                        Namespaces::CAPTURE_SCRATCH, Namespaces::REVALIDATE_SELF_SCRATCH,
                        Namespaces::REVALIDATE_PROCESS_SCRATCH, observations::PROCESS_VALIDATE_SCRATCH,
                        staging.scratch(), Stage::storage_for_sources(source)?,
                        Stage::SPAWN_SCRATCH, Child::STORAGE, Child::OPERATION_SCRATCH,
                        launch_io::ATTEMPT_SCRATCH, validation.scratch(),
                        4 * Admission::PAIR_STORAGE, ADMISSION_STORAGE, Admission::IO_STORAGE,
                        $Managed::ENVELOPE])?,
                })
            }

            #[allow(unsafe_code)]
            fn stage_transfers<const ROOT: bool>(&self, supervisor: &SupervisorCap,
                policy: &PolicyCap, channels: &Channels, b: &mut Budget<'_>)
                -> LaunchResult<(Stage, usize)> {
                let floor = launch::sum(&[self.launch_input_storage(supervisor, policy)?, Channels::STORAGE])?;
                b.with_prepaid_scope(floor, ENTRY_WORK, launch::LOCAL_WORK, LAUNCH_FRAME, |b| {
                    self.revalidate_inner::<ROOT>(supervisor, policy, b)?;
                    let (helper, c) = self.helper.try_clone_for_exec(b).map_err(Error::from)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (daemon, c) = self.daemon.try_clone_for_exec(b).map_err(Error::from)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (lifecycle, c) = self.lifecycle.try_clone_for_transfer(b).map_err(Error::from)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (policy_file, c) = policy.try_clone_for_transfer(b).map_err(Error::from)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (supervisor_file, c) = supervisor.try_clone_for_transfer(b).map_err(Error::from)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (deployment, c) = self.deployment.try_clone_for_transfer(b).map_err(Error::from)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (provisioning, c) = self.provisioning.try_clone_for_transfer(b).map_err(Error::from)?;
                    b.reserve_storage(c.additional_storage())?;
                    let (key, c) = self.key_template.try_clone_for_transfer(b).map_err(Error::from)?;
                    b.reserve_storage(c.additional_storage())?;
                    b.reserve_storage(BINDINGS_STORAGE)?;
                    let sources = [channels.child.as_fd(), self.root.as_fd(), daemon.as_fd(),
                        lifecycle.as_fd(), policy_file.as_fd(), supervisor_file.as_fd(),
                        deployment.as_fd(), key.as_fd(), provisioning.as_fd()];
                    // All destinations are fixed and below the shared staging floor.
                    let mut bindings = [Binding::new(sources[0], DESTINATIONS[0])
                        .map_err(|_| LaunchError::Invalid("native anchor binding"))?; 9];
                    for ((binding, source), destination) in bindings.iter_mut().zip(sources).zip(DESTINATIONS) {
                        *binding = Binding::new(source, destination)
                            .map_err(|_| LaunchError::Invalid("native anchor binding"))?;
                    }
                    // SAFETY: all full source charges derive from retained native owners
                    // and this closed fixed table. Originals and their locks remain live;
                    // no raw transfers escape. Exact final Files are checked below.
                    let (stage, c) = unsafe { Stage::stage(&helper, &bindings,
                        channels.profile_writer.as_fd(), channels.gate_reader.as_fd(),
                        channels.child.as_fd(), self.transfer_source_storage()?, b) }?;
                    b.reserve_storage(c.additional_storage())?;
                    self.validate_staged::<ROOT>(&stage, supervisor, policy, channels, b)?;
                    Ok((stage, c.additional_storage()))
                })
            }

            fn validate_staged<const ROOT: bool>(&self, stage: &Stage, supervisor: &SupervisorCap,
                policy: &PolicyCap, channels: &Channels, b: &mut Budget<'_>) -> LaunchResult<()> {
                self.revalidate_inner::<ROOT>(supervisor, policy, b)?;
                let file = |fd| stage.binding(fd).ok_or(LaunchError::Invalid("missing native staged input"));
                self.helper.revalidate_exec_clone(stage.executable(), b).map_err(Error::from)?;
                self.daemon.revalidate_exec_clone(file(5)?, b).map_err(Error::from)?;
                self.lifecycle.validate_transfer(file(6)?, b).map_err(Error::from)?;
                policy.validate_transfer(file(202)?, b).map_err(Error::from)?;
                supervisor.validate_transfer(file(220)?, b).map_err(Error::from)?;
                self.deployment.validate_transfer(file(221)?, b).map_err(Error::from)?;
                self.key_template.validate_transfer(file(222)?, self.deployment.deployment(), b).map_err(Error::from)?;
                self.provisioning.validate_transfer(file(223)?, b).map_err(Error::from)?;
                if native::state_root(file(4)?, self.deployment.deployment().service()).map_err(Error::from)?
                    != self.root_snapshot {
                    return Err(Error::Invalid(Failure::StateRootChanged).into());
                }
                let original = rustix::fs::fstat(&channels.child).map_err(|e| launch::io("inspect bootstrap source", e))?;
                let staged = rustix::fs::fstat(file(3)?).map_err(|e| launch::io("inspect staged bootstrap", e))?;
                if (original.st_dev, original.st_ino, original.st_mode) != (staged.st_dev, staged.st_ino, staged.st_mode)
                    || !rustix::io::fcntl_getfd(file(3)?).map_err(|e| launch::io("inspect bootstrap flags", e))?
                        .contains(rustix::io::FdFlags::CLOEXEC) {
                    return Err(LaunchError::Invalid("native bootstrap object changed"));
                }
                Ok(())
            }

            /// Consumes root-prepared inputs into admitted native daemon custody.
            /// Actual same-family contexts are required, not just identities. Keep the
            /// full input floor reserved; reserve returned growth before retention.
            /// On failure inputs close but their reservations remain for retirement.
            /// Cleanup may defer/quarantine a child; refusal never means it was reaped.
            /// Call retain_cleanup_guard before the first launch. Launch validates the
            /// installed guard against this actual root-bound lease before spawning.
            ///
            /// ```
            #[doc = concat!("use fe2o3_external_anchor_coordinator::{", stringify!($Prepared), " as Prepared, ", stringify!($Managed), " as Managed, ExternalAnchorLaunchErrorV2 as Error, ExternalAnchorLaunchStorageV2 as Storage};")]
            #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Supervisor};")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// use fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2 as Cleanup;
            /// fn launch(p: Prepared, s: &Supervisor, policy: &Policy, c: &mut Cleanup,
            ///     b: &mut Budget<'_>) -> Result<(Managed, Storage), Error> {
            ///     p.retain_cleanup_guard(s, policy, c, b)?;
            ///     p.launch(s, policy, std::time::Duration::from_secs(30), c, b)
            /// }
            /// ```
            #[allow(unsafe_code)]
            pub fn launch(self, supervisor: &SupervisorCap, policy: &PolicyCap, timeout: Duration,
                cleanup: &mut Cleanup, b: &mut Budget<'_>) -> LaunchResult<($Managed, LaunchStorage)> {
                let floor = self.launch_input_storage(supervisor, policy)?;
                b.with_prepaid_scope(floor, ENTRY_WORK, launch::LOCAL_WORK,
                    LAUNCH_FRAME + launch_io::ATTEMPT_SCRATCH, |b| {
                    let deadline = launch_io::bounded_deadline(timeout)
                        .map_err(|_| LaunchError::Invalid("invalid native anchor launch timeout"))?;
                    native::require_root::<true>()?;
                    self.cleanup_guard::<true>(supervisor, policy, cleanup, false, b)?;
                    let (namespaces, c) = Namespaces::capture_self(b)?;
                    b.reserve_storage(c.additional_storage())?;
                    let credentials = Credentials::new(self.deployment.deployment().service().uid(),
                        self.deployment.deployment().service().gid())?;
                    b.reserve_storage(Channels::STORAGE)?;
                    let channels = Channels::new()?;
                    let (stage, charge) = self.stage_transfers::<true>(supervisor, policy, &channels, b)?;
                    b.reserve_storage(charge)?;
                    if std::time::Instant::now() >= deadline {
                        return Err(LaunchError::Timeout("native input staging"));
                    }
                    // SAFETY: stage_transfers validated each final File against actual
                    // native owners and contexts; credentials come from that deployment.
                    // This closed coordinator owns all consuming waits. The guarded path
                    // below requires profile/namespaces, readiness, exec and endpoint admission.
                    let (mut child, c) = unsafe { stage.spawn(credentials, cleanup, b) }?;
                    b.reserve_storage(c.additional_storage())?;
                    drop(stage);
                    let Channels { root, child: bootstrap_child, profile_reader, profile_writer,
                        gate_reader, gate_writer } = channels;
                    drop(bootstrap_child); drop(profile_writer); drop(gate_reader);
                    launch_io::await_profile_ready(profile_reader.as_fd(), root.as_fd(),
                        &mut launch::Observer { child: &child, budget: b }, deadline).map_err(launch::protocol)?;
                    namespaces.revalidate_self(b)?;
                    namespaces.revalidate_process(child.pid(), b)?;
                    b.with_prepaid_scope(Child::STORAGE, 0, observations::PROCESS_VALIDATE_WORK,
                        observations::PROCESS_VALIDATE_SCRATCH, |_| -> LaunchResult<()> {
                            Ok(observations::validate_process(credentials, child.pid())?)
                        })?;
                    self.revalidate(supervisor, policy, b)?;
                    launch_io::release_child(gate_writer.as_fd(),
                        &mut launch::Observer { child: &child, budget: b }, deadline).map_err(launch::protocol)?;
                    drop(gate_writer);
                    let sender = launch_io::MessageSender::new(child.pid().as_raw_pid(),
                        credentials.uid(), credentials.gid());
                    let (ready, endpoint) = launch_io::receive_ready_from(root.as_fd(), sender,
                        &mut launch::Observer { child: &child, budget: b }, deadline).map_err(launch::protocol)?;
                    b.reserve_storage(launch::FILE_STORAGE)?;
                    launch_io::await_exec_eof(root.as_fd(),
                        &mut launch::Observer { child: &child, budget: b }, deadline).map_err(launch::protocol)?;
                    if !child.is_live(b)? { return Err(LaunchError::ChildExited("daemon exec")); }
                    let (pidfd, c) = child.try_clone_pidfd(b)?;
                    b.reserve_storage(c.additional_storage())?;
                    b.reserve_storage(Admission::PAIR_STORAGE)?;
                    let (admission, c) = Admission::admit(endpoint, pidfd,
                        self.deployment.deployment().service(), b)?;
                    b.reserve_storage(c.additional_storage())?;
                    admission.validate_continuity(b)?;
                    if std::time::Instant::now() >= deadline {
                        return Err(LaunchError::Timeout("native endpoint admission"));
                    }
                    // SAFETY: this measured helper's canonical ready transfer preceded
                    // CLOEXEC EOF; the same live child and endpoint passed native admission.
                    // No other wait consumer or inherited bootstrap alias escapes this path.
                    unsafe { child.confirm_exec(b) }?;
                    let (retained, growth) = $Managed::retained_storage_for(self.retained,
                        admission.retained_storage())?;
                    b.reserve_storage($Managed::ENVELOPE)?;
                    Ok(($Managed { admission, child, prepared: self, disposition: ready.disposition(), retained },
                        LaunchStorage(growth)))
                })
            }
        }

        /// Root-retained native daemon, exact endpoint and complete preparation custody.
        /// Drop performs one prepaid cleanup step, deferring pending/quarantined custody.
        /// No blocking wait, new budget or eventual-reaping guarantee is provided.
        /// The caller retires full retained_storage only after this owner is dropped.
        /// No compiler, publication or GPU authority is granted.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Managed), " as Managed;")]
        /// fn clone<T: Clone>() {} clone::<Managed>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Managed), " as Managed;")]
        /// fn fd<T: std::os::fd::AsFd>() {} fd::<Managed>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::{", stringify!($Managed), " as Managed, RootManagedExternalAnchorV1 as Old};")]
        /// fn upgrade(v: Old) -> Managed { v.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Managed), " as Managed;")]
        #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionPolicyCapabilityV", $other, " as Policy, CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Supervisor};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(m: &Managed, s: &Supervisor, p: &Policy, b: &mut Budget<'_>) {
        ///     let _ = m.validate_continuity(s, p, b);
        /// }
        /// ```
        pub struct $Managed {
            admission: Admission,
            child: Child,
            prepared: $Prepared,
            disposition: crate::ExternalAnchorProvisioningReadyDispositionV1,
            retained: usize,
        }
        impl $Managed {
            const ENVELOPE: usize = size_of::<(Self, LaunchStorage)>() - size_of::<Admission>()
                - size_of::<Child>() - size_of::<$Prepared>();
            /// Whether the helper opened existing state or initialized genesis.
            pub const fn disposition(&self) -> crate::ExternalAnchorProvisioningReadyDispositionV1 { self.disposition }
            /// Full retained owner charge, excluding borrowed contexts and global cleanup funding.
            pub const fn retained_storage(&self) -> usize { self.retained }

            /// Conservative full managed-owner charge, excluding borrowed supervisor/
            /// policy and independent cleanup funding. Adds exact child, admission and
            /// enclosing-owner growth to the conservative prepared-owner bound.
            pub fn maximum_retained_storage() -> LaunchResult<usize> {
                Self::retained_storage_for($Prepared::maximum_retained_storage()?, ADMISSION_STORAGE)
                    .map(|(retained, _)| retained)
            }

            fn retained_storage_for(prepared: usize, admission: usize) -> LaunchResult<(usize, usize)> {
                let growth = launch::sum(&[Child::STORAGE, admission, Self::ENVELOPE])?;
                Ok((launch::sum(&[prepared, growth])?, growth))
            }

            /// Complete continuity work and additional peak above owner and context charges.
            pub fn continuity_quota(&self) -> LaunchResult<LaunchQuota> {
                Self::continuity_quota_for(self.prepared.revalidation_quota()?)
            }

            /// Continuity envelope at both protocol image ceilings. Does not fabricate
            /// child/admission custody; full managed/context floors remain separate.
            pub fn maximum_continuity_quota() -> LaunchResult<LaunchQuota> {
                Self::continuity_quota_for($Prepared::maximum_revalidation_quota()?)
            }

            fn continuity_quota_for(q: Quota) -> LaunchResult<LaunchQuota> {
                Ok(LaunchQuota {
                    work: launch::sum(&[launch::LOCAL_WORK, q.work(), Child::OPERATION_WORK,
                        Admission::REVALIDATION_WORK])?,
                    scratch: launch::sum(&[LAUNCH_FRAME,
                        maximum(&[q.scratch(), Child::OPERATION_SCRATCH, Admission::IO_STORAGE])])?,
                })
            }
            /// Rechecks actual context, retained preparation, child and endpoint on the original ledger.
            pub fn validate_continuity(&self, supervisor: &SupervisorCap, policy: &PolicyCap,
                b: &mut Budget<'_>) -> LaunchResult<()> {
                let floor = launch::sum(&[self.retained, supervisor.retained_storage(), policy.retained_storage()])?;
                b.with_prepaid_scope(floor, ENTRY_WORK, launch::LOCAL_WORK, LAUNCH_FRAME, |b| {
                    self.prepared.revalidate(supervisor, policy, b)?;
                    if !self.child.is_live(b)? { return Err(LaunchError::ChildExited("managed native anchor")); }
                    Ok(self.admission.validate_continuity(b)?)
                })
            }
            /// Consumes the owner using its prepaid emergency cleanup allowance.
            /// Pending/quarantined means the shared pool still retains child custody.
            pub fn cancel(mut self) -> CleanupPoll { self.child.cancel() }
        }
        impl fmt::Debug for $Managed {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Managed)).field("authority", &"native-service-custody-only")
                    .field("process", &self.admission.service_process_identity())
                    .field("disposition", &self.disposition).finish_non_exhaustive()
            }
        }
    };
}
pub(crate) use launch;
