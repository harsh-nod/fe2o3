// Actual borrowed preparation launches only the root-gated application supervisor.
use super::*;
use crate::NativeApplicationRootPreparationV3 as Preparation;
use fe2o3_broker_authority_service::{
    PendingRootNativeApplicationV3 as Registration, RootNativeApplicationSupervisorV3 as Registry,
};
use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Installation;
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_runtime_protocol::NativeApplicationRootTransferV1 as Transfer;
use std::{marker::PhantomData, os::fd::OwnedFd};

/// Original direct application-supervisor child, separately funded cleanup and
/// actual root installation. This is not a decoded readiness or caller pidfd.
/// Only actual registry admission can borrow the original child; no raw FD escapes.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_coordinator::NativeApplicationSupervisorChildV3 as C;
/// fn raw<T: std::os::fd::AsFd>() {} raw::<C<'static,'static>>();
/// ```
pub struct NativeApplicationSupervisorChildV3<'root, 'work> {
    child: PlainChild,
    readiness: Ready,
    installation: &'root Installation<'work>,
    ledger: Ledger,
    account: Option<Account>,
    thread: i32,
    retained: usize,
}

/// Original root-side bootstrap after authenticated native supervisor readiness.
/// Can only move into the actual registry receiver on the same Work/account.
/// It has no arbitrary-descriptor constructor or public descriptor extraction.
pub struct NativeApplicationSupervisorControlV3<'work> {
    control: OwnedFd,
    ledger: Ledger,
    account: Option<Account>,
    thread: i32,
    retained: usize,
    work: PhantomData<&'work Work>,
}

struct PlainObserver<'a, 'work> {
    child: &'a PlainChild,
    budget: &'a mut Budget<'work>,
}
impl launch_io::Observer for PlainObserver<'_, '_> {
    type Error = Error;
    fn before_attempt(&mut self, boundary: launch_io::Boundary) -> Result<()> {
        self.budget.charge_work(boundary.work())?;
        Ok(())
    }
    fn is_live(&mut self) -> Result<bool> {
        Ok(self.child.is_live(self.budget)?)
    }
}

