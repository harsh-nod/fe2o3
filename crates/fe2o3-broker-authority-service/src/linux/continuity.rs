//! One endpoint/pidfd continuity schedule, with explicit legacy and bounded I/O.
use super::checks::{self, CheckError as Check};
use super::*;

#[cfg(test)]
#[path = "continuity_tests.rs"]
mod tests;

pub(super) trait InspectionIo {
    type Error: From<Check>;
    const MIN_DUP_FD: i32;
    fn fdinfo(pidfd: &OwnedFd) -> Result<PidfdTargetObservationV1, Self::Error>;
    fn start_time(pid: u32) -> Result<u64, Self::Error>;
    fn poll(pidfd: &OwnedFd) -> Result<(i32, i16), Self::Error>;
    fn io_error(kind: AdmissionErrorKindV1, message: &'static str, error: io::Error)
    -> Self::Error;
}

pub(super) struct Legacy;
impl InspectionIo for Legacy {
    type Error = ProtectedServiceAdmissionErrorV1;
    const MIN_DUP_FD: i32 = 0;
    fn fdinfo(pidfd: &OwnedFd) -> Result<PidfdTargetObservationV1, Self::Error> {
        inspect_pidfd_target_from_procfs(pidfd)
    }
    fn start_time(pid: u32) -> Result<u64, Self::Error> {
        inspect_process_start_time_ticks(pid)
    }
    fn poll(pidfd: &OwnedFd) -> Result<(i32, i16), Self::Error> {
        poll_legacy(pidfd)
    }
    fn io_error(
        kind: AdmissionErrorKindV1,
        message: &'static str,
        error: io::Error,
    ) -> Self::Error {
        ProtectedServiceAdmissionErrorV1::io(kind, message, error)
    }
}

pub(super) struct Native;
impl InspectionIo for Native {
    type Error = Check;
    const MIN_DUP_FD: i32 = 3;
    fn fdinfo(pidfd: &OwnedFd) -> Result<PidfdTargetObservationV1, Self::Error> {
        super::native_io::inspect_pidfd_target_from_procfs(pidfd)
    }
    fn start_time(pid: u32) -> Result<u64, Self::Error> {
        super::native_io::inspect_process_start_time_ticks(pid)
    }
    fn poll(pidfd: &OwnedFd) -> Result<(i32, i16), Self::Error> {
        super::native_io::poll_once(pidfd)
    }
    fn io_error(
        kind: AdmissionErrorKindV1,
        message: &'static str,
        error: io::Error,
    ) -> Self::Error {
        // Native callers only supply last_os_error from the single ioctl below.
        Check::io(
            kind,
            message,
            rustix::io::Errno::from_raw_os_error(error.raw_os_error().unwrap_or(libc::EIO)),
        )
    }
}

pub(super) fn inspect_target<M: InspectionIo>(
    pidfd: &OwnedFd,
) -> Result<PidfdTargetObservationV1, M::Error> {
    // SAFETY: this integer-only request is zero-initialized and writable for the
    // exact 64-byte Linux v0 ABI encoded in the ioctl number.
    let mut info = unsafe { MaybeUninit::<PidfdInfoV0>::zeroed().assume_init() };
    info.mask = PIDFD_INFO_PID_V0;
    // SAFETY: the descriptor is borrowed and the request has the exact ABI size.
    let result = unsafe { libc::ioctl(pidfd.as_raw_fd(), PIDFD_GET_INFO_V0, &mut info) };
    if result != 0 {
        return dispatch_ioctl_error::<M>(pidfd, io::Error::last_os_error());
    }
    if info.mask & PIDFD_INFO_PID_V0 == 0 || info.pid == 0 || info.tgid != info.pid {
        return Err(Check::new(
            AdmissionErrorKindV1::InspectClientPidfd,
            "PIDFD_GET_INFO omitted a usable process-leader target PID",
        )
        .into());
    }
    Ok(PidfdTargetObservationV1 {
        pid: info.pid,
        source: PidfdIdentitySourceV1::KernelIoctl,
    })
}

pub(super) fn dispatch_ioctl_error<M: InspectionIo>(
    pidfd: &OwnedFd,
    error: io::Error,
) -> Result<PidfdTargetObservationV1, M::Error> {
    match error.raw_os_error() {
        // These exact v0-request failures permit a procfs proof, not admission
        // by errno. Keep the Linux 6.12 EINVAL compatibility boundary.
        Some(libc::ENOTTY) | Some(libc::EINVAL) => M::fdinfo(pidfd),
        Some(libc::ESRCH) => Err(Check::new(
            AdmissionErrorKindV1::ClientAlreadyDead,
            "client pidfd target exited before identity inspection",
        )
        .into()),
        _ => Err(M::io_error(
            AdmissionErrorKindV1::InspectClientPidfd,
            "cannot inspect client pidfd with PIDFD_GET_INFO",
            error,
        )),
    }
}

pub(super) fn require_not_pollable<M: InspectionIo>(pidfd: &OwnedFd) -> Result<(), M::Error> {
    let (ready, events) = M::poll(pidfd)?;
    if ready == 0 {
        return Ok(());
    }
    if events & (libc::POLLIN | libc::POLLHUP) != 0 {
        return Err(Check::new(
            AdmissionErrorKindV1::ClientAlreadyDead,
            "client pidfd reports process exit",
        )
        .into());
    }
    Err(Check::poll_events(events).into())
}

pub(super) fn require_live<M: InspectionIo>(pidfd: &OwnedFd) -> Result<(), M::Error> {
    require_not_pollable::<M>(pidfd)?;
    let options = rustix::process::WaitIdOptions::EXITED
        | rustix::process::WaitIdOptions::NOHANG
        | rustix::process::WaitIdOptions::NOWAIT;
    match rustix::process::waitid(rustix::process::WaitId::PidFd(pidfd.as_fd()), options) {
        Ok(Some(_)) => {
            return Err(Check::new(
                AdmissionErrorKindV1::ClientAlreadyDead,
                "client pidfd identifies an exited waitable child",
            )
            .into());
        }
        Ok(None) | Err(rustix::io::Errno::CHILD) => {}
        Err(error) => {
            return Err(Check::io(
                AdmissionErrorKindV1::InspectClientPidfd,
                "cannot perform non-reaping waitid liveness probe on client pidfd",
                error,
            )
            .into());
        }
    }
    require_not_pollable::<M>(pidfd)
}

impl LiveClientPidfdIdentityV1 {
    pub(super) fn admit_with<M: InspectionIo>(
        pidfd: OwnedFd,
        expected_client: ExpectedClientProcessIdentityV1,
    ) -> Result<Self, M::Error> {
        checks::require_close_on_exec(
            &pidfd,
            AdmissionErrorKindV1::ClientPidfdCloseOnExec,
            "client pidfd",
        )?;
        let descriptor_identity = checks::inspect_object(
            &pidfd,
            AdmissionErrorKindV1::InspectClientPidfd,
            "client pidfd",
        )?;
        let observation = inspect_target::<M>(&pidfd)?;
        checks::require_process_pidfd_mode(&pidfd)?;
        checks::require_pidfd_target(observation.pid, expected_client.pid)?;
        let start_time_ticks = M::start_time(expected_client.pid)?;
        let identity = Self {
            pidfd,
            expected_client,
            descriptor_identity,
            identity_source: observation.source,
            start_time_ticks,
        };
        identity.validate_liveness_with::<M>()?;
        Ok(identity)
    }

