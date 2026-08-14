//! Fail-closed Linux launch profile for descriptor-bearing applications.

use std::fs::File;
use std::io::{self, Write};
use std::mem::{self, MaybeUninit};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::net::UnixDatagram;
use std::ptr;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(crate) const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
const BPF_LOAD_WORD_ABSOLUTE: u16 = 0x20;
const BPF_JUMP_EQUAL: u16 = 0x15;
const BPF_RETURN: u16 = 0x06;
const SECCOMP_DATA_NUMBER_OFFSET: u32 = 0;
const SECCOMP_DATA_ARCH_OFFSET: u32 = 4;
const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
const SECCOMP_RET_USER_NOTIF: u32 = 0x7fc0_0000;
const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
const SECCOMP_SET_MODE_FILTER: libc::c_uint = 1;
const SECCOMP_FILTER_FLAG_NEW_LISTENER: libc::c_ulong = 1 << 3;
const SECCOMP_USER_NOTIF_FLAG_CONTINUE: u32 = 1;
const SUPERVISOR_READY_TIMEOUT: Duration = Duration::from_secs(5);
const LISTENER_MESSAGE_MAGIC: [u8; 8] = *b"f2exec01";

#[repr(C)]
#[derive(Clone, Copy)]
struct ListenerMessage {
    magic: [u8; 8],
    pid: u32,
    reserved: u32,
}

pub(crate) struct PendingApplicationSandbox {
    child_socket: Option<UnixDatagram>,
    ready: Receiver<Result<u32, String>>,
    shutdown: Option<File>,
    worker: Option<JoinHandle<Result<(), String>>>,
}

pub(crate) struct ApplicationSandboxGuard {
    shutdown: Option<File>,
    worker: Option<JoinHandle<Result<(), String>>>,
}

pub(crate) struct SandboxAdmissionFailure {
    message: String,
    cleanup: Option<ApplicationSandboxGuard>,
}

impl SandboxAdmissionFailure {
    pub(crate) fn into_parts(mut self) -> (String, ApplicationSandboxGuard) {
        (
            self.message,
            self.cleanup
                .take()
                .expect("sandbox admission failure owns cleanup state"),
        )
    }
}

impl PendingApplicationSandbox {
    pub(crate) fn start() -> Result<Self, String> {
        let (parent_socket, child_socket) = UnixDatagram::pair()
            .map_err(|error| format!("failed to create seccomp listener channel: {error}"))?;
        parent_socket.set_nonblocking(true).map_err(|error| {
            format!("failed to make seccomp listener channel nonblocking: {error}")
        })?;
        let (shutdown_read, shutdown_write) = cloexec_pipe()
            .map_err(|error| format!("failed to create seccomp shutdown pipe: {error}"))?;
        let (ready_send, ready) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("fe2o3-seccomp-exec-supervisor".into())
            .spawn(move || supervise_exec_notifications(parent_socket, shutdown_read, ready_send))
            .map_err(|error| format!("failed to start seccomp exec supervisor: {error}"))?;
        Ok(Self {
            child_socket: Some(child_socket),
            ready,
            shutdown: Some(shutdown_write),
            worker: Some(worker),
        })
    }

    pub(crate) fn child_socket_fd(&self) -> RawFd {
        self.child_socket
            .as_ref()
            .expect("pending sandbox owns its child socket")
            .as_raw_fd()
    }

    pub(crate) fn complete(
        mut self,
        child_id: u32,
    ) -> Result<ApplicationSandboxGuard, SandboxAdmissionFailure> {
        drop(self.child_socket.take());
        let supervised_id = match self.ready.recv_timeout(SUPERVISOR_READY_TIMEOUT) {
            Ok(Ok(supervised_id)) => supervised_id,
            Ok(Err(error)) => return Err(self.failure(error)),
            Err(error) => {
                return Err(self.failure(format!(
                    "seccomp exec supervisor did not admit initial exec: {error}"
                )));
            }
        };
        if supervised_id != child_id {
            return Err(self.failure(format!(
                "seccomp exec supervisor admitted PID {supervised_id}, expected {child_id}"
            )));
        }
        Ok(ApplicationSandboxGuard {
            shutdown: self.shutdown.take(),
            worker: self.worker.take(),
        })
    }

