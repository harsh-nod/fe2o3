use super::*;
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::process::{Pid, Signal, WaitId, WaitIdOptions, kill_process_group, waitid};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const OUTPUT_BOUND: u64 = 32 * 1024 * 1024;
const REPLY_BOUND: usize = 1024 * 1024;
const POLL: Duration = Duration::from_millis(10);

pub struct Job {
    child: Child,
    pid: Pid,
    stdout: File,
    stderr: File,
    consumed: u64,
    deadline: Instant,
    bound: u64,
    reaped: bool,
}

pub struct Capture {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn fresh(path: &Path) -> File {
    OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(path)
        .unwrap_or_else(|error| panic!("create {}: {error}", path.display()))
}

pub fn write_json(path: &Path, value: &Value) {
    let mut file = fresh(path);
    serde_json::to_writer(&mut file, value).unwrap();
    file.write_all(b"\n").unwrap();
}

impl Job {
    pub fn start(command: &mut Command, directory: &Path, label: &str, timeout: u64) -> Self {
        Self::start_with_bound(
            command,
            directory,
            label,
            Duration::from_secs(timeout),
            OUTPUT_BOUND,
        )
    }

    pub(super) fn start_with_bound(
        command: &mut Command,
        directory: &Path,
        label: &str,
        timeout: Duration,
        bound: u64,
    ) -> Self {
        let output_path = directory.join(format!("{label}.stdout"));
        let error_path = directory.join(format!("{label}.stderr"));
        let output_writer = fresh(&output_path);
        let error_writer = fresh(&error_path);
        let stdout = File::open(&output_path).unwrap();
        let stderr = File::open(&error_path).unwrap();
        let child = command
            .stdin(Stdio::piped())
            .stdout(output_writer)
            .stderr(error_writer)
            .process_group(0)
            .spawn()
            .expect("spawn bounded child");
        let pid = Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
        let job = Self {
            child,
            pid,
            stdout,
            stderr,
            consumed: 0,
            deadline: Instant::now() + timeout,
            bound,
            reaped: false,
        };
        let stdin = job.child.stdin.as_ref().expect("owned child stdin");
        let flags = fcntl_getfl(stdin).expect("read child stdin flags");
        fcntl_setfl(stdin, flags | OFlags::NONBLOCK).expect("make child stdin nonblocking");
        job
    }

    fn check(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("child exceeded deadline".into());
        }
        let output = self.stdout.metadata().map_err(|e| e.to_string())?.len();
        let errors = self.stderr.metadata().map_err(|e| e.to_string())?.len();
        if output > self.bound
            || errors > self.bound.min(2 * 1024 * 1024)
            || output.saturating_add(errors) > self.bound
        {
            return Err("child exceeded output bound".into());
        }
        Ok(())
    }

