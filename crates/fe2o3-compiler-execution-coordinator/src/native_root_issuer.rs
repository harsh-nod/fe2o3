// Direct root-to-issuer composition. No handoff, deployment or occurrence authority
// is manufactured here. The production owning attempt and privileged observer are
// separate integrations; readiness precedes compiler resume and publication.
use super::PreparedCompilerExecutionSupervisorV3 as Prepared;
use crate::compiler_child_channel::CompilerTrace;
use crate::native_launch::{
    self as launch, CompilerExecutionLaunchErrorV2 as Error,
    CompilerExecutionLaunchQuotaV2 as Quota, CompilerExecutionLaunchStorageV2 as Storage,
    FILE_STORAGE, Observer, Result, sum,
};
use fe2o3_broker_authority_service::{
    RootConnectionV3 as RootConnection, RootControlSessionV3 as RootSession,
    RootLaunchChannelV3 as RootChannel,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionPolicyCapabilityV3 as PolicyCap,
    CompilerExecutionServiceLaunchCapabilityV3 as ManifestCap,
    CompilerExecutionSigningKeyServiceTransferV3 as ServiceKey,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V3 as MANIFEST_SCRATCH,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3 as MANIFEST_WORK,
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V3 as READY_BYTES,
    COMPILER_EXECUTION_SERVICE_READY_STORAGE_V3 as READY_SCRATCH,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V3 as READY_WORK,
    CompilerExecutionAttestationStorageV3 as RecordStorage,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionServiceLaunchManifestV3 as Manifest, CompilerExecutionServiceReadyV3 as Ready,
};
use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV2 as Inputs;
use fe2o3_external_anchor_coordinator::ExternalAnchorSupervisorTransferV3 as AnchorTransfer;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_spawn::{
    ProtectedServiceCleanupServiceV2 as Cleanup, ProtectedServiceDescriptorBindingV1 as Binding,
    RetainedDependencyV2 as Dependency, RetainedResourcesV2 as Resources,
    cleanup_bridge::CleanupPollV1 as CleanupPoll,
    launch_io,
    native_spawn::{
        RootOwnedProtectedServiceChildV2 as PlainChild,
        RootOwnedRetainedServiceChildV2 as RetainedChild, StagedProtectedServiceExecV2 as Stage,
    },
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableOperationV2 as ImageOperation, ProtectedStaticExecutableV2 as Image,
};
pub(crate) use quota::IssuerCleanupQuota;
use rustix::{event, fs, io, net, pipe};
use staging::{Channels, Readers};
use std::{
    fmt,
    marker::PhantomData,
    mem::size_of,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    rc::Rc,
    time::{Duration, Instant},
};

type Child<T> = RetainedChild<Payload<T>>;
const READY_OWNER: usize = size_of::<(Ready, RecordStorage)>();
const MANIFEST_OWNER: usize = size_of::<(Manifest, RecordStorage)>();
// Fixed table/metadata checks, descriptor creation/closure, and refusal cleanup.
// Every native image, record, retained access and transport charges separately.
const LOCAL_WORK: usize = 8 + 128 * 1088;
const FRAME: usize = 4 * size_of::<(ManagedIssuer<'static, ()>, Storage)>()
    + 4 * size_of::<Stage>()
    + 8 * size_of::<Error>()
    + 4 * size_of::<fs::Stat>()
    + 16384;

struct Payload<T: Send + 'static> {
    // The unused supervisor and launcher, all leases and live anchor stay owned.
    prepared: Prepared,
    _dependency: Dependency<T>,
    key: ServiceKey,
    manifest: ManifestCap,
    retained: usize,
}
impl<T: Send + 'static> Payload<T> {
    const ENVELOPE: usize = size_of::<(Self, usize)>()
        - size_of::<Prepared>()
        - size_of::<Dependency<T>>()
        - size_of::<ServiceKey>()
        - size_of::<ManifestCap>();
}