    fn failure(&mut self, message: String) -> SandboxAdmissionFailure {
        let mut cleanup = ApplicationSandboxGuard {
            shutdown: self.shutdown.take(),
            worker: self.worker.take(),
        };
        cleanup.request_shutdown();
        SandboxAdmissionFailure {
            message,
            cleanup: Some(cleanup),
        }
    }

    #[cfg(test)]
    fn test_stalled_pending(
        release: std::sync::Arc<std::sync::atomic::AtomicBool>,
        completed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        use std::sync::atomic::Ordering;

        let (parent_socket, child_socket) = UnixDatagram::pair().unwrap();
        let (_ready_send, ready) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            drop(parent_socket);
            while !release.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(5));
            }
            completed.store(true, Ordering::Release);
            Ok(())
        });
        Self {
            child_socket: Some(child_socket),
            ready,
            shutdown: None,
            worker: Some(worker),
        }
    }
}

impl Drop for PendingApplicationSandbox {
    fn drop(&mut self) {
        drop(self.child_socket.take());
        let _ = stop_supervisor_without_blocking(&mut self.shutdown, &mut self.worker);
    }
}

impl ApplicationSandboxGuard {
    pub(crate) fn request_shutdown(&mut self) {
        if let Some(mut shutdown) = self.shutdown.take() {
            let _ = shutdown.write_all(&[0]);
        }
    }

    pub(crate) fn try_finish(&mut self) -> Result<bool, String> {
        self.request_shutdown();
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            return Ok(false);
        }
        let Some(worker) = self.worker.take() else {
            return Ok(true);
        };
        worker
            .join()
            .map_err(|_| "seccomp exec supervisor panicked".to_string())??;
        Ok(true)
    }

    #[cfg(test)]
    pub(crate) fn test_stalled_guard(
        release: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        use std::sync::atomic::Ordering;

        let worker = thread::spawn(move || {
            while !release.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(5));
            }
            Ok(())
        });
        Self {
            shutdown: None,
            worker: Some(worker),
        }
    }
}

impl Drop for ApplicationSandboxGuard {
    fn drop(&mut self) {
        let _ = stop_supervisor_without_blocking(&mut self.shutdown, &mut self.worker);
    }
}

pub(crate) fn no_fork_application_filter() -> Vec<libc::sock_filter> {
    let allowed = allowed_application_syscalls();
    let mut filter = Vec::with_capacity(9 + allowed.len() * 2);
    filter.push(statement(BPF_LOAD_WORD_ABSOLUTE, SECCOMP_DATA_ARCH_OFFSET));
    filter.push(jump(BPF_JUMP_EQUAL, AUDIT_ARCH_X86_64, 1, 0));
    filter.push(statement(BPF_RETURN, SECCOMP_RET_KILL_PROCESS));
    filter.push(statement(
        BPF_LOAD_WORD_ABSOLUTE,
        SECCOMP_DATA_NUMBER_OFFSET,
    ));
    for syscall in [libc::SYS_execve, libc::SYS_execveat] {
        filter.push(jump(BPF_JUMP_EQUAL, syscall as u32, 0, 1));
        filter.push(statement(BPF_RETURN, SECCOMP_RET_USER_NOTIF));
    }
    for syscall in allowed {
        filter.push(jump(BPF_JUMP_EQUAL, *syscall as u32, 0, 1));
        filter.push(statement(BPF_RETURN, SECCOMP_RET_ALLOW));
    }
    filter.push(statement(
        BPF_RETURN,
        SECCOMP_RET_ERRNO | libc::EPERM as u32,
    ));
    filter
}

