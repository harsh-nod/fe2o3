//! Launch-derived pidfd custody. No caller PID, attach, or address constructor.
use super::config::{PinnedFile, decimal};
use fe2o3_private_one_stop_protocol::Refusal;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
use std::fs;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::process::Child;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub(super) struct Stamp {
    pub(super) pid: u32,
    pub(super) parent: u32,
    pub(super) start: u64,
}
pub(super) fn parse_stat(
    bytes: &[u8],
    expected_pid: u32,
    expected_parent: u32,
) -> Result<Stamp, Refusal> {
    if bytes.len() > 4096 {
        return Err(Refusal::Bound);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Refusal::Syntax)?;
    let open = text.find(" (").ok_or(Refusal::Shape)?;
    let close = text.rfind(") ").ok_or(Refusal::Shape)?;
    if close < open + 2 {
        return Err(Refusal::Shape);
    }
    let pid = u32::try_from(decimal(&bytes[..open])?).map_err(|_| Refusal::Bound)?;
    let fields: Vec<_> = text[close + 2..]
        .split_ascii_whitespace()
        .take(65)
        .collect();
    if fields.len() < 20
        || fields.len() > 64
        || fields[0].len() != 1
        || !matches!(fields[0], "R" | "S" | "D" | "T" | "t")
    {
        return Err(Refusal::Process);
    }
    let parent = u32::try_from(decimal(fields[1].as_bytes())?).map_err(|_| Refusal::Bound)?;
    let start = decimal(fields[19].as_bytes())?;
    if pid != expected_pid || parent != expected_parent || start == 0 {
        return Err(Refusal::Process);
    }
    Ok(Stamp { pid, parent, start })
}
pub(super) fn read_bounded(path: impl AsRef<Path>, max: usize) -> Result<Vec<u8>, Refusal> {
    let mut data = Vec::new();
    data.try_reserve_exact(max + 1)
        .map_err(|_| Refusal::Bound)?;
    fs::File::open(path)
        .map_err(|_| Refusal::Process)?
        .take((max + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|_| Refusal::Process)?;
    if data.len() > max {
        return Err(Refusal::Bound);
    }
    Ok(data)
}
pub(super) fn current(pid: u32, parent: u32) -> Result<Stamp, Refusal> {
    parse_stat(
        &read_bounded(format!("/proc/{pid}/stat"), 4096)?,
        pid,
        parent,
    )
}
pub(super) fn clean_environment(raw: &[u8]) -> Result<(), Refusal> {
    if raw.len() > 128 {
        return Err(Refusal::Bound);
    }
    let mut values: Vec<_> = raw.split(|b| *b == 0).collect();
    if values.pop() != Some(b"".as_slice()) {
        return Err(Refusal::Shape);
    }
    values.sort_unstable();
    if values != [b"LANG=C".as_slice(), b"LC_ALL=C".as_slice()] {
        return Err(Refusal::Process);
    }
    Ok(())
}
fn task_and_environment(pid: u32) -> Result<(), Refusal> {
    let mut tasks = fs::read_dir(format!("/proc/{pid}/task")).map_err(|_| Refusal::Process)?;
    let first = tasks
        .next()
        .ok_or(Refusal::Process)?
        .map_err(|_| Refusal::Process)?;
    if first.file_name() != pid.to_string().as_str() || tasks.next().is_some() {
        return Err(Refusal::Process);
    }
    clean_environment(&read_bounded(format!("/proc/{pid}/environ"), 128)?)?;
    if !read_bounded(format!("/proc/{pid}/task/{pid}/children"), 128)?
        .iter()
        .all(u8::is_ascii_whitespace)
    {
        return Err(Refusal::Process);
    }
    Ok(())
}
fn fresh_fds(pid: u32) -> Result<(), Refusal> {
    let mut seen = [false; 3];
    for row in fs::read_dir(format!("/proc/{pid}/fd")).map_err(|_| Refusal::Process)? {
        let row = row.map_err(|_| Refusal::Process)?;
        let name = row
            .file_name()
            .into_string()
            .map_err(|_| Refusal::Process)?;
        let index = usize::try_from(decimal(name.as_bytes())?).map_err(|_| Refusal::Process)?;
        if index >= 3 || seen[index] {
            return Err(Refusal::Process);
        }
        seen[index] = true;
        let kind = fs::metadata(row.path())
            .map_err(|_| Refusal::Process)?
            .file_type();
        let link = fs::read_link(row.path()).map_err(|_| Refusal::Process)?;
        if !(kind.is_fifo() || kind.is_char_device() && link == Path::new("/dev/null")) {
            return Err(Refusal::Process);
        }
    }
    if seen != [true; 3] {
        return Err(Refusal::Process);
    }
    Ok(())
}
fn exited(fd: &OwnedFd) -> Result<bool, Refusal> {
    let mut descriptors = [PollFd::new(fd, PollFlags::IN)];
    poll(
        &mut descriptors,
        Some(&Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .map_err(|_| Refusal::Process)?;
    let flags = descriptors[0].revents();
    if flags.intersects(PollFlags::ERR | PollFlags::NVAL) {
        return Err(Refusal::Process);
    }
    Ok(flags.contains(PollFlags::IN))
}

/// This first stage grants only cleanup of an observed child of our unreaped
/// debugger. It grants NO permission to resume past the first-entry host stop.
pub(super) struct LaunchedInferior {
    stamp: Stamp,
    fd: OwnedFd,
    entry_verified: bool,
}
impl LaunchedInferior {
    pub(super) fn observe_child(child: &mut Child, pid: u32) -> Result<Self, Refusal> {
        if child.try_wait().map_err(|_| Refusal::Process)?.is_some() {
            return Err(Refusal::Exit);
        }
        let before = current(pid, child.id())?;
        let raw = i32::try_from(pid)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(Refusal::Process)?;
        let fd = pidfd_open(raw, PidfdFlags::empty()).map_err(|_| Refusal::Process)?;
        if exited(&fd)?
            || current(pid, child.id())? != before
            || child.try_wait().map_err(|_| Refusal::Process)?.is_some()
        {
            return Err(Refusal::Changed);
        }
        Ok(Self {
            stamp: before,
            fd,
            entry_verified: false,
        })
    }
    pub(super) fn stamp(&self) -> Stamp {
        self.stamp
    }
    pub(super) fn verified_start(&self) -> Result<u64, Refusal> {
        if !self.entry_verified {
            return Err(Refusal::State);
        }
        Ok(self.stamp.start)
    }
    pub(super) fn pid(&self) -> u32 {
        self.stamp.pid
    }
    pub(super) fn verify_entry(
        &mut self,
        child: &mut Child,
        executable: &PinnedFile,
    ) -> Result<(), Refusal> {
        if self.entry_verified {
            return Err(Refusal::State);
        }
        self.check_identity(child, executable)?;
        fresh_fds(self.pid())?;
        task_and_environment(self.pid())?;
        self.check_identity(child, executable)?;
        self.entry_verified = true;
        Ok(())
    }
    fn check_identity(&self, child: &mut Child, executable: &PinnedFile) -> Result<(), Refusal> {
        if exited(&self.fd)?
            || child.id() != self.stamp.parent
            || child.try_wait().map_err(|_| Refusal::Process)?.is_some()
            || current(self.pid(), child.id())? != self.stamp
        {
            return Err(Refusal::Changed);
        }
        executable.matches_proc_executable(Path::new(&format!("/proc/{}/exe", self.pid())))?;
        task_and_environment(self.pid())?;
        if exited(&self.fd)? || current(self.pid(), child.id())? != self.stamp {
            return Err(Refusal::Changed);
        }
        Ok(())
    }
    pub(super) fn current(
        &self,
        child: &mut Child,
        executable: &PinnedFile,
    ) -> Result<(), Refusal> {
        if !self.entry_verified {
            return Err(Refusal::State);
        }
        self.check_identity(child, executable)
    }
    pub(super) fn exited(&self) -> Result<bool, Refusal> {
        exited(&self.fd)
    }
    pub(super) fn kill_owned(&self) -> Result<(), Refusal> {
        if !self.exited()? {
            pidfd_send_signal(&self.fd, Signal::KILL).map_err(|_| Refusal::Process)?;
        }
        Ok(())
    }
}

/// Separate pidfd for the direct debugger child. Never constructed from a CLI PID.
pub(super) struct DebuggerChild {
    stamp: Stamp,
    fd: OwnedFd,
}
impl DebuggerChild {
    pub(super) fn observe(child: &mut Child, executable: &PinnedFile) -> Result<Self, Refusal> {
        if child.try_wait().map_err(|_| Refusal::Process)?.is_some() {
            return Err(Refusal::Exit);
        }
        let before = current(child.id(), std::process::id())?;
        let pid = i32::try_from(child.id())
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(Refusal::Process)?;
        let fd = pidfd_open(pid, PidfdFlags::empty()).map_err(|_| Refusal::Process)?;
        let value = Self { stamp: before, fd };
        value.check(child, executable)?;
        Ok(value)
    }
    pub(super) fn check(&self, child: &mut Child, executable: &PinnedFile) -> Result<(), Refusal> {
        if child.id() != self.stamp.pid
            || child.try_wait().map_err(|_| Refusal::Process)?.is_some()
            || exited(&self.fd)?
            || current(self.stamp.pid, self.stamp.parent)? != self.stamp
        {
            return Err(Refusal::Changed);
        }
        executable.matches_proc_executable(Path::new(&format!("/proc/{}/exe", self.stamp.pid)))?;
        if exited(&self.fd)? || current(self.stamp.pid, self.stamp.parent)? != self.stamp {
            return Err(Refusal::Changed);
        }
        Ok(())
    }
    pub(super) fn stamp(&self) -> Stamp {
        self.stamp
    }
    pub(super) fn exited(&self) -> Result<bool, Refusal> {
        exited(&self.fd)
    }
    pub(super) fn kill_owned(&self) -> Result<(), Refusal> {
        if !self.exited()? {
            pidfd_send_signal(&self.fd, Signal::KILL).map_err(|_| Refusal::Process)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn stat(pid: u32, parent: u32, start: u64) -> Vec<u8> {
        let mut fields = vec!["0".to_owned(); 20];
        fields[0] = "t".into();
        fields[1] = parent.to_string();
        fields[19] = start.to_string();
        format!("{pid} (name with ) paren) {}\n", fields.join(" ")).into_bytes()
    }
    #[test]
    fn pid_parent_start_are_all_exact() {
        let good = stat(43, 42, 99);
        let x = parse_stat(&good, 43, 42).unwrap();
        assert_eq!((x.pid, x.parent, x.start), (43, 42, 99));
        assert!(parse_stat(&good, 44, 42).is_err());
        assert!(parse_stat(&good, 43, 41).is_err());
        assert!(parse_stat(&stat(43, 42, 0), 43, 42).is_err());
    }
    #[test]
    fn zombie_or_reused_stamp_not_live_owner() {
        let a = parse_stat(&stat(43, 42, 99), 43, 42).unwrap();
        let b = parse_stat(&stat(43, 42, 100), 43, 42).unwrap();
        assert_ne!(a, b);
        let x = String::from_utf8(stat(43, 42, 99))
            .unwrap()
            .replace(") t ", ") Z ");
        assert!(parse_stat(x.as_bytes(), 43, 42).is_err());
    }
    #[test]
    fn target_environment_does_not_admit_loader_or_python() {
        clean_environment(b"LC_ALL=C\0LANG=C\0").unwrap();
        for x in [
            b"LANG=C\0LC_ALL=C\0LD_PRELOAD=x\0".as_slice(),
            b"LANG=C\0",
            b"LANG=C\0LC_ALL=C",
            b"LANG=C\0LC_ALL=C\0PYTHONPATH=x\0",
        ] {
            assert!(clean_environment(x).is_err());
        }
    }
}
