//! Dedicated process boundary for descriptor-bearing Cargo applications.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, Instant};

pub(crate) const INTERNAL_SUPERVISOR_ARG: &str = "__fe2o3-application-supervisor-v1";

const SUPERVISOR_CAPACITY: usize = 32;
const SUPERVISOR_READY_TIMEOUT: Duration = Duration::from_secs(5);
const SUPERVISOR_EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const PROTOCOL_MAGIC: [u8; 8] = *b"f2supv01";
const PROTOCOL_READY: u8 = 1;
const PROTOCOL_STATUS: u8 = 2;
const PROTOCOL_ERROR: u8 = 3;
const PROTOCOL_HEADER_BYTES: usize = 8 + 32 + 1 + 1 + 4 + 4;
const MAX_PROTOCOL_MESSAGE_BYTES: usize = 64 * 1024;

pub(crate) fn run_frontend(args: &[std::ffi::OsString]) -> Result<ExitStatus, String> {
    let admission = SupervisorAdmission::acquire()?;
    let (mut frontend, supervisor_channel) = UnixStream::pair()
        .map_err(|error| format!("create application supervisor channel: {error}"))?;
    set_cloexec(frontend.as_raw_fd())?;
    set_cloexec(supervisor_channel.as_raw_fd())?;
    let challenge = random_challenge()?;
    let executable = env::current_exe()
        .map_err(|error| format!("locate application supervisor executable: {error}"))?;
    let channel_fd = supervisor_channel.as_raw_fd();
    let slot_fd = admission.file.as_raw_fd();
    let mut command = Command::new(executable);
    command
        .arg(INTERNAL_SUPERVISOR_ARG)
        .arg(channel_fd.to_string())
        .arg(slot_fd.to_string())
        .arg(hex_encode(&challenge))
        .args(args)
        .process_group(0);
    // SAFETY: the callback changes only two inherited descriptor flags via async-signal-safe
    // fcntl calls. Both owning values remain live through spawn.
    unsafe {
        command.pre_exec(move || {
            crate::application_exec::protect_all_nonstdio_descriptors()?;
            crate::application_exec::expose_descriptor(channel_fd)?;
            crate::application_exec::expose_descriptor(slot_fd)?;
            Ok(())
        });
    }
    let mut supervisor = crate::process_execution::spawn(&mut command)
        .map_err(|error| format!("start dedicated application supervisor: {error}"))?;
    drop(supervisor_channel);
    drop(admission);

    let ready = match read_frame(&mut frontend, Some(SUPERVISOR_READY_TIMEOUT)) {
        Ok(frame) => frame,
        Err(error) => {
            terminate_failed_start(&mut supervisor);
            return Err(format!(
                "application supervisor startup handshake failed: {error}"
            ));
        }
    };
    if ready.challenge != challenge || ready.kind != PROTOCOL_READY || ready.pending {
        terminate_failed_start(&mut supervisor);
        return Err("application supervisor startup handshake was not authentic".to_string());
    }

    let result = read_frame(&mut frontend, None)
        .map_err(|error| format!("application supervisor result channel failed: {error}"))?;
    if result.challenge != challenge || !matches!(result.kind, PROTOCOL_STATUS | PROTOCOL_ERROR) {
        return Err("application supervisor returned an unauthenticated result".to_string());
    }
    if !result.pending {
        let status = wait_for_exit(&mut supervisor, SUPERVISOR_EXIT_TIMEOUT).ok_or_else(|| {
            "application supervisor reported completion but did not exit within its bound"
                .to_string()
        })?;
        if !status.success() {
            return Err(format!(
                "application supervisor exited unsuccessfully after reporting completion: {status}"
            ));
        }
    }
    match result.kind {
        PROTOCOL_STATUS => Ok(ExitStatus::from_raw(result.raw_status)),
        PROTOCOL_ERROR => Err(String::from_utf8(result.message)
            .map_err(|_| "application supervisor error was not UTF-8".to_string())?),
        _ => unreachable!(),
    }
}

pub(crate) fn run_supervisor(
    args: &[std::ffi::OsString],
    run_application: impl FnOnce(&[std::ffi::OsString]) -> Result<ExitStatus, String>,
    cleanup_pending: impl Fn() -> bool,
    finish_cleanup: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    run_supervisor_at(
        args,
        run_application,
        cleanup_pending,
        finish_cleanup,
        &admission_directory(),
    )
}

