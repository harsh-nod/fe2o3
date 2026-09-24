//! Native consuming custody over the one clone/pidfd/cleanup engine.
use super::{
    ChildProcessError, ChildProfileV1, IssuerChild, ProfileReportRead, ReportError, StagedLaunchV1,
    child_work, spawn_child,
};
use crate::{
    PreparedProtectedIssuerLaunchV2 as Prepared, ProtectedIssuerCleanupServiceV2 as Cleanup,
    ProtectedIssuerSupervisorV2 as Supervisor, launch_v2::LaunchedInputsV2,
    process_cleanup::CleanupPollV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES,
    CompilerExecutionServiceReadyV2 as Readiness,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_profile::observations::{
    CHILD_NAMESPACE_REPORT_BYTES, CHILD_NAMESPACE_REPORT_CAPTURE_SCRATCH,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProcessProfileV2 as Profile,
    require_owned_sigchld_v2,
};
use rustix::{
    io::Errno,
    net::SendFlags,
    pipe::{PipeFlags, pipe_with},
};
use std::{
    fmt,
    mem::size_of,
    os::fd::{AsRawFd, OwnedFd},
};

#[path = "process_native_error.rs"]
mod error;
#[path = "process_native_wait.rs"]
mod wait;
use ProtectedIssuerBoundaryV2 as Boundary;
use ProtectedIssuerLaunchErrorV2 as Error;
use ProtectedIssuerWaitV2 as Wait;
pub use error::ProtectedIssuerLaunchErrorV2;
pub use wait::{ProtectedIssuerBoundaryV2, ProtectedIssuerWaitV2};
use wait::{attempts, before_deadline, live};
type Result<T> = std::result::Result<T, Error>;
const ENTRY: usize = 8;

/// Fixed outer work for pipes, staging, parent descriptor closure, transfer and Drop.
/// Native observations, child execution and finite waits charge additional work.
pub const PROTECTED_ISSUER_LAUNCH_WORK_V2: usize = ENTRY + 128 * 1024;
/// Outer logical scratch above every retained input; not RSS or generated stack.
pub const PROTECTED_ISSUER_LAUNCH_SCRATCH_V2: usize = 8 * size_of::<Session<'static, 'static>>()
    + 4 * size_of::<StagedLaunchV1>()
    + 4 * size_of::<Profile>()
    + 4 * size_of::<Namespaces>()
    + CHILD_NAMESPACE_REPORT_CAPTURE_SCRATCH
    + 4 * size_of::<ProfileReportRead>()
    + 16 * size_of::<OwnedFd>()
    + 8192;
const SESSION_SCRATCH: usize = 4 * size_of::<Session<'static, 'static>>() + 4 * READY_BYTES + 4096;
const OWNER_GROWTH: usize = size_of::<Session<'static, 'static>>();

struct Core<'s> {
    supervisor: &'s Supervisor,
    process: IssuerChild,
    inputs: LaunchedInputsV2,
    readiness: Option<Readiness>,
    retained: usize,
}
impl Core<'_> {
    fn floor(&self) -> Result<usize> {
        self.retained
            .checked_add(self.supervisor.retained_storage())
            .ok_or_else(|| Resource::Arithmetic.into())
    }
    fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
        self.supervisor.revalidate(b)?;
        self.inputs.capability.revalidate(b)?;
        let readiness = self
            .readiness
            .as_ref()
            .ok_or(Error::State("issuer is not ready"))?;
        if !readiness.matches_launch(
            self.process.pid_u32(),
            self.inputs.capability.manifest(),
            self.supervisor.policy(),
            b,
        )? {
            return Err(Error::State(
                "native readiness names another launch or policy",
            ));
        }
        live(&self.process, Boundary::Readiness)
    }
    fn cancel(&mut self) -> Result<()> {
        match self.process.cancel_once() {
            CleanupPollV1::Reaped => Ok(()),
            CleanupPollV1::Pending => Err(Error::CleanupPending),
            CleanupPollV1::Quarantined => {
                Err(Error::State("issuer custody retained in quarantine"))
            }
        }
    }
}