/// Leaves Cargo unrestricted except for making every descendant exec observable by the parent.
pub(crate) fn cargo_exec_notification_filter() -> Vec<libc::sock_filter> {
    vec![
        statement(BPF_LOAD_WORD_ABSOLUTE, SECCOMP_DATA_ARCH_OFFSET),
        jump(BPF_JUMP_EQUAL, AUDIT_ARCH_X86_64, 1, 0),
        statement(BPF_RETURN, SECCOMP_RET_KILL_PROCESS),
        statement(BPF_LOAD_WORD_ABSOLUTE, SECCOMP_DATA_NUMBER_OFFSET),
        jump(BPF_JUMP_EQUAL, libc::SYS_execve as u32, 0, 1),
        statement(BPF_RETURN, SECCOMP_RET_USER_NOTIF),
        jump(BPF_JUMP_EQUAL, libc::SYS_execveat as u32, 0, 1),
        statement(BPF_RETURN, SECCOMP_RET_USER_NOTIF),
        statement(BPF_RETURN, SECCOMP_RET_ALLOW),
    ]
}

/// Installs a permanent, single-threaded profile and transfers its notification listener to the
/// already-running parent supervisor. The supervisor admits exactly the controlled initial exec;
/// both exec variants remain trapped after the application image begins.
pub(crate) fn install_application_profile(
    filter: &[libc::sock_filter],
    supervisor_socket: RawFd,
) -> io::Result<()> {
    let length =
        u16::try_from(filter.len()).map_err(|_| io::Error::from_raw_os_error(libc::E2BIG))?;
    // SAFETY: the documented scalar `PR_SET_NO_NEW_PRIVS` arguments affect only this pre-exec
    // child. Failure is propagated to `Command::spawn`, so launch remains fail closed.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let program = libc::sock_fprog {
        len: length,
        filter: filter.as_ptr().cast_mut(),
    };
    // SAFETY: `program` and its immutable filter storage remain live for the syscall. Seccomp
    // copies the filter into the kernel before returning.
    let listener = unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            SECCOMP_SET_MODE_FILTER,
            SECCOMP_FILTER_FLAG_NEW_LISTENER,
            &program,
        )
    };
    if listener < 0 {
        return Err(io::Error::last_os_error());
    }
    let listener = i32::try_from(listener).map_err(|_| io::Error::from_raw_os_error(libc::EIO))?;
    let result = send_listener(supervisor_socket, listener);
    // SAFETY: NEW_LISTENER returned this owned descriptor and SCM_RIGHTS duplicated it on success.
    unsafe { libc::close(listener) };
    result
}

fn supervise_exec_notifications(
    socket: UnixDatagram,
    shutdown: File,
    ready: mpsc::SyncSender<Result<u32, String>>,
) -> Result<(), String> {
    let (listener, child_pid) = match wait_for_listener(socket.as_raw_fd(), shutdown.as_raw_fd()) {
        Ok(Some(received)) => received,
        Ok(None) => {
            let error = "seccomp exec supervisor stopped before listener delivery".to_string();
            let _ = ready.send(Err(error.clone()));
            return Err(error);
        }
        Err(error) => {
            let _ = ready.send(Err(error.clone()));
            return Err(error);
        }
    };
    let initial = match wait_for_notification(listener.as_raw_fd(), shutdown.as_raw_fd()) {
        Ok(Some(notification)) => notification,
        Ok(None) => {
            let error = "seccomp exec supervisor stopped before initial exec".to_string();
            let _ = ready.send(Err(error.clone()));
            return Err(error);
        }
        Err(error) => {
            let _ = ready.send(Err(error.clone()));
            return Err(error);
        }
    };
    if initial.pid != child_pid
        || initial.flags != 0
        || initial.data.arch != AUDIT_ARCH_X86_64
        || initial.data.nr != libc::SYS_execve as i32
    {
        let _ = respond_to_notification(listener.as_raw_fd(), initial.id, false);
        let error = format!(
            "seccomp initial exec notification was not the controlled child execve: pid={} nr={}",
            initial.pid, initial.data.nr
        );
        let _ = ready.send(Err(error.clone()));
        return Err(error);
    }
    respond_to_notification(listener.as_raw_fd(), initial.id, true)?;
    ready
        .send(Ok(child_pid))
        .map_err(|_| "seccomp launch owner disappeared".to_string())?;

    while let Some(notification) =
        wait_for_notification(listener.as_raw_fd(), shutdown.as_raw_fd())?
    {
        if notification.pid != child_pid
            || notification.flags != 0
            || notification.data.arch != AUDIT_ARCH_X86_64
            || !matches!(
                notification.data.nr as libc::c_long,
                libc::SYS_execve | libc::SYS_execveat
            )
        {
            return Err("seccomp exec supervisor received an invalid notification".to_string());
        }
        respond_to_notification(listener.as_raw_fd(), notification.id, false)?;
    }
    Ok(())
}