fn run_supervisor_at(
    args: &[std::ffi::OsString],
    run_application: impl FnOnce(&[std::ffi::OsString]) -> Result<ExitStatus, String>,
    cleanup_pending: impl Fn() -> bool,
    finish_cleanup: impl FnOnce() -> Result<(), String>,
    admission_directory: &Path,
) -> Result<(), String> {
    if args.len() < 4 {
        return Err(
            "application supervisor requires channel, slot, challenge, and runner arguments"
                .to_string(),
        );
    }
    let channel_fd = parse_fd(&args[0], "protocol channel")?;
    let slot_fd = parse_fd(&args[1], "admission slot")?;
    if channel_fd == slot_fd {
        return Err("application supervisor descriptors alias".to_string());
    }
    let AdoptedSupervisorDescriptors { mut channel, slot } = adopt_supervisor_descriptors(
        channel_fd,
        slot_fd,
        // SAFETY: getppid has no memory preconditions and no descriptor side effects.
        unsafe { libc::getppid() },
        admission_directory,
    )?;
    let challenge = parse_challenge(&args[2])?;
    write_frame(&mut channel, &Frame::ready(challenge))?;

    let result = run_application(&args[3..]);
    let pending = cleanup_pending();
    if pending {
        let reported = write_frame(&mut channel, &Frame::result(challenge, true, &result));
        drop(channel);
        let finished = finish_cleanup();
        reported?;
        finished?;
    } else {
        let finish = finish_cleanup();
        let result = match (result, finish) {
            (result, Ok(())) => result,
            (Ok(_), Err(error)) => Err(error),
            (Err(application), Err(cleanup)) => Err(format!(
                "{application}; supervisor shutdown failed: {cleanup}"
            )),
        };
        write_frame(&mut channel, &Frame::result(challenge, false, &result))?;
    }
    drop(slot);
    Ok(())
}

struct SupervisorAdmission {
    file: File,
}

impl SupervisorAdmission {
    fn acquire() -> Result<Self, String> {
        Self::acquire_at(&admission_directory())
    }

    fn acquire_at(directory: &Path) -> Result<Self, String> {
        prepare_admission_directory(directory)?;
        for slot in 0..SUPERVISOR_CAPACITY {
            let path = directory.join(format!("slot-{slot}"));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .mode(0o600)
                .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
                .open(&path)
                .map_err(|error| format!("open application supervisor slot: {error}"))?;
            validate_slot_metadata(&file)?;
            // SAFETY: flock acts on this owned regular-file descriptor and does not dereference
            // memory. The open file description and lock transfer together across exec.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Ok(Self { file });
            }
            let error = io::Error::last_os_error();
            if !error
                .raw_os_error()
                .is_some_and(|code| code == libc::EWOULDBLOCK || code == libc::EAGAIN)
            {
                return Err(format!("lock application supervisor slot: {error}"));
            }
        }
        Err(format!(
            "application supervisor admission is saturated at {SUPERVISOR_CAPACITY} processes"
        ))
    }
}

fn admission_directory() -> PathBuf {
    PathBuf::from(format!(
        "/tmp/fe2o3-application-supervisors-{}",
        // SAFETY: geteuid has no memory preconditions.
        unsafe { libc::geteuid() }
    ))
}

fn prepare_admission_directory(path: &Path) -> Result<(), String> {
    match fs::create_dir(path) {
        Ok(()) => {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|error| format!("secure application supervisor directory: {error}"))?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(format!(
                "create application supervisor admission directory: {error}"
            ));
        }
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect application supervisor directory: {error}"))?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o777 != 0o700
    {
        return Err("application supervisor admission directory is not private".to_string());
    }
    Ok(())
}

#[cfg(test)]
fn validate_slot(file: &File, directory: &Path) -> Result<(), String> {
    validate_slot_raw(file.as_raw_fd(), directory)
}