// Field order is the lifetime invariant: retire or defer child custody and close
// endpoints before releasing their original request-account reservation.
struct Funded<'a, 'work, T> {
    owner: T,
    funding: RequestFunding<'a, 'work>,
}
type Session<'a, 'work> = Funded<'a, 'work, Core<'a>>;
struct RequestFunding<'a, 'work> {
    budget: &'a mut Budget<'work>,
    retained: usize,
}
impl RequestFunding<'_, '_> {
    fn grow(&mut self, bytes: usize) -> Result<()> {
        let next = self
            .retained
            .checked_add(bytes)
            .ok_or(Resource::Arithmetic)?;
        self.budget.reserve_storage(bytes)?;
        self.retained = next;
        Ok(())
    }
}
impl Drop for RequestFunding<'_, '_> {
    fn drop(&mut self) {
        self.budget
            .release_storage(self.retained)
            .expect("native request owners retain their exclusive ledger reservation");
    }
}

/// Move-only native launched child, not ready service or compiler authority.
///
/// This borrows the original native supervisor and retains its exact launch
/// capability. Exclusively borrows the original request ledger for its complete
/// lifetime; the caller cannot retire or replace that ledger during custody.
/// Consuming failure drops endpoints and uses the request-prepaid single
/// cleanup transition; pending custody moves into the persistently funded pool.
/// No admitted legacy process, policy, readiness, or handoff owner is used.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::LaunchedProtectedIssuerV2;
/// fn clone<T: Clone>() {}
/// clone::<LaunchedProtectedIssuerV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::LaunchedProtectedIssuerV2;
/// fn fd<T: std::os::fd::AsFd>() {}
/// fd::<LaunchedProtectedIssuerV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV2 as Supervisor,
///     PreparedProtectedIssuerLaunchV2 as Prepared, ProtectedIssuerCleanupServiceV2 as Cleanup,
///     ProtectedIssuerWaitV2 as Wait};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn release_live(s: &Supervisor, p: Prepared, c: &mut Cleanup, w: Wait, b: &mut Budget<'_>) {
///     let mut child = s.launch(p, c, w, b).unwrap();
///     b.release_storage(1).unwrap();
///     child.is_live().unwrap();
/// }
/// ```
pub struct LaunchedProtectedIssuerV2<'a, 'work> {
    session: Session<'a, 'work>,
}

/// Native exact private-pipe readiness plus retained live pidfd custody.
/// Readiness remains inert; compiler and publication authority need their own checks.
pub struct ReadyProtectedIssuerV2<'a, 'work> {
    session: Session<'a, 'work>,
}

/// Native issuer custody after atomic readiness publication to Cargo.
/// No descriptor, signing operation, compiler receipt, or GPU authority is exposed.
pub struct ServingProtectedIssuerV2<'a, 'work> {
    session: Session<'a, 'work>,
}

/// Inert result of one exact consuming terminal wait, never a deferred-cleanup claim.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ReadyProtectedIssuerV2;
/// fn clone<T: Clone>() {}
/// clone::<ReadyProtectedIssuerV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ServingProtectedIssuerV2;
/// fn clone<T: Clone>() {}
/// clone::<ServingProtectedIssuerV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ExitedProtectedIssuerV2;
/// fn clone<T: Clone>() {}
/// clone::<ExitedProtectedIssuerV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV2 as Supervisor,
///     PreparedProtectedIssuerLaunchV2 as Prepared, ProtectedIssuerCleanupServiceV2 as Cleanup,
///     ProtectedIssuerWaitV2 as Wait};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn release_exited(s: &Supervisor, p: Prepared, c: &mut Cleanup, w: Wait, b: &mut Budget<'_>) {
///     let exited = s.launch(p, c, w, b).unwrap().await_readiness(w).unwrap()
///         .publish_readiness(w).unwrap().wait_for_exit(w).unwrap();
///     b.release_storage(1).unwrap();
///     drop(exited);
/// }
/// ```
pub struct ExitedProtectedIssuerV2<'a, 'work> {
    pid: u32,
    readiness: Readiness,
    termination: super::ProtectedIssuerTerminationV1,
    funding: RequestFunding<'a, 'work>,
}

