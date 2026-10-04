//! Launch-owned real transport. No CLI PID, arbitrary MI input or retry route.
use super::{
    argv_readiness,
    clock::{self, Clock},
    config::{self, Options},
    custody::{self, DebuggerChild, LaunchedInferior},
    failure_diagnostic::FailureDiagnostic,
    scope::Scope,
    setup_diagnostic::{SetupDiagnostic, SetupStage, SetupTrace},
    streams::{self, Item, ReadBudget, Stream},
};
use fe2o3_private_one_stop_protocol::{
    Cleanup, MAX_BYTES, MAX_COMMANDS, MAX_RECORDS, Peer, Refusal,
};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
fn configure(command: &mut Command) {
    command
        .env_clear()
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONSAFEPATH", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1");
}
fn expected_command(token: u64) -> Option<String> {
    Some(match token {
        1 => "-gdb-set pagination off".into(),
        2 => "-gdb-set confirm off".into(),
        3 => "-gdb-set mi-async on".into(),
        4 => "-gdb-set follow-fork-mode parent".into(),
        5 => "-gdb-set detach-on-fork off".into(),
        6 => "-interpreter-exec console \"unset environment\"".into(),
        7 => "-gdb-set environment LANG=C".into(),
        8 => "-gdb-set environment LC_ALL=C".into(),
        9 => format!(
            "-file-exec-and-symbols \"{}\"",
            fe2o3_private_one_stop_protocol::TARGET_PATH
        ),
        10 => "-exec-arguments --acknowledge-fixed-one-stop-observer-unqualified".into(),
        11 => format!(
            "-break-insert -t {}",
            fe2o3_private_one_stop_protocol::ENTRY
        ),
        12 => "-exec-run".into(),
        13 => "-interpreter-exec console \"fe2o3-one-stop-select-v1\"".into(),
        14..=16 => "-exec-continue".into(),
        17 => "-gdb-exit".into(),
        _ => return None,
    })
}
fn command_matches(sent: u64, token: u64, text: &str) -> Result<(), Refusal> {
    if sent >= MAX_COMMANDS || token != sent + 1 || expected_command(token).as_deref() != Some(text)
    {
        Err(Refusal::State)
    } else {
        Ok(())
    }
}
fn cleanup_late(deadline: Instant, observed: Instant) -> bool {
    observed >= deadline
}
// Pure control-flow predicate, never an ownership or cleanup certificate.
// Finished reader threads may still have lines and EOFs queued for this owner.
fn cleanup_drained(
    reaped: bool,
    inferior_done: bool,
    readers_finished: bool,
    eof: [bool; 2],
) -> bool {
    reaped && inferior_done && readers_finished && eof == [true; 2]
}
fn reserve(n: usize) -> Result<Vec<u8>, Refusal> {
    let mut x = Vec::new();
    x.try_reserve_exact(n).map_err(|_| Refusal::Bound)?;
    Ok(x)
}
pub(super) struct SpawnFailure {
    pub refusal: Refusal,
    pub cleanup: Option<Cleanup>,
    pub diagnostic: SetupDiagnostic,
    pub failure_observation: FailureDiagnostic,
}
pub(super) struct NativePeer<'a> {
    options: &'a Options,
    clock: Clock,
    scope: Scope,
    child: Child,
    debugger: Option<DebuggerChild>,
    argv: Vec<u8>,
    input: Option<ChildStdin>,
    inferior: Option<LaunchedInferior>,
    rx: mpsc::Receiver<Item>,
    readers: Vec<JoinHandle<()>>,
    out: Vec<u8>,
    err: Vec<u8>,
    commands: Vec<u8>,
    eof: [bool; 2],
    records: usize,
    sent: u64,
    may_have_inferior: bool,
    closed: bool,
    cleanup: Option<Cleanup>,
}
struct InitialArgv<'a> {
    child: &'a mut Child,
    debugger: &'a DebuggerChild,
    executable: &'a config::PinnedFile,
    scope: &'a Scope,
    clock: Clock,
}
impl argv_readiness::Source for InitialArgv<'_> {
    fn clock(&mut self) -> Result<(), Refusal> {
        self.clock.check()
    }
    fn identity(&mut self) -> Result<(), Refusal> {
        self.scope.current(self.clock)?;
        self.debugger.check(self.child, self.executable)?;
        self.scope.member(self.child.id())
    }
    fn read_cmdline(&mut self) -> Result<Vec<u8>, Refusal> {
        custody::read_bounded(
            format!("/proc/{}/cmdline", self.child.id()),
            argv_readiness::MAX_CMDLINE,
        )
    }
    fn yield_once(&mut self) {
        thread::yield_now();
    }
}
impl<'a> NativePeer<'a> {
    // Fixed inline failure samples avoid allocation after an owned child exists.
    #[allow(
        clippy::result_large_err,
        reason = "bounded allocation-free failure diagnostics"
    )]
    pub(super) fn spawn(options: &'a Options, clock: Clock) -> Result<Self, SpawnFailure> {
        let mut trace = SetupTrace::new();
        let pre = (|| {
            clock.check()?;
            options.recheck(clock)?;
            let scope = Scope::observe(options, clock)?;
            // Retained streams and command buffers allocated before any child exists.
            let out = reserve(MAX_BYTES)?;
            let err = reserve(MAX_BYTES)?;
            let commands = reserve(MAX_COMMANDS as usize * 2048)?;
            let mut argv = reserve(2048)?;
            for a in std::iter::once(options.debugger.path.as_os_str().as_encoded_bytes())
                .chain(options.arguments.iter().map(|v| v.as_bytes()))
            {
                if argv.len() + a.len() + 1 > 2048 {
                    return Err(Refusal::Bound);
                }
                argv.extend_from_slice(a);
                argv.push(0);
            }
            clock.check()?;
            Ok::<_, Refusal>((scope, out, err, commands, argv))
        })()
        .map_err(|refusal| SpawnFailure {
            refusal,
            cleanup: None,
            diagnostic: trace.freeze(0),
            failure_observation: FailureDiagnostic::unobserved(),
        })?;
        let (scope, out, err, commands, argv) = pre;
        let (tx, rx) = mpsc::sync_channel(8);
        let budget = Arc::new(ReadBudget::default());
        let readers = Vec::with_capacity(2);
        let mut command = Command::new(&options.debugger.path);
        configure(&mut command);
        command
            .args(&options.arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // No source binding means this code is typechecked but never entered.
        trace
            .step(SetupStage::SpawnClock, || clock.check())
            .map_err(|refusal| SpawnFailure {
                refusal,
                cleanup: None,
                diagnostic: trace.freeze(0),
                failure_observation: FailureDiagnostic::unobserved(),
            })?;
        let child = trace
            .step(SetupStage::Spawn, || {
                command.spawn().map_err(|_| Refusal::Process)
            })
            .map_err(|refusal| SpawnFailure {
                refusal,
                cleanup: None,
                diagnostic: trace.freeze(0),
                failure_observation: FailureDiagnostic::unobserved(),
            })?;
        trace.child(child.id());
        let mut result = Self {
            options,
            clock,
            scope,
            child,
            debugger: None,
            argv,
            input: None,
            inferior: None,
            rx,
            readers,
            out,
            err,
            commands,
            eof: [false; 2],
            records: 0,
            sent: 0,
            may_have_inferior: false,
            closed: false,
            cleanup: None,
        };
        let setup = (|| {
            result.debugger = Some(DebuggerChild::observe(
                &mut result.child,
                &options.debugger,
                &mut trace,
            )?);
            trace.custody_acquired();
            trace.step(SetupStage::ScopeMember, || {
                result.scope.member(result.child.id())
            })?;
            argv_readiness::initial(
                &mut InitialArgv {
                    child: &mut result.child,
                    debugger: result.debugger.as_ref().ok_or(Refusal::Process)?,
                    executable: &options.debugger,
                    scope: &result.scope,
                    clock: result.clock,
                },
                &result.argv,
                &mut trace,
            )?;
            result.input = Some(trace.step(SetupStage::TakeStdin, || {
                result.child.stdin.take().ok_or(Refusal::Incomplete)
            })?);
            let out = trace.step(SetupStage::TakeStdout, || {
                result.child.stdout.take().ok_or(Refusal::Incomplete)
            })?;
            let err = trace.step(SetupStage::TakeStderr, || {
                result.child.stderr.take().ok_or(Refusal::Incomplete)
            })?;
            result
                .readers
                .push(trace.step(SetupStage::StartStdoutReader, || {
                    streams::start(out, Stream::Out, tx.clone(), budget.clone())
                })?);
            trace.stdout_reader_started();
            result
                .readers
                .push(trace.step(SetupStage::StartStderrReader, || {
                    streams::start(err, Stream::Err, tx.clone(), budget)
                })?);
            trace.stderr_reader_started();
            trace.step(SetupStage::FinalClock, || result.clock.check())
        })();
        drop(tx);
        if let Err(refusal) = setup {
            // Freeze the first failure before unchanged cleanup does any further work.
            let diagnostic = trace.freeze(result.sent);
            let no_readers_or_commands =
                result.readers.is_empty() && result.input.is_none() && result.sent == 0;
            let failure_observation =
                FailureDiagnostic::observe(&mut result.child, result.clock, no_readers_or_commands);
            let cleanup = result.teardown();
            return Err(SpawnFailure {
                refusal,
                cleanup: Some(cleanup),
                diagnostic,
                failure_observation,
            });
        }
        Ok(result)
    }
    fn owner_current(&mut self) -> Result<(), Refusal> {
        self.clock.check()?;
        self.scope.current(self.clock)?;
        self.debugger
            .as_ref()
            .ok_or(Refusal::Process)?
            .check(&mut self.child, &self.options.debugger)?;
        self.scope.member(self.child.id())?;
        if custody::read_bounded(format!("/proc/{}/cmdline", self.child.id()), 2048)? != self.argv {
            return Err(Refusal::Changed);
        }
        self.clock.check()
    }
    fn receive_until(&mut self, deadline: Instant) -> Result<Option<Vec<u8>>, Refusal> {
        loop {
            clock::before(deadline)?;
            if self.eof == [true; 2] {
                return Ok(None);
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(Refusal::Deadline)?;
            let item = self
                .rx
                .recv_timeout(remaining)
                .map_err(|_| Refusal::Incomplete)?;
            self.records = self.records.checked_add(1).ok_or(Refusal::Bound)?;
            if self.records > MAX_RECORDS + 2 {
                return Err(Refusal::Bound);
            }
            let visible = match item {
                Item::Failed => return Err(Refusal::Incomplete),
                Item::Eof(s) => {
                    let i = if matches!(s, Stream::Out) { 0 } else { 1 };
                    if self.eof[i] {
                        return Err(Refusal::Duplicate);
                    }
                    self.eof[i] = true;
                    None
                }
                Item::Line(s, line) => {
                    if self
                        .out
                        .len()
                        .checked_add(self.err.len())
                        .and_then(|n| n.checked_add(line.len()))
                        .is_none_or(|v| v > MAX_BYTES)
                    {
                        return Err(Refusal::Bound);
                    }
                    let output = if matches!(s, Stream::Out) {
                        &mut self.out
                    } else {
                        &mut self.err
                    };
                    // Exact maximum reserved before spawn; never grows while processing a line.
                    if output.capacity() - output.len() < line.len() {
                        return Err(Refusal::Bound);
                    }
                    output.extend_from_slice(&line);
                    if matches!(s, Stream::Out) {
                        Some(line)
                    } else {
                        None
                    }
                }
            };
            clock::before(deadline)?;
            if visible.is_some() {
                return Ok(visible);
            }
        }
    }
    fn join_finished(&mut self) -> bool {
        let mut okay = true;
        for handle in self.readers.drain(..) {
            okay &= handle.join().is_ok();
        }
        okay
    }
    fn kill_debugger(&mut self) {
        if let Some(owner) = &self.debugger {
            let _ = owner.kill_owned();
        } else {
            let _ = self.child.kill();
        } // Still an unreaped Child handle, not a supplied PID.
    }
    pub(super) fn teardown(&mut self) -> Cleanup {
        if let Some(c) = self.cleanup {
            return c;
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut c = Cleanup {
            unadmitted_inferior_cleanup_not_proven: self.may_have_inferior,
            ..Cleanup::default()
        };
        if let Some(i) = &self.inferior {
            let _ = i.kill_owned();
        }
        self.kill_debugger();
        self.input.take();
        // Continue draining known bounded streams even after a reader reports failure.
        while Instant::now() < deadline {
            if self.eof != [true; 2] {
                let _ =
                    self.receive_until(deadline.min(Instant::now() + Duration::from_millis(10)));
            }
            if !c.debugger_direct_child_reaped {
                c.debugger_direct_child_reaped = matches!(self.child.try_wait(), Ok(Some(_)));
            }
            c.owned_inferior_pidfd_exit_observed = self
                .inferior
                .as_ref()
                .is_some_and(|i| i.exited() == Ok(true));
            if cleanup_drained(
                c.debugger_direct_child_reaped,
                self.inferior.is_none() || c.owned_inferior_pidfd_exit_observed,
                self.readers.iter().all(JoinHandle::is_finished),
                self.eof,
            ) {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        c.streams_complete = self.eof == [true; 2];
        if self.readers.iter().all(JoinHandle::is_finished) {
            c.reader_threads_joined = self.join_finished();
        }
        c.cleanup_deadline_expired = cleanup_late(deadline, Instant::now());
        self.closed = true;
        self.cleanup = Some(c);
        c
    }
    // Failure-only, borrowed post-cleanup bytes; no native query or new I/O.
    pub(super) fn publication_failure_context(&self) -> super::publication::RetainedTransport<'_> {
        super::publication::RetainedTransport {
            sent_consumed: self.sent,
            records_after_cleanup: self.records,
            eof_after_cleanup: self.eof,
            closed_after_cleanup: self.closed,
            stdout: &self.out,
            stderr: &self.err,
            commands: &self.commands,
        }
    }
    pub(super) fn encode_transcript(&self, out: &mut impl Write) -> Result<(), Refusal> {
        self.clock.check()?;
        out.write_all(b"{").map_err(|_| Refusal::Incomplete)?;
        for (i, (name, bytes)) in [
            ("stdout", self.out.as_slice()),
            ("stderr", self.err.as_slice()),
            ("commands", self.commands.as_slice()),
        ]
        .into_iter()
        .enumerate()
        {
            if i > 0 {
                out.write_all(b",").map_err(|_| Refusal::Incomplete)?;
            }
            let digest = config::hex(&Sha256::digest(bytes));
            self.clock.check()?;
            write!(
                out,
                "\"{name}_bytes\":{},\"{name}_sha256\":\"{digest}\",\"{name}_hex\":\"",
                bytes.len()
            )
            .map_err(|_| Refusal::Incomplete)?;
            // Stream hex into caller's pre-reserved bounded report, no second 16 MiB string.
            for chunk in bytes.chunks(4096) {
                self.clock.check()?;
                out.write_all(config::hex(chunk).as_bytes())
                    .map_err(|_| Refusal::Incomplete)?;
            }
            out.write_all(b"\"").map_err(|_| Refusal::Incomplete)?;
        }
        out.write_all(b"}").map_err(|_| Refusal::Incomplete)?;
        self.clock.check()
    }
    pub(super) fn identity(&self) -> serde_json::Value {
        serde_json::json!({"debugger":self.debugger.as_ref().map(DebuggerChild::stamp),
   "inferior":self.inferior.as_ref().map(LaunchedInferior::stamp),
   "scope_current_membership":self.scope.relative(),"scope_parent_invocation_observed":self.scope.invocation(),
   "independent_scope_generation_proof":false,"whole_family_cleanup_proved":false,
   "debugger_sha256":config::hex(&self.options.debugger.sha256),
   "target_sha256":config::hex(&self.options.observer.sha256),
   "artifact_sha256":config::hex(&self.options.artifact.sha256),
   "startup_prerequisite_sha256":config::hex(&self.options.startup.sha256)})
    }
}
impl Peer for NativePeer<'_> {
    fn send(&mut self, token: u64, command: &str) -> Result<(), Refusal> {
        self.clock.check()?;
        if self.closed {
            return Err(Refusal::State);
        }
        self.owner_current()?;
        command_matches(self.sent, token, command)?;
        // Consume this exact command before any write. Unknown/partial outcomes never retry.
        let line = format!("{token}{command}\n");
        if line.len() > 2048 || self.commands.capacity() - self.commands.len() < line.len() {
            return Err(Refusal::Bound);
        }
        self.sent = token;
        if command == "-exec-run" {
            self.may_have_inferior = true;
        }
        self.commands.extend_from_slice(line.as_bytes());
        self.input
            .as_mut()
            .ok_or(Refusal::State)?
            .write_all(line.as_bytes())
            .map_err(|_| Refusal::Incomplete)?;
        self.clock.check()
    }
    fn next(&mut self) -> Result<Option<Vec<u8>>, Refusal> {
        self.receive_until(self.clock.deadline())
    }
    fn observe_child(&mut self, pid: u32) -> Result<(), Refusal> {
        self.owner_current()?;
        if self.inferior.is_some() {
            return Err(Refusal::Duplicate);
        }
        self.inferior = Some(LaunchedInferior::observe_child(&mut self.child, pid)?);
        self.scope.member(pid)?;
        self.clock.check()
    }
    fn entry(&mut self) -> Result<(), Refusal> {
        self.owner_current()?;
        self.options.recheck(self.clock)?;
        self.inferior
            .as_mut()
            .ok_or(Refusal::Process)?
            .verify_entry(&mut self.child, &self.options.observer)?;
        self.clock.check()
    }
    fn current(&mut self) -> Result<(), Refusal> {
        self.owner_current()?;
        self.options.recheck(self.clock)?;
        let i = self.inferior.as_ref().ok_or(Refusal::Process)?;
        self.scope.member(i.pid())?;
        i.current(&mut self.child, &self.options.observer)?;
        self.clock.check()
    }
    fn observed_inferior_start(&mut self) -> Result<u64, Refusal> {
        self.clock.check()?;
        let start = self
            .inferior
            .as_ref()
            .ok_or(Refusal::Process)?
            .verified_start()?;
        self.clock.check()?;
        Ok(start)
    }
    fn elapsed_ns(&mut self) -> Result<u64, Refusal> {
        self.clock.elapsed()
    }
    fn finish(&mut self) -> Result<Cleanup, Refusal> {
        self.clock.check()?;
        if self.closed || self.eof != [true; 2] {
            return Err(Refusal::Incomplete);
        }
        let mut reaped = false;
        loop {
            if !reaped {
                match self.child.try_wait().map_err(|_| Refusal::Exit)? {
                    Some(status) if status.success() => reaped = true,
                    Some(_) => return Err(Refusal::Exit),
                    None => {}
                }
            }
            let dead = self.inferior.as_ref().ok_or(Refusal::Process)?.exited()?;
            let debugger_dead = self.debugger.as_ref().ok_or(Refusal::Process)?.exited()?;
            self.clock.check()?;
            if reaped && dead && debugger_dead && self.readers.iter().all(JoinHandle::is_finished) {
                if !self.join_finished() {
                    return Err(Refusal::Incomplete);
                }
                self.scope.current(self.clock)?;
                self.options.rehash(self.clock)?;
                self.scope.rehash(self.clock)?;
                self.clock.check()?;
                let c = Cleanup {
                    owned_inferior_pidfd_exit_observed: true,
                    debugger_direct_child_reaped: true,
                    streams_complete: true,
                    reader_threads_joined: true,
                    ..Cleanup::default()
                };
                self.closed = true;
                self.cleanup = Some(c);
                return Ok(c);
            }
            thread::sleep(Duration::from_millis(2));
        }
    }
    fn cleanup(&mut self) -> Cleanup {
        self.teardown()
    }
}
impl Drop for NativePeer<'_> {
    fn drop(&mut self) {
        if !self.closed {
            if let Some(i) = &self.inferior {
                let _ = i.kill_owned();
            }
            self.kill_debugger();
            // Best effort only, no receipt or unrelated-family ownership inferred.
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_command_roster_has_no_read_attach_or_retry() {
        for t in 1..=17 {
            let c = expected_command(t).unwrap();
            command_matches(t - 1, t, &c).unwrap();
            assert!(command_matches(t, t, &c).is_err());
            assert!(command_matches(t - 1, t, &format!("{c} ")).is_err());
        }
        for c in [
            "-data-evaluate-expression x",
            "-thread-info",
            "-exec-interrupt",
            "-target-attach 1",
        ] {
            assert!(command_matches(13, 14, c).is_err());
        }
        assert!(expected_command(18).is_none());
    }
    #[test]
    fn late_cleanup_preserves_facts_but_never_qualifies() {
        let end = Instant::now() + Duration::from_secs(5);
        assert!(!cleanup_late(end, end - Duration::from_nanos(1)));
        assert!(cleanup_late(end, end));
        assert!(cleanup_late(end, end + Duration::from_nanos(1)));
        let c = Cleanup {
            owned_inferior_pidfd_exit_observed: true,
            debugger_direct_child_reaped: true,
            streams_complete: true,
            reader_threads_joined: true,
            cleanup_deadline_expired: cleanup_late(end, end),
            ..Cleanup::default()
        };
        assert!(c.cleanup_deadline_expired && c.debugger_direct_child_reaped);
    }
    #[test]
    fn environment_is_exact_and_not_inherited() {
        let mut c = Command::new("/fixture/not-executed");
        c.env("LD_PRELOAD", "x");
        configure(&mut c);
        let v = std::collections::BTreeMap::from_iter(
            c.get_envs()
                .map(|(k, v)| (k.to_str().unwrap(), v.unwrap().to_str().unwrap())),
        );
        assert_eq!(
            v,
            std::collections::BTreeMap::from([
                ("LANG", "C"),
                ("LC_ALL", "C"),
                ("PYTHONNOUSERSITE", "1"),
                ("PYTHONSAFEPATH", "1"),
                ("PYTHONDONTWRITEBYTECODE", "1")
            ])
        );
    }

    #[test]
    fn cleanup_drained_requires_every_actual_terminal_fact() {
        for bits in 0_u8..32 {
            let flag = |n: u8| bits & (1 << n) != 0;
            assert_eq!(
                cleanup_drained(flag(0), flag(1), flag(2), [flag(3), flag(4)]),
                bits == 31
            );
        }
    }
    #[test]
    fn finished_readers_do_not_hide_queued_lines_or_eofs() {
        // No threads or child processes: these are actual transport Item variants.
        // Both readers may already have returned after queueing this finite suffix.
        let mut pending = std::collections::VecDeque::from([
            Item::Line(Stream::Out, b"last stdout\n".to_vec()),
            Item::Eof(Stream::Out),
            Item::Line(Stream::Err, b"last stderr\n".to_vec()),
            Item::Eof(Stream::Err),
        ]);
        let mut eof = [false; 2];
        let mut lines = 0;
        assert!(!cleanup_drained(true, true, true, eof));
        while let Some(item) = pending.pop_front() {
            match item {
                Item::Line(_, _) => lines += 1,
                Item::Eof(Stream::Out) => eof[0] = true,
                Item::Eof(Stream::Err) => eof[1] = true,
                Item::Failed => panic!("fixture contains no reader failure"),
            }
            assert_eq!(cleanup_drained(true, true, true, eof), pending.is_empty());
        }
        assert_eq!(lines, 2);
        assert_eq!(eof, [true; 2]);
    }
    #[test]
    fn failed_reader_does_not_substitute_for_eof() {
        for failed_stream in [0, 1] {
            let mut eof = [false; 2];
            for item in [
                Item::Failed,
                Item::Eof(if failed_stream == 0 {
                    Stream::Err
                } else {
                    Stream::Out
                }),
            ] {
                match item {
                    Item::Failed => {} // Existing receive_until refuses; it never sets either EOF.
                    Item::Eof(Stream::Out) => eof[0] = true,
                    Item::Eof(Stream::Err) => eof[1] = true,
                    Item::Line(_, _) => panic!("fixture contains no line"),
                }
                assert!(!cleanup_drained(true, true, true, eof));
            }
            assert!(!eof[failed_stream]);
            assert!(eof[1 - failed_stream]);
        }
    }
    #[test]
    fn drained_suffix_does_not_extend_cleanup_deadline() {
        let end = Instant::now() + Duration::from_secs(5);
        assert!(cleanup_drained(true, true, true, [true; 2]));
        for observed in [end, end + Duration::from_nanos(1)] {
            let c = Cleanup {
                debugger_direct_child_reaped: true,
                owned_inferior_pidfd_exit_observed: true,
                streams_complete: true,
                reader_threads_joined: true,
                cleanup_deadline_expired: cleanup_late(end, observed),
                ..Cleanup::default()
            };
            assert!(c.streams_complete && c.reader_threads_joined && c.cleanup_deadline_expired);
        }
    }
}
