//! Independently observes which Cargo exec events may acquire brokered build capabilities.
//!
//! On Linux/x86-64, a seccomp user-notification filter is installed before the pinned Cargo
//! image starts. The parent observes every descendant `execve`/`execveat` while the caller is
//! stopped in the kernel. A one-use broker permit is issued only when the pinned wrapper is the
//! requested image, the caller still runs the pinned Cargo image, and the caller is a fresh direct
//! child of the supervised Cargo process. Every later exec notification for that PID revokes the
//! permit before it can continue, while the broker separately authenticates the live wrapper image.
//! Seccomp `CONTINUE` is not an atomic pathname pin: a pathname race may execute another image, but
//! that image cannot consume the permit or retain it across a later exec into the genuine wrapper.
//! Consequently a build script cannot gain a permit by replacing itself with, or spawning, the
//! genuine wrapper; the same applies to a procedural macro descendant. Procedural macro code
//! remains trusted inside its already-authorized rustc process, where compiler descriptors are
//! necessarily visible.
//!
//! The boundary assumes the kernel, procfs, seccomp, and the supervising `cargo-fe2o3` process are
//! trusted. Same-uid ptrace/process injection or mutation of a stopped child's memory is outside
//! this process boundary and requires OS-level isolation of untrusted project code.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod platform {
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::fs::{self, File};
    use std::io::{self, Write};
    use std::os::fd::{AsRawFd, FromRawFd as _};
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::net::UnixDatagram;
    use std::os::unix::process::CommandExt as _;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::mpsc::{self, Receiver};
    use std::sync::{Arc, Mutex};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use fe2o3_process_identity::LinuxObjectIdentityV3;

    use crate::application_sandbox::{
        AUDIT_ARCH_X86_64, cargo_exec_notification_filter, cloexec_pipe,
        install_application_profile, respond_to_notification, stop_supervisor_without_blocking,
        wait_for_listener, wait_for_notification,
    };
    use crate::pinned_executable::PinnedExecutable;

    const SUPERVISOR_READY_TIMEOUT: Duration = Duration::from_secs(30);
    const INVOCATION_PERMIT_LIFETIME: Duration = Duration::from_secs(120);
    const MAX_EXEC_PATH_BYTES: usize = 4096;
    const MAX_PROC_STAT_BYTES: usize = 4096;
    const MAX_PENDING_PERMITS: usize = 256;

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub(crate) struct ProcessIdentityV1 {
        pid: u32,
        start_time_ticks: u64,
    }

    impl ProcessIdentityV1 {
        pub(crate) fn observe(pid: u32) -> Result<Self, String> {
            let observation = process_observation(pid)?;
            Ok(Self {
                pid,
                start_time_ticks: observation.start_time_ticks,
            })
        }

        pub(crate) fn require_current(self) -> Result<(), String> {
            if Self::observe(self.pid)? != self {
                return Err("authorized wrapper process identity changed".to_owned());
            }
            Ok(())
        }
    }

    #[derive(Clone)]
    pub(crate) struct InvocationAuthorizationRegistryV1 {
        state: Arc<Mutex<BTreeMap<ProcessIdentityV1, InvocationPermitV1>>>,
    }

    struct InvocationPermitV1 {
        expires_at: Instant,
        process: Option<File>,
    }

    impl InvocationAuthorizationRegistryV1 {
        pub(crate) fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(BTreeMap::new())),
            }
        }

        fn authorize(&self, process: ProcessIdentityV1) -> Result<(), String> {
            let process_fd = open_process_pidfd(process.pid)?;
            self.authorize_with_process_fd(process, Some(process_fd))
        }

        fn authorize_with_process_fd(
            &self,
            process: ProcessIdentityV1,
            process_fd: Option<File>,
        ) -> Result<(), String> {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let now = Instant::now();
            state.retain(|_, permit| permit.expires_at > now);
            if state.len() >= MAX_PENDING_PERMITS {
                return Err("Cargo invocation authorization registry is full".to_owned());
            }
            if state
                .insert(
                    process,
                    InvocationPermitV1 {
                        expires_at: now + INVOCATION_PERMIT_LIFETIME,
                        process: process_fd,
                    },
                )
                .is_some()
            {
                return Err("Cargo invocation already has a pending authorization".to_owned());
            }
            Ok(())
        }

        pub(crate) fn consume(&self, process: ProcessIdentityV1) -> Result<(), String> {
            let permit = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&process)
                .ok_or_else(|| {
                    "wrapper invocation was not independently authorized by the Cargo exec boundary"
                        .to_owned()
                })?;
            if permit.expires_at <= Instant::now() {
                return Err("Cargo wrapper invocation authorization expired".to_owned());
            }
            if permit
                .process
                .as_ref()
                .is_some_and(|process| !pidfd_is_live(process))
            {
                return Err("authorized Cargo wrapper process is no longer live".to_owned());
            }
            Ok(())
        }

        fn revoke_pid(&self, pid: u32) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .retain(|process, _| process.pid != pid);
        }

        fn clear(&self) {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
        }

        #[cfg(test)]
        pub(crate) fn authorize_test_process(
            &self,
            process: ProcessIdentityV1,
        ) -> Result<(), String> {
            self.authorize_with_process_fd(process, None)
        }
    }

    #[derive(Clone, Copy)]
    struct ExecutableIdentityV1 {
        object: LinuxObjectIdentityV3,
        sha256: [u8; 32],
    }

    impl ExecutableIdentityV1 {
        fn from_pinned(executable: &PinnedExecutable) -> Self {
            Self {
                object: executable.object_identity(),
                sha256: *executable.sha256(),
            }
        }

        fn matches_pinned(self, executable: &PinnedExecutable) -> bool {
            self.object == executable.object_identity() && self.sha256 == *executable.sha256()
        }
    }

    pub(crate) struct PendingCargoInvocationBoundary {
        child_socket: Option<UnixDatagram>,
        ready: Receiver<Result<u32, String>>,
        shutdown: Option<File>,
        worker: Option<JoinHandle<Result<(), String>>>,
    }

    pub(crate) struct CargoInvocationBoundaryGuard {
        authorization: InvocationAuthorizationRegistryV1,
        shutdown: Option<File>,
        worker: Option<JoinHandle<Result<(), String>>>,
    }

    impl PendingCargoInvocationBoundary {
        pub(crate) fn start(
            cargo: &PinnedExecutable,
            wrapper: &PinnedExecutable,
            authorization: InvocationAuthorizationRegistryV1,
        ) -> Result<Self, String> {
            let cargo = ExecutableIdentityV1::from_pinned(cargo);
            let wrapper = ExecutableIdentityV1::from_pinned(wrapper);
            let (parent_socket, child_socket) = UnixDatagram::pair().map_err(|error| {
                format!("failed to create Cargo exec-boundary listener channel: {error}")
            })?;
            parent_socket.set_nonblocking(true).map_err(|error| {
                format!("failed to configure Cargo exec-boundary listener channel: {error}")
            })?;
            let (shutdown_read, shutdown_write) = cloexec_pipe()
                .map_err(|error| format!("failed to create Cargo exec-boundary pipe: {error}"))?;
            let (ready_send, ready) = mpsc::sync_channel(1);
            let worker_authorization = authorization.clone();
            let worker = thread::Builder::new()
                .name("fe2o3-cargo-exec-boundary".into())
                .spawn(move || {
                    supervise(
                        parent_socket,
                        shutdown_read,
                        ready_send,
                        cargo,
                        wrapper,
                        worker_authorization,
                    )
                })
                .map_err(|error| {
                    format!("failed to start Cargo exec-boundary supervisor: {error}")
                })?;
            Ok(Self {
                child_socket: Some(child_socket),
                ready,
                shutdown: Some(shutdown_write),
                worker: Some(worker),
            })
        }

        pub(crate) fn configure_child(&self, command: &mut Command) {
            let filter = cargo_exec_notification_filter();
            let socket = self
                .child_socket
                .as_ref()
                .expect("pending Cargo boundary owns its child socket")
                .as_raw_fd();
            // SAFETY: the callback affects only the post-fork Cargo child. Its borrowed socket and
            // owned filter outlive the synchronous spawn that executes this callback.
            unsafe {
                command.pre_exec(move || install_application_profile(&filter, socket));
            }
        }

        pub(crate) fn complete(
            mut self,
            child_id: u32,
            authorization: InvocationAuthorizationRegistryV1,
        ) -> Result<CargoInvocationBoundaryGuard, String> {
            drop(self.child_socket.take());
            let supervised_id =
                self.ready
                    .recv_timeout(SUPERVISOR_READY_TIMEOUT)
                    .map_err(|error| {
                        format!("Cargo exec boundary did not admit initial exec: {error}")
                    })??;
            if supervised_id != child_id {
                return Err(format!(
                    "Cargo exec boundary admitted PID {supervised_id}, expected {child_id}"
                ));
            }
            Ok(CargoInvocationBoundaryGuard {
                authorization,
                shutdown: self.shutdown.take(),
                worker: self.worker.take(),
            })
        }
    }

    impl Drop for PendingCargoInvocationBoundary {
        fn drop(&mut self) {
            drop(self.child_socket.take());
            let _ = stop_supervisor_without_blocking(&mut self.shutdown, &mut self.worker);
        }
    }

    impl CargoInvocationBoundaryGuard {
        pub(crate) fn finish(mut self) -> Result<(), String> {
            if let Some(mut shutdown) = self.shutdown.take() {
                let _ = shutdown.write_all(&[0]);
            }
            let result = match self.worker.take() {
                Some(worker) => worker
                    .join()
                    .map_err(|_| "Cargo exec-boundary supervisor panicked".to_owned())?,
                None => Ok(()),
            };
            self.authorization.clear();
            result
        }
    }

    impl Drop for CargoInvocationBoundaryGuard {
        fn drop(&mut self) {
            self.authorization.clear();
            let _ = stop_supervisor_without_blocking(&mut self.shutdown, &mut self.worker);
        }
    }

    fn supervise(
        socket: UnixDatagram,
        shutdown: File,
        ready: mpsc::SyncSender<Result<u32, String>>,
        cargo: ExecutableIdentityV1,
        wrapper: ExecutableIdentityV1,
        authorization: InvocationAuthorizationRegistryV1,
    ) -> Result<(), String> {
        let (listener, cargo_pid) =
            match wait_for_listener(socket.as_raw_fd(), shutdown.as_raw_fd()) {
                Ok(Some(received)) => received,
                Ok(None) => {
                    return report_ready_error(
                        &ready,
                        "Cargo exec boundary stopped before listener delivery",
                    );
                }
                Err(error) => return report_ready_error(&ready, &error),
            };
        let initial = match wait_for_notification(listener.as_raw_fd(), shutdown.as_raw_fd()) {
            Ok(Some(notification)) => notification,
            Ok(None) => {
                return report_ready_error(
                    &ready,
                    "Cargo exec boundary stopped before initial exec",
                );
            }
            Err(error) => return report_ready_error(&ready, &error),
        };
        let initial_result = validate_notification(&initial)
            .and_then(|()| {
                requested_executable_matches(initial, cargo).map(|value| (initial, value))
            })
            .and_then(|(initial, executable_matches)| {
                if initial.pid != cargo_pid || !executable_matches {
                    Err(
                        "Cargo exec boundary initial image is not the pinned Cargo executable"
                            .to_owned(),
                    )
                } else {
                    Ok(initial)
                }
            });
        let initial = match initial_result {
            Ok(initial) => initial,
            Err(error) => {
                let _ = respond_to_notification(listener.as_raw_fd(), initial.id, false);
                return report_ready_error(&ready, &error);
            }
        };
        respond_to_notification(listener.as_raw_fd(), initial.id, true)?;
        ready
            .send(Ok(cargo_pid))
            .map_err(|_| "Cargo exec-boundary owner disappeared".to_owned())?;
        let cargo_process = ProcessIdentityV1::observe(cargo_pid)?;

        while let Some(notification) =
            wait_for_notification(listener.as_raw_fd(), shutdown.as_raw_fd())?
        {
            let outcome = authorize_notification(
                notification,
                cargo_pid,
                cargo_process,
                cargo,
                wrapper,
                &authorization,
            );
            match outcome {
                Ok(()) => respond_to_notification(listener.as_raw_fd(), notification.id, true)?,
                Err(error) => {
                    let _ = respond_to_notification(listener.as_raw_fd(), notification.id, false);
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn authorize_notification(
        notification: libc::seccomp_notif,
        cargo_pid: u32,
        cargo_process: ProcessIdentityV1,
        cargo: ExecutableIdentityV1,
        wrapper: ExecutableIdentityV1,
        authorization: &InvocationAuthorizationRegistryV1,
    ) -> Result<(), String> {
        validate_notification(&notification)?;
        // A permit belongs to exactly one successful transition into the wrapper image. Any
        // later exec by the same PID invalidates it before that new image can run, including a
        // pathname-swap adversary that first entered another image and then execs the wrapper.
        authorization.revoke_pid(notification.pid);
        if !requested_executable_matches(notification, wrapper)? {
            return Ok(());
        }

        let observation = process_observation(notification.pid)?;
        let process = ProcessIdentityV1 {
            pid: notification.pid,
            start_time_ticks: observation.start_time_ticks,
        };
        let current = process_executable_object(notification.pid)?;
        if !is_direct_pinned_cargo_child(
            observation.parent_pid,
            cargo_pid,
            ProcessIdentityV1::observe(cargo_pid)? == cargo_process,
            current == cargo.object,
        ) {
            // The exec itself is allowed so the genuine wrapper can report a broker rejection, but
            // no authorization is minted for build-script/proc-macro descendants or replacements.
            return Ok(());
        }
        authorization.authorize(process)
    }

    fn open_process_pidfd(pid: u32) -> Result<File, String> {
        // SAFETY: pidfd_open takes a scalar PID and zero flags and returns a new owned descriptor.
        let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        if descriptor < 0 {
            return Err(format!(
                "cannot pin authorized wrapper PID {pid}: {}",
                io::Error::last_os_error()
            ));
        }
        // SAFETY: a nonnegative pidfd_open result is a newly owned descriptor.
        Ok(unsafe { File::from_raw_fd(descriptor as i32) })
    }

    fn pidfd_is_live(process: &File) -> bool {
        // SAFETY: pidfd_send_signal with signal zero performs existence/permission checking and
        // does not deliver a signal. The siginfo pointer is unused for signal zero.
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                process.as_raw_fd(),
                0,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            ) == 0
        }
    }

    const fn is_direct_pinned_cargo_child(
        parent_pid: u32,
        cargo_pid: u32,
        cargo_process_is_current: bool,
        caller_runs_pinned_cargo: bool,
    ) -> bool {
        parent_pid == cargo_pid && cargo_process_is_current && caller_runs_pinned_cargo
    }

    fn validate_notification(notification: &libc::seccomp_notif) -> Result<(), String> {
        if notification.pid == 0
            || notification.flags != 0
            || notification.data.arch != AUDIT_ARCH_X86_64
            || !matches!(
                notification.data.nr as libc::c_long,
                libc::SYS_execve | libc::SYS_execveat
            )
        {
            return Err("Cargo exec boundary received an invalid notification".to_owned());
        }
        Ok(())
    }

    fn requested_executable_matches(
        notification: libc::seccomp_notif,
        expected: ExecutableIdentityV1,
    ) -> Result<bool, String> {
        let pid = notification.pid;
        let path_address = if notification.data.nr == libc::SYS_execve as i32 {
            notification.data.args[0]
        } else {
            notification.data.args[1]
        };
        let bytes = read_process_cstring(pid, path_address)?;
        let path = if notification.data.nr == libc::SYS_execveat as i32 {
            resolve_execveat_path(
                pid,
                notification.data.args[0] as i32,
                &bytes,
                notification.data.args[4],
            )?
        } else {
            resolve_exec_path(pid, &bytes)?
        };
        let file = match File::open(&path) {
            Ok(file) => file,
            // PATH search legitimately probes nonexistent candidates. Such an exec cannot be
            // identified as the pinned wrapper and therefore receives no broker permit.
            Err(_) => return Ok(false),
        };
        let metadata = match file.metadata() {
            Ok(metadata) => metadata,
            Err(_) => return Ok(false),
        };
        let object =
            LinuxObjectIdentityV3::from_linux_stat(metadata.dev(), metadata.ino(), metadata.mode());
        if object != expected.object {
            return Ok(false);
        }
        Ok(PinnedExecutable::from_transferred_file(file, path)
            .is_ok_and(|executable| expected.matches_pinned(&executable)))
    }

    fn resolve_exec_path(pid: u32, bytes: &[u8]) -> Result<PathBuf, String> {
        if bytes.is_empty() {
            return Err("execve requested an empty pathname".to_owned());
        }
        let path = PathBuf::from(OsString::from_vec(bytes.to_vec()));
        if path.is_absolute() {
            resolve_process_local_proc_path(pid, &path)
        } else {
            Ok(PathBuf::from(format!("/proc/{pid}/cwd")).join(path))
        }
    }

    fn resolve_execveat_path(
        pid: u32,
        directory_fd: i32,
        bytes: &[u8],
        flags: u64,
    ) -> Result<PathBuf, String> {
        const AT_EMPTY_PATH_U64: u64 = libc::AT_EMPTY_PATH as u64;
        if flags & !((libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW) as u64) != 0 {
            return Err("execveat requested unsupported flags".to_owned());
        }
        if bytes.is_empty() {
            if flags & AT_EMPTY_PATH_U64 == 0 || directory_fd < 0 {
                return Err("execveat requested an invalid empty pathname".to_owned());
            }
            return Ok(PathBuf::from(format!("/proc/{pid}/fd/{directory_fd}")));
        }
        let path = PathBuf::from(OsString::from_vec(bytes.to_vec()));
        if path.is_absolute() {
            resolve_process_local_proc_path(pid, &path)
        } else if directory_fd == libc::AT_FDCWD {
            Ok(PathBuf::from(format!("/proc/{pid}/cwd")).join(path))
        } else if directory_fd >= 0 {
            Ok(PathBuf::from(format!("/proc/{pid}/fd/{directory_fd}")).join(path))
        } else {
            Err("execveat requested an invalid directory descriptor".to_owned())
        }
    }

    fn resolve_process_local_proc_path(pid: u32, path: &Path) -> Result<PathBuf, String> {
        let text = path.to_str().ok_or_else(|| {
            "requested exec path is not valid UTF-8 under /proc/self/fd".to_owned()
        })?;
        if text == "/proc/self/exe" || text == "/proc/thread-self/exe" {
            return Ok(PathBuf::from(format!("/proc/{pid}/exe")));
        }
        match text.strip_prefix("/proc/self/fd/") {
            Some(descriptor)
                if !descriptor.is_empty()
                    && descriptor.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                Ok(PathBuf::from(format!("/proc/{pid}/fd/{descriptor}")))
            }
            _ => Ok(path.to_path_buf()),
        }
    }

    fn read_process_cstring(pid: u32, address: u64) -> Result<Vec<u8>, String> {
        if address == 0 {
            return Err("requested exec pathname pointer is null".to_owned());
        }
        let mut output = Vec::with_capacity(256);
        while output.len() < MAX_EXEC_PATH_BYTES {
            let mut chunk = [0_u8; 256];
            let remaining = MAX_EXEC_PATH_BYTES - output.len();
            let length = remaining.min(chunk.len());
            let local = libc::iovec {
                iov_base: chunk.as_mut_ptr().cast(),
                iov_len: length,
            };
            let remote = libc::iovec {
                iov_base: (address as usize + output.len()) as *mut libc::c_void,
                iov_len: length,
            };
            // SAFETY: the local iovec is writable; the remote address is read by the kernel from
            // the stopped tracee and any invalid range is returned as an error/short read.
            let read = unsafe { libc::process_vm_readv(pid as _, &local, 1, &remote, 1, 0) };
            if read <= 0 {
                return Err(format!(
                    "cannot read requested exec pathname from PID {pid}: {}",
                    if read == 0 {
                        "empty process read".to_owned()
                    } else {
                        io::Error::last_os_error().to_string()
                    }
                ));
            }
            let read = read as usize;
            if let Some(end) = chunk[..read].iter().position(|byte| *byte == 0) {
                output.extend_from_slice(&chunk[..end]);
                return Ok(output);
            }
            output.extend_from_slice(&chunk[..read]);
        }
        Err(format!(
            "requested exec pathname exceeds {MAX_EXEC_PATH_BYTES} bytes"
        ))
    }

    #[derive(Clone, Copy)]
    struct ProcessObservation {
        parent_pid: u32,
        start_time_ticks: u64,
    }

    fn process_observation(pid: u32) -> Result<ProcessObservation, String> {
        let path = PathBuf::from(format!("/proc/{pid}/stat"));
        let bytes = fs::read(&path)
            .map_err(|error| format!("cannot read process {}: {error}", path.display()))?;
        if bytes.is_empty() || bytes.len() > MAX_PROC_STAT_BYTES {
            return Err(format!(
                "process {} has invalid stat length",
                path.display()
            ));
        }
        let close = bytes
            .iter()
            .rposition(|byte| *byte == b')')
            .ok_or_else(|| "process stat has no command terminator".to_owned())?;
        let fields = bytes[close + 1..]
            .split(u8::is_ascii_whitespace)
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        let parse = |index: usize, name: &str| -> Result<u64, String> {
            fields
                .get(index)
                .and_then(|value| std::str::from_utf8(value).ok())
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(|| format!("process stat has no valid {name} field"))
        };
        let parent_pid = u32::try_from(parse(1, "parent PID")?)
            .map_err(|_| "process parent PID exceeds u32".to_owned())?;
        let start_time_ticks = parse(19, "start-time")?;
        if parent_pid == 0 || start_time_ticks == 0 {
            return Err("process identity fields must be nonzero".to_owned());
        }
        Ok(ProcessObservation {
            parent_pid,
            start_time_ticks,
        })
    }

    fn process_executable_object(pid: u32) -> Result<LinuxObjectIdentityV3, String> {
        let path = PathBuf::from(format!("/proc/{pid}/exe"));
        let metadata = fs::metadata(&path).map_err(|error| {
            format!(
                "cannot inspect process executable {}: {error}",
                path.display()
            )
        })?;
        Ok(LinuxObjectIdentityV3::from_linux_stat(
            metadata.dev(),
            metadata.ino(),
            metadata.mode(),
        ))
    }

    fn report_ready_error(
        ready: &mpsc::SyncSender<Result<u32, String>>,
        error: &str,
    ) -> Result<(), String> {
        let error = error.to_owned();
        let _ = ready.send(Err(error.clone()));
        Err(error)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn authorization_is_one_use_and_process_specific() {
            let registry = InvocationAuthorizationRegistryV1::new();
            let first = ProcessIdentityV1 {
                pid: 10,
                start_time_ticks: 20,
            };
            let other = ProcessIdentityV1 {
                pid: 10,
                start_time_ticks: 21,
            };
            registry.authorize_test_process(first).unwrap();
            assert!(registry.consume(other).is_err());
            registry.consume(first).unwrap();
            assert!(registry.consume(first).is_err());
        }

        #[test]
        fn any_later_exec_notification_revokes_the_old_process_permit() {
            let registry = InvocationAuthorizationRegistryV1::new();
            let process = ProcessIdentityV1 {
                pid: 10,
                start_time_ticks: 20,
            };
            registry.authorize_test_process(process).unwrap();
            registry.revoke_pid(process.pid);
            assert!(registry.consume(process).is_err());
        }

        #[test]
        fn pidfd_pins_the_current_kernel_process_lifetime() {
            let process = open_process_pidfd(std::process::id()).unwrap();
            assert!(pidfd_is_live(&process));
        }

        #[test]
        fn clone_parent_and_non_cargo_images_cannot_qualify_as_cargo_launches() {
            assert!(is_direct_pinned_cargo_child(41, 41, true, true));
            assert!(
                !is_direct_pinned_cargo_child(7, 41, true, true),
                "CLONE_PARENT cannot substitute a different parent"
            );
            assert!(
                !is_direct_pinned_cargo_child(41, 41, true, false),
                "a build-script image with Cargo as parent cannot qualify"
            );
            assert!(!is_direct_pinned_cargo_child(41, 41, false, true));
        }

        #[test]
        fn proc_self_and_execveat_empty_paths_are_resolved_in_the_tracee() {
            assert_eq!(
                resolve_process_local_proc_path(71, Path::new("/proc/self/exe")).unwrap(),
                PathBuf::from("/proc/71/exe")
            );
            assert_eq!(
                resolve_process_local_proc_path(71, Path::new("/proc/self/fd/9")).unwrap(),
                PathBuf::from("/proc/71/fd/9")
            );
            assert_eq!(
                resolve_execveat_path(71, 9, b"", libc::AT_EMPTY_PATH as u64).unwrap(),
                PathBuf::from("/proc/71/fd/9")
            );
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) use platform::*;

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
mod unsupported {
    use std::process::Command;

    use crate::pinned_executable::PinnedExecutable;

    #[derive(Clone, Copy)]
    pub(crate) struct ProcessIdentityV1;

    impl ProcessIdentityV1 {
        pub(crate) fn observe(_pid: u32) -> Result<Self, String> {
            Err("Cargo invocation authorization requires Linux/x86-64 seccomp".to_owned())
        }

        pub(crate) fn require_current(self) -> Result<(), String> {
            Err("Cargo invocation authorization requires Linux/x86-64 seccomp".to_owned())
        }
    }

    #[derive(Clone)]
    pub(crate) struct InvocationAuthorizationRegistryV1;

    impl InvocationAuthorizationRegistryV1 {
        pub(crate) fn new() -> Self {
            Self
        }

        pub(crate) fn consume(&self, _process: ProcessIdentityV1) -> Result<(), String> {
            Err("Cargo invocation authorization requires Linux/x86-64 seccomp".to_owned())
        }
    }

    pub(crate) struct PendingCargoInvocationBoundary;
    pub(crate) struct CargoInvocationBoundaryGuard;

    impl PendingCargoInvocationBoundary {
        pub(crate) fn start(
            _cargo: &PinnedExecutable,
            _wrapper: &PinnedExecutable,
            _authorization: InvocationAuthorizationRegistryV1,
        ) -> Result<Self, String> {
            Err("Cargo invocation authorization requires Linux/x86-64 seccomp".to_owned())
        }

        pub(crate) fn configure_child(&self, _command: &mut Command) {}

        pub(crate) fn complete(
            self,
            _child_id: u32,
            _authorization: InvocationAuthorizationRegistryV1,
        ) -> Result<CargoInvocationBoundaryGuard, String> {
            Err("Cargo invocation authorization requires Linux/x86-64 seccomp".to_owned())
        }
    }

    impl CargoInvocationBoundaryGuard {
        pub(crate) fn finish(self) -> Result<(), String> {
            Err("Cargo invocation authorization requires Linux/x86-64 seccomp".to_owned())
        }
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub(crate) use unsupported::*;
