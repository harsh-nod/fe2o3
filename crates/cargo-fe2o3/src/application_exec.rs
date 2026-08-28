//! Descriptor policy applied immediately before an application-boundary exec.

use std::io;
use std::os::fd::RawFd;

/// Marks every non-stdio descriptor close-on-exec without closing descriptors that trusted
/// pre-exec setup still needs. Callers may subsequently expose only their exact application ABI.
pub(crate) fn protect_all_nonstdio_descriptors() -> io::Result<()> {
    // SAFETY: close_range with CLOEXEC changes descriptor flags only; it neither dereferences
    // userspace memory nor closes descriptors before the final exec.
    if unsafe {
        libc::syscall(
            libc::SYS_close_range,
            3_u32,
            u32::MAX,
            libc::CLOSE_RANGE_CLOEXEC,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Exposes one already-validated descriptor as an intentional child ABI descriptor.
pub(crate) fn expose_descriptor(fd: RawFd) -> io::Result<()> {
    let flags = get_descriptor_flags(fd)?;
    set_descriptor_flags(fd, flags & !libc::FD_CLOEXEC)?;
    if get_descriptor_flags(fd)? & libc::FD_CLOEXEC != 0 {
        return Err(io::Error::from_raw_os_error(libc::EIO));
    }
    Ok(())
}

/// Validates one protected connected Unix `SOCK_SEQPACKET` endpoint before exposing it.
pub(crate) fn validate_and_expose_connected_seqpacket_descriptor(fd: RawFd) -> io::Result<()> {
    if get_descriptor_flags(fd)? & libc::FD_CLOEXEC == 0 {
        return Err(io::Error::from_raw_os_error(libc::ESTALE));
    }
    let mut socket_type = 0_i32;
    let mut socket_type_length = std::mem::size_of_val(&socket_type) as libc::socklen_t;
    // SAFETY: both output pointers name initialized stack storage for the complete syscall.
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            std::ptr::addr_of_mut!(socket_type).cast(),
            std::ptr::addr_of_mut!(socket_type_length),
        )
    } != 0
        || socket_type_length as usize != std::mem::size_of_val(&socket_type)
        || socket_type != libc::SOCK_SEQPACKET
    {
        return Err(io::Error::from_raw_os_error(libc::ESTALE));
    }

    // Socketpair endpoints are connected unnamed AF_UNIX sockets. Requiring a peer rejects an
    // unconnected descriptor substitution even when its socket type matches.
    let mut peer = unsafe { std::mem::zeroed::<libc::sockaddr_storage>() };
    let mut peer_length = std::mem::size_of_val(&peer) as libc::socklen_t;
    // SAFETY: the peer buffer and length remain writable for the complete syscall.
    if unsafe {
        libc::getpeername(
            fd,
            std::ptr::addr_of_mut!(peer).cast(),
            std::ptr::addr_of_mut!(peer_length),
        )
    } != 0
        || peer_length < std::mem::size_of::<libc::sa_family_t>() as libc::socklen_t
        || i32::from(peer.ss_family) != libc::AF_UNIX
    {
        return Err(io::Error::from_raw_os_error(libc::ESTALE));
    }
    expose_descriptor(fd)
}

fn get_descriptor_flags(fd: RawFd) -> io::Result<i32> {
    loop {
        // SAFETY: F_GETFD reads flags for only the supplied raw descriptor.
        let result = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        if result >= 0 {
            return Ok(result);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn set_descriptor_flags(fd: RawFd, flags: i32) -> io::Result<()> {
    loop {
        // SAFETY: F_SETFD updates flags for only the supplied raw descriptor.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, flags) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    use super::*;

    fn socket_pair(kind: i32) -> (OwnedFd, OwnedFd) {
        let mut descriptors = [-1_i32; 2];
        assert_eq!(
            unsafe {
                libc::socketpair(
                    libc::AF_UNIX,
                    kind | libc::SOCK_CLOEXEC,
                    0,
                    descriptors.as_mut_ptr(),
                )
            },
            0
        );
        unsafe {
            (
                OwnedFd::from_raw_fd(descriptors[0]),
                OwnedFd::from_raw_fd(descriptors[1]),
            )
        }
    }

    #[test]
    fn only_connected_cloexec_seqpacket_descriptors_are_exposed() {
        let (seqpacket, _peer) = socket_pair(libc::SOCK_SEQPACKET);
        validate_and_expose_connected_seqpacket_descriptor(seqpacket.as_raw_fd()).unwrap();
        assert_eq!(
            get_descriptor_flags(seqpacket.as_raw_fd()).unwrap() & libc::FD_CLOEXEC,
            0
        );

        let (stream, _peer) = socket_pair(libc::SOCK_STREAM);
        assert!(validate_and_expose_connected_seqpacket_descriptor(stream.as_raw_fd()).is_err());

        let unconnected =
            unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
        assert!(unconnected >= 0);
        let unconnected = unsafe { OwnedFd::from_raw_fd(unconnected) };
        assert!(
            validate_and_expose_connected_seqpacket_descriptor(unconnected.as_raw_fd()).is_err()
        );
    }
}