    pub(super) fn validate_liveness_with<M: InspectionIo>(&self) -> Result<(), M::Error> {
        checks::require_close_on_exec(
            &self.pidfd,
            AdmissionErrorKindV1::ClientPidfdCloseOnExec,
            "client pidfd",
        )?;
        if checks::inspect_object(
            &self.pidfd,
            AdmissionErrorKindV1::InspectClientPidfd,
            "client pidfd",
        )? != self.descriptor_identity
        {
            return Err(Check::new(
                AdmissionErrorKindV1::ClientPidfdIdentityChanged,
                "retained client pidfd descriptor identity changed",
            )
            .into());
        }
        let observation = inspect_target::<M>(&self.pidfd)?;
        checks::require_process_pidfd_mode(&self.pidfd)?;
        if observation.source != self.identity_source {
            return Err(Check::new(
                AdmissionErrorKindV1::ClientPidfdIdentityChanged,
                "retained client pidfd identity probe changed",
            )
            .into());
        }
        checks::require_pidfd_target(observation.pid, self.expected_client.pid)?;
        checks::require_client_start_time(
            M::start_time(self.expected_client.pid)?,
            self.start_time_ticks,
        )?;
        require_live::<M>(&self.pidfd)?;
        if inspect_target::<M>(&self.pidfd)? != observation {
            return Err(Check::new(
                AdmissionErrorKindV1::ClientPidfdIdentityChanged,
                "retained client pidfd target changed while checking liveness",
            )
            .into());
        }
        if checks::inspect_object(
            &self.pidfd,
            AdmissionErrorKindV1::InspectClientPidfd,
            "client pidfd",
        )? != self.descriptor_identity
        {
            return Err(Check::new(
                AdmissionErrorKindV1::ClientPidfdIdentityChanged,
                "retained client pidfd descriptor changed while checking liveness",
            )
            .into());
        }
        checks::require_client_start_time(
            M::start_time(self.expected_client.pid)?,
            self.start_time_ticks,
        )?;
        Ok(())
    }
}

impl ProtectedExternalAnchorServiceAdmissionV1 {
    pub(super) fn admit_with<M: InspectionIo, const DISTINCT: bool>(
        peer: OwnedFd,
        pidfd: OwnedFd,
        expected_service: CompilerExecutionExternalAnchorServiceIdentityV1,
    ) -> Result<Self, M::Error> {
        let issuer_uid = rustix::process::geteuid().as_raw();
        checks::require_close_on_exec(
            &peer,
            AdmissionErrorKindV1::PeerCloseOnExec,
            "external-anchor peer",
        )?;
        checks::validate_external_anchor_peer_status(&peer)?;
        let peer_identity = checks::validate_peer_shape(&peer)?;
        let credentials = checks::inspect_peer_credentials(&peer)?;
        if (credentials.uid, credentials.gid) != (expected_service.uid(), expected_service.gid()) {
            return Err(Check::new(
                AdmissionErrorKindV1::ExternalAnchorServiceCredentialsMismatch,
                "external-anchor peer credentials differ from the pinned service identity",
            )
            .into());
        }
        if DISTINCT && credentials.uid == issuer_uid {
            return Err(Check::new(
                AdmissionErrorKindV1::SameUidExternalAnchorService,
                "external-anchor service UID equals the protected issuer UID",
            )
            .into());
        }
        // Credential inspection already proved that pid is nonzero.
        let expected_process = ExpectedClientProcessIdentityV1 {
            pid: credentials.pid,
            uid: credentials.uid,
            gid: credentials.gid,
        };
        let live_service = LiveClientPidfdIdentityV1::admit_with::<M>(pidfd, expected_process)?;
        checks::require_distinct_peer_and_pidfd(peer_identity, live_service.descriptor_identity)?;
        let admitted = Self {
            peer,
            live_service,
            expected_service,
            issuer_uid,
            peer_identity,
            #[cfg(any(test, feature = "test-support"))]
            non_authoritative_same_uid_test: !DISTINCT,
        };
        admitted.validate_continuity_with::<M, DISTINCT>()?;
        Ok(admitted)
    }