fn validate_slot_raw(fd: RawFd, directory: &Path) -> Result<(), String> {
    raw_descriptor_flags(fd, "application supervisor slot")?;
    let inherited = raw_fstat(fd, "application supervisor slot")?;
    validate_slot_stat(&inherited)?;
    let status = raw_status_flags(fd, "application supervisor slot")?;
    if status & libc::O_ACCMODE != libc::O_RDWR {
        return Err("application supervisor admission slot is not read-write".to_string());
    }
    prepare_admission_directory(directory)?;
    let canonical = (0..SUPERVISOR_CAPACITY).any(|slot| {
        fs::symlink_metadata(directory.join(format!("slot-{slot}"))).is_ok_and(|metadata| {
            metadata.dev() == inherited.st_dev && metadata.ino() == inherited.st_ino
        })
    });
    if !canonical {
        return Err(
            "application supervisor slot is not a member of the fixed admission pool".to_string(),
        );
    }
    // Re-taking an inherited flock on the same open description is a no-op. If a malformed
    // caller supplied an unlocked slot, this acquires it before any application is launched.
    if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(format!(
            "application supervisor did not inherit its admission lock: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn validate_slot_metadata(file: &File) -> Result<(), String> {
    let stat = raw_fstat(file.as_raw_fd(), "application supervisor slot")?;
    validate_slot_stat(&stat)
}

fn validate_slot_stat(stat: &libc::stat) -> Result<(), String> {
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG
        || stat.st_uid != unsafe { libc::geteuid() }
        || stat.st_nlink != 1
        || stat.st_mode & 0o777 != 0o600
    {
        return Err("application supervisor admission slot is unsafe".to_string());
    }
    Ok(())
}

#[cfg(test)]
fn validate_channel(channel: &UnixStream, expected_parent: libc::pid_t) -> Result<(), String> {
    validate_channel_raw(channel.as_raw_fd(), expected_parent)
}

fn validate_channel_raw(fd: RawFd, expected_parent: libc::pid_t) -> Result<(), String> {
    raw_descriptor_flags(fd, "application supervisor protocol")?;
    let metadata = raw_fstat(fd, "application supervisor channel")?;
    if (metadata.st_mode & libc::S_IFMT) != libc::S_IFSOCK {
        return Err("application supervisor protocol descriptor is not a socket".to_string());
    }
    let mut kind = 0_i32;
    let mut length = std::mem::size_of::<i32>() as libc::socklen_t;
    // SAFETY: kind and length are writable getsockopt outputs for this socket descriptor.
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            std::ptr::from_mut(&mut kind).cast(),
            &mut length,
        )
    } != 0
        || kind != libc::SOCK_STREAM
        || length as usize != std::mem::size_of::<i32>()
    {
        return Err("application supervisor protocol descriptor is not a stream socket".into());
    }
    let mut credentials = std::mem::MaybeUninit::<libc::ucred>::zeroed();
    let mut credentials_len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: credentials and its length are writable SO_PEERCRED outputs for this stream.
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            credentials.as_mut_ptr().cast(),
            &mut credentials_len,
        )
    } != 0
        || credentials_len as usize != std::mem::size_of::<libc::ucred>()
    {
        return Err(format!(
            "inspect application supervisor protocol peer: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: successful getsockopt initialized the complete ucred record.
    let credentials = unsafe { credentials.assume_init() };
    if credentials.pid != expected_parent || credentials.uid != unsafe { libc::geteuid() } {
        return Err(
            "application supervisor protocol peer identity does not match its parent".to_string(),
        );
    }
    Ok(())
}

#[derive(Debug)]
struct Frame {
    challenge: [u8; 32],
    kind: u8,
    pending: bool,
    raw_status: i32,
    message: Vec<u8>,
}

impl Frame {
    fn ready(challenge: [u8; 32]) -> Self {
        Self {
            challenge,
            kind: PROTOCOL_READY,
            pending: false,
            raw_status: 0,
            message: Vec::new(),
        }
    }

    fn result(challenge: [u8; 32], pending: bool, result: &Result<ExitStatus, String>) -> Self {
        match result {
            Ok(status) => Self {
                challenge,
                kind: PROTOCOL_STATUS,
                pending,
                raw_status: (*status).into_raw(),
                message: Vec::new(),
            },
            Err(error) => Self {
                challenge,
                kind: PROTOCOL_ERROR,
                pending,
                raw_status: 0,
                message: error.as_bytes().to_vec(),
            },
        }
    }
}

fn write_frame(channel: &mut UnixStream, frame: &Frame) -> Result<(), String> {
    if frame.message.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err("application supervisor result exceeds its protocol bound".to_string());
    }
    let mut header = [0_u8; PROTOCOL_HEADER_BYTES];
    header[..8].copy_from_slice(&PROTOCOL_MAGIC);
    header[8..40].copy_from_slice(&frame.challenge);
    header[40] = frame.kind;
    header[41] = u8::from(frame.pending);
    header[42..46].copy_from_slice(&frame.raw_status.to_le_bytes());
    header[46..50].copy_from_slice(&(frame.message.len() as u32).to_le_bytes());
    channel
        .write_all(&header)
        .and_then(|()| channel.write_all(&frame.message))
        .map_err(|error| format!("write application supervisor protocol: {error}"))
}

fn read_frame(channel: &mut UnixStream, timeout: Option<Duration>) -> Result<Frame, String> {
    let deadline =
        match timeout {
            Some(timeout) => Some(Instant::now().checked_add(timeout).ok_or_else(|| {
                "application supervisor protocol deadline overflowed".to_string()
            })?),
            None => None,
        };
    channel
        .set_nonblocking(deadline.is_some())
        .map_err(|error| format!("configure application supervisor protocol: {error}"))?;
    let mut header = [0_u8; PROTOCOL_HEADER_BYTES];
    read_exact_until(channel, &mut header, deadline)
        .map_err(|error| format!("read application supervisor protocol header: {error}"))?;
    if header[..8] != PROTOCOL_MAGIC || header[41] > 1 {
        return Err("application supervisor protocol header is invalid".to_string());
    }
    let challenge = header[8..40].try_into().unwrap();
    let raw_status = i32::from_le_bytes(header[42..46].try_into().unwrap());
    let message_len = u32::from_le_bytes(header[46..50].try_into().unwrap()) as usize;
    if message_len > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err("application supervisor protocol message exceeds its bound".to_string());
    }
    let mut message = vec![0; message_len];
    read_exact_until(channel, &mut message, deadline)
        .map_err(|error| format!("read application supervisor protocol body: {error}"))?;
    if deadline.is_some() {
        channel
            .set_nonblocking(false)
            .map_err(|error| format!("restore application supervisor protocol: {error}"))?;
    }
    Ok(Frame {
        challenge,
        kind: header[40],
        pending: header[41] == 1,
        raw_status,
        message,
    })
}