impl<'a, 'work> LaunchedProtectedIssuerV2<'a, 'work> {
    /// Exact child PID paired with the private pidfd.
    pub fn pid(&self) -> u32 {
        self.session.owner.process.pid_u32()
    }
    /// Conservative full charge, preserved through state changes until final drop.
    pub const fn retained_storage(&self) -> usize {
        self.session.funding.retained
    }
    /// One prepaid non-consuming pidfd observation, not a future liveness guarantee.
    pub fn is_live(&mut self) -> Result<bool> {
        let core = &self.session.owner;
        self.session.funding.budget.with_prepaid_scope(
            core.floor()?,
            ENTRY,
            ENTRY + 2 * 1024,
            SESSION_SCRATCH,
            |_| Ok(core.process.observe_live()?),
        )
    }
    /// Consumes one private, exactly framed readiness record followed by EOF.
    /// EINTR, partial reads and pending observations all consume finite attempts.
    pub fn await_readiness(mut self, limits: Wait) -> Result<ReadyProtectedIssuerV2<'a, 'work>> {
        let core = &mut self.session.owner;
        let floor = core.floor()?;
        let readiness = self.session.funding.budget.with_prepaid_scope(
            floor,
            ENTRY,
            limits.work(),
            SESSION_SCRATCH,
            |b| {
                let deadline = limits.deadline()?;
                core.supervisor.revalidate(b)?;
                core.inputs.capability.revalidate(b)?;
                let reader = core
                    .inputs
                    .readiness_reader
                    .as_ref()
                    .ok_or(Error::State("private readiness reader is absent"))?;
                let mut bytes = [0; READY_BYTES];
                let mut used = 0;
                attempts(limits, deadline, Boundary::Readiness, || {
                    live(&core.process, Boundary::Readiness)?;
                    if used == bytes.len() {
                        let mut trailing = [0];
                        return match rustix::io::read(reader, &mut trailing) {
                            Ok(0) => Ok(Some(())),
                            Ok(_) => Err(Error::State("native readiness has trailing bytes")),
                            Err(Errno::AGAIN | Errno::INTR) => Ok(None),
                            Err(errno) => Err(Error::Io {
                                operation: "read native readiness EOF",
                                errno,
                            }),
                        };
                    }
                    match rustix::io::read(reader, &mut bytes[used..]) {
                        Ok(0) => Err(Error::State("native readiness is truncated")),
                        Ok(count) => {
                            used += count;
                            Ok(None)
                        }
                        Err(Errno::AGAIN | Errno::INTR) => Ok(None),
                        Err(errno) => Err(Error::Io {
                            operation: "read native readiness",
                            errno,
                        }),
                    }
                })?;
                let (readiness, growth) = Readiness::decode(&bytes, b)?;
                b.reserve_storage(growth.additional_storage())?;
                if !readiness.matches_launch(
                    core.process.pid_u32(),
                    core.inputs.capability.manifest(),
                    core.supervisor.policy(),
                    b,
                )? {
                    return Err(Error::State(
                        "native readiness names another launch or policy",
                    ));
                }
                before_deadline(deadline, Boundary::Readiness)?;
                live(&core.process, Boundary::Readiness)?;
                Ok(readiness)
            },
        )?;
        self.session.funding.grow(readiness.retained_storage())?;
        core.retained = self.session.funding.retained;
        core.process.release_spawn_after_exec();
        drop(core.inputs.readiness_reader.take());
        core.readiness = Some(readiness);
        Ok(ReadyProtectedIssuerV2 {
            session: self.session,
        })
    }
    /// Uses only the request-prepaid emergency transition; pending custody is retained.
    pub fn cancel(mut self) -> Result<()> {
        self.session.owner.cancel()
    }
}

