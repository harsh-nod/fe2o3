//! Bounded native I/O only for this launch-owned qualification controller.
//! This module never accepts an attach PID or an arbitrary MI command from CLI.
use super::config::{Options, hex};
use super::custody::LaunchedInferior;
use super::protocol::Peer;
use crate::parser::Refusal;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_LINE: usize = 192 * 1024;
const MAX_TRANSCRIPT: usize = 8 * 1024 * 1024;
const MAX_RECORDS: usize = 8192;
const MAX_COMMANDS: usize = 64;

// These flags affect only the debugger's embedded Python startup. System site
// hooks still belong to the supervisor-reviewed dependency closure. The MI
// protocol independently clears the inferior environment before launch.
fn configure_debugger_environment(command: &mut Command) {
    command
        .env_clear()
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONSAFEPATH", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1");
}

#[derive(Clone, Copy)]
enum Stream {
    Out,
    Err,
}
enum Item {
    Line(Stream, Vec<u8>),
    Eof(Stream),
    Failed,
}
pub(super) fn bounded_line(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, Refusal> {
    let mut line = Vec::new();
    reader
        .take((MAX_LINE + 1) as u64)
        .read_until(b'\n', &mut line)
        .map_err(|_| Refusal::Incomplete)?;
    if line.is_empty() {
        return Ok(None);
    }
    if line.len() > MAX_LINE || line.last() != Some(&b'\n') {
        return Err(Refusal::Bound);
    }
    Ok(Some(line))
}
fn reader(
    input: impl Read + Send + 'static,
    which: Stream,
    tx: mpsc::SyncSender<Item>,
    budget: Arc<AtomicUsize>,
) -> Result<JoinHandle<()>, Refusal> {
    thread::Builder::new()
        .name("noqueue-mi-reader".into())
        .spawn(move || {
            let mut input = BufReader::new(input);
            loop {
                let item = match bounded_line(&mut input) {
                    Ok(Some(line)) => {
                        let n = line.len();
                        if budget
                            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                                v.checked_add(n).filter(|v| *v <= MAX_TRANSCRIPT)
                            })
                            .is_err()
                        {
                            Item::Failed
                        } else {
                            Item::Line(which, line)
                        }
                    }
                    Ok(None) => Item::Eof(which),
                    Err(_) => Item::Failed,
                };
                let terminal = !matches!(&item, Item::Line(..));
                if tx.send(item).is_err() || terminal {
                    break;
                }
            }
        })
        .map_err(|_| Refusal::Incomplete)
}
fn abort_spawn(child: &mut Child) {
    let _ = child.kill();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if !matches!(child.try_wait(), Ok(None)) {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
}
#[derive(Serialize, Default)]
pub(super) struct Cleanup {
    pub(super) owned_inferior_pidfd_exit_observed: bool,
    pub(super) debugger_direct_child_reaped: bool,
    pub(super) streams_complete: bool,
    pub(super) reader_threads_joined: bool,
    pub(super) unadmitted_inferior_cleanup_not_proven: bool,
    pub(super) inferior_reaped_by_controller: bool,
}
#[derive(Serialize)]
pub(super) struct Transcript {
    stdout_bytes: usize,
    stdout_sha256: String,
    stdout_hex: String,
    stderr_bytes: usize,
    stderr_sha256: String,
    stderr_hex: String,
    commands_bytes: usize,
    commands_sha256: String,
    commands_hex: String,
}
pub(super) struct Native<'a> {
    options: &'a Options,
    child: Child,
    input: ChildStdin,
    inferior: Option<LaunchedInferior>,
    rx: mpsc::Receiver<Item>,
    readers: Vec<JoinHandle<()>>,
    out: Vec<u8>,
    err: Vec<u8>,
    commands: Vec<u8>,
    eof: [bool; 2],
    records: usize,
    sent: usize,
    deadline: Instant,
    may_have_inferior: bool,
    closed: bool,
}
impl<'a> Native<'a> {
    pub(super) fn spawn(options: &'a Options) -> Result<Self, Refusal> {
        options.debugger.recheck()?;
        options.observer.recheck()?;
        options.artifact.recheck()?;
        // The wrapper, user init files, auto-load scripts and inherited tools
        // environment are outside this fixed command. The supervisor separately
        // pins/reviews the debugger's dynamic-loader closure.
        let mut command = Command::new(&options.debugger.path);
        configure_debugger_environment(&mut command);
        command
            .args([
                "--nx",
                "--quiet",
                "--interpreter=mi3",
                "-iex",
                "set auto-load off",
                "-iex",
                "set debuginfod enabled off",
                "-iex",
                "set startup-with-shell off",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|_| Refusal::Process)?;
        if let Err(error) = options
            .debugger
            .matches_proc_executable(std::path::Path::new(&format!("/proc/{}/exe", child.id())))
        {
            abort_spawn(&mut child);
            return Err(error);
        }
        // These are guaranteed by Stdio::piped. A malformed setup is killed and
        // explicitly refused rather than starting an inferior without capture.
        let Some(input) = child.stdin.take() else {
            abort_spawn(&mut child);
            return Err(Refusal::Incomplete);
        };
        let Some(output) = child.stdout.take() else {
            abort_spawn(&mut child);
            return Err(Refusal::Incomplete);
        };
        let Some(errors) = child.stderr.take() else {
            abort_spawn(&mut child);
            return Err(Refusal::Incomplete);
        };
        let (tx, rx) = mpsc::sync_channel(8);
        let budget = Arc::new(AtomicUsize::new(0));
        let first = match reader(output, Stream::Out, tx.clone(), budget.clone()) {
            Ok(handle) => handle,
            Err(error) => {
                abort_spawn(&mut child);
                return Err(error);
            }
        };
        let second = match reader(errors, Stream::Err, tx, budget) {
            Ok(handle) => handle,
            Err(error) => {
                abort_spawn(&mut child);
                return Err(error);
            }
        };
        let readers = vec![first, second];
        Ok(Self {
            options,
            child,
            input,
            inferior: None,
            rx,
            readers,
            out: Vec::new(),
            err: Vec::new(),
            commands: Vec::new(),
            eof: [false; 2],
            records: 0,
            sent: 0,
            deadline: Instant::now() + Duration::from_secs(60),
            may_have_inferior: false,
            closed: false,
        })
    }
    fn receive(&mut self) -> Result<Option<Vec<u8>>, Refusal> {
        loop {
            if self.eof == [true; 2] {
                return Ok(None);
            }
            let remaining = self
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or(Refusal::Incomplete)?;
            let item = self
                .rx
                .recv_timeout(remaining)
                .map_err(|_| Refusal::Incomplete)?;
            self.records += 1;
            if self.records > MAX_RECORDS {
                return Err(Refusal::Bound);
            }
            match item {
                Item::Failed => return Err(Refusal::Incomplete),
                Item::Eof(stream) => {
                    let index = match stream {
                        Stream::Out => 0,
                        Stream::Err => 1,
                    };
                    if self.eof[index] {
                        return Err(Refusal::Duplicate);
                    }
                    self.eof[index] = true;
                }
                Item::Line(stream, line) => {
                    if self.out.len() + self.err.len() + line.len() > MAX_TRANSCRIPT {
                        return Err(Refusal::Bound);
                    }
                    let output = match stream {
                        Stream::Out => &mut self.out,
                        Stream::Err => &mut self.err,
                    };
                    output
                        .try_reserve_exact(line.len())
                        .map_err(|_| Refusal::Bound)?;
                    if output.capacity() > MAX_TRANSCRIPT {
                        return Err(Refusal::Bound);
                    }
                    output.extend_from_slice(&line);
                    if matches!(stream, Stream::Out) {
                        return Ok(Some(line));
                    }
                }
            }
        }
    }
    pub(super) fn transcript(&self) -> Transcript {
        let sha = |v: &[u8]| hex(&Sha256::digest(v));
        Transcript {
            stdout_bytes: self.out.len(),
            stdout_sha256: sha(&self.out),
            stdout_hex: hex(&self.out),
            stderr_bytes: self.err.len(),
            stderr_sha256: sha(&self.err),
            stderr_hex: hex(&self.err),
            commands_bytes: self.commands.len(),
            commands_sha256: sha(&self.commands),
            commands_hex: hex(&self.commands),
        }
    }
    pub(super) fn cleanup(&mut self) -> Cleanup {
        let mut result = Cleanup {
            // A failed protocol can leave an unreported fork/exec transition.
            // Only the outer launch-family supervisor can close that uncertainty.
            unadmitted_inferior_cleanup_not_proven: self.may_have_inferior,
            ..Cleanup::default()
        };
        if let Some(inferior) = &self.inferior {
            let _ = inferior.kill_owned();
        }
        let _ = self.child.kill();
        self.deadline = Instant::now() + Duration::from_secs(5);
        // Keep draining bounded pipes so reader threads cannot block in send.
        while self.eof != [true; 2] && Instant::now() < self.deadline {
            if self.receive().is_err() {
                break;
            }
        }
        while Instant::now() < self.deadline {
            if !result.debugger_direct_child_reaped {
                result.debugger_direct_child_reaped = matches!(self.child.try_wait(), Ok(Some(_)));
            }
            result.owned_inferior_pidfd_exit_observed = self
                .inferior
                .as_ref()
                .is_some_and(|p| p.exited() == Ok(true));
            if result.debugger_direct_child_reaped
                && (self.inferior.is_none() || result.owned_inferior_pidfd_exit_observed)
            {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        result.streams_complete = self.eof == [true; 2];
        if self.readers.iter().all(JoinHandle::is_finished) {
            result.reader_threads_joined = self.readers.drain(..).all(|h| h.join().is_ok());
        }
        // A pidfd exit observation is not a waitpid/reap certificate: ROCgDB
        // owns the inferior parent relation, not this controller.
        self.closed = true;
        result
    }
}
impl Peer for Native<'_> {
    fn send(&mut self, token: u64, command: &str) -> Result<(), Refusal> {
        if Instant::now() >= self.deadline
            || self.sent == MAX_COMMANDS
            || command.contains(['\n', '\r'])
        {
            return Err(Refusal::Bound);
        }
        let line = format!("{token}{command}\n");
        if line.len() > 2048 {
            return Err(Refusal::Bound);
        }
        if command == "-exec-run" {
            self.may_have_inferior = true;
        }
        // One bounded command outstanding. Previous response proves the prior
        // input was consumed; each write fits Linux PIPE_BUF without a shell.
        self.commands
            .try_reserve_exact(line.len())
            .map_err(|_| Refusal::Bound)?;
        if self.commands.capacity() > MAX_COMMANDS * 2048 {
            return Err(Refusal::Bound);
        }
        self.commands.extend_from_slice(line.as_bytes());
        self.input
            .write_all(line.as_bytes())
            .map_err(|_| Refusal::Incomplete)?;
        self.sent += 1;
        Ok(())
    }
    fn next(&mut self) -> Result<Option<Vec<u8>>, Refusal> {
        self.receive()
    }
    fn observe_child(&mut self, pid: u32) -> Result<(), Refusal> {
        if self.inferior.is_some() {
            return Err(Refusal::Duplicate);
        }
        self.inferior = Some(LaunchedInferior::observe_child(&mut self.child, pid)?);
        Ok(())
    }
    fn entry(&mut self) -> Result<(), Refusal> {
        self.options
            .debugger
            .matches_proc_executable(std::path::Path::new(&format!(
                "/proc/{}/exe",
                self.child.id()
            )))?;
        self.options.artifact.recheck()?;
        self.inferior
            .as_mut()
            .ok_or(Refusal::Process)?
            .verify_entry(&mut self.child, &self.options.observer)
    }
    fn current(&mut self) -> Result<(), Refusal> {
        self.options
            .debugger
            .matches_proc_executable(std::path::Path::new(&format!(
                "/proc/{}/exe",
                self.child.id()
            )))?;
        self.options.artifact.recheck()?;
        self.inferior
            .as_ref()
            .ok_or(Refusal::Process)?
            .current(&mut self.child, &self.options.observer)
    }
    fn finish(&mut self) -> Result<Cleanup, Refusal> {
        if self.eof != [true; 2] {
            return Err(Refusal::Incomplete);
        }
        let mut reap = false;
        while Instant::now() < self.deadline {
            if !reap {
                match self.child.try_wait().map_err(|_| Refusal::Exit)? {
                    Some(status) if status.success() => reap = true,
                    Some(_) => return Err(Refusal::Exit),
                    None => {}
                }
            }
            let dead = self.inferior.as_ref().ok_or(Refusal::Process)?.exited()?;
            if reap && dead && self.readers.iter().all(JoinHandle::is_finished) {
                if !self.readers.drain(..).all(|h| h.join().is_ok()) {
                    return Err(Refusal::Incomplete);
                }
                self.closed = true;
                return Ok(Cleanup {
                    owned_inferior_pidfd_exit_observed: true,
                    debugger_direct_child_reaped: true,
                    streams_complete: true,
                    reader_threads_joined: true,
                    ..Cleanup::default()
                });
            }
            thread::sleep(Duration::from_millis(5));
        }
        Err(Refusal::Incomplete)
    }
}
impl Drop for Native<'_> {
    fn drop(&mut self) {
        if !self.closed {
            // Best effort only. No cleanup receipt is fabricated on panic/drop.
            if let Some(inferior) = &self.inferior {
                let _ = inferior.kill_owned();
            }
            let _ = self.child.kill();
        }
    }
}

#[cfg(test)]
mod startup_environment_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn exact_debugger_environment_replaces_prior_command_overrides() {
        // Construct only: no subprocess, loader, Python or native call occurs.
        let mut command = Command::new("/fixture/not-executed");
        command.env("LD_PRELOAD", "fixture-injection");
        command.env("PYTHONPATH", "/fixture/foreign-modules");
        command.env("PYTHONNOUSERSITE", "0");
        configure_debugger_environment(&mut command);
        let observed: BTreeMap<_, _> = command
            .get_envs()
            .map(|(key, value)| (key.to_str().unwrap(), value.unwrap().to_str().unwrap()))
            .collect();
        assert_eq!(
            observed,
            BTreeMap::from([
                ("LANG", "C"),
                ("LC_ALL", "C"),
                ("PYTHONDONTWRITEBYTECODE", "1"),
                ("PYTHONNOUSERSITE", "1"),
                ("PYTHONSAFEPATH", "1"),
            ])
        );
    }
}
