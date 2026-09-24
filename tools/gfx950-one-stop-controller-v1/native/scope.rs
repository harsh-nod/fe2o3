//! Current membership only. The separate outer manager/subreaper proves family
//! generation and cleanup; this module never signals or claims preexisting actors.
use super::{
    clock::Clock,
    config::Options,
    custody::{self, Stamp},
    parent::ParentExecutableObservation,
};
use fe2o3_private_one_stop_protocol::Refusal;
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
const PREFIX: &str =
    "/user.slice/user-9661.slice/user@9661.service/app.slice/fe2o3-one-stop-credential-one-stop-";
fn lower_hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn scope_name(raw: &[u8]) -> Result<(String, String), Refusal> {
    let s = std::str::from_utf8(raw).map_err(|_| Refusal::Shape)?;
    let cg = s
        .strip_prefix("0::")
        .and_then(|x| x.strip_suffix('\n'))
        .ok_or(Refusal::Shape)?;
    let nonce = cg
        .strip_prefix(PREFIX)
        .and_then(|x| x.strip_suffix(".scope/workload"))
        .ok_or(Refusal::Process)?;
    if !lower_hex(nonce, 32) {
        return Err(Refusal::Process);
    }
    Ok((cg.to_owned(), nonce.to_owned()))
}
fn cgroup(pid: u32) -> Result<Vec<u8>, Refusal> {
    custody::read_bounded(format!("/proc/{pid}/cgroup"), 4096)
}
fn self_parent() -> Result<u32, Refusal> {
    let raw = custody::read_bounded("/proc/self/stat", 4096)?;
    let text = std::str::from_utf8(&raw).map_err(|_| Refusal::Shape)?;
    let close = text.rfind(") ").ok_or(Refusal::Shape)?;
    let parent = text[close + 2..]
        .split_ascii_whitespace()
        .nth(1)
        .ok_or(Refusal::Shape)?;
    u32::try_from(super::config::decimal(parent.as_bytes())?).map_err(|_| Refusal::Process)
}
fn generation(pid: u32) -> Result<String, Refusal> {
    let raw = custody::read_bounded(format!("/proc/{pid}/environ"), 4096)?;
    let mut values = raw
        .split(|b| *b == 0)
        .filter_map(|s| s.strip_prefix(b"INVOCATION_ID="));
    let id =
        std::str::from_utf8(values.next().ok_or(Refusal::Process)?).map_err(|_| Refusal::Shape)?;
    if !lower_hex(id, 32) || values.next().is_some() {
        return Err(Refusal::Process);
    }
    Ok(id.to_owned())
}
fn credentials(pid: u32) -> Result<Vec<u8>, Refusal> {
    let raw = custody::read_bounded(format!("/proc/{pid}/status"), 16384)?;
    let mut result = Vec::with_capacity(512);
    let mut seen = [false; 3];
    for line in raw.split(|b| *b == b'\n') {
        for (i, prefix) in [b"Uid:".as_slice(), b"Gid:", b"Groups:"].iter().enumerate() {
            if line.starts_with(prefix) {
                if seen[i] || line.len() > 256 {
                    return Err(Refusal::Process);
                }
                seen[i] = true;
                if i == 0 {
                    let s = std::str::from_utf8(&line[4..]).map_err(|_| Refusal::Shape)?;
                    if s.split_ascii_whitespace().ne(["9661"; 4]) {
                        return Err(Refusal::Process);
                    }
                }
                if result.len() + line.len() + 1 > 512 {
                    return Err(Refusal::Bound);
                }
                result.extend_from_slice(line);
                result.push(b'\n');
            }
        }
    }
    if seen != [true; 3] {
        return Err(Refusal::Process);
    }
    Ok(result)
}
pub(super) struct Scope {
    process: Stamp,
    parent: Stamp,
    parent_exe: ParentExecutableObservation,
    relative: String,
    nonce: String,
    generation: String,
    directory: File,
    directory_path: PathBuf,
    directory_identity: (u64, u64),
    credentials: Vec<u8>,
}
impl Scope {
    pub(super) fn observe(options: &Options, clock: Clock) -> Result<Self, Refusal> {
        clock.check()?;
        let pid = std::process::id();
        let parent = self_parent()?;
        let process = custody::current(pid, parent)?;
        let raw = custody::read_bounded(format!("/proc/{parent}/stat"), 4096)?;
        let text = std::str::from_utf8(&raw).map_err(|_| Refusal::Shape)?;
        let close = text.rfind(") ").ok_or(Refusal::Shape)?;
        let pp = u32::try_from(super::config::decimal(
            text[close + 2..]
                .split_ascii_whitespace()
                .nth(1)
                .ok_or(Refusal::Shape)?
                .as_bytes(),
        )?)
        .map_err(|_| Refusal::Shape)?;
        let parent = custody::current(parent, pp)?;
        let (relative, nonce) = scope_name(&cgroup(pid)?)?;
        let path = Path::new("/sys/fs/cgroup").join(relative.trim_start_matches('/'));
        if fs::canonicalize(&path).ok().as_ref() != Some(&path) {
            return Err(Refusal::Changed);
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|_| Refusal::Process)?;
        let meta = directory.metadata().map_err(|_| Refusal::Process)?;
        let generation = generation(parent.pid)?;
        if generation == nonce {
            return Err(Refusal::Process);
        }
        let parent_exe =
            ParentExecutableObservation::observe(options.scope_owner_path, parent, clock)?;
        let value = Self {
            process,
            parent,
            parent_exe,
            relative,
            nonce,
            generation,
            directory,
            directory_path: path,
            directory_identity: (meta.dev(), meta.ino()),
            credentials: credentials(pid)?,
        };
        value.current(clock)?;
        Ok(value)
    }
    pub(super) fn current(&self, clock: Clock) -> Result<(), Refusal> {
        clock.check()?;
        if custody::current(self.process.pid, self.parent.pid)? != self.process
            || custody::current(self.parent.pid, self.parent.parent)? != self.parent
            || scope_name(&cgroup(self.process.pid)?)?
                != (self.relative.clone(), self.nonce.clone())
            || generation(self.parent.pid)? != self.generation
            || credentials(self.process.pid)? != self.credentials
            || credentials(self.parent.pid)? != self.credentials
        {
            return Err(Refusal::Changed);
        }
        let root = self
            .relative
            .strip_suffix("/workload")
            .ok_or(Refusal::Process)?;
        if cgroup(self.parent.pid)? != format!("0::{root}/supervisor\n").as_bytes() {
            return Err(Refusal::Changed);
        }
        self.parent_exe.current(clock)?;
        let argv = custody::read_bounded(format!("/proc/{}/cmdline", self.parent.pid), 4096)?;
        let fields = argv.split(|b| *b == 0).collect::<Vec<_>>();
        if fields.len() != 6
            || fields[0] != self.parent_exe.path().as_os_str().as_encoded_bytes()
            || fields[1] != b"inner"
            || fields[2] != b"one-stop"
            || fields[3] != self.nonce.as_bytes()
            || !fields[5].is_empty()
            || !std::str::from_utf8(fields[4]).is_ok_and(|s| lower_hex(s, 64))
        {
            return Err(Refusal::Process);
        }
        let m = self.directory.metadata().map_err(|_| Refusal::Process)?;
        let current = fs::symlink_metadata(&self.directory_path).map_err(|_| Refusal::Changed)?;
        if !m.is_dir()
            || !current.is_dir()
            || (m.dev(), m.ino()) != self.directory_identity
            || (current.dev(), current.ino()) != self.directory_identity
            || fs::canonicalize(&self.directory_path).ok().as_ref() != Some(&self.directory_path)
        {
            return Err(Refusal::Changed);
        }
        clock.check()
    }
    pub(super) fn rehash(&self, clock: Clock) -> Result<(), Refusal> {
        self.current(clock)?;
        self.parent_exe.rehash(clock)?;
        self.current(clock)
    }
    pub(super) fn member(&self, pid: u32) -> Result<(), Refusal> {
        if cgroup(pid)? != format!("0::{}\n", self.relative).as_bytes() {
            return Err(Refusal::Changed);
        }
        Ok(())
    }
    pub(super) fn relative(&self) -> &str {
        &self.relative
    }
    pub(super) fn invocation(&self) -> &str {
        &self.generation
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_new_scope_only_and_no_claim_for_old_scopes() {
        let s = format!("0::{PREFIX}{}.scope/workload\n", "a".repeat(32));
        assert!(scope_name(s.as_bytes()).is_ok());
        for x in [
            s.replace("one-stop-credential", "empty-queue-credential"),
            s.replace("/workload", "/supervisor"),
            s.replace(&"a".repeat(32), &"a".repeat(31)),
            format!("{s}{s}"),
            s.replace("user-9661", "user-0"),
        ] {
            assert!(scope_name(x.as_bytes()).is_err());
        }
    }
}
