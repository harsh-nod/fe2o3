//! Root-only, fresh per-controller scope. Never recover or kill an unowned name.

use crate::{other, require};
use fe2o3_protected_service_spawn::RootOwnedProofControllerChildV1;
use rustix::fs::{AtFlags, Mode, OFlags, ResolveFlags};
use std::{
    fs::File,
    io::{self, Read, Write},
    os::unix::fs::MetadataExt,
    time::{Duration, Instant},
};

const CGROUP2_MAGIC: i64 = 0x6367_7270;
const CONTROL_LIMIT: u64 = 4096;
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Eq, PartialEq)]
struct Identity(u64, u64);
fn raw_identity(file: &File) -> io::Result<Identity> {
    let m = file.metadata()?;
    Ok(Identity(m.dev(), m.ino()))
}
fn identity(file: &File) -> io::Result<Identity> {
    let m = file.metadata()?;
    require(
        m.is_dir() && m.uid() == 0 && m.gid() == 0 && m.mode() & 0o022 == 0,
        "cgroup directory is not exclusively root controlled",
    )?;
    require(
        rustix::fs::fstatfs(file)?.f_type == CGROUP2_MAGIC,
        "not a cgroup2 filesystem",
    )?;
    Ok(Identity(m.dev(), m.ino()))
}
fn open(parent: &File, name: &str, flags: OFlags) -> io::Result<File> {
    Ok(rustix::fs::openat2(
        parent,
        name,
        flags | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )?
    .into())
}
fn read(file: File) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    file.take(CONTROL_LIMIT + 1).read_to_end(&mut bytes)?;
    require(
        bytes.len() <= CONTROL_LIMIT as usize,
        "oversized cgroup control",
    )?;
    Ok(bytes)
}
fn read_control(parent: &File, name: &str) -> io::Result<Vec<u8>> {
    read(open(parent, name, OFlags::RDONLY)?)
}
fn write_control(parent: &File, name: &str, bytes: &[u8]) -> io::Result<()> {
    let mut file = open(parent, name, OFlags::WRONLY)?;
    let m = file.metadata()?;
    require(
        m.is_file() && m.uid() == 0 && m.gid() == 0 && m.mode() & 0o022 == 0,
        "cgroup control is not exclusively root controlled",
    )?;
    require(
        file.write(bytes)? == bytes.len(),
        "partial cgroup control write",
    )
}
fn membership() -> io::Result<Vec<u8>> {
    read(File::open("/proc/thread-self/cgroup")?)
}

fn relative_path(bytes: &[u8]) -> io::Result<&str> {
    let text = std::str::from_utf8(bytes).map_err(other)?;
    let path = text
        .strip_prefix("0::/")
        .and_then(|s| s.strip_suffix('\n'))
        .ok_or_else(|| io::Error::other("not one unified cgroup membership"))?;
    require(
        !path.contains(['\n', '\r', '\0'])
            && (path.is_empty()
                || path
                    .split('/')
                    .all(|s| !s.is_empty() && s != "." && s != "..")),
        "noncanonical cgroup membership",
    )?;
    Ok(if path.is_empty() { "." } else { path })
}

fn populated(bytes: &[u8]) -> io::Result<bool> {
    let text = std::str::from_utf8(bytes).map_err(other)?;
    let mut populated = None;
    let mut frozen = None;
    for line in text.lines() {
        let (key, value) = line
            .split_once(' ')
            .ok_or_else(|| io::Error::other("malformed cgroup events"))?;
        let slot = match key {
            "populated" => &mut populated,
            "frozen" => &mut frozen,
            _ => return Err(io::Error::other("unknown cgroup event")),
        };
        require(
            slot.is_none() && matches!(value, "0" | "1"),
            "duplicate or invalid cgroup event",
        )?;
        *slot = Some(value == "1");
    }
    require(
        frozen == Some(false),
        "cgroup is frozen or omitted frozen state",
    )?;
    populated.ok_or_else(|| io::Error::other("cgroup omitted populated state"))
}

