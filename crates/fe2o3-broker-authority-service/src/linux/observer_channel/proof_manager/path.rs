use super::*;
use rustix::fs::{AtFlags, Mode};
use std::{fs::File, os::unix::fs::MetadataExt};

const DIRECTORY: &str = "fe2o3-proof-custodian";
const ENTRY: &str = "control.sock";
pub(super) const PATH: &str = "/run/fe2o3-proof-custodian/control.sock";
type Identity = (u64, u64, u32, u32, u32);
fn identity(file: &File) -> io::Result<Identity> {
    let m = file.metadata()?;
    Ok((m.dev(), m.ino(), m.mode(), m.uid(), m.gid()))
}
pub(super) struct SocketPath {
    root: File,
    directory: File,
    root_identity: Identity,
    directory_identity: Identity,
    socket: Option<(u64, u64)>,
    owns_entry: bool,
}
impl SocketPath {
    fn open() -> Result<Self> {
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let root: File = rustix::fs::open("/run", flags, Mode::empty())?.into();
        let directory: File = rustix::fs::openat(&root, DIRECTORY, flags, Mode::empty())?.into();
        let root_identity = identity(&root)?;
        let directory_identity = identity(&directory)?;
        if root_identity.3 != 0
            || root_identity.4 != 0
            || root_identity.2 & 0o022 != 0
            || directory_identity.2 != libc::S_IFDIR | 0o700
            || directory_identity.3 != 0
            || directory_identity.4 != 0
        {
            return Err(invalid(
                "manager runtime directory is not private root custody",
            ));
        }
        Ok(Self {
            root,
            directory,
            root_identity,
            directory_identity,
            socket: None,
            owns_entry: false,
        })
    }
    pub(super) fn connect() -> Result<Option<(Self, OwnedFd)>> {
        Self::connect_with(|| Ok(()))
    }
    fn connect_with(after_connect: impl FnOnce() -> Result<()>) -> Result<Option<(Self, OwnedFd)>> {
        let pending_path = (|| {
            let mut path = Self::open()?;
            path.capture_socket()?;
            path.revalidate()?;
            Ok(path)
        })();
        let path = match pending_path {
            Ok(path) => path,
            Err(CompilerExecutionObserverErrorV1::Io(error))
                if error.raw_os_error() == Some(libc::ENOENT) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let peer = socket()?;
        rustix::net::bind(&peer, &rustix::net::SocketAddrUnix::new_unnamed())?;
        match rustix::net::connect(&peer, &rustix::net::SocketAddrUnix::new(PATH)?) {
            Ok(()) => {}
            Err(error) if listener_not_ready(error) => return Ok(None),
            Err(error) => return Err(error.into()),
        }
        // Once connect succeeds, every subsequent error is terminal, including ENOENT.
        after_connect()?;
        path.revalidate()?;
        Ok(Some((path, peer)))
    }
    pub(super) fn listen() -> Result<(Self, OwnedFd)> {
        let mut path = Self::open()?;
        let peer = socket()?;
        rustix::net::bind(&peer, &rustix::net::SocketAddrUnix::new(PATH)?)?;
        // Own only the newly bound entry; never unlink a preexisting or substituted socket.
        let created = rustix::fs::statat(&path.directory, ENTRY, AtFlags::SYMLINK_NOFOLLOW)?;
        path.socket = Some((created.st_dev, created.st_ino));
        path.owns_entry = true;
        rustix::fs::chmodat(
            &path.directory,
            ENTRY,
            Mode::from_raw_mode(0o600),
            AtFlags::empty(),
        )?;
        path.capture_socket()?;
        rustix::net::listen(&peer, 1)?;
        path.revalidate()?;
        Ok((path, peer))
    }
    fn capture_socket(&mut self) -> Result<()> {
        let value = rustix::fs::statat(&self.directory, ENTRY, AtFlags::SYMLINK_NOFOLLOW)?;
        if value.st_mode != libc::S_IFSOCK | 0o600
            || value.st_uid != 0
            || value.st_gid != 0
            || value.st_nlink != 1
            || self
                .socket
                .is_some_and(|old| old != (value.st_dev, value.st_ino))
        {
            return Err(invalid(
                "manager socket path changed or is not private root-owned",
            ));
        }
        self.socket = Some((value.st_dev, value.st_ino));
        Ok(())
    }
    pub(super) fn revalidate(&self) -> Result<()> {
        let mut current = Self::open()?;
        current.capture_socket()?;
        if identity(&self.root)? != self.root_identity
            || identity(&self.directory)? != self.directory_identity
            || current.root_identity != self.root_identity
            || current.directory_identity != self.directory_identity
            || current.socket != self.socket
        {
            return Err(invalid("manager socket path continuity changed"));
        }
        Ok(())
    }
}

fn listener_not_ready(error: rustix::io::Errno) -> bool {
    matches!(
        error,
        rustix::io::Errno::NOENT | rustix::io::Errno::CONNREFUSED
    )
}

#[cfg(test)]
pub(in super::super) fn reject_post_connect_removal_for_qualification() {
    let result = SocketPath::connect_with(|| {
        std::fs::remove_file(PATH)?;
        Ok(())
    });
    assert!(
        matches!(result, Err(CompilerExecutionObserverErrorV1::Io(error))
        if error.raw_os_error() == Some(libc::ENOENT))
    );
}

impl Drop for SocketPath {
    fn drop(&mut self) {
        if self.owns_entry
            && let Ok(value) = rustix::fs::statat(&self.directory, ENTRY, AtFlags::SYMLINK_NOFOLLOW)
            && self.socket == Some((value.st_dev, value.st_ino))
        {
            let _ = rustix::fs::unlinkat(&self.directory, ENTRY, AtFlags::empty());
        }
    }
}
fn socket() -> Result<OwnedFd> {
    let peer = rustix::net::socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )?;
    rustix::net::sockopt::set_socket_passcred(&peer, true)?;
    Ok(peer)
}
pub(in super::super) fn addresses(
    peer: &OwnedFd,
) -> Result<(rustix::net::SocketAddrUnix, rustix::net::SocketAddrUnix)> {
    if rustix::net::sockopt::socket_domain(peer)? != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(peer)? != SocketType::SEQPACKET
        || rustix::net::sockopt::socket_acceptconn(peer)?
    {
        return Err(invalid("manager endpoint is not connected seqpacket"));
    }
    let local = rustix::net::SocketAddrUnix::try_from(rustix::net::getsockname(peer)?)?;
    let remote = rustix::net::SocketAddrUnix::try_from(
        rustix::net::getpeername(peer)?.ok_or_else(|| invalid("manager endpoint disconnected"))?,
    )?;
    let expected = rustix::net::SocketAddrUnix::new(PATH)?;
    if !((local == expected && remote.abstract_name().is_some())
        || (remote == expected && local.abstract_name().is_some()))
    {
        return Err(invalid(
            "manager endpoint does not match fixed path and abstract client",
        ));
    }
    Ok((local, remote))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_absent_initial_listener_is_retryable() {
        for code in [rustix::io::Errno::NOENT, rustix::io::Errno::CONNREFUSED] {
            assert!(listener_not_ready(code));
        }
        for code in [
            rustix::io::Errno::ACCESS,
            rustix::io::Errno::AGAIN,
            rustix::io::Errno::INTR,
            rustix::io::Errno::NOTDIR,
            rustix::io::Errno::LOOP,
        ] {
            assert!(!listener_not_ready(code));
        }
    }
}
