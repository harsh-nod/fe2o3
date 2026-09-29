//! Root-created channel provenance, not issuer or compiler admission.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use rustix::{fs, io, net, process};
use std::{
    fmt,
    marker::PhantomData,
    mem::size_of,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    rc::Rc,
};

const ENTRY: usize = 8;
const FD_STORAGE: usize = size_of::<(OwnedFd, usize)>();
// Root checks, socketpair, two passcred settings, both complete endpoint checks,
// and refusal cleanup. All operations are single-attempt and nonallocating.
const WORK: usize = ENTRY + 64 * 1088;
const FRAME: usize = 4 * size_of::<RootLaunchChannelV3<'static>>()
    + 8 * size_of::<Error>()
    + 4 * size_of::<net::SocketAddrAny>()
    + 4096;

/// Full unreserved channel charge. Reserve it on the original account before
/// retaining the owner; release only after the whole channel owner is dropped.
#[derive(Debug)]
pub struct RootLaunchChannelStorageV3(usize);
impl RootLaunchChannelStorageV3 {
    pub const fn additional_storage(&self) -> usize {
        self.0
    }
}

/// Move-only root-created socketpair. No arbitrary-descriptor constructor,
/// root-end extraction, issuer authentication, or compiler authority.
///
/// Create inside the confirmed held-exec input callback, AFTER compiler clone.
/// Stage only `issuer_endpoint`; never pass either end to the compiler. Close
/// all stage/parent issuer aliases before readiness completes, then close this
/// owner's issuer alias. These historical launch obligations cannot be inferred
/// merely by inspecting a socketpair. The later broker session must independently
/// validate the actual retained issuer, challenge it after readiness, and retain
/// compiler occurrence custody separately from the replaceable connection.
///
/// Keep the original Work borrow live and Budget at its admitting address, on
/// the creator thread, through owner destruction. Work/account identity is not
/// persistent identity after that borrow. All calls preserve entry storage and
/// cumulative work/denial history; Drop only closes owned descriptors.
/// The conservative full retained charge stays fixed after parent-alias closure.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootLaunchChannelV3 as C;
/// fn duplicate(c: C<'_>) { let _ = c.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootLaunchChannelV3 as C;
/// fn descriptor<T: std::os::fd::AsFd>() {} descriptor::<C<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootLaunchChannelV3 as C;
/// fn send<T: Send>() {} send::<C<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootLaunchChannelV3 as C;
/// fn root(c: C<'_>) { let _ = c.root; }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootLaunchChannelV3 as C;
/// fn adopt(fd: std::os::fd::OwnedFd) { let _ = C::from_fd(fd); }
/// ```
pub struct RootLaunchChannelV3<'work> {
    root: OwnedFd,
    issuer: Option<OwnedFd>,
    account: Ledger,
    budget_address: usize,
    creator: process::Pid,
    process: process::Pid,
    _work: PhantomData<(&'work Work, Rc<()>)>,
}

impl<'work> RootLaunchChannelV3<'work> {
    pub const WORK: usize = WORK;
    pub const SCRATCH: usize = FRAME;
    pub const STORAGE: usize = size_of::<(Self, RootLaunchChannelStorageV3)>() + 2 * FD_STORAGE;

    pub fn create(b: &mut Budget<'work>) -> Result<(Self, RootLaunchChannelStorageV3)> {
        b.with_prepaid_scope(0, ENTRY, WORK, FRAME, |b| {
            require_root()?;
            let (root, issuer) = net::socketpair(
                net::AddressFamily::UNIX,
                net::SocketType::SEQPACKET,
                net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
                None,
            )?;
            net::sockopt::set_socket_passcred(&root, true)?;
            net::sockopt::set_socket_passcred(&issuer, true)?;
            let pid = process::getpid();
            validate_endpoint(root.as_fd(), pid)?;
            validate_endpoint(issuer.as_fd(), pid)?;
            Ok((
                Self {
                    root,
                    issuer: Some(issuer),
                    account: b.work_ledger_identity_v1(),
                    budget_address: b as *const Budget<'_> as usize,
                    creator: rustix::thread::gettid(),
                    process: pid,
                    _work: PhantomData,
                },
                RootLaunchChannelStorageV3(Self::STORAGE),
            ))
        })
    }