    pub(super) fn validate_continuity_with<M: InspectionIo, const DISTINCT: bool>(
        &self,
    ) -> Result<(), M::Error> {
        if rustix::process::geteuid().as_raw() != self.issuer_uid {
            return Err(Check::new(
                AdmissionErrorKindV1::ServiceIdentityChanged,
                "protected issuer UID changed after external-anchor admission",
            )
            .into());
        }
        checks::require_close_on_exec(
            &self.peer,
            AdmissionErrorKindV1::PeerCloseOnExec,
            "external-anchor peer",
        )?;
        checks::validate_external_anchor_peer_status(&self.peer)?;
        self.live_service.validate_liveness_with::<M>()?;
        let peer_identity = checks::validate_peer_shape(&self.peer)?;
        if peer_identity != self.peer_identity {
            return Err(Check::new(
                AdmissionErrorKindV1::PeerIdentityChanged,
                "external-anchor peer descriptor identity changed",
            )
            .into());
        }
        checks::require_distinct_peer_and_pidfd(
            peer_identity,
            self.live_service.descriptor_identity,
        )?;
        let credentials = checks::inspect_peer_credentials(&self.peer)?;
        if credentials != self.live_service.expected_client.credentials()
            || (credentials.uid, credentials.gid)
                != (self.expected_service.uid(), self.expected_service.gid())
        {
            return Err(Check::new(
                AdmissionErrorKindV1::ExternalAnchorServiceCredentialsMismatch,
                "external-anchor peer credentials changed after admission",
            )
            .into());
        }
        if DISTINCT && credentials.uid == self.issuer_uid {
            return Err(Check::new(
                AdmissionErrorKindV1::SameUidExternalAnchorService,
                "external-anchor service UID equals the protected issuer UID",
            )
            .into());
        }
        self.live_service.validate_liveness_with::<M>()
    }

