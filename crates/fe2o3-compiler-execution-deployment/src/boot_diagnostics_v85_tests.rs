use std::fs::File;
use std::io::Write as _;
use std::os::fd::AsFd as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rustix::process::{Pid, PidfdFlags, pidfd_open};

use super::super::{
    RunningSystemdMachineV1, compiler_execution_systemd_machine_error_v85, machine_exit_error,
};
use super::*;

#[test]
fn machine_stderr_pipe_is_bounded_nonblocking_and_retains_writer_mode() {
    let (mut capture, writer) = MachineStderrV85::new().unwrap();
    assert!(fcntl_getpipe_size(&capture.reader).unwrap() <= MAX_BYTES);
    assert!(
        fcntl_getfl(&capture.reader)
            .unwrap()
            .contains(OFlags::NONBLOCK)
    );
    assert!(!fcntl_getfl(&writer).unwrap().contains(OFlags::NONBLOCK));
    capture.drain().unwrap();
    assert!(capture.bytes.is_empty() && !capture.eof);
    let mut writer = File::from(writer);
    writer.write_all(b"retained diagnostic\n").unwrap();
    capture.drain().unwrap();
    assert_eq!(capture.bytes, b"retained diagnostic\n");
    drop(writer);
    capture.drain().unwrap();
    assert!(capture.eof);
    capture.admitted_capacity += 1;
    assert!(
        capture
            .drain()
            .unwrap_err()
            .to_string()
            .contains("capacity changed")
    );
}

#[test]
fn machine_stderr_exact_bound_accepts_and_one_extra_byte_sticks() {
    let (mut capture, writer) = MachineStderrV85::new().unwrap();
    let mut writer = File::from(writer);
    for _ in 0..MAX_BYTES / 1024 {
        writer.write_all(&[b'a'; 1024]).unwrap();
        capture.drain().unwrap();
    }
    assert_eq!(capture.bytes.len(), MAX_BYTES);
    capture.drain().unwrap();
    writer.write_all(b"!").unwrap();
    assert!(capture.drain().unwrap_err().to_string().contains("64KiB"));
    drop(writer);
    assert!(capture.drain().unwrap_err().to_string().contains("64KiB"));
    assert_eq!(capture.bytes.len(), MAX_BYTES);
}

#[test]
fn machine_stderr_prefix_escapes_hostile_bytes_and_truncates_before_formatting() {
    let (mut capture, writer) = MachineStderrV85::new().unwrap();
    let mut writer = File::from(writer);
    writer
        .write_all(b"\xff\0\n\r\x1b\"\\FE2O3_VM_QUALIFICATION_STATUS=0")
        .unwrap();
    capture.drain().unwrap();
    let prefix = capture.prefix();
    assert!(prefix.is_ascii());
    assert!(!prefix.bytes().any(|b| b.is_ascii_control()));
    assert!(prefix.contains("\\xff") && prefix.contains("\\n") && prefix.contains("\\x1b"));
    writer.write_all(&[b'x'; 2048]).unwrap();
    capture.drain().unwrap();
    let prefix = capture.prefix();
    assert!(prefix.len() <= 1024 && prefix.ends_with("[truncated]"));
}

#[test]
fn machine_helper_error_formatter_bounds_hostile_display_work() {
    use std::cell::Cell;
    struct Hostile<'a>(&'a Cell<usize>);
    impl std::fmt::Display for Hostile<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for _ in 0..1_000_000 {
                self.0.set(self.0.get() + 1);
                f.write_str("\x1b\n")?;
            }
            Ok(())
        }
    }
    let calls = Cell::new(0);
    let record = compiler_execution_systemd_machine_error_v85("machine-exec", &Hostile(&calls));
    assert!(record.starts_with("FE2O3_MACHINE_ERROR stage=\"machine-exec\""));
    assert!(record.len() <= 1024 && calls.get() < 1024);
    assert_eq!(record.bytes().filter(|b| *b == b'\n').count(), 1);
    assert!(record.ends_with("[truncated]\"\n"));
}

fn child(command: &str) -> RunningSystemdMachineV1 {
    let (stderr, writer) = MachineStderrV85::new().unwrap();
    let mut child = Command::new("/bin/sh")
        .args(["-c", command])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(writer))
        .spawn()
        .unwrap();
    let pidfd = match pidfd_open(Pid::from_child(&child), PidfdFlags::empty()) {
        Ok(fd) => fd,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("pidfd: {error}");
        }
    };
    RunningSystemdMachineV1 {
        child: Some(child),
        pidfd,
        stderr,
    }
}

#[test]
fn machine_child_nonzero_status_keeps_real_stderr_and_reaps() {
    let mut machine = child("printf 'actual-machine-error\\n' >&2; exit 7");
    let pid = Pid::from_child(machine.child.as_ref().unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = machine.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    machine.child.take();
    assert_eq!(status.code(), Some(7));
    let error = machine_exit_error("before readiness", status, &machine.stderr).to_string();
    assert!(error.contains("exit_code=Some(7)") && error.contains("actual-machine-error\\n"));
    assert!(!error.contains('\n'));
    assert_eq!(
        rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG).err(),
        Some(Errno::CHILD)
    );
}

#[test]
fn machine_live_child_output_overflow_refuses_and_drop_reaps() {
    let mut machine = child(
        "i=0; while test \"$i\" -lt 257; do printf '%0256d' 0 >&2; i=$((i + 1)); done; exec /bin/sleep 30",
    );
    let pid = Pid::from_child(machine.child.as_ref().unwrap());
    let pidfd = rustix::io::dup(machine.pidfd.as_fd()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match machine.try_wait() {
            Err(error) => {
                assert!(error.to_string().contains("64KiB"));
                break;
            }
            Ok(status) => assert!(status.is_none()),
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(machine.stderr.bytes.len() <= MAX_BYTES);
    assert!(machine.stderr.refused);
    assert!(
        machine
            .try_wait()
            .unwrap_err()
            .to_string()
            .contains("64KiB")
    );
    drop(machine);
    assert_eq!(
        rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG).err(),
        Some(Errno::CHILD)
    );
    assert_eq!(
        rustix::process::pidfd_send_signal(&pidfd, rustix::process::Signal::TERM).err(),
        Some(Errno::SRCH)
    );
}

#[test]
fn machine_capture_drop_still_stops_and_reaps_exact_live_child() {
    let machine = child("exec /bin/sleep 30");
    let pid = Pid::from_child(machine.child.as_ref().unwrap());
    let pidfd = rustix::io::dup(machine.pidfd.as_fd()).unwrap();
    drop(machine);
    assert_eq!(
        rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG).err(),
        Some(Errno::CHILD)
    );
    assert_eq!(
        rustix::process::pidfd_send_signal(&pidfd, rustix::process::Signal::TERM).err(),
        Some(Errno::SRCH)
    );
}