impl<'a, 'work> ReadyProtectedIssuerV2<'a, 'work> {
    /// Exact pidfd-bound issuer PID.
    pub fn pid(&self) -> u32 {
        self.session.owner.process.pid_u32()
    }
    /// Full conservative retained charge, not current allocator use.
    pub const fn retained_storage(&self) -> usize {
        self.session.funding.retained
    }
    /// Inert readiness record, not a process handle.
    pub fn readiness(&self) -> &Readiness {
        self.session.owner.readiness.as_ref().expect("ready state")
    }
    /// Rechecks native supervisor/capability binding and exact pidfd liveness.
    pub fn revalidate(&mut self) -> Result<()> {
        let core = &self.session.owner;
        self.session.funding.budget.with_prepaid_scope(
            core.floor()?,
            ENTRY,
            ENTRY + 4 * 1024,
            SESSION_SCRATCH,
            |b| core.revalidate(b),
        )
    }
    /// Publishes exactly one sequenced readiness packet before entering serving custody.
    pub fn publish_readiness(
        mut self,
        limits: Wait,
    ) -> Result<ServingProtectedIssuerV2<'a, 'work>> {
        let core = &mut self.session.owner;
        let floor = core.floor()?;
        self.session
            .funding
            .budget
            .with_prepaid_scope::<(), Error>(floor, ENTRY, limits.work(), SESSION_SCRATCH, |b| {
                let deadline = limits.deadline()?;
                core.revalidate(b)?;
                let control = core
                    .inputs
                    .control
                    .as_ref()
                    .ok_or(Error::State("native Cargo control is absent"))?;
                let bytes = core
                    .readiness
                    .as_ref()
                    .ok_or(Error::State("ready record is absent"))?
                    .canonical_bytes();
                attempts(limits, deadline, Boundary::Publication, || {
                    live(&core.process, Boundary::Publication)?;
                    match rustix::net::send(
                        control,
                        bytes,
                        SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
                    ) {
                        Ok(count) if count == bytes.len() => Ok(Some(())),
                        Ok(_) => Err(Error::State(
                            "native Cargo control accepted a partial packet",
                        )),
                        Err(Errno::AGAIN | Errno::INTR) => Ok(None),
                        Err(errno) => Err(Error::Io {
                            operation: "publish native readiness",
                            errno,
                        }),
                    }
                })?;
                Ok(())
            })?;
        drop(core.inputs.control.take());
        Ok(ServingProtectedIssuerV2 {
            session: self.session,
        })
    }
    /// Performs one prepaid emergency cleanup transition and transfers any remainder.
    pub fn cancel(mut self) -> Result<()> {
        self.session.owner.cancel()
    }
}

impl<'a, 'work> ServingProtectedIssuerV2<'a, 'work> {
    /// Exact pidfd-bound issuer PID.
    pub fn pid(&self) -> u32 {
        self.session.owner.process.pid_u32()
    }
    /// Full conservative retained charge, kept until this owner is dropped or transferred.
    pub const fn retained_storage(&self) -> usize {
        self.session.funding.retained
    }
    /// Inert readiness record previously published to Cargo.
    pub fn readiness(&self) -> &Readiness {
        self.session
            .owner
            .readiness
            .as_ref()
            .expect("serving state")
    }
    /// Rechecks native readiness binding and current pidfd liveness.
    pub fn revalidate(&mut self) -> Result<()> {
        let core = &self.session.owner;
        self.session.funding.budget.with_prepaid_scope(
            core.floor()?,
            ENTRY,
            ENTRY + 4 * 1024,
            SESSION_SCRATCH,
            |b| core.revalidate(b),
        )
    }
    /// Consumes the exact terminal wait or fails closed into prepaid cleanup.
    /// The inert result retains the ledger borrow and charge until its own drop.
    pub fn wait_for_exit(mut self, limits: Wait) -> Result<ExitedProtectedIssuerV2<'a, 'work>> {
        let core = &mut self.session.owner;
        let floor = core.floor()?;
        let termination = self.session.funding.budget.with_prepaid_scope(
            floor,
            ENTRY,
            limits.work(),
            SESSION_SCRATCH,
            |_| {
                let deadline = limits.deadline()?;
                attempts(limits, deadline, Boundary::Exit, || {
                    Ok(core.process.try_reap()?)
                })
            },
        )?;
        let pid = core.process.pid_u32();
        let readiness = core
            .readiness
            .take()
            .ok_or(Error::State("serving readiness absent"))?;
        let Funded {
            owner: core,
            funding,
        } = self.session;
        drop(core);
        Ok(ExitedProtectedIssuerV2 {
            pid,
            readiness,
            termination,
            funding,
        })
    }
    /// Performs one prepaid emergency cleanup transition and transfers any remainder.
    pub fn cancel(mut self) -> Result<()> {
        self.session.owner.cancel()
    }
}
impl ExitedProtectedIssuerV2<'_, '_> {
    /// PID formerly paired with the exactly reaped pidfd.
    pub const fn pid(&self) -> u32 {
        self.pid
    }
    /// Inert readiness acknowledged before serving.
    pub const fn readiness(&self) -> &Readiness {
        &self.readiness
    }
    /// Exact observed terminal status, not a later cleanup-pump observation.
    pub const fn termination(&self) -> super::ProtectedIssuerTerminationV1 {
        self.termination
    }
    /// Full inherited conservative charge, automatically retired on Drop.
    pub const fn retained_storage(&self) -> usize {
        self.funding.retained
    }
}