    fn exited(&self) -> Result<bool, String> {
        // Hold the leader unreaped until its entire owned group is terminated.
        waitid(
            WaitId::Pid(self.pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        )
        .map(|status| status.is_some())
        .map_err(|error| error.to_string())
    }

    fn cleanup(&mut self) -> Result<ExitStatus, String> {
        self.child.stdin.take();
        let signal = kill_process_group(self.pid, Signal::KILL);
        self.reap_after_group_signal(signal)
    }

    fn reap_after_group_signal(
        &mut self,
        signal: Result<(), rustix::io::Errno>,
    ) -> Result<ExitStatus, String> {
        let mut failure = match signal {
            Ok(()) | Err(rustix::io::Errno::SRCH) => None,
            Err(error) => {
                let mut message = format!("terminate owned child group: {error}");
                if let Err(error) = self.child.kill() {
                    message.push_str(&format!("; terminate leader: {error}"));
                }
                Some(message)
            }
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.reaped = true;
                    return failure.map_or(Ok(status), Err);
                }
                Ok(None) => {}
                Err(error) => {
                    let message = format!("observe owned child termination: {error}");
                    return Err(match failure {
                        Some(prior) => format!("{prior}; {message}"),
                        None => message,
                    });
                }
            }
            if Instant::now() >= deadline {
                let message = "owned child did not terminate within cleanup bound";
                return Err(match failure.take() {
                    Some(prior) => format!("{prior}; {message}"),
                    None => message.into(),
                });
            }
            std::thread::sleep(POLL);
        }
    }

    pub fn send(&mut self, request: &Value) -> Result<(), String> {
        self.check()?;
        let mut bytes = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        if bytes.len() > 4096 {
            return Err("debugger request exceeds bounded pipe write".into());
        }
        let send_deadline = Instant::now() + Duration::from_secs(30);
        let mut written = 0;
        while written < bytes.len() {
            self.check()?;
            if Instant::now() >= send_deadline {
                return Err("debugger request write timed out".into());
            }
            let result = self
                .child
                .stdin
                .as_mut()
                .ok_or("closed debugger stdin")?
                .write(&bytes[written..]);
            match result {
                Ok(0) => return Err("debugger stdin stopped accepting bytes".into()),
                Ok(count) => written += count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if self.exited()? {
                        return Err("debugger exited during request write".into());
                    }
                    std::thread::sleep(POLL);
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        self.check()
    }

    pub fn reply(&mut self) -> Result<Value, String> {
        let reply_deadline = Instant::now() + Duration::from_secs(30);
        loop {
            self.check()?;
            let available = self.stdout.metadata().map_err(|e| e.to_string())?.len();
            let length = available
                .checked_sub(self.consumed)
                .ok_or("stdout shrank")?;
            if length > REPLY_BOUND as u64 {
                return Err("debugger reply exceeds bound".into());
            }
            let mut bytes = vec![0; length as usize];
            self.stdout
                .seek(SeekFrom::Start(self.consumed))
                .map_err(|e| e.to_string())?;
            self.stdout
                .read_exact(&mut bytes)
                .map_err(|e| e.to_string())?;
            if let Some(end) = bytes.iter().position(|byte| *byte == b'\n') {
                if end + 1 != bytes.len() {
                    return Err("debugger emitted unsolicited trailing output".into());
                }
                self.consumed += bytes.len() as u64;
                return serde_json::from_slice(&bytes[..end]).map_err(|e| e.to_string());
            }
            if self.exited()? {
                return Err("debugger exited before a complete reply".into());
            }
            if Instant::now() >= reply_deadline {
                return Err("debugger reply timed out".into());
            }
            std::thread::sleep(POLL);
        }
    }

    pub fn finish(mut self) -> Result<Capture, String> {
        self.child.stdin.take();
        let result = loop {
            self.check()?;
            if self.exited()? {
                break self.cleanup();
            }
            std::thread::sleep(POLL);
        };
        let status = result?;
        self.check_output_after_exit()?;
        let stdout = read_bounded(&mut self.stdout, self.bound)?;
        let stderr = read_bounded(&mut self.stderr, self.bound.min(2 * 1024 * 1024))?;
        if (stdout.len() as u64).saturating_add(stderr.len() as u64) > self.bound {
            return Err("child exceeded final output bound".into());
        }
        if self.consumed != 0 && stdout.len() as u64 != self.consumed {
            return Err("debugger emitted trailing output".into());
        }
        Ok(Capture {
            status,
            stdout,
            stderr,
        })
    }

    fn check_output_after_exit(&self) -> Result<(), String> {
        let output = self.stdout.metadata().map_err(|e| e.to_string())?.len();
        let errors = self.stderr.metadata().map_err(|e| e.to_string())?.len();
        if errors > self.bound.min(2 * 1024 * 1024) || output.saturating_add(errors) > self.bound {
            Err("child exceeded final output bound".into())
        } else if self.consumed != 0 && output != self.consumed {
            Err("debugger emitted trailing output".into())
        } else {
            Ok(())
        }
    }
}

fn read_bounded(file: &mut File, bound: u64) -> Result<Vec<u8>, String> {
    file.rewind().map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.take(bound + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > bound {
        return Err("child exceeded final output bound".into());
    }
    Ok(bytes)
}

impl Drop for Job {
    fn drop(&mut self) {
        if !self.reaped {
            if let Err(error) = self.cleanup() {
                eprintln!("bounded child cleanup failed: {error}");
            }
        }
    }
}

pub fn run(command: &mut Command, directory: &Path, label: &str, timeout: u64) -> Capture {
    Job::start(command, directory, label, timeout)
        .finish()
        .unwrap_or_else(|error| {
            panic!(
                "{label}: {error}; evidence retained at {}",
                directory.display()
            )
        })
}

pub fn success(capture: &Capture, label: &str) {
    assert!(
        capture.status.success(),
        "{label}: {}\n{}",
        capture.status,
        String::from_utf8_lossy(&capture.stderr)
    );
}

pub fn silent(capture: &Capture, label: &str) {
    success(capture, label);
    assert!(
        capture.stdout.is_empty() && capture.stderr.is_empty(),
        "{label} emitted text"
    );
}

#[test]
fn bounded_child_captures_success_without_readers() {
    let scratch = Scratch::new("capture");
    let capture = run(
        Command::new("/bin/sh").args(["-c", "printf bounded"]),
        &scratch.0,
        "capture",
        5,
    );
    success(&capture, "capture");
    assert_eq!(capture.stdout, b"bounded");
}

#[test]
fn bounded_child_refuses_output_overflow() {
    let scratch = Scratch::new("overflow");
    let job = Job::start_with_bound(
        Command::new("/bin/sh").args(["-c", "printf '%4096s' x"]),
        &scratch.0,
        "overflow",
        Duration::from_secs(5),
        64,
    );
    assert!(job.finish().err().unwrap().contains("output bound"));
}

#[test]
fn bounded_child_timeout_reaps_owned_leader() {
    let scratch = Scratch::new("timeout");
    let job = Job::start_with_bound(
        Command::new("/bin/sh").args(["-c", "sleep 60"]),
        &scratch.0,
        "timeout",
        Duration::from_millis(50),
        64,
    );
    let pid = job.pid;
    assert!(job.finish().err().unwrap().contains("deadline"));
    assert!(matches!(
        waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG
        ),
        Err(rustix::io::Errno::CHILD)
    ));
}

#[test]
fn bounded_child_panic_reaps_owned_leader() {
    let scratch = Scratch::new("panic");
    let job = Job::start(
        Command::new("/bin/sh").args(["-c", "sleep 60"]),
        &scratch.0,
        "panic",
        5,
    );
    let pid = job.pid;
    assert!(
        std::panic::catch_unwind(move || {
            let _job = job;
            panic!("controlled cleanup probe");
        })
        .is_err()
    );
    assert!(matches!(
        waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG
        ),
        Err(rustix::io::Errno::CHILD)
    ));
}