pub(crate) fn wait_for_listener(
    socket: RawFd,
    shutdown: RawFd,
) -> Result<Option<(File, u32)>, String> {
    loop {
        let mut descriptors = [
            libc::pollfd {
                fd: shutdown,
                events: libc::POLLIN | libc::POLLHUP,
                revents: 0,
            },
            libc::pollfd {
                fd: socket,
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
        ];
        // SAFETY: both descriptors remain owned by this supervisor for the poll duration.
        let result = unsafe { libc::poll(descriptors.as_mut_ptr(), descriptors.len() as _, -1) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!(
                "failed to wait for seccomp listener delivery: {error}"
            ));
        }
        if descriptors[0].revents != 0 {
            return Ok(None);
        }
        if descriptors[1].revents & libc::POLLIN != 0 {
            return receive_listener(socket).map(Some);
        }
        if descriptors[1].revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            return Err("seccomp listener channel failed before descriptor delivery".to_string());
        }
    }
}

pub(crate) fn wait_for_notification(
    listener: RawFd,
    shutdown: RawFd,
) -> Result<Option<libc::seccomp_notif>, String> {
    loop {
        let mut descriptors = [
            libc::pollfd {
                fd: shutdown,
                events: libc::POLLIN | libc::POLLHUP,
                revents: 0,
            },
            libc::pollfd {
                fd: listener,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: both poll descriptors are valid for the duration of this call.
        let result = unsafe { libc::poll(descriptors.as_mut_ptr(), descriptors.len() as _, -1) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("failed to wait for seccomp notification: {error}"));
        }
        if descriptors[0].revents != 0 {
            return Ok(None);
        }
        if descriptors[1].revents & libc::POLLIN == 0 {
            if descriptors[1].revents & libc::POLLHUP != 0 {
                return Ok(None);
            }
            return Err("seccomp notification listener failed while polling".to_string());
        }
        let mut notification = MaybeUninit::<libc::seccomp_notif>::zeroed();
        // SAFETY: the kernel initializes the complete notification structure for this ioctl.
        if unsafe {
            libc::ioctl(
                listener,
                libc::_IOWR::<libc::seccomp_notif>(b'!' as u32, 0),
                notification.as_mut_ptr(),
            )
        } == 0
        {
            // SAFETY: successful SECCOMP_IOCTL_NOTIF_RECV initialized the value.
            return Ok(Some(unsafe { notification.assume_init() }));
        }
        let error = io::Error::last_os_error();
        if matches!(
            error.kind(),
            io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
        ) {
            continue;
        }
        return Err(format!("failed to receive seccomp notification: {error}"));
    }
}

pub(crate) fn respond_to_notification(
    listener: RawFd,
    id: u64,
    permit_initial: bool,
) -> Result<(), String> {
    let response = libc::seccomp_notif_resp {
        id,
        val: 0,
        error: if permit_initial { 0 } else { -libc::EPERM },
        flags: if permit_initial {
            SECCOMP_USER_NOTIF_FLAG_CONTINUE
        } else {
            0
        },
    };
    // SAFETY: the response has the exact Linux UAPI layout and remains live for the ioctl.
    if unsafe {
        libc::ioctl(
            listener,
            libc::_IOWR::<libc::seccomp_notif_resp>(b'!' as u32, 1),
            &response,
        )
    } != 0
    {
        return Err(format!(
            "failed to answer seccomp notification: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(())
}

pub(crate) fn notification_is_valid(listener: RawFd, id: u64) -> Result<(), String> {
    // SAFETY: the ioctl only reads the live notification ID from this stack value.
    if unsafe { libc::ioctl(listener, libc::_IOW::<u64>(b'!' as u32, 2), &id) } != 0 {
        return Err(format!(
            "seccomp notification expired while authorizing its process: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn send_listener(socket: RawFd, listener: RawFd) -> io::Result<()> {
    let message = ListenerMessage {
        magic: LISTENER_MESSAGE_MAGIC,
        // SAFETY: this is called in the post-fork child and returns that child's PID.
        pid: unsafe { libc::getpid() } as u32,
        reserved: 0,
    };
    let mut io_vector = libc::iovec {
        iov_base: ptr::from_ref(&message).cast_mut().cast(),
        iov_len: mem::size_of::<ListenerMessage>(),
    };
    let mut control = [0_usize; 8];
    let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
    header.msg_iov = &mut io_vector;
    header.msg_iovlen = 1;
    header.msg_control = control.as_mut_ptr().cast();
    header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
    // SAFETY: the aligned control storage is large enough for one SCM_RIGHTS descriptor.
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&header);
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize;
        ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>(), listener);
    }
    // SAFETY: all message pointers reference live storage and the socket is owned by the child.
    let sent = unsafe { libc::sendmsg(socket, &header, libc::MSG_NOSIGNAL) };
    if sent == mem::size_of::<ListenerMessage>() as isize {
        Ok(())
    } else if sent < 0 {
        Err(io::Error::last_os_error())
    } else {
        Err(io::Error::from_raw_os_error(libc::EIO))
    }
}

fn receive_listener(socket: RawFd) -> Result<(File, u32), String> {
    let mut message = MaybeUninit::<ListenerMessage>::zeroed();
    let mut io_vector = libc::iovec {
        iov_base: message.as_mut_ptr().cast(),
        iov_len: mem::size_of::<ListenerMessage>(),
    };
    let mut control = [0_usize; 8];
    let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
    header.msg_iov = &mut io_vector;
    header.msg_iovlen = 1;
    header.msg_control = control.as_mut_ptr().cast();
    header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
    // SAFETY: all receive buffers are writable for their declared lengths.
    let received = unsafe {
        libc::recvmsg(
            socket,
            &mut header,
            libc::MSG_CMSG_CLOEXEC | libc::MSG_DONTWAIT,
        )
    };
    if received < 0 {
        return Err(format!(
            "failed to receive seccomp listener: {}",
            io::Error::last_os_error()
        ));
    }
    if received as usize != mem::size_of::<ListenerMessage>()
        || header.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
    {
        return Err("seccomp listener transfer was truncated".to_string());
    }
    // SAFETY: recvmsg initialized the exact payload after the length check.
    let message = unsafe { message.assume_init() };
    if message.magic != LISTENER_MESSAGE_MAGIC || message.pid == 0 || message.reserved != 0 {
        return Err("seccomp listener transfer header is invalid".to_string());
    }
    // SAFETY: recvmsg initialized the aligned ancillary buffer described by `header`.
    let descriptor = unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&header);
        if cmsg.is_null()
            || (*cmsg).cmsg_level != libc::SOL_SOCKET
            || (*cmsg).cmsg_type != libc::SCM_RIGHTS
            || (*cmsg).cmsg_len != libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize
            || !libc::CMSG_NXTHDR(&header, cmsg).is_null()
        {
            return Err("seccomp listener transfer descriptor is invalid".to_string());
        }
        ptr::read_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>())
    };
    if descriptor < 0 {
        return Err("seccomp listener transfer returned an invalid descriptor".to_string());
    }
    // SAFETY: SCM_RIGHTS transferred one owned descriptor with MSG_CMSG_CLOEXEC.
    Ok((unsafe { File::from_raw_fd(descriptor) }, message.pid))
}

pub(crate) fn cloexec_pipe() -> io::Result<(File, File)> {
    let mut descriptors = [-1_i32; 2];
    // SAFETY: successful pipe2 initializes both output descriptors.
    if unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: pipe2 returned two independently owned descriptors.
    Ok(unsafe {
        (
            File::from_raw_fd(descriptors[0]),
            File::from_raw_fd(descriptors[1]),
        )
    })
}

pub(crate) fn stop_supervisor_without_blocking(
    shutdown: &mut Option<File>,
    worker: &mut Option<JoinHandle<Result<(), String>>>,
) -> Result<(), String> {
    if let Some(mut shutdown) = shutdown.take() {
        let _ = shutdown.write_all(&[0]);
    }
    if worker.as_ref().is_some_and(|worker| !worker.is_finished()) {
        // Dropping a JoinHandle detaches only this already-signalled worker. The dedicated
        // application supervisor process remains the fail-stop lifetime boundary.
        drop(worker.take());
        return Ok(());
    }
    let Some(worker) = worker.take() else {
        return Ok(());
    };
    worker
        .join()
        .map_err(|_| "seccomp exec supervisor panicked".to_string())?
}

const fn statement(code: u16, value: u32) -> libc::sock_filter {
    libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k: value,
    }
}

