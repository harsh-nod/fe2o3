//! Original legacy child custody, cancellation and consuming reaping.
use std::{
    io,
    os::fd::{AsFd, OwnedFd},
};

pub(crate) struct RootOwnedProtectedServiceChildV1 {
    pub(super) pid: rustix::process::Pid,
    pub(super) pidfd: Option<OwnedFd>,
    pub(super) reaping_ownership_lost: bool,
}

impl RootOwnedProtectedServiceChildV1 {
    #[cfg(feature = "test-support")]
    pub(crate) fn admit_non_authoritative_test(
        pid: rustix::process::Pid,
        pidfd: OwnedFd,
    ) -> io::Result<Self> {
        if !rustix::io::fcntl_getfd(&pidfd)
            .map_err(io::Error::from)?
            .contains(rustix::io::FdFlags::CLOEXEC)
        {
            return Err(io::Error::from_raw_os_error(libc::EPERM));
        }
        Ok(Self {
            pid,
            pidfd: Some(pidfd),
            reaping_ownership_lost: false,
        })
    }

    pub(crate) const fn pid(&self) -> rustix::process::Pid {
        self.pid
    }

    pub(super) fn pidfd(&self) -> &OwnedFd {
        self.pidfd.as_ref().expect("live child retains pidfd")
    }

    pub(crate) fn is_live(&self) -> io::Result<bool> {
        match rustix::process::waitid(
            rustix::process::WaitId::PidFd(self.pidfd().as_fd()),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        ) {
            Ok(None) | Err(rustix::io::Errno::INTR) => Ok(true),
            Ok(Some(_)) => Ok(false),
            Err(source) => Err(source.into()),
        }
    }

    pub(crate) fn try_clone_pidfd(&self) -> io::Result<OwnedFd> {
        rustix::io::fcntl_dupfd_cloexec(self.pidfd(), 0).map_err(io::Error::from)
    }

    pub(crate) fn exit_description(&self, fallback: &'static str) -> String {
        rustix::process::waitid(
            rustix::process::WaitId::PidFd(self.pidfd().as_fd()),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        )
        .ok()
        .flatten()
        .map_or_else(|| fallback.to_owned(), |status| format!("{status:?}"))
    }

    pub(crate) fn cancel_and_reap(&mut self) -> Result<(), ReapErrorV1> {
        if self.reaping_ownership_lost {
            return Err(ReapErrorV1::OwnershipLost);
        }
        let Some(pidfd) = self.pidfd.as_ref() else {
            return Ok(());
        };
        let signal_error =
            match rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => None,
                Err(pidfd_source) => {
                    match rustix::process::kill_process(self.pid, rustix::process::Signal::KILL) {
                        Ok(()) | Err(rustix::io::Errno::SRCH) => {
                            Some(io::Error::from(pidfd_source))
                        }
                        Err(_) => return Err(ReapErrorV1::Io(pidfd_source.into())),
                    }
                }
            };
        let wait_result = loop {
            match rustix::process::waitid(
                rustix::process::WaitId::PidFd(pidfd.as_fd()),
                rustix::process::WaitIdOptions::EXITED,
            ) {
                Ok(Some(_)) => break Ok(()),
                Ok(None) | Err(rustix::io::Errno::INTR) => {}
                Err(rustix::io::Errno::CHILD) => break Err(ReapErrorV1::OwnershipLost),
                Err(source) => break Err(ReapErrorV1::Io(source.into())),
            }
        };
        match wait_result {
            Ok(()) => {
                self.pidfd.take();
                signal_error.map_or(Ok(()), |source| Err(ReapErrorV1::Io(source)))
            }
            Err(ReapErrorV1::OwnershipLost) => {
                self.pidfd.take();
                self.reaping_ownership_lost = true;
                Err(ReapErrorV1::OwnershipLost)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn poll_cancel_and_reap(&mut self) -> Result<bool, ReapErrorV1> {
        if self.reaping_ownership_lost {
            return Err(ReapErrorV1::OwnershipLost);
        }
        let Some(pidfd) = self.pidfd.as_ref() else {
            return Ok(true);
        };
        match rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => (),
            Err(error) => return Err(ReapErrorV1::Io(error.into())),
        }
        match rustix::process::waitid(
            rustix::process::WaitId::PidFd(pidfd.as_fd()),
            rustix::process::WaitIdOptions::EXITED | rustix::process::WaitIdOptions::NOHANG,
        ) {
            Ok(None) | Err(rustix::io::Errno::INTR) => Ok(false),
            Ok(Some(_)) => {
                self.pidfd.take();
                Ok(true)
            }
            Err(rustix::io::Errno::CHILD) => {
                self.pidfd.take();
                self.reaping_ownership_lost = true;
                Err(ReapErrorV1::OwnershipLost)
            }
            Err(error) => Err(ReapErrorV1::Io(error.into())),
        }
    }
}

impl Drop for RootOwnedProtectedServiceChildV1 {
    fn drop(&mut self) {
        while self.pidfd.is_some() {
            let _ = self.cancel_and_reap();
            if self.pidfd.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
}

pub(crate) enum ReapErrorV1 {
    OwnershipLost,
    Io(io::Error),
}