fn read_exact_until(
    channel: &mut UnixStream,
    mut buffer: &mut [u8],
    deadline: Option<Instant>,
) -> Result<(), String> {
    while !buffer.is_empty() {
        match channel.read(buffer) {
            Ok(0) => return Err("application supervisor protocol closed early".to_string()),
            Ok(count) => buffer = &mut buffer[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                let deadline = deadline.ok_or_else(|| {
                    "blocking application supervisor read would block".to_string()
                })?;
                wait_readable(channel.as_raw_fd(), deadline)?;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

fn wait_readable(fd: RawFd, deadline: Instant) -> Result<(), String> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("application supervisor startup handshake timed out".to_string());
        }
        let milliseconds = i32::try_from(remaining.as_millis().max(1)).unwrap_or(i32::MAX);
        let mut descriptor = libc::pollfd {
            fd,
            events: libc::POLLIN | libc::POLLHUP,
            revents: 0,
        };
        // SAFETY: descriptor is a writable one-element poll array.
        let result = unsafe { libc::poll(&mut descriptor, 1, milliseconds) };
        if result > 0 {
            return Ok(());
        }
        if result == 0 {
            if Instant::now() >= deadline {
                return Err("application supervisor startup handshake timed out".to_string());
            }
            continue;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!("poll application supervisor protocol: {error}"));
        }
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            _ => return None,
        }
    }
}

fn terminate_failed_start(child: &mut Child) {
    let process_group = child.id() as libc::pid_t;
    // SAFETY: the frontend created the supervisor as this fresh process-group leader.
    let _ = unsafe { libc::kill(-process_group, libc::SIGKILL) };
    let _ = wait_for_exit(child, SUPERVISOR_EXIT_TIMEOUT);
}