// Accounting identity only, meaningful while the original Work borrow remains
// live. Captured in the authenticated trace callback, never from decoded input.
struct RequestAccount {
    ledger: Ledger,
    address: usize,
}
impl RequestAccount {
    fn capture(b: &Budget<'_>) -> Self {
        Self {
            ledger: b.work_ledger_identity_v1(),
            address: b as *const Budget<'_> as usize,
        }
    }

    fn with<R>(
        &self,
        retained: usize,
        b: &mut Budget<'_>,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<R>,
    ) -> Result<R> {
        b.with_prepaid_scope(retained, 8, LOCAL_WORK, FRAME, |b| {
            if self.ledger != b.work_ledger_identity_v1()
                || self.address != b as *const Budget<'_> as usize
            {
                return Err(Resource::Accounting.into());
            }
            operation(b)
        })
    }
}

/// Move-only originating-thread issuer custody, not production occurrence admission.
/// The actual compiler trace stays with the caller; this owner cannot resume it.
/// Keep the creator thread and original cleanup controller alive until termination,
/// including deferred/quarantined cleanup. Retire the FULL charge only after Drop.
/// Keep the original Work borrow live and Budget at its admitting address until
/// this owner drops; address equality is not persistent identity after that borrow.
pub(crate) struct ManagedIssuer<'work, T: Send + 'static> {
    child: Child<T>,
    ready: Ready,
    root: RootSession<'work>,
    connection: RootConnection<'work>,
    retained: usize,
    continuity: Quota,
    account: RequestAccount,
    // Never send the foreground launch owner or imply a Send escape for the trace.
    _creator: PhantomData<(&'work Budget<'work>, Rc<()>)>,
}
impl<T: Send + 'static> ManagedIssuer<'_, T> {
    const ENVELOPE: usize = size_of::<(Self, Storage)>()
        - size_of::<Child<T>>()
        - size_of::<Ready>()
        - size_of::<RootSession<'static>>()
        - size_of::<RootConnection<'static>>();
    pub(crate) fn pid(&self) -> rustix::process::Pid {
        self.child.pid()
    }
    pub(crate) const fn readiness(&self) -> &Ready {
        &self.ready
    }
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) const fn continuity_quota(&self) -> Quota {
        self.continuity
    }

    /// Owning attempt must check readiness/continuity before resuming its compiler.
    /// This does not perform Prepare/Issue or inspect a not-yet-created publication.
    pub(crate) fn validate_ready(
        &self,
        trace: &CompilerTrace<'_, T>,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        self.account.with(self.retained, b, |b| {
            self.child.with_resources(b, |p, b| -> Result<()> {
                p.prepared.validate_process(self.child.pid(), b)?;
                match_ready(&self.ready, self.child.pid(), &p.manifest, &p.prepared, b)?;
                // Connection validation includes the actual running image on
                // this same retained child and policy, after the root join.
                trace.with_observation(b, |original, b| -> Result<()> {
                    Ok(self.connection.validate(
                        &self.root,
                        original,
                        &self.child,
                        p.prepared.trust.policy().policy(),
                        p.manifest.manifest(),
                        b,
                    )?)
                })?;
                Ok(())
            })?;
            require_live(&self.child, b)
        })
    }
    pub(crate) fn cancel(mut self) -> CleanupPoll {
        self.child.cancel()
    }
}

// Readiness alone cannot construct ManagedIssuer. The original held compiler
// trace must pass its scoped identity check and the actual issuer must complete the
// fresh root challenge before this intermediate owner is consumed.
struct ReadyIssuer<'work, T: Send + 'static> {
    child: Child<T>,
    ready: Ready,
    channel: RootChannel<'work>,
    continuity: Quota,
    account: RequestAccount,
}
impl<T: Send + 'static> ReadyIssuer<'_, T> {
    const ENVELOPE: usize = size_of::<(Self, Storage)>()
        - size_of::<Child<T>>()
        - size_of::<Ready>()
        - size_of::<RootChannel<'static>>();
}
impl<T: Send + 'static> fmt::Debug for ManagedIssuer<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagedIssuer")
            .field("pid", &self.pid())
            .field("readiness", &self.ready.identity())
            .field("authority", &"issuer-custody-only")
            .finish_non_exhaustive()
    }
}