pub(crate) struct Scope {
    root: File,
    root_identity: Identity,
    parent: File,
    parent_identity: Identity,
    scope: Option<File>,
    scope_identity: Option<Identity>,
    created: bool,
    membership: Vec<u8>,
    name: String,
    expected_child: Vec<u8>,
}
impl Scope {
    pub(crate) fn create() -> io::Result<Self> {
        let before = membership()?;
        let relative = relative_path(&before)?;
        let root: File = rustix::fs::open(
            "/sys/fs/cgroup",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?
        .into();
        let root_identity = identity(&root)?;
        let parent = open(&root, relative, OFlags::RDONLY | OFlags::DIRECTORY)?;
        let parent_identity = identity(&parent)?;
        require(
            read_control(&parent, "cgroup.type")? == b"domain\n",
            "cgroup parent is not a domain",
        )?;
        require(before == membership()?, "launcher cgroup changed")?;
        let nonce = crate::wire::nonce()?;
        let mut name = String::from("fe2o3-proof-");
        use std::fmt::Write as _;
        for byte in nonce {
            write!(&mut name, "{byte:02x}").unwrap();
        }
        let parent_path = if relative == "." {
            String::new()
        } else {
            format!("{relative}/")
        };
        let expected_child = format!("0::/{parent_path}{name}\n").into_bytes();
        let mut value = Self {
            root,
            root_identity,
            parent,
            parent_identity,
            scope: None,
            scope_identity: None,
            created: false,
            membership: before,
            name,
            expected_child,
        };
        rustix::fs::mkdirat(
            &value.parent,
            value.name.as_str(),
            Mode::from_raw_mode(0o755),
        )?;
        value.created = true;
        // Own the name immediately. Capture raw identity before any semantic rejection.
        value.scope = Some(open(
            &value.parent,
            &value.name,
            OFlags::RDONLY | OFlags::DIRECTORY,
        )?);
        value.scope_identity = Some(raw_identity(value.scope())?);
        value.revalidate()?;
        require(
            read_control(value.scope(), "cgroup.type")? == b"domain\n"
                && read_control(value.scope(), "cgroup.procs")?.is_empty()
                && !populated(&read_control(value.scope(), "cgroup.events")?)?,
            "new cgroup is not empty",
        )?;
        let kill = open(value.scope(), "cgroup.kill", OFlags::WRONLY)?;
        let m = kill.metadata()?;
        require(
            m.is_file() && m.uid() == 0 && m.gid() == 0 && m.mode() & 0o022 == 0,
            "cgroup kill control is not exclusively root owned",
        )?;
        Ok(value)
    }
    fn scope(&self) -> &File {
        self.scope.as_ref().expect("retained active scope")
    }
    fn revalidate(&self) -> io::Result<()> {
        require(membership()? == self.membership, "launcher cgroup changed")?;
        let root: File = rustix::fs::open(
            "/sys/fs/cgroup",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?
        .into();
        require(
            identity(&root)? == self.root_identity && identity(&self.root)? == self.root_identity,
            "cgroup root changed",
        )?;
        let parent = open(
            &root,
            relative_path(&self.membership)?,
            OFlags::RDONLY | OFlags::DIRECTORY,
        )?;
        require(
            identity(&parent)? == self.parent_identity
                && identity(&self.parent)? == self.parent_identity,
            "cgroup parent changed",
        )?;
        let scope = open(&parent, &self.name, OFlags::RDONLY | OFlags::DIRECTORY)?;
        require(
            Some(identity(&scope)?) == self.scope_identity
                && Some(identity(self.scope())?) == self.scope_identity,
            "owned cgroup path changed",
        )
    }
    pub(crate) fn attach(&self, child: &RootOwnedProofControllerChildV1) -> io::Result<()> {
        self.revalidate()?;
        require(child.is_live().map_err(other)?, "gated controller is dead")?;
        require(
            read_control(self.scope(), "cgroup.procs")?.is_empty(),
            "scope already occupied",
        )?;
        let pid = child.pid().as_raw_pid();
        let pid_line = format!("{pid}\n");
        write_control(self.scope(), "cgroup.procs", pid_line.as_bytes())?;
        let observed = read(File::open(format!("/proc/{pid}/cgroup"))?)?;
        require(
            child.is_live().map_err(other)?
                && observed == self.expected_child
                && read_control(self.scope(), "cgroup.procs")? == pid_line.as_bytes()
                && populated(&read_control(self.scope(), "cgroup.events")?)?,
            "controller attachment did not bind exact scope",
        )?;
        self.revalidate()
    }
    pub(crate) fn kill(&self) -> io::Result<()> {
        self.revalidate()?;
        write_control(self.scope(), "cgroup.kill", b"1\n")
    }
    pub(crate) fn remove_empty(&mut self) -> io::Result<()> {
        self.revalidate()?;
        let deadline = Instant::now() + CLEANUP_TIMEOUT;
        while populated(&read_control(self.scope(), "cgroup.events")?)? {
            require(Instant::now() < deadline, "cgroup failed to become empty")?;
            std::thread::sleep(Duration::from_millis(10));
        }
        self.revalidate()?;
        self.remove_original_empty()
    }
    fn remove_original_empty(&mut self) -> io::Result<()> {
        let expected = self
            .scope_identity
            .ok_or_else(|| io::Error::other("scope identity unavailable"))?;
        let retained = self
            .scope
            .as_ref()
            .ok_or_else(|| io::Error::other("scope handle unavailable"))?;
        let current = open(&self.parent, &self.name, OFlags::RDONLY | OFlags::DIRECTORY)?;
        require(
            raw_identity(retained)? == expected && raw_identity(&current)? == expected,
            "cannot remove a substituted scope",
        )?;
        require(
            read_control(retained, "cgroup.procs")?.is_empty()
                && !populated(&read_control(retained, "cgroup.events")?)?,
            "cannot remove a populated scope",
        )?;
        rustix::fs::unlinkat(&self.parent, self.name.as_str(), AtFlags::REMOVEDIR)?;
        self.created = false;
        self.scope = None;
        Ok(())
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        // If initial open/identity capture failed, never guess which directory to remove.
        // A manager-level cgroup remains the death-containment backstop for fail-stop paths.
        if self.created && self.remove_original_empty().is_err() {
            std::process::abort();
        }
    }
}

/// Owns ordering on every path, including panic and post-attachment admission failure.
pub(crate) struct ContainedChild {
    pub(crate) child: Option<RootOwnedProofControllerChildV1>,
    scope: Option<Scope>,
}
impl ContainedChild {
    pub(crate) fn new(scope: Scope) -> Self {
        Self {
            child: None,
            scope: Some(scope),
        }
    }
    pub(crate) fn install(&mut self, child: RootOwnedProofControllerChildV1) {
        self.child = Some(child);
    }
    pub(crate) fn child(&self) -> &RootOwnedProofControllerChildV1 {
        self.child.as_ref().expect("installed child")
    }
    pub(crate) fn attach(&self) -> io::Result<()> {
        self.scope.as_ref().unwrap().attach(self.child())
    }
    pub(crate) fn stop(&mut self) -> io::Result<()> {
        if let Some(scope) = &self.scope {
            scope.kill()?;
        }
        if let Some(child) = &mut self.child {
            child.cancel_and_reap().map_err(other)?;
        }
        if let Some(scope) = &mut self.scope {
            scope.remove_empty()?;
        }
        self.child = None;
        self.scope = None;
        Ok(())
    }
}
impl Drop for ContainedChild {
    fn drop(&mut self) {
        // Never return from Drop while releasing deployment owners with an uncontained tree.
        // The independently deployed manager must additionally use whole-cgroup kill on death.
        if self.stop().is_err() {
            std::process::abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn membership_is_exact_and_cannot_escape_root() {
        assert_eq!(relative_path(b"0::/\n").unwrap(), ".");
        assert_eq!(relative_path(b"0::/init.scope\n").unwrap(), "init.scope");
        for text in [
            "0::/../bad\n",
            "0:://bad\n",
            "0::/a/\n",
            "0::/a\n0::/b\n",
            "1:cpu:/\n",
        ] {
            assert!(relative_path(text.as_bytes()).is_err(), "{text}");
        }
    }
    #[test]
    fn events_reject_missing_duplicate_frozen_and_unknown_state() {
        assert!(!populated(b"populated 0\nfrozen 0\n").unwrap());
        assert!(populated(b"populated 1\nfrozen 0\n").unwrap());
        for text in [
            "populated 0\n",
            "populated 0\nfrozen 1\n",
            "populated 0\npopulated 1\nfrozen 0\n",
            "populated 0\nfrozen 0\nother 0\n",
        ] {
            assert!(populated(text.as_bytes()).is_err());
        }
    }
}
