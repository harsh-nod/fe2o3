//! Fixed syscall operations funded by the example's outer logical I/O charge.
use super::*;

fn errno() -> Option<i32> {
    std::io::Error::last_os_error().raw_os_error()
}

pub(super) fn require_closed(fd: RawFd) -> Result<()> {
    // SAFETY: F_GETFD only observes the scalar descriptor; it borrows no memory.
    require(unsafe { libc::fcntl(fd, libc::F_GETFD) } == -1 && errno() == Some(libc::EBADF))
}

pub(super) fn close_checked(fd: RawFd) -> Result<()> {
    // SAFETY: the caller transfers this descriptor's sole ownership. Linux
    // closes even on EINTR; never retry a close against a potentially reused fd.
    require(unsafe { libc::close(fd) } == 0)?;
    require_closed(fd)
}

pub(super) fn close_unused() -> Result<()> {
    // SAFETY: this single-threaded fixture owns its inherited descriptor table.
    // Do not retain root, signing-key or anchor handles, or any high-fd aliases.
    require(unsafe { libc::syscall(libc::SYS_close_range, 12_u32, u32::MAX, 0_u32) } == 0)?;
    for fd in [0, 1, 2, 3, 5, 7, 10, 11] {
        close_checked(fd)?;
    }
    Ok(())
}

fn take(fd: RawFd) -> Result<OwnedFd> {
    // SAFETY: scalar fcntl queries/updates on a fixed inherited descriptor.
    require(unsafe { libc::fcntl(fd, libc::F_GETFD) } == 0)?;
    // SAFETY: this example is the sole owner; no duplicate of the writer is made.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: F_SETFD changes flags only; owned remains live throughout the call.
    require(unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == 0)?;
    Ok(owned)
}

pub(super) fn take_writer() -> Result<OwnedFd> {
    let writer = take(READY_FD)?;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat writes one initialized struct on success; writer stays live.
    require(unsafe { libc::fstat(writer.as_raw_fd(), stat.as_mut_ptr()) } == 0)?;
    // SAFETY: the successful fstat above initialized the complete struct.
    let stat = unsafe { stat.assume_init() };
    // SAFETY: these are scalar queries on the owned pipe descriptor.
    let flags = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) };
    // SAFETY: F_GETPIPE_SZ only queries the owned descriptor.
    let capacity = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETPIPE_SZ) };
    // SAFETY: fpathconf queries the owned descriptor with a fixed scalar key.
    let atomic = unsafe { libc::fpathconf(writer.as_raw_fd(), libc::_PC_PIPE_BUF) };
    require(
        stat.st_mode & libc::S_IFMT == libc::S_IFIFO
            && flags >= 0
            && flags & libc::O_ACCMODE == libc::O_WRONLY
            && flags & libc::O_NONBLOCK != 0
            && flags & (libc::O_APPEND | libc::O_ASYNC | libc::O_DIRECT) == 0
            && capacity >= (READY_BYTES + 1) as i32
            && atomic >= (READY_BYTES + 1) as libc::c_long,
    )?;
    Ok(writer)
}

pub(super) fn take_peer() -> Result<OwnedFd> {
    let peer = take(PEER_FD)?;
    for (key, expected) in [
        (libc::SO_TYPE, libc::SOCK_SEQPACKET),
        (libc::SO_DOMAIN, libc::AF_UNIX),
    ] {
        let mut value: libc::c_int = 0;
        let mut length = size_of::<libc::c_int>() as libc::socklen_t;
        // SAFETY: getsockopt writes the fixed scalar output and its length.
        require(
            unsafe {
                libc::getsockopt(
                    peer.as_raw_fd(),
                    libc::SOL_SOCKET,
                    key,
                    (&raw mut value).cast(),
                    &mut length,
                )
            } == 0,
        )?;
        require(length as usize == size_of::<libc::c_int>() && value == expected)?;
    }
    Ok(peer)
}

fn poll(fd: RawFd, events: libc::c_short) -> Result<()> {
    let mut descriptor = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    // SAFETY: the fixed pollfd is writable; the timeout is always finite.
    let result = unsafe { libc::poll(&mut descriptor, 1, 50) };
    require(result >= 0 || errno() == Some(libc::EINTR))?;
    require(descriptor.revents & (libc::POLLERR | libc::POLLNVAL) == 0)
}

pub(super) fn write_packet(writer: &OwnedFd, bytes: &[u8]) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    for _ in 0..WRITE_ATTEMPTS {
        require(Instant::now() < deadline)?;
        // SAFETY: the borrowed slice is readable and the nonblocking writer is
        // live. Its admitted PIPE_BUF covers this entire atomic packet.
        let written =
            unsafe { libc::write(writer.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
        if written == bytes.len() as isize {
            return Ok(());
        }
        require(written == -1 && matches!(errno(), Some(libc::EINTR | libc::EAGAIN)))?;
        poll(writer.as_raw_fd(), libc::POLLOUT)?;
    }
    Err(Refused)
}

pub(super) fn wait_for_finish(peer: &OwnedFd, accept_finish: bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(25);
    for _ in 0..WAIT_ATTEMPTS {
        require(Instant::now() < deadline)?;
        if !accept_finish {
            // poll ignores negative descriptors and performs only the finite wait.
            poll(-1, 0)?;
            continue;
        }
        poll(peer.as_raw_fd(), libc::POLLIN)?;
        let mut bytes = [0; 1];
        let mut vector = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        // SAFETY: zero is valid for an empty msghdr; all buffers are installed below.
        let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
        message.msg_iov = &mut vector;
        message.msg_iovlen = 1;
        // SAFETY: fixed writable packet/iovec/msghdr storage remains live. No
        // ancillary buffer is supplied; the kernel discards undisclosed rights.
        let length = unsafe {
            libc::recvmsg(
                peer.as_raw_fd(),
                &mut message,
                libc::MSG_DONTWAIT | libc::MSG_TRUNC,
            )
        };
        if length == -1 && matches!(errno(), Some(libc::EINTR | libc::EAGAIN)) {
            continue;
        }
        require(
            length == 1
                && bytes == [0x01]
                && message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) == 0,
        )?;
        return Ok(());
    }
    Err(Refused)
}