struct AdoptedSupervisorDescriptors {
    channel: UnixStream,
    slot: File,
}

fn adopt_supervisor_descriptors(
    channel_fd: RawFd,
    slot_fd: RawFd,
    expected_parent: libc::pid_t,
    admission_directory: &Path,
) -> Result<AdoptedSupervisorDescriptors, String> {
    if channel_fd == slot_fd {
        return Err("application supervisor descriptors alias".to_string());
    }

    // The hidden CLI is attacker-controlled. Prove both raw numbers safe with libc before any
    // Rust borrowed/owned descriptor API can acquire them.
    validate_channel_raw(channel_fd, expected_parent)?;
    validate_slot_raw(slot_fd, admission_directory)?;
    set_and_verify_cloexec_raw(channel_fd, "application supervisor protocol")?;
    set_and_verify_cloexec_raw(slot_fd, "application supervisor slot")?;

    let channel_duplicate = duplicate_cloexec_raw(channel_fd, "application supervisor protocol")?;
    let slot_duplicate = match duplicate_cloexec_raw(slot_fd, "application supervisor slot") {
        Ok(descriptor) => descriptor,
        Err(error) => {
            close_after_failed_adoption(channel_duplicate);
            return Err(error);
        }
    };
    if let Err(error) = verify_cloexec_raw(channel_duplicate, "duplicated supervisor protocol")
        .and_then(|()| verify_cloexec_raw(slot_duplicate, "duplicated supervisor slot"))
    {
        close_after_failed_adoption(channel_duplicate);
        close_after_failed_adoption(slot_duplicate);
        return Err(error);
    }

    // Duplicates now retain the channel and the slot's open-file-description lock. Close both
    // attacker-selected numbers before wrapping only the validated fresh descriptors.
    if let Err(error) = close_adopted_original(channel_fd, "application supervisor protocol") {
        close_after_failed_adoption(channel_duplicate);
        close_after_failed_adoption(slot_duplicate);
        return Err(error);
    }
    if let Err(error) = close_adopted_original(slot_fd, "application supervisor slot") {
        close_after_failed_adoption(channel_duplicate);
        close_after_failed_adoption(slot_duplicate);
        return Err(error);
    }

    // SAFETY: F_DUPFD_CLOEXEC returned two distinct newly owned descriptors after complete raw
    // validation, and both attacker-selected originals have been explicitly closed.
    let channel = unsafe { UnixStream::from_raw_fd(channel_duplicate) };
    // SAFETY: the second validated duplicate owns the inherited regular-file open description.
    let slot = unsafe { File::from_raw_fd(slot_duplicate) };
    Ok(AdoptedSupervisorDescriptors { channel, slot })
}

fn set_cloexec(fd: RawFd) -> Result<(), String> {
    set_and_verify_cloexec_raw(fd, "application supervisor descriptor")
}

fn raw_descriptor_flags(fd: RawFd, kind: &str) -> Result<i32, String> {
    raw_fcntl_get(fd, libc::F_GETFD)
        .map_err(|error| format!("{kind} descriptor is not open: {error}"))
}

fn raw_status_flags(fd: RawFd, kind: &str) -> Result<i32, String> {
    raw_fcntl_get(fd, libc::F_GETFL)
        .map_err(|error| format!("inspect {kind} status flags: {error}"))
}

fn raw_fstat(fd: RawFd, kind: &str) -> Result<libc::stat, String> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    loop {
        // SAFETY: stat is writable and fstat does not acquire ownership of the raw descriptor.
        if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } == 0 {
            // SAFETY: successful fstat initialized the complete record.
            return Ok(unsafe { stat.assume_init() });
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!("inspect {kind}: {error}"));
        }
    }
}

fn set_and_verify_cloexec_raw(fd: RawFd, kind: &str) -> Result<(), String> {
    let flags = raw_descriptor_flags(fd, kind)?;
    raw_fcntl_set(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC)
        .map_err(|error| format!("protect {kind} descriptor: {error}"))?;
    verify_cloexec_raw(fd, kind)
}

fn verify_cloexec_raw(fd: RawFd, kind: &str) -> Result<(), String> {
    let flags = raw_descriptor_flags(fd, kind)?;
    if flags & libc::FD_CLOEXEC == 0 {
        return Err(format!("{kind} descriptor did not retain close-on-exec"));
    }
    Ok(())
}

