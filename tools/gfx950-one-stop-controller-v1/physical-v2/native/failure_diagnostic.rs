//! Failure-only observation of the already owned child; never admission or cleanup proof.
use super::clock::Clock;
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::{Errno, read};
use std::fmt;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ExitStatus};

const RETAIN: usize = 256;
const READ_CALLS: u8 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WaitObservation {
    Unobserved,
    Deadline,
    Running,
    Exited {
        raw: i32,
        code: Option<i32>,
        signal: Option<i32>,
        core_dumped: bool,
    },
    Error(Option<i32>),
}
fn waited(value: std::io::Result<Option<ExitStatus>>) -> WaitObservation {
    match value {
        Ok(None) => WaitObservation::Running,
        Ok(Some(status)) => WaitObservation::Exited {
            raw: status.into_raw(),
            code: status.code(),
            signal: status.signal(),
            core_dumped: status.core_dumped(),
        },
        Err(error) => WaitObservation::Error(error.raw_os_error()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PipeState {
    Unobserved,
    OwnershipNotIntact,
    Deadline,
    GetFlagsError(i32),
    SetFlagsError(i32),
    WouldBlock,
    Eof,
    ByteLimit,
    ReadLimit,
    ReadError(i32),
    InvalidReadCount,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Restore {
    Unchanged,
    Restored,
    Error(i32),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PipeObservation {
    state: PipeState,
    restore: Restore,
    read_calls: u8,
    used: usize,
    bytes: [u8; RETAIN + 1],
}
impl PipeObservation {
    fn empty(state: PipeState) -> Self {
        Self {
            state,
            restore: Restore::Unchanged,
            read_calls: 0,
            used: 0,
            bytes: [0; RETAIN + 1],
        }
    }
}
impl fmt::Display for PipeObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "state={:?},restore={:?},read_calls={},observed_bytes={},retained_bytes={},hex=",
            self.state,
            self.restore,
            self.read_calls,
            self.used,
            self.used.min(RETAIN)
        )?;
        for byte in &self.bytes[..self.used.min(RETAIN)] {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn pipes_intact(no_readers_or_commands: bool, stdin: bool, stdout: bool, stderr: bool) -> bool {
    no_readers_or_commands && stdin && stdout && stderr
}

// A private seam for scripted CPU controls. Production wraps only Child's original pipes.
trait PipeOps {
    fn flags(&mut self) -> rustix::io::Result<OFlags>;
    fn set_flags(&mut self, flags: OFlags) -> rustix::io::Result<()>;
    fn read(&mut self, buf: &mut [u8]) -> rustix::io::Result<usize>;
}
struct OwnedPipe<'a>(BorrowedFd<'a>);
impl PipeOps for OwnedPipe<'_> {
    fn flags(&mut self) -> rustix::io::Result<OFlags> {
        fcntl_getfl(self.0)
    }
    fn set_flags(&mut self, flags: OFlags) -> rustix::io::Result<()> {
        fcntl_setfl(self.0, flags)
    }
    fn read(&mut self, buf: &mut [u8]) -> rustix::io::Result<usize> {
        read(self.0, buf)
    }
}
fn capture(pipe: &mut impl PipeOps, current: &mut impl FnMut() -> bool) -> PipeObservation {
    let mut out = PipeObservation::empty(PipeState::Deadline);
    if !current() {
        return out;
    }
    let flags = match pipe.flags() {
        Ok(value) => value,
        Err(error) => {
            out.state = PipeState::GetFlagsError(error.raw_os_error());
            return out;
        }
    };
    let changed = !flags.contains(OFlags::NONBLOCK);
    if changed {
        if !current() {
            return out;
        }
        if let Err(error) = pipe.set_flags(flags | OFlags::NONBLOCK) {
            out.state = PipeState::SetFlagsError(error.raw_os_error());
            return out;
        }
    }
    // No sleeps, readiness wait, reopen, thread, command or retry of a failed read.
    // EINTR is retained as an error, not silently retried.
    out.state = PipeState::ReadLimit;
    for _ in 0..READ_CALLS {
        if !current() {
            out.state = PipeState::Deadline;
            break;
        }
        out.read_calls += 1;
        let available = out.bytes.len() - out.used;
        match pipe.read(&mut out.bytes[out.used..]) {
            Ok(0) => {
                out.state = PipeState::Eof;
                break;
            }
            Ok(n) if n <= available => {
                out.used += n;
                if out.used > RETAIN {
                    out.state = PipeState::ByteLimit;
                    break;
                }
            }
            Ok(_) => {
                out.state = PipeState::InvalidReadCount;
                break;
            }
            Err(Errno::AGAIN) => {
                out.state = PipeState::WouldBlock;
                break;
            }
            Err(error) => {
                out.state = PipeState::ReadError(error.raw_os_error());
                break;
            }
        }
    }
    if changed {
        // One nonwaiting restorative syscall, even if the observation deadline expired.
        // It does not restart a clock or admit a result, and its failure remains explicit.
        out.restore = match pipe.set_flags(flags) {
            Ok(()) => Restore::Restored,
            Err(error) => Restore::Error(error.raw_os_error()),
        };
    }
    out
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FailureDiagnostic {
    wait: WaitObservation,
    stdout: PipeObservation,
    stderr: PipeObservation,
}
impl FailureDiagnostic {
    pub(super) fn unobserved() -> Self {
        Self {
            wait: WaitObservation::Unobserved,
            stdout: PipeObservation::empty(PipeState::Unobserved),
            stderr: PipeObservation::empty(PipeState::Unobserved),
        }
    }

    // This function is called only after the immutable first-failure trace is frozen.
    // It cannot return success to spawn(), update any cleanup fact, or send MI commands.
    pub(super) fn observe(child: &mut Child, clock: Clock, no_readers_or_commands: bool) -> Self {
        let mut out = Self::unobserved();
        out.wait = if clock.check().is_ok() {
            waited(child.try_wait())
        } else {
            WaitObservation::Deadline
        };
        let intact = pipes_intact(
            no_readers_or_commands,
            child.stdin.is_some(),
            child.stdout.is_some(),
            child.stderr.is_some(),
        );
        if !intact {
            out.stdout.state = PipeState::OwnershipNotIntact;
            out.stderr.state = PipeState::OwnershipNotIntact;
            return out;
        }
        // Borrow only; do not take, close or replace the original pipe handles.
        if let (Some(stdout), Some(stderr)) = (child.stdout.as_ref(), child.stderr.as_ref()) {
            let mut current = || clock.check().is_ok();
            out.stderr = capture(&mut OwnedPipe(stderr.as_fd()), &mut current);
            out.stdout = capture(&mut OwnedPipe(stdout.as_fd()), &mut current);
        }
        out
    }
}
impl fmt::Display for FailureDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pre_teardown_wait={:?};stderr=[{}];stdout=[{}];diagnostic_only=true",
            self.wait, self.stderr, self.stdout
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    enum ReadStep {
        Bytes(Vec<u8>),
        Error(Errno),
        Invalid,
    }
    struct FakePipe {
        flags: rustix::io::Result<OFlags>,
        sets: Vec<OFlags>,
        set_results: VecDeque<rustix::io::Result<()>>,
        reads: VecDeque<ReadStep>,
        calls: Vec<&'static str>,
    }
    impl FakePipe {
        fn new(reads: Vec<ReadStep>) -> Self {
            Self {
                flags: Ok(OFlags::RDONLY),
                sets: Vec::new(),
                set_results: VecDeque::new(),
                reads: reads.into(),
                calls: Vec::new(),
            }
        }
    }
    impl PipeOps for FakePipe {
        fn flags(&mut self) -> rustix::io::Result<OFlags> {
            self.calls.push("get");
            self.flags
        }
        fn set_flags(&mut self, flags: OFlags) -> rustix::io::Result<()> {
            self.calls.push("set");
            self.sets.push(flags);
            self.set_results.pop_front().unwrap_or(Ok(()))
        }
        fn read(&mut self, buf: &mut [u8]) -> rustix::io::Result<usize> {
            self.calls.push("read");
            match self.reads.pop_front().expect("bounded scripted read") {
                ReadStep::Bytes(bytes) => {
                    assert!(bytes.len() <= buf.len());
                    buf[..bytes.len()].copy_from_slice(&bytes);
                    Ok(bytes.len())
                }
                ReadStep::Error(error) => Err(error),
                ReadStep::Invalid => Ok(buf.len() + 1),
            }
        }
    }
    fn bytes(value: &[u8]) -> ReadStep {
        ReadStep::Bytes(value.to_vec())
    }

    #[test]
    fn any_partial_reader_command_or_pipe_state_refuses_capture() {
        for mask in 0..16 {
            assert_eq!(
                pipes_intact(mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0),
                mask == 15
            );
        }
    }
    #[test]
    fn wait_preserves_running_exit_code_signal_and_error() {
        assert_eq!(waited(Ok(None)), WaitObservation::Running);
        assert_eq!(
            waited(Ok(Some(ExitStatus::from_raw(7 << 8)))),
            WaitObservation::Exited {
                raw: 7 << 8,
                code: Some(7),
                signal: None,
                core_dumped: false
            }
        );
        assert_eq!(
            waited(Ok(Some(ExitStatus::from_raw(11 | 128)))),
            WaitObservation::Exited {
                raw: 139,
                code: None,
                signal: Some(11),
                core_dumped: true
            }
        );
        assert_eq!(
            waited(Err(std::io::Error::from_raw_os_error(10))),
            WaitObservation::Error(Some(10))
        );
        assert_ne!(waited(Ok(None)), FailureDiagnostic::unobserved().wait);
    }
    #[test]
    fn expired_observation_makes_no_pipe_syscalls() {
        let mut pipe = FakePipe::new(Vec::new());
        let out = capture(&mut pipe, &mut || false);
        assert_eq!(out.state, PipeState::Deadline);
        assert!(pipe.calls.is_empty());
    }
    #[test]
    fn flag_refusals_do_not_read_or_claim_restoration() {
        let mut pipe = FakePipe::new(Vec::new());
        pipe.flags = Err(Errno::BADF);
        let out = capture(&mut pipe, &mut || true);
        assert_eq!(
            out.state,
            PipeState::GetFlagsError(Errno::BADF.raw_os_error())
        );
        assert_eq!(pipe.calls, ["get"]);
        pipe = FakePipe::new(Vec::new());
        pipe.set_results.push_back(Err(Errno::PERM));
        let out = capture(&mut pipe, &mut || true);
        assert_eq!(
            out.state,
            PipeState::SetFlagsError(Errno::PERM.raw_os_error())
        );
        assert_eq!(out.restore, Restore::Unchanged);
        assert_eq!(pipe.calls, ["get", "set"]);
    }
    #[test]
    fn would_block_and_eof_are_distinct_and_restore_mode() {
        for (step, state) in [
            (ReadStep::Error(Errno::AGAIN), PipeState::WouldBlock),
            (bytes(b""), PipeState::Eof),
        ] {
            let mut pipe = FakePipe::new(vec![step]);
            let out = capture(&mut pipe, &mut || true);
            assert_eq!(out.state, state);
            assert_eq!(out.used, 0);
            assert_eq!(out.restore, Restore::Restored);
            assert_eq!(
                pipe.sets,
                [OFlags::RDONLY | OFlags::NONBLOCK, OFlags::RDONLY]
            );
            assert_eq!(pipe.calls, ["get", "set", "read", "set"]);
        }
    }
    #[test]
    fn partial_bytes_survive_would_block_and_are_only_hex() {
        let mut pipe = FakePipe::new(vec![bytes(b"a\n\0"), ReadStep::Error(Errno::AGAIN)]);
        let out = capture(&mut pipe, &mut || true);
        assert_eq!(out.used, 3);
        assert_eq!(out.state, PipeState::WouldBlock);
        let display = out.to_string();
        assert!(display.ends_with("hex=610a00"));
        assert!(!display.contains('\n') && !display.contains('\0'));
    }
    #[test]
    fn one_overflow_byte_marks_truncation_and_never_retains_more_than_cap() {
        let mut pipe = FakePipe::new(vec![ReadStep::Bytes(vec![255; RETAIN + 1])]);
        let out = capture(&mut pipe, &mut || true);
        assert_eq!(out.state, PipeState::ByteLimit);
        assert_eq!(out.used, RETAIN + 1);
        assert_eq!(out.read_calls, 1);
        assert_eq!(
            out.to_string().split("hex=").nth(1).unwrap().len(),
            RETAIN * 2
        );
    }
    #[test]
    fn exact_cap_requires_an_eof_or_overflow_observation() {
        let mut pipe = FakePipe::new(vec![ReadStep::Bytes(vec![42; RETAIN]), bytes(b"")]);
        let out = capture(&mut pipe, &mut || true);
        assert_eq!(out.state, PipeState::Eof);
        assert_eq!(out.used, RETAIN);
        assert_eq!(out.read_calls, 2);
    }
    #[test]
    fn four_short_reads_stop_without_waiting_for_more() {
        let mut pipe = FakePipe::new((0..READ_CALLS).map(|_| bytes(b"x")).collect());
        let out = capture(&mut pipe, &mut || true);
        assert_eq!(out.state, PipeState::ReadLimit);
        assert_eq!(out.used, usize::from(READ_CALLS));
        assert_eq!(out.read_calls, READ_CALLS);
        assert_eq!(pipe.calls.len(), 7);
    }
    #[test]
    fn errors_including_interrupted_are_not_retried() {
        for error in [Errno::IO, Errno::INTR] {
            let mut pipe = FakePipe::new(vec![bytes(b"x"), ReadStep::Error(error)]);
            let out = capture(&mut pipe, &mut || true);
            assert_eq!(out.state, PipeState::ReadError(error.raw_os_error()));
            assert_eq!(out.used, 1);
            assert_eq!(out.read_calls, 2);
            assert_eq!(out.restore, Restore::Restored);
        }
    }
    #[test]
    fn restoration_failure_is_not_hidden_and_does_not_retry() {
        let mut pipe = FakePipe::new(vec![bytes(b"")]);
        pipe.set_results = [Ok(()), Err(Errno::IO)].into();
        let out = capture(&mut pipe, &mut || true);
        assert_eq!(out.state, PipeState::Eof);
        assert_eq!(out.restore, Restore::Error(Errno::IO.raw_os_error()));
        assert_eq!(pipe.calls.len(), 4);
    }
    #[test]
    fn deadline_after_mode_change_restores_without_a_read_or_new_window() {
        let mut pipe = FakePipe::new(Vec::new());
        let mut checks = [true, true, false].into_iter();
        let out = capture(&mut pipe, &mut || checks.next().unwrap());
        assert_eq!(out.state, PipeState::Deadline);
        assert_eq!(out.read_calls, 0);
        assert_eq!(out.restore, Restore::Restored);
        assert_eq!(pipe.calls, ["get", "set", "set"]);
    }
    #[test]
    fn already_nonblocking_pipe_is_never_reconfigured() {
        let mut pipe = FakePipe::new(vec![bytes(b"")]);
        pipe.flags = Ok(OFlags::RDONLY | OFlags::NONBLOCK);
        let out = capture(&mut pipe, &mut || true);
        assert_eq!(out.state, PipeState::Eof);
        assert_eq!(out.restore, Restore::Unchanged);
        assert!(pipe.sets.is_empty());
    }
    #[test]
    fn invalid_scripted_read_count_cannot_expose_uninitialized_bytes() {
        let mut pipe = FakePipe::new(vec![ReadStep::Invalid]);
        let out = capture(&mut pipe, &mut || true);
        assert_eq!(out.state, PipeState::InvalidReadCount);
        assert_eq!(out.used, 0);
        assert_eq!(out.restore, Restore::Restored);
    }
    #[test]
    fn complete_failure_diagnostic_stays_inside_original_four_kib_envelope() {
        let mut pipe = FakePipe::new(vec![ReadStep::Bytes(vec![255; RETAIN + 1])]);
        let sample = capture(&mut pipe, &mut || true);
        let failure = FailureDiagnostic {
            wait: WaitObservation::Error(Some(i32::MIN)),
            stdout: sample,
            stderr: sample,
        };
        let first = super::super::setup_diagnostic::SetupTrace::new().freeze(u64::MAX);
        let raw = format!(
            "one-stop debugger setup refused: Changed; known cleanup={:?}; diagnostic={}; failure_observation={}; no whole-family claim\n",
            Some(fe2o3_private_one_stop_protocol::Cleanup::default()),
            first,
            failure
        );
        assert!(raw.len() <= 4096);
        assert_eq!(raw.bytes().filter(|b| *b == b'\n').count(), 1);
    }
}