    pub(super) fn clone_transfer_with<M: InspectionIo, const DISTINCT: bool>(
        &self,
    ) -> Result<(OwnedFd, OwnedFd), M::Error> {
        self.validate_continuity_with::<M, DISTINCT>()?;
        let peer = rustix::io::fcntl_dupfd_cloexec(&self.peer, M::MIN_DUP_FD).map_err(|e| {
            Check::io(
                AdmissionErrorKindV1::InspectPeer,
                "cannot clone external-anchor peer for issuer transfer",
                e,
            )
        })?;
        let pidfd = rustix::io::fcntl_dupfd_cloexec(&self.live_service.pidfd, M::MIN_DUP_FD)
            .map_err(|e| {
                Check::io(
                    AdmissionErrorKindV1::InspectClientPidfd,
                    "cannot clone external-anchor pidfd for issuer transfer",
                    e,
                )
            })?;
        self.validate_transfer_with::<M, DISTINCT>(&peer, &pidfd)?;
        self.validate_continuity_with::<M, DISTINCT>()?;
        Ok((peer, pidfd))
    }

    pub(super) fn validate_transfer_with<M: InspectionIo, const DISTINCT: bool>(
        &self,
        peer: &impl AsFd,
        pidfd: &impl AsFd,
    ) -> Result<(), M::Error> {
        self.validate_continuity_with::<M, DISTINCT>()?;
        for (fd, kind, label) in [
            (
                peer.as_fd(),
                AdmissionErrorKindV1::PeerCloseOnExec,
                "transferred external-anchor peer",
            ),
            (
                pidfd.as_fd(),
                AdmissionErrorKindV1::ClientPidfdCloseOnExec,
                "transferred external-anchor pidfd",
            ),
        ] {
            let flags = rustix::io::fcntl_getfd(fd).map_err(|e| {
                Check::parts(
                    kind,
                    "cannot inspect ",
                    label,
                    " descriptor flags",
                    Some(e.raw_os_error()),
                )
            })?;
            if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
                return Err(Check::parts(
                    kind,
                    "",
                    label,
                    " descriptor does not have FD_CLOEXEC",
                    None,
                )
                .into());
            }
        }
        let peer = rustix::io::fcntl_dupfd_cloexec(peer, M::MIN_DUP_FD).map_err(|e| {
            Check::io(
                AdmissionErrorKindV1::InspectPeer,
                "cannot retain transferred external-anchor peer for revalidation",
                e,
            )
        })?;
        let pidfd = rustix::io::fcntl_dupfd_cloexec(pidfd, M::MIN_DUP_FD).map_err(|e| {
            Check::io(
                AdmissionErrorKindV1::InspectClientPidfd,
                "cannot retain transferred external-anchor pidfd for revalidation",
                e,
            )
        })?;
        let transferred = Self::admit_with::<M, DISTINCT>(peer, pidfd, self.expected_service)?;
        if transferred.peer_identity != self.peer_identity {
            return Err(Check::new(
                AdmissionErrorKindV1::PeerIdentityChanged,
                "transferred external-anchor endpoint is not the admitted socket object",
            )
            .into());
        }
        if transferred.live_service.descriptor_identity != self.live_service.descriptor_identity
            || transferred.live_service.expected_client != self.live_service.expected_client
            || transferred.live_service.start_time_ticks != self.live_service.start_time_ticks
        {
            return Err(Check::new(
                AdmissionErrorKindV1::ClientPidfdIdentityChanged,
                "transferred external-anchor pidfd is not the admitted live service process",
            )
            .into());
        }
        transferred.validate_continuity_with::<M, DISTINCT>()?;
        self.validate_continuity_with::<M, DISTINCT>()
    }
}