fn duplicate_cloexec_raw(fd: RawFd, kind: &str) -> Result<RawFd, String> {
    loop {
        // SAFETY: fcntl duplicates the validated raw descriptor and returns a new owned number.
        let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicate >= 0 {
            return Ok(duplicate);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!("duplicate {kind} descriptor: {error}"));
        }
    }
}

fn close_adopted_original(fd: RawFd, kind: &str) -> Result<(), String> {
    // SAFETY: adoption has already duplicated this validated descriptor; this consumes only the
    // original inherited number. Linux closes it even when reporting EINTR, so it is never retried.
    if unsafe { libc::close(fd) } == 0 {
        Ok(())
    } else {
        Err(format!(
            "close original {kind} descriptor: {}",
            io::Error::last_os_error()
        ))
    }
}

fn close_after_failed_adoption(fd: RawFd) {
    // SAFETY: the descriptor was freshly returned by F_DUPFD_CLOEXEC and is not wrapped elsewhere.
    let _ = unsafe { libc::close(fd) };
}

fn raw_fcntl_get(fd: RawFd, operation: libc::c_int) -> io::Result<i32> {
    loop {
        // SAFETY: F_GETFD/F_GETFL inspect only the supplied raw descriptor.
        let result = unsafe { libc::fcntl(fd, operation) };
        if result >= 0 {
            return Ok(result);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn raw_fcntl_set(fd: RawFd, operation: libc::c_int, value: i32) -> io::Result<()> {
    loop {
        // SAFETY: F_SETFD changes descriptor flags for only this raw descriptor.
        if unsafe { libc::fcntl(fd, operation, value) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn parse_fd(value: &std::ffi::OsStr, kind: &str) -> Result<RawFd, String> {
    value
        .to_str()
        .and_then(|value| value.parse::<RawFd>().ok())
        .filter(|fd| *fd >= 3)
        .ok_or_else(|| format!("application supervisor {kind} descriptor is invalid"))
}

fn random_challenge() -> Result<[u8; 32], String> {
    let mut challenge = [0_u8; 32];
    let mut offset = 0;
    while offset < challenge.len() {
        // SAFETY: the remaining challenge slice is writable for the supplied length.
        let result = unsafe {
            libc::getrandom(
                challenge[offset..].as_mut_ptr().cast(),
                challenge.len() - offset,
                0,
            )
        };
        if result > 0 {
            offset += result as usize;
            continue;
        }
        if result == 0 {
            return Err("getrandom returned no application supervisor challenge bytes".into());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!(
                "generate application supervisor protocol challenge: {error}"
            ));
        }
    }
    Ok(challenge)
}

fn parse_challenge(value: &std::ffi::OsStr) -> Result<[u8; 32], String> {
    let value = value
        .to_str()
        .ok_or_else(|| "application supervisor challenge is not UTF-8".to_string())?;
    if value.len() != 64 {
        return Err("application supervisor challenge has the wrong length".to_string());
    }
    let mut challenge = [0_u8; 32];
    for (index, byte) in challenge.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| "application supervisor challenge is not hexadecimal".to_string())?;
    }
    Ok(challenge)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::os::fd::IntoRawFd;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);
    const CAPACITY_HELPER_ENV: &str = "FE2O3_INTERNAL_TEST_SUPERVISOR_CAPACITY";
    const CAPACITY_HELPER_TEST: &str =
        "application_supervisor::tests::supervisor_admission_has_fixed_capacity_and_recovers";
    const PROCESS_HELPER_ENV: &str = "FE2O3_INTERNAL_TEST_SUPERVISOR_PROCESS";
    const PROCESS_HELPER_TEST: &str =
        "application_supervisor::tests::supervisor_process_retains_admission_after_frontend_result";

    fn test_directory() -> PathBuf {
        env::temp_dir().join(format!(
            "cargo-fe2o3-supervisor-admission-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn supervisor_protocol_rejects_challenge_substitution() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        write_frame(&mut writer, &Frame::ready([7; 32])).unwrap();
        let frame = read_frame(&mut reader, Some(Duration::from_secs(1))).unwrap();
        assert_ne!(frame.challenge, [8; 32]);
        assert_eq!(frame.kind, PROTOCOL_READY);
    }

    #[test]
    fn supervisor_startup_deadline_bounds_partial_frame_slow_drip() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        let drip = std::thread::spawn(move || {
            for byte in PROTOCOL_MAGIC {
                if writer.write_all(&[byte]).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let started = Instant::now();
        let error = read_frame(&mut reader, Some(Duration::from_millis(60))).unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        assert!(started.elapsed() >= Duration::from_millis(50));
        assert!(started.elapsed() < Duration::from_millis(500));
        drop(reader);
        drip.join().unwrap();
    }

    #[test]
    fn supervisor_rejects_noncanonical_slot_and_wrong_peer_identity() {
        let directory = test_directory();
        fs::create_dir(&directory).unwrap();
        let forged_path = directory.join("forged");
        let forged = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&forged_path)
            .unwrap();
        let slot_error = validate_slot(&forged, &admission_directory()).unwrap_err();
        assert!(slot_error.contains("fixed admission pool"), "{slot_error}");

        let (channel, _peer) = UnixStream::pair().unwrap();
        let peer_error = validate_channel(&channel, i32::MAX).unwrap_err();
        assert!(peer_error.contains("peer identity"), "{peer_error}");
        drop(forged);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn supervisor_adoption_closes_originals_and_protects_owned_duplicates() {
        let directory = test_directory();
        let SupervisorAdmission { file } = SupervisorAdmission::acquire_at(&directory).unwrap();
        let slot_source = file.into_raw_fd();
        let (channel, peer) = UnixStream::pair().unwrap();
        let channel_source = channel.into_raw_fd();
        // Keep the post-adoption EBADF assertions outside the descriptor range concurrently used
        // by the rest of the test process. Otherwise an unrelated parallel test can reuse a just
        // closed low descriptor before this test observes it.
        let mut limit = std::mem::MaybeUninit::<libc::rlimit>::zeroed();
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()) },
            0
        );
        let limit = unsafe { limit.assume_init() };
        let descriptor_ceiling = limit.rlim_cur.min(10_002);
        assert!(descriptor_ceiling >= 5);
        let channel_minimum = i32::try_from(descriptor_ceiling - 2).unwrap();
        let slot_minimum = channel_minimum + 1;
        let channel_fd =
            unsafe { libc::fcntl(channel_source, libc::F_DUPFD_CLOEXEC, channel_minimum) };
        assert!(channel_fd >= channel_minimum);
        let slot_fd = unsafe { libc::fcntl(slot_source, libc::F_DUPFD_CLOEXEC, slot_minimum) };
        assert!(slot_fd >= slot_minimum);
        assert_eq!(unsafe { libc::close(channel_source) }, 0);
        assert_eq!(unsafe { libc::close(slot_source) }, 0);
        let adopted = adopt_supervisor_descriptors(
            channel_fd,
            slot_fd,
            std::process::id() as libc::pid_t,
            &directory,
        )
        .unwrap();

        assert_eq!(unsafe { libc::fcntl(channel_fd, libc::F_GETFD) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
        assert_eq!(unsafe { libc::fcntl(slot_fd, libc::F_GETFD) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
        for descriptor in [adopted.channel.as_raw_fd(), adopted.slot.as_raw_fd()] {
            assert_ne!(
                unsafe { libc::fcntl(descriptor, libc::F_GETFD) } & libc::FD_CLOEXEC,
                0
            );
        }

        drop(adopted);
        drop(peer);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn supervisor_admission_has_fixed_capacity_and_recovers() {
        if env::var_os(CAPACITY_HELPER_ENV).is_none() {
            let mut command = Command::new(env::current_exe().unwrap());
            command
                .args(["--exact", CAPACITY_HELPER_TEST, "--nocapture"])
                .env(CAPACITY_HELPER_ENV, "1");
            let output = crate::process_execution::capture_output(&mut command).unwrap();
            assert!(
                output.status.success(),
                "isolated capacity helper failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let directory = test_directory();
        let mut admissions = Vec::new();
        for _ in 0..SUPERVISOR_CAPACITY {
            admissions.push(SupervisorAdmission::acquire_at(&directory).unwrap());
        }
        let error = match SupervisorAdmission::acquire_at(&directory) {
            Ok(_) => panic!("supervisor admission exceeded fixed capacity"),
            Err(error) => error,
        };
        assert!(error.contains("saturated"), "{error}");
        admissions.pop();
        admissions.push(SupervisorAdmission::acquire_at(&directory).unwrap());
        drop(admissions);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn supervisor_process_retains_admission_after_frontend_result() {
        if env::var_os(PROCESS_HELPER_ENV).is_some() {
            let channel = env::var_os("FE2O3_INTERNAL_TEST_SUPERVISOR_CHANNEL").unwrap();
            let slot = env::var_os("FE2O3_INTERNAL_TEST_SUPERVISOR_SLOT").unwrap();
            let challenge = env::var_os("FE2O3_INTERNAL_TEST_SUPERVISOR_CHALLENGE").unwrap();
            let admission_directory =
                PathBuf::from(env::var_os("FE2O3_INTERNAL_TEST_SUPERVISOR_ADMISSION").unwrap());
            run_supervisor_at(
                &[channel, slot, challenge, OsString::from("runner-argument")],
                |_| Err("injected pending cleanup".to_string()),
                || true,
                || {
                    std::thread::sleep(Duration::from_secs(2));
                    Ok(())
                },
                &admission_directory,
            )
            .unwrap();
            return;
        }

        let directory = test_directory();
        let mut admissions = (0..SUPERVISOR_CAPACITY)
            .map(|_| SupervisorAdmission::acquire_at(&directory).unwrap())
            .collect::<Vec<_>>();
        let inherited = admissions.pop().unwrap();
        let (mut frontend, supervisor_channel) = UnixStream::pair().unwrap();
        set_cloexec(frontend.as_raw_fd()).unwrap();
        set_cloexec(supervisor_channel.as_raw_fd()).unwrap();
        let challenge = [9_u8; 32];
        let channel_fd = supervisor_channel.as_raw_fd();
        let slot_fd = inherited.file.as_raw_fd();
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .args(["--exact", PROCESS_HELPER_TEST, "--nocapture"])
            .env(PROCESS_HELPER_ENV, "1")
            .env(
                "FE2O3_INTERNAL_TEST_SUPERVISOR_CHANNEL",
                channel_fd.to_string(),
            )
            .env("FE2O3_INTERNAL_TEST_SUPERVISOR_SLOT", slot_fd.to_string())
            .env("FE2O3_INTERNAL_TEST_SUPERVISOR_ADMISSION", &directory)
            .env(
                "FE2O3_INTERNAL_TEST_SUPERVISOR_CHALLENGE",
                hex_encode(&challenge),
            );
        // SAFETY: this test callback changes only the two inherited descriptor flags.
        unsafe {
            command.pre_exec(move || {
                crate::application_exec::protect_all_nonstdio_descriptors()?;
                crate::application_exec::expose_descriptor(channel_fd)?;
                crate::application_exec::expose_descriptor(slot_fd)?;
                Ok(())
            });
        }
        let mut process = crate::process_execution::spawn(&mut command).unwrap();
        drop(supervisor_channel);
        drop(inherited);

        let ready = read_frame(&mut frontend, Some(Duration::from_secs(5))).unwrap();
        assert_eq!(ready.challenge, challenge);
        assert_eq!(ready.kind, PROTOCOL_READY);
        let result = read_frame(&mut frontend, Some(Duration::from_secs(5))).unwrap();
        assert_eq!(result.challenge, challenge);
        assert_eq!(result.kind, PROTOCOL_ERROR);
        assert!(result.pending);
        assert!(process.try_wait().unwrap().is_none());
        let saturated = match SupervisorAdmission::acquire_at(&directory) {
            Ok(_) => panic!("inherited supervisor slot was released before cleanup"),
            Err(error) => error,
        };
        assert!(saturated.contains("saturated"), "{saturated}");

        let status = wait_for_exit(&mut process, Duration::from_secs(5)).unwrap();
        assert!(status.success());
        admissions.push(SupervisorAdmission::acquire_at(&directory).unwrap());
        drop(admissions);
        fs::remove_dir_all(directory).unwrap();
    }
}