// Must outlive the OUTERMOST accounting scope: inner input transfer can commit
// successfully before that scope detects a ledger error or unwinds.
struct CompilerCancellation<'a, 'work, T: Send + 'static> {
    trace: &'a mut CompilerTrace<'work, T>,
    committed: bool,
}
impl<T: Send + 'static> Drop for CompilerCancellation<'_, '_, T> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.trace.cancel();
        }
    }
}

impl Prepared {
    /// Consumes actual prepared root authority and the confirmed trace's ONE input
    /// callback. No descriptor intake, PID reopen or AcceptedHandoff is available.
    /// Keep this preparation and the full compiler trace prepaid on the SAME Budget.
    /// The returned charge is GROWTH above Prepared; the trace reservation remains
    /// independent. On error/unwind the original compiler is cancelled, even when
    /// only final outer accounting failed. Never resumes the compiler.
    ///
    /// # Safety
    /// Keep the actual cloning thread alive until issuer termination and unresolved
    /// cleanup; preserve sole consuming-wait and descriptor/profile custody. The
    /// original T must satisfy its bounded, nonpanicking, independently funded Drop
    /// contract even if issuer cleanup releases the last dependency on another thread.
    /// Cleanup must be the already funded controller carrying this Prepared's root
    /// lifecycle guard; no account may be renewed. The production owning attempt,
    /// approved runtime enforcement and privileged publication observer are separate.
    #[allow(unsafe_code)]
    pub(crate) unsafe fn launch_issuer<'work, T: Send + 'static>(
        self,
        trace: &mut CompilerTrace<'work, T>,
        timeout: Duration,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> Result<(ManagedIssuer<'work, T>, Storage)> {
        let input = self.retained_storage();
        let mut attempt = CompilerCancellation {
            trace,
            committed: false,
        };
        let result = b.with_prepaid_scope(
            sum(&[input, attempt.trace.retained_storage()])?,
            8,
            LOCAL_WORK,
            FRAME,
            |b| -> Result<_> {
                let deadline = launch_io::bounded_deadline(timeout)?;
                self.validate_cleanup_guard(cleanup, b)?;
                let (pending, growth) =
                    attempt
                        .trace
                        .with_issuer_inputs(b, |client, peer, pidfd, dependency, b| {
                            let account = RequestAccount::capture(b);
                            // SAFETY: the caller supplies the creator/custody/Drop contract;
                            // these inputs come only from this original confirmed root trace.
                            unsafe {
                                launch_inputs(
                                    self, client, peer, pidfd, dependency, account, deadline,
                                    cleanup, b,
                                )
                            }
                        })?;
                b.reserve_storage(growth.additional_storage())?;
                let ReadyIssuer {
                    child,
                    ready,
                    channel,
                    continuity,
                    account,
                    ..
                } = pending;
                let (root, connection, retained) =
                    attempt
                        .trace
                        .with_observation(b, |original, b| -> Result<_> {
                            let (root, charge) = RootSession::create(original, b)?;
                            b.reserve_storage(charge.additional_storage())?;
                            let (connection, charge) =
                                child.with_resources(b, |p, b| -> Result<_> {
                                    check_deadline(deadline, "issuer root admission")?;
                                    Ok(root.connect_after_readiness(
                                        original,
                                        &child,
                                        p.prepared.trust.policy().policy(),
                                        p.manifest.manifest(),
                                        p.prepared.credentials,
                                        &ready,
                                        channel,
                                        deadline.saturating_duration_since(Instant::now()),
                                        b,
                                    )?)
                                })?;
                            b.reserve_storage(charge.additional_storage())?;
                            let retained = sum(&[
                                child.retained_storage(),
                                ready.retained_storage(),
                                root.retained_storage(),
                                connection.retained_storage(),
                                ManagedIssuer::<T>::ENVELOPE,
                            ])?;
                            b.reserve_storage(ManagedIssuer::<T>::ENVELOPE)?;
                            Ok((root, connection, retained))
                        })?;
                let growth = retained.checked_sub(input).ok_or(Resource::Accounting)?;
                Ok((
                    ManagedIssuer {
                        child,
                        ready,
                        root,
                        connection,
                        retained,
                        continuity,
                        account,
                        _creator: PhantomData,
                    },
                    Storage(growth),
                ))
            },
        )?;
        attempt.committed = true;
        Ok(result)
    }
}

#[allow(unsafe_code, clippy::too_many_arguments)]
unsafe fn launch_inputs<'work, T: Send + 'static>(
    prepared: Prepared,
    client: Client,
    peer: BorrowedFd<'_>,
    pidfd: BorrowedFd<'_>,
    dependency: Dependency<T>,
    account: RequestAccount,
    deadline: Instant,
    cleanup: &mut Cleanup,
    b: &mut Budget<'work>,
) -> Result<(ReadyIssuer<'work, T>, Storage)> {
    let original = prepared.retained_storage();
    let floor = sum(&[original, dependency.retained_storage()])?;
    b.with_prepaid_scope(
        floor,
        8,
        LOCAL_WORK,
        FRAME + launch_io::ATTEMPT_SCRATCH + launch_io::PIPE_ATTEMPT_SCRATCH,
        |b| {
            prepared.revalidate(b)?;
            let continuity = prepared.issuer_continuity_quota::<T>()?;
            let credentials = prepared.credentials;
            let (anchor, charge) = prepared.anchor.try_clone_for_supervisor(
                prepared.trust.deployment(),
                prepared.trust.policy(),
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            // Service comes from the actual live anchor transfer, not inert deployment input.
            let (manifest, charge) = Manifest::new(
                client,
                anchor.service(),
                prepared.trust.policy().policy(),
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            let (manifest, charge) = ManifestCap::create(manifest, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (key, charge) = prepared.trust.key_template().reissue_for_deployed_service(
                prepared.trust.deployment().deployment(),
                prepared.trust.policy().policy(),
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            let retained = payload_storage::<T>(
                original,
                dependency.retained_storage(),
                key.retained_storage(),
                manifest.retained_storage(),
            )?;
            b.reserve_storage(Payload::<T>::ENVELOPE)?;
            let payload = Payload {
                prepared,
                _dependency: dependency,
                key,
                manifest,
                retained,
            };
            b.reserve_storage(Channels::STORAGE)?;
            let channels = Channels::new()?;
            // The compiler already exists and is held at its confirmed exec.
            // Only the issuer end enters the staged descriptor table below.
            let (mut root_channel, charge) = RootChannel::create(b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (stage, stage_charge) =
                staging::stage(&payload, anchor, peer, pidfd, &channels, &root_channel, b)?;
            b.reserve_storage(stage_charge)?;
            check_deadline(deadline, "issuer staging")?;
            // SAFETY: final staged image/root/policy/fresh-key/manifest/anchor Files
            // and exact source peer/pidfd duplicates were checked in staging::stage.
            // The COMPLETE payload enters the original funded slot BEFORE clone;
            // it contains no readiness writer. Caller preserves creator and T contracts.
            let (mut child, charge) =
                unsafe { stage.spawn_retaining(credentials, payload, retained, cleanup, b) }?;
            b.reserve_storage(charge.additional_storage())?;
            // Stage contains a readiness writer alias, so it must close before EOF wait.
            drop(stage);
            b.release_storage(stage_charge)?;
            root_channel.close_parent_issuer_endpoint(b)?;
            let Readers {
                ready,
                profile,
                gate,
                exec,
            } = channels.close_child_ends();
            launch_io::await_profile_ready(
                profile.as_fd(),
                exec.as_fd(),
                &mut Observer {
                    child: &child,
                    budget: b,
                },
                deadline,
            )?;
            child.with_resources(b, |p, b| p.prepared.validate_process(child.pid(), b))?;
            launch_io::release_child(
                gate.as_fd(),
                &mut Observer {
                    child: &child,
                    budget: b,
                },
                deadline,
            )?;
            drop(gate);
            launch_io::await_exec_eof(
                exec.as_fd(),
                &mut Observer {
                    child: &child,
                    budget: b,
                },
                deadline,
            )?;
            b.reserve_storage(READY_BYTES)?;
            let bytes = launch_io::receive_ready_pipe::<READY_BYTES, _>(
                ready.as_fd(),
                &mut Observer {
                    child: &child,
                    budget: b,
                },
                deadline,
            )?;
            let (ready_record, charge) = Ready::decode(&bytes, b)?;
            b.reserve_storage(charge.additional_storage())?;
            child.with_resources(b, |p, b| -> Result<()> {
                p.prepared.validate_process(child.pid(), b)?;
                match_ready(&ready_record, child.pid(), &p.manifest, &p.prepared, b)?;
                fe2o3_broker_authority_service::validate_retained_issuer_image_v3(
                    &child,
                    p.prepared.trust.policy().policy(),
                    b,
                )?;
                Ok(())
            })?;
            require_live(&child, b)?;
            check_deadline(deadline, "issuer final validation")?;
            // SAFETY: native exec-status EOF, exact private pipe record PLUS EOF,
            // matches_launch, measured running image, final prepared process/profile
            // and pidfd liveness passed.
            // ALL parent/stage child-end aliases closed before these observations.
            unsafe { child.confirm_exec(b) }?;
            let retained = sum(&[
                child.retained_storage(),
                charge.additional_storage(),
                root_channel.retained_storage(),
                ReadyIssuer::<T>::ENVELOPE,
            ])?;
            let growth = retained.checked_sub(original).ok_or(Resource::Accounting)?;
            b.reserve_storage(ReadyIssuer::<T>::ENVELOPE)?;
            Ok((
                ReadyIssuer {
                    child,
                    ready: ready_record,
                    channel: root_channel,
                    continuity,
                    account,
                },
                Storage(growth),
            ))
        },
    )
}

fn payload_storage<T: Send + 'static>(
    prepared: usize,
    dependency: usize,
    key: usize,
    manifest: usize,
) -> Result<usize> {
    sum(&[prepared, dependency, key, manifest, Payload::<T>::ENVELOPE])
}
fn match_ready(
    ready: &Ready,
    pid: rustix::process::Pid,
    manifest: &ManifestCap,
    prepared: &Prepared,
    b: &mut Budget<'_>,
) -> Result<()> {
    match_ready_records(
        ready,
        launch::pid_u32(pid)?,
        manifest.manifest(),
        prepared.trust.policy().policy(),
        b,
    )
}
fn match_ready_records(
    ready: &Ready,
    pid: u32,
    manifest: &Manifest,
    policy: &fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3,
    b: &mut Budget<'_>,
) -> Result<()> {
    if !ready.matches_launch(pid, manifest, policy, b)? {
        return Err(Error::Invalid("issuer readiness names another launch"));
    }
    Ok(())
}
fn require_live<T: Send + 'static>(child: &Child<T>, b: &mut Budget<'_>) -> Result<()> {
    if !child.is_live(b)? {
        return Err(launch_io::Failure::ChildExited("issuer readiness").into());
    }
    Ok(())
}
fn check_deadline(deadline: Instant, phase: &'static str) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(launch_io::Failure::Timeout(phase).into());
    }
    Ok(())
}

mod staging {
    include!("native_root_issuer_staging.rs");
}
mod quota {
    include!("native_root_issuer_quota.rs");
}
#[cfg(test)]
mod tests {
    include!("native_root_issuer_tests.rs");
}