impl<'root, 'work> Preparation<'root, 'work> {
    /// Launches the actual pinned application supervisor while retaining original
    /// Prepared custody for the later consuming currentness issuer. The native
    /// application gate is queued before clone/gate release; the old indirect
    /// compiler launch remains refused. Returns FULL new child/control charges.
    ///
    /// # Safety
    /// The caller owns the sole original root creator thread, child wait custody
    /// and cleanup pool. Keep this actual preparation or its consuming issuer
    /// successor alive until all application startup children are terminal, and
    /// keep pumping the same independently funded cleanup account after failure.
    /// No signal handler or foreign code may steal child waits or staged inputs.
    /// Root-controlled deployment files must remain administratively stable.
    #[allow(unsafe_code)]
    pub unsafe fn launch_supervisor(
        &self,
        timeout: Duration,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> Result<(
        NativeApplicationSupervisorChildV3<'root, 'work>,
        NativeApplicationSupervisorControlV3<'work>,
        Storage,
    )> {
        let floor = sum(&[self.retained, self.installation.retained_storage()?])?;
        b.with_prepaid_scope(
            floor,
            8,
            LOCAL_WORK,
            FRAME + launch_io::ATTEMPT_SCRATCH,
            |b| {
                self.revalidate(b)?;
                self.inner.service_inputs.validate_native_application(b)?;
                self.inner.validate_cleanup_guard(cleanup, b)?;
                let deadline = launch_io::bounded_deadline(timeout)?;
                b.reserve_storage(Channels::STORAGE)?;
                let channels = Channels::new()?;
                // Credentials must already be enabled when the pre-exec gate is queued.
                rustix::net::sockopt::set_socket_passcred(&channels.child, true)
                    .map_err(|e| launch::io("enable application supervisor root credentials", e))?;
                let mut nonce = [0; 32];
                if rustix::rand::getrandom(&mut nonce, rustix::rand::GetRandomFlags::NONBLOCK)
                    .map_err(|e| launch::io("native supervisor gate entropy", e))?
                    != nonce.len()
                    || nonce == [0; 32]
                {
                    return Err(Error::Invalid(
                        "native application supervisor nonce unavailable",
                    ));
                }
                let gate = Transfer::supervisor_gate_packet(
                    *self.installation.policy().identity().as_bytes(),
                    *self.installation.supervisor().identity().as_bytes(),
                    nonce,
                    b,
                )
                .map_err(|e| Error::ApplicationCurrentness(std::io::Error::other(e)))?;
                b.charge_work(launch_io::packet_receive_work(gate.len()))?;
                if launch_io::send_packet(channels.root.as_fd(), &gate)?.is_none() {
                    return Err(Error::Invalid(
                        "native application supervisor gate unavailable",
                    ));
                }
                let (stage, stage_charge) = self.inner.stage_transfers(&channels, b)?;
                b.reserve_storage(stage_charge)?;
                self.revalidate(b)?;
                self.inner.validate_staged(&stage, &channels, b)?;
                if Instant::now() >= deadline {
                    return Err(Error::Invalid("native supervisor staging deadline"));
                }
                // SAFETY: actual final staged image/keys/anchor/lifecycle/files were
                // validated above, and the caller retains preparation/cleanup custody.
                let (mut child, charge) =
                    unsafe { stage.spawn(self.inner.credentials, cleanup, b) }?;
                b.reserve_storage(charge.additional_storage())?;
                drop(stage);
                b.release_storage(stage_charge)?;
                let Channels {
                    root,
                    child: bootstrap_child,
                    profile_reader,
                    profile_writer,
                    gate_reader,
                    gate_writer,
                    exec_reader,
                    exec_writer,
                } = channels;
                drop((bootstrap_child, profile_writer, gate_reader, exec_writer));
                launch_io::await_profile_ready(
                    profile_reader.as_fd(),
                    exec_reader.as_fd(),
                    &mut PlainObserver {
                        child: &child,
                        budget: b,
                    },
                    deadline,
                )?;
                self.inner.validate_process(child.pid(), b)?;
                launch_io::release_child(
                    gate_writer.as_fd(),
                    &mut PlainObserver {
                        child: &child,
                        budget: b,
                    },
                    deadline,
                )?;
                drop(gate_writer);
                launch_io::await_exec_eof(
                    exec_reader.as_fd(),
                    &mut PlainObserver {
                        child: &child,
                        budget: b,
                    },
                    deadline,
                )?;
                b.reserve_storage(READY_BYTES)?;
                let credentials = self.inner.credentials;
                let (bytes, descriptor) = launch_io::receive_ready_from::<READY_BYTES, false, _>(
                    root.as_fd(),
                    launch_io::MessageSender::new(
                        child.pid().as_raw_pid(),
                        credentials.uid(),
                        credentials.gid(),
                    ),
                    &mut PlainObserver {
                        child: &child,
                        budget: b,
                    },
                    deadline,
                )?;
                if descriptor.is_some() {
                    return Err(Error::Invalid(
                        "native application supervisor readiness rights",
                    ));
                }
                let (readiness, ready_charge) =
                    decode_ready(&bytes, child.pid(), self.installation.supervisor(), b)?;
                b.reserve_storage(ready_charge)?;
                self.revalidate(b)?;
                self.inner.validate_process(child.pid(), b)?;
                if !child.is_live(b)? || Instant::now() >= deadline {
                    return Err(Error::Invalid(
                        "native application supervisor readiness changed",
                    ));
                }
                // SAFETY: original child exec-status EOF plus actual native Ready and
                // final image/profile/namespace checks preceded release of this lease.
                unsafe { child.confirm_exec(b) }?;
                let child_retained = sum(&[
                    child.retained_storage(),
                    ready_charge,
                    size_of::<NativeApplicationSupervisorChildV3<'root, 'work>>(),
                ])?;
                let control_retained =
                    size_of::<NativeApplicationSupervisorControlV3<'work>>() + launch::FILE_STORAGE;
                let child = NativeApplicationSupervisorChildV3 {
                    child,
                    readiness,
                    installation: self.installation,
                    ledger: self.ledger,
                    account: self.account,
                    thread: self.thread,
                    retained: child_retained,
                };
                let control = NativeApplicationSupervisorControlV3 {
                    control: root,
                    ledger: self.ledger,
                    account: self.account,
                    thread: self.thread,
                    retained: control_retained,
                    work: PhantomData,
                };
                Ok((
                    child,
                    control,
                    Storage(sum(&[child_retained, control_retained])?),
                ))
            },
        )
    }

    /// Consumes the same genuine preparation into the original-root currentness
    /// issuer. Keep the consumed source/preparation charge and add returned growth.
    ///
    /// # Safety
    /// Retain the original creator thread, exclusive wait custody and funded
    /// cleanup account until every launched or quarantined child is terminal.
    /// All obligations of the actual `launch_application_currentness` apply.
    #[allow(unsafe_code)]
    pub unsafe fn launch_currentness<'registry, 'custody>(
        self,
        registration: &Registration<'registry, 'custody, 'work>,
        timeout: Duration,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> Result<(crate::ApplicationCurrentnessIssuerV3<'work>, Storage)> {
        self.revalidate(b)?;
        // SAFETY: actual preparation and the original process/account contract
        // are consumed directly; no compiler trace or old-family owner is invented.
        unsafe {
            self.inner
                .launch_application_currentness(registration, timeout, cleanup, b)
        }
    }
}

impl<'root, 'work> NativeApplicationSupervisorChildV3<'root, 'work> {
    pub(crate) fn require_original_installation_ready(
        &self,
        installation: &Installation<'work>,
        b: &mut Budget<'work>,
    ) -> std::io::Result<()> {
        if !std::ptr::eq(self.installation, installation) {
            return Err(std::io::Error::other(
                "application supervisor belongs to another original installation",
            ));
        }
        // Reuse actual readiness, child, image, namespace and native deployment
        // admission. Matching inert Ready bytes alone cannot open the phase gate.
        let (registry, charge) = self.admit_registry(b)?;
        b.reserve_storage(charge).map_err(std::io::Error::other)?;
        registry.revalidate(b).map_err(std::io::Error::other)?;
        drop(registry);
        b.release_storage(charge).map_err(std::io::Error::other)
    }

    /// Full charge on the unchanged original request account.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Admits the measured original child against the same retained installation.
    /// The returned registry cannot outlive this original child or installation.
    pub fn admit_registry<'child>(
        &'child self,
        b: &mut Budget<'work>,
    ) -> std::io::Result<(Registry<'child, 'work>, usize)>
    where
        'root: 'child,
    {
        b.charge_work(8).map_err(std::io::Error::other)?;
        if self.ledger != b.work_ledger_identity_v1()
            || self.account != b.storage_account_identity_v1()
            || self.thread != rustix::thread::gettid().as_raw_pid()
            || b.storage() < self.retained
        {
            return Err(std::io::Error::other(Resource::Accounting));
        }
        if !self
            .readiness
            .matches_deployment(
                launch::pid_u32(self.child.pid()).map_err(std::io::Error::other)?,
                self.installation.supervisor(),
                b,
            )
            .map_err(std::io::Error::other)?
        {
            return Err(std::io::Error::other(
                "native application supervisor readiness binding",
            ));
        }
        let (registry, charge) =
            Registry::admit(self.installation, &self.child, b).map_err(std::io::Error::other)?;
        Ok((registry, charge.additional_storage()))
    }

    /// One original prepaid cancellation step. Reaped concerns only this child,
    /// never the independently retained anchor, currentness issuer or GPU work.
    pub fn cancel(mut self) -> CleanupPoll {
        self.child.cancel()
    }
}

impl<'work> NativeApplicationSupervisorControlV3<'work> {
    /// Full original-account charge of the original control and fixed metadata.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Consumes only the original staged control into authenticated registration.
    /// Reserve returned growth above this consumed charge; retain all original
    /// child and registry reservations. There is no raw descriptor escape.
    pub fn receive<'registry, 'custody>(
        self,
        registry: &'registry Registry<'custody, 'work>,
        timeout: Duration,
        b: &mut Budget<'work>,
    ) -> std::io::Result<(Registration<'registry, 'custody, 'work>, usize)> {
        b.charge_work(8).map_err(std::io::Error::other)?;
        if self.ledger != b.work_ledger_identity_v1()
            || self.account != b.storage_account_identity_v1()
            || self.thread != rustix::thread::gettid().as_raw_pid()
            || b.storage() < self.retained
        {
            return Err(std::io::Error::other(Resource::Accounting));
        }
        Registration::receive(registry, self.control, timeout, b)
    }
}