const fn jump(code: u16, value: u32, on_equal: u8, on_other: u8) -> libc::sock_filter {
    libc::sock_filter {
        code,
        jt: on_equal,
        jf: on_other,
        k: value,
    }
}

fn allowed_application_syscalls() -> &'static [libc::c_long] {
    &[
        libc::SYS_read,
        libc::SYS_write,
        libc::SYS_readv,
        libc::SYS_writev,
        libc::SYS_sendmsg,
        libc::SYS_pread64,
        libc::SYS_pwrite64,
        libc::SYS_close,
        libc::SYS_lseek,
        libc::SYS_fstat,
        libc::SYS_newfstatat,
        libc::SYS_statx,
        libc::SYS_access,
        libc::SYS_faccessat,
        libc::SYS_faccessat2,
        libc::SYS_openat,
        libc::SYS_openat2,
        libc::SYS_getdents64,
        libc::SYS_getcwd,
        libc::SYS_chdir,
        libc::SYS_fchdir,
        libc::SYS_readlink,
        libc::SYS_readlinkat,
        libc::SYS_unlink,
        libc::SYS_unlinkat,
        libc::SYS_rename,
        libc::SYS_renameat,
        libc::SYS_renameat2,
        libc::SYS_mkdir,
        libc::SYS_mkdirat,
        libc::SYS_rmdir,
        libc::SYS_link,
        libc::SYS_linkat,
        libc::SYS_fchmod,
        libc::SYS_fchmodat,
        libc::SYS_umask,
        libc::SYS_fcntl,
        libc::SYS_flock,
        libc::SYS_fsync,
        libc::SYS_fdatasync,
        libc::SYS_dup,
        libc::SYS_dup2,
        libc::SYS_dup3,
        libc::SYS_ioctl,
        libc::SYS_poll,
        libc::SYS_ppoll,
        libc::SYS_select,
        libc::SYS_pselect6,
        libc::SYS_epoll_create1,
        libc::SYS_epoll_ctl,
        libc::SYS_epoll_wait,
        libc::SYS_epoll_pwait,
        libc::SYS_mmap,
        libc::SYS_mprotect,
        libc::SYS_munmap,
        libc::SYS_mremap,
        libc::SYS_madvise,
        libc::SYS_brk,
        libc::SYS_rt_sigaction,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigreturn,
        libc::SYS_sigaltstack,
        libc::SYS_futex,
        libc::SYS_sched_yield,
        libc::SYS_sched_getaffinity,
        libc::SYS_nanosleep,
        libc::SYS_clock_gettime,
        libc::SYS_clock_nanosleep,
        libc::SYS_gettimeofday,
        libc::SYS_getrandom,
        libc::SYS_getpid,
        libc::SYS_getppid,
        libc::SYS_gettid,
        libc::SYS_getuid,
        libc::SYS_geteuid,
        libc::SYS_getgid,
        libc::SYS_getegid,
        libc::SYS_getrusage,
        libc::SYS_wait4,
        libc::SYS_uname,
        libc::SYS_sysinfo,
        libc::SYS_arch_prctl,
        libc::SYS_set_tid_address,
        libc::SYS_set_robust_list,
        libc::SYS_rseq,
        libc::SYS_prlimit64,
        libc::SYS_exit,
        libc::SYS_exit_group,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    #[test]
    fn process_creation_and_exec_replacement_are_not_allowlisted() {
        let allowed = allowed_application_syscalls();
        for forbidden in [
            libc::SYS_fork,
            libc::SYS_vfork,
            libc::SYS_clone,
            libc::SYS_clone3,
            libc::SYS_unshare,
            libc::SYS_setns,
            libc::SYS_setsid,
            libc::SYS_io_uring_setup,
            libc::SYS_io_uring_enter,
            libc::SYS_io_uring_register,
            libc::SYS_execve,
            libc::SYS_execveat,
        ] {
            assert!(!allowed.contains(&forbidden), "syscall {forbidden} escaped");
        }
    }

    #[test]
    fn filter_kills_foreign_architectures_and_denies_unknown_syscalls() {
        let filter = no_fork_application_filter();
        assert_eq!(filter[0].k, SECCOMP_DATA_ARCH_OFFSET);
        assert_eq!(filter[1].k, AUDIT_ARCH_X86_64);
        assert_eq!(filter[2].k, SECCOMP_RET_KILL_PROCESS);
        assert!(filter.iter().any(|instruction| {
            instruction.code == BPF_RETURN && instruction.k == SECCOMP_RET_USER_NOTIF
        }));
        assert_eq!(
            filter.last().unwrap().k,
            SECCOMP_RET_ERRNO | libc::EPERM as u32
        );
    }

    #[test]
    fn shutdown_before_listener_delivery_terminates_and_joins() {
        let (parent_socket, child_socket) = UnixDatagram::pair().unwrap();
        parent_socket.set_nonblocking(true).unwrap();
        let (shutdown_read, mut shutdown_write) = cloexec_pipe().unwrap();
        let (ready_send, ready) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            supervise_exec_notifications(parent_socket, shutdown_read, ready_send)
        });
        drop(child_socket);

        let started = Instant::now();
        shutdown_write.write_all(&[0]).unwrap();
        let failure = ready
            .recv_timeout(Duration::from_secs(1))
            .expect("supervisor reports pre-listener cancellation");
        assert!(failure.unwrap_err().contains("before listener delivery"));
        assert!(worker.join().unwrap().is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn dropping_pending_launch_without_listener_is_bounded() {
        let pending = PendingApplicationSandbox::start().unwrap();
        let started = Instant::now();
        drop(pending);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn dropping_stalled_pending_admission_never_joins() {
        let release = Arc::new(AtomicBool::new(false));
        let completed = Arc::new(AtomicBool::new(false));
        let pending = PendingApplicationSandbox::test_stalled_pending(
            Arc::clone(&release),
            Arc::clone(&completed),
        );

        let started = Instant::now();
        drop(pending);
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "stalled pending sandbox drop blocked for {:?}",
            started.elapsed()
        );
        assert!(!completed.load(Ordering::Acquire));

        release.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !completed.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(completed.load(Ordering::Acquire));
    }
}