#[test]
fn bounded_child_nonreading_stdin_times_out_and_reaps() {
    let scratch = Scratch::new("backpressure");
    let mut job = Job::start(
        Command::new("/bin/sh").args(["-c", "exec sleep 60"]),
        &scratch.0,
        "backpressure",
        5,
    );
    let pid = job.pid;
    let mut full = false;
    for _ in 0..4096 {
        match job.child.stdin.as_mut().unwrap().write(&[b'x'; 4096]) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                full = true;
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => panic!("fill owned stdin: {error}"),
        }
    }
    assert!(full, "owned pipe never reached backpressure");
    job.deadline = Instant::now() + Duration::from_millis(50);
    assert!(
        job.send(&serde_json::json!({"request": 1}))
            .unwrap_err()
            .contains("deadline")
    );
    drop(job);
    assert!(matches!(
        waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG
        ),
        Err(rustix::io::Errno::CHILD)
    ));
}

#[test]
fn bounded_child_group_error_still_reaps_leader() {
    let scratch = Scratch::new("signal-error");
    let mut job = Job::start(
        Command::new("/bin/sh").args(["-c", "exec sleep 60"]),
        &scratch.0,
        "signal-error",
        5,
    );
    let pid = job.pid;
    job.child.stdin.take();
    let error = job
        .reap_after_group_signal(Err(rustix::io::Errno::PERM))
        .unwrap_err();
    assert!(error.starts_with("terminate owned child group:"));
    assert!(job.reaped);
    assert!(matches!(
        waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG
        ),
        Err(rustix::io::Errno::CHILD)
    ));
}