    /// Borrow only the issuer side for exact descriptor staging. This borrow
    /// cannot overlap closure of this owner's alias. External duplicates remain
    /// the launcher's explicitly tracked responsibility.
    pub fn issuer_endpoint(&self, b: &mut Budget<'_>) -> Result<BorrowedFd<'_>> {
        b.with_prepaid_scope(Self::STORAGE, ENTRY, WORK, FRAME, |b| {
            self.check_account(b)?;
            let issuer = self
                .issuer
                .as_ref()
                .ok_or(Error::Refused("issuer endpoint closed"))?;
            validate_endpoint(self.root.as_fd(), self.process)?;
            validate_endpoint(issuer.as_fd(), self.process)?;
            Ok(issuer.as_fd())
        })
    }

    /// Closes this one parent alias, idempotently. Does NOT certify closure of
    /// stage/child duplicates, receipt acknowledgment, or occurrence retirement.
    pub fn close_parent_issuer_endpoint(&mut self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(Self::STORAGE, ENTRY, WORK, FRAME, |b| {
            self.check_account(b)?;
            drop(self.issuer.take());
            Ok(())
        })
    }

    pub const fn retained_storage(&self) -> usize {
        Self::STORAGE
    }

    fn check_account(&self, b: &Budget<'_>) -> Result<()> {
        if self.account != b.work_ledger_identity_v1()
            || self.budget_address != b as *const Budget<'_> as usize
            || self.creator != rustix::thread::gettid()
            || self.process != process::getpid()
        {
            return Err(Resource::Accounting.into());
        }
        require_root()
    }
}

fn require_root() -> Result<()> {
    fe2o3_protected_service_spawn::require_exact_root_identity_v1()
        .map_err(|_| Error::Refused("root control requires exact root identity"))
}

fn validate_endpoint(fd: BorrowedFd<'_>, creator: process::Pid) -> Result<()> {
    let unnamed: net::SocketAddrAny = net::SocketAddrUnix::new_unnamed().into();
    if io::fcntl_getfd(fd)? != io::FdFlags::CLOEXEC
        || fs::fcntl_getfl(fd)? != (fs::OFlags::RDWR | fs::OFlags::NONBLOCK)
        || net::sockopt::socket_type(fd)? != net::SocketType::SEQPACKET
        || !net::sockopt::socket_passcred(fd)?
        || net::getsockname(fd)? != unnamed
        || net::getpeername(fd)? != Some(unnamed)
    {
        return Err(Error::Refused("root control endpoint shape"));
    }
    let peer = net::sockopt::socket_peercred(fd)?;
    if peer.pid != creator || peer.uid.as_raw() != 0 || peer.gid.as_raw() != 0 {
        return Err(Error::Refused("root control socket creator"));
    }
    Ok(())
}

#[derive(Debug)]
pub enum RootLaunchChannelErrorV3 {
    Resource(Resource),
    Io(io::Errno),
    Refused(&'static str),
}
use RootLaunchChannelErrorV3 as Error;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<io::Errno> for Error {
    fn from(e: io::Errno) -> Self {
        Self::Io(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Io(e) => e.fmt(f),
            Self::Refused(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Refused(_) => None,
        }
    }
}
impl fmt::Debug for RootLaunchChannelV3<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootLaunchChannelV3")
            .field("issuer_alias_open", &self.issuer.is_some())
            .field("authority", &"channel-custody-only")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "compiler_execution_root_channel_lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_denies_work_and_scratch_before_root_or_descriptor_inspection() {
        for (work, storage, spent) in [
            (ENTRY - 1, FRAME, 0),
            (WORK - 1, FRAME, ENTRY),
            (WORK, FRAME - 1, WORK),
        ] {
            let mut work = Work::new(work);
            let mut b = Budget::new(&mut work, storage);
            assert!(matches!(
                RootLaunchChannelV3::create(&mut b),
                Err(Error::Resource(_))
            ));
            assert_eq!(b.work(), spent);
            assert_eq!(b.storage(), 0);
        }
    }

    #[test]
    fn creation_requires_real_root() {
        if require_root().is_ok() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "compiler_execution_root_channel::tests::unprivileged_role",
                    "--test-threads=1",
                ])
                .env("FE2O3_ROOT_CHANNEL_UNPRIVILEGED_TEST", "1")
                .status()
                .unwrap();
            assert!(status.success());
        } else {
            assert_root_refusal();
        }
    }

    #[test]
    fn unprivileged_role() {
        if std::env::var_os("FE2O3_ROOT_CHANNEL_UNPRIVILEGED_TEST").is_none() {
            return;
        }
        if require_root().is_ok() {
            // SAFETY: this explicitly selected subprocess alone drops its own
            // credentials after exec, without depending on build-path traversal.
            assert_eq!(unsafe { libc::setgroups(0, std::ptr::null()) }, 0);
            assert_eq!(unsafe { libc::setresgid(65534, 65534, 65534) }, 0);
            assert_eq!(unsafe { libc::setresuid(65534, 65534, 65534) }, 0);
        }
        assert!(require_root().is_err());
        assert_root_refusal();
    }

    fn assert_root_refusal() {
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, FRAME);
        assert!(matches!(
            RootLaunchChannelV3::create(&mut b),
            Err(Error::Refused("root control requires exact root identity"))
        ));
        assert_eq!(b.storage(), 0);
        assert_eq!(b.work(), WORK);
        assert!(RootLaunchChannelV3::STORAGE <= FRAME);
    }
}