impl Supervisor {
    /// Consumes native prepared custody into the shared pidfd-owned process engine.
    ///
    /// The caller must already have the exact protected process profile. This
    /// does not install privileges or select another deployment/runtime. The
    /// original supervisor and prepared reservations must be prepaid on `budget`.
    /// This consumes the prepared reservation, reserves foreground growth before
    /// clone, and exclusively borrows the ledger until custody ends. Drop retires
    /// the consumed reservation only after endpoints close and child custody is
    /// retired or transferred. An invalid incoming floor is left unchanged.
    /// Cleanup's independent
    /// persistent account must be pumped explicitly by its service controller.
    /// The gated child reports freshly observed namespaces through its private
    /// pipe. Exact framing, EOF and PID/baseline matching are required before
    /// gate release; the locked profile is never relaxed to inspect the child.
    ///
    /// Parent protocol attempts and the complete direct-child syscall allowance
    /// are debited before clone. Mutex, scheduler and kernel wait latency are not
    /// bounded by these logical quotas. No legacy admitted owner is constructed.
    pub fn launch<'a, 'work>(
        &'a self,
        prepared: Prepared,
        cleanup: &mut Cleanup,
        limits: Wait,
        budget: &'a mut Budget<'work>,
    ) -> Result<LaunchedProtectedIssuerV2<'a, 'work>> {
        let consumed = prepared.retained_storage();
        let floor = prepared
            .retained_storage()
            .checked_add(self.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        let funded_floor = floor
            .checked_add(OWNER_GROWTH)
            .ok_or(Resource::Arithmetic)?;
        if budget.storage() < floor {
            budget.charge_work(ENTRY)?;
            return Err(Resource::Accounting.into());
        }
        let mut guard = Funded {
            owner: Some(prepared),
            funding: RequestFunding {
                budget,
                retained: consumed,
            },
        };
        guard.funding.budget.charge_work(ENTRY)?;
        guard.funding.grow(OWNER_GROWTH)?;
        let retained = guard.funding.retained;
        let prepared = guard.owner.take().expect("prepared native input");
        let core = guard.funding.budget.with_prepaid_scope(
            funded_floor,
            0,
            PROTECTED_ISSUER_LAUNCH_WORK_V2 - ENTRY,
            PROTECTED_ISSUER_LAUNCH_SCRATCH_V2,
            |b| {
                let deadline = limits.deadline()?;
                self.revalidate(b)?;
                prepared.revalidate(self, b)?;
                let (profile, charge) = Profile::capture(self.credentials(), b)?;
                b.reserve_storage(charge.additional_storage())?;
                require_owned_sigchld_v2(b)?;
                let (namespaces, charge) = Namespaces::capture_self(b)?;
                b.reserve_storage(charge.additional_storage())?;
                let child_profile = ChildProfileV1 {
                    uid: profile.credentials().uid(),
                    gid: profile.credentials().gid(),
                    securebits: profile.credentials().securebits(),
                    cap_last_cap: profile.cap_last_cap(),
                };
                b.charge_work(
                    child_work(Some(child_profile))
                        .checked_add(2 * limits.work())
                        .ok_or(Resource::Arithmetic)?,
                )?;
                let reservation = cleanup.reserve_launch(b)?;
                let (profile_reader, profile_writer) = pipe(PipeFlags::NONBLOCK)?;
                let (gate_reader, gate_writer) = pipe(PipeFlags::empty())?;
                let (exec_reader, exec_writer) = pipe(PipeFlags::NONBLOCK)?;
                // Duplicated executable descriptors carry complete logical image
                // charges while the original prepared inputs still coexist.
                let staged_storage = prepared.retained_storage();
                b.reserve_storage(staged_storage)?;
                let staged = StagedLaunchV1::new(
                    prepared.staged_input(),
                    &profile_writer,
                    &gate_reader,
                    &exec_writer,
                )?;
                let spawn_lease =
                    fe2o3_artifact_transaction::try_acquire_artifact_process_spawn_lease_v1()
                        .map_err(|_| Error::State("artifact spawn coordinator is full"))?;
                let expected_parent_pid = prepared.static_manifest().parent_pid();
                let parent = rustix::process::Pid::from_raw(expected_parent_pid)
                    .ok_or(Error::State("invalid native launch parent PID"))?;
                before_deadline(deadline, Boundary::Profile)?;
                let process = spawn_child(
                    &staged,
                    Some(child_profile),
                    expected_parent_pid,
                    [
                        profile_reader.as_raw_fd(),
                        gate_writer.as_raw_fd(),
                        exec_reader.as_raw_fd(),
                        profile_writer.as_raw_fd(),
                    ],
                    reservation.into_slot(),
                    spawn_lease,
                )?;
                process.check_pidfd()?;
                drop(profile_writer);
                drop(gate_reader);
                drop(exec_writer);
                drop(staged);
                b.release_storage(staged_storage)?;
                let mut report = ProfileReportRead::new();
                attempts(limits, deadline, Boundary::Profile, || {
                    live(&process, Boundary::Profile)?;
                    match report.observe(|bytes| rustix::io::read(&profile_reader, bytes)) {
                        Ok(progress) => Ok(progress),
                        Err(ReportError::Truncated) => {
                            child_failure(&exec_reader, Boundary::Profile)
                        }
                        Err(error) => Err(ChildProcessError::from(error).into()),
                    }
                })?;
                namespaces.revalidate_self(b)?;
                namespaces.require_child_report(process.pid, parent, report.report()?, b)?;
                profile.revalidate_current(b)?;
                profile.revalidate_process(process.pid, b)?;
                self.revalidate(b)?;
                prepared.revalidate(self, b)?;
                before_deadline(deadline, Boundary::Exec)?;
                match rustix::io::write(&gate_writer, &[super::GATE_RELEASE_V1]) {
                    Ok(1) => {}
                    Ok(_) => {
                        return Err(Error::State(
                            "native launch gate accepted a partial release",
                        ));
                    }
                    Err(errno) => {
                        return Err(Error::Io {
                            operation: "release native launch gate",
                            errno,
                        });
                    }
                }
                drop(gate_writer);
                attempts(limits, deadline, Boundary::Exec, || {
                    live(&process, Boundary::Exec)?;
                    let mut record = [0; 2];
                    match rustix::io::read(&exec_reader, &mut record) {
                        Ok(0) => Ok(Some(())),
                        Ok(1) => Err(Error::ChildStage(record[0])),
                        Ok(_) => Err(Error::State("noncanonical native child exec status")),
                        Err(Errno::AGAIN | Errno::INTR) => Ok(None),
                        Err(errno) => Err(Error::Io {
                            operation: "read native child exec status",
                            errno,
                        }),
                    }
                })?;
                live(&process, Boundary::Exec)?;
                Ok(Core {
                    supervisor: self,
                    process,
                    inputs: prepared.into_launched(),
                    readiness: None,
                    retained,
                })
            },
        )?;
        Ok(LaunchedProtectedIssuerV2 {
            session: Funded {
                owner: core,
                funding: guard.funding,
            },
        })
    }
}

fn pipe(flags: PipeFlags) -> Result<(OwnedFd, OwnedFd)> {
    pipe_with(PipeFlags::CLOEXEC | flags).map_err(|errno| Error::Io {
        operation: "create native issuer protocol pipe",
        errno,
    })
}
fn child_failure<T>(status: &OwnedFd, boundary: Boundary) -> Result<Option<T>> {
    let mut record = [0; 2];
    match rustix::io::read(status, &mut record) {
        Ok(1) => Err(Error::ChildStage(record[0])),
        Ok(0) | Err(Errno::AGAIN | Errno::INTR) => Err(Error::ChildExited(boundary)),
        Ok(_) => Err(Error::State("noncanonical native child failure record")),
        Err(errno) => Err(Error::Io {
            operation: "read native child failure",
            errno,
        }),
    }
}

#[cfg(test)]
#[path = "process_native_tests.rs"]
mod tests;
