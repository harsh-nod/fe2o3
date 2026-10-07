use rustix::fs::{Mode, OFlags};
use std::{fs::File, io, os::unix::fs::MetadataExt};

fn require(value: bool, reason: &'static str) -> io::Result<()> {
    if value {
        Ok(())
    } else {
        Err(io::Error::other(reason))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Snapshot {
    dev: u64,
    ino: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    len: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}
fn snapshot(file: &File) -> io::Result<Snapshot> {
    let m = file.metadata()?;
    Ok(Snapshot {
        dev: m.dev(),
        ino: m.ino(),
        mode: m.mode(),
        uid: m.uid(),
        gid: m.gid(),
        links: m.nlink(),
        len: m.len(),
        mtime: m.mtime(),
        mtime_ns: m.mtime_nsec(),
        ctime: m.ctime(),
        ctime_ns: m.ctime_nsec(),
    })
}
fn check(file: &File, directory: bool, mode: u32, len: u64) -> io::Result<Snapshot> {
    let s = snapshot(file)?;
    require(
        s.uid == 0 && s.gid == 0,
        "installed object is not root owned",
    )?;
    require(
        if directory {
            s.mode & libc::S_IFMT == libc::S_IFDIR && s.mode & 0o022 == 0 && s.mode & 0o100 != 0
        } else {
            s.mode == libc::S_IFREG | mode && s.links == 1 && s.len == len
        },
        "installed object metadata differs",
    )?;
    let mut attributes = [0u8; 1];
    require(
        rustix::fs::flistxattr(file, &mut attributes)? == 0,
        "installed object has extended attributes",
    )?;
    require(
        rustix::io::fcntl_getfd(file)? == rustix::io::FdFlags::CLOEXEC
            && rustix::fs::fcntl_getfl(file)?
                & (OFlags::ACCMODE
                    | OFlags::PATH
                    | OFlags::APPEND
                    | OFlags::ASYNC
                    | OFlags::DIRECT
                    | OFlags::NONBLOCK)
                == OFlags::RDONLY
                    | if directory {
                        OFlags::empty()
                    } else {
                        OFlags::NONBLOCK
                    },
        "installed descriptor flags differ",
    )?;
    Ok(s)
}

/// Retained root-owned path edges and exact read-only leaf metadata.
///
/// This generic filesystem observation does not approve a service or a deployment.
/// Security-sensitive callers must select their own fixed independently installed path.
pub struct RootInstalledFileV1 {
    path: &'static str,
    handles: Vec<File>,
    snapshots: Vec<Snapshot>,
    mode: u32,
    len: u64,
}
impl RootInstalledFileV1 {
    /// Opens a canonical absolute path through root-owned non-writable directories.
    /// Requires one regular root-owned leaf, no attributes, and exact mode and length.
    pub fn open(path: &'static str, mode: u32, len: u64) -> io::Result<Self> {
        require(
            path.starts_with('/')
                && path
                    .split('/')
                    .skip(1)
                    .all(|part| !part.is_empty() && part != "." && part != ".."),
            "installed path must be absolute and canonical",
        )?;
        let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
        let root = File::from(rustix::fs::open(
            "/",
            flags | OFlags::DIRECTORY,
            Mode::empty(),
        )?);
        let mut snapshots = vec![check(&root, true, 0, 0)?];
        let mut handles = vec![root];
        let parts: Vec<_> = path.split('/').filter(|part| !part.is_empty()).collect();
        for (index, part) in parts.iter().enumerate() {
            let directory = index + 1 < parts.len();
            let file = File::from(rustix::fs::openat(
                handles.last().unwrap(),
                *part,
                flags
                    | if directory {
                        OFlags::DIRECTORY
                    } else {
                        OFlags::NONBLOCK
                    },
                Mode::empty(),
            )?);
            snapshots.push(check(&file, directory, mode, len)?);
            handles.push(file);
        }
        Ok(Self {
            path,
            handles,
            snapshots,
            mode,
            len,
        })
    }
    /// Borrows the original read-only, close-on-exec leaf, not a reopened pathname.
    pub fn leaf(&self) -> &File {
        self.handles.last().unwrap()
    }
    /// Rechecks original objects and every current path edge against retained identities.
    pub fn revalidate(&self) -> io::Result<()> {
        let reopened = Self::open(self.path, self.mode, self.len)?;
        for (index, ((old, expected), actual)) in self
            .handles
            .iter()
            .zip(&self.snapshots)
            .zip(&reopened.snapshots)
            .enumerate()
        {
            let directory = index + 1 < self.handles.len();
            let retained = check(old, directory, self.mode, self.len)?;
            // Directory entries may change; the original path, ownership and mode may not.
            let same = |s: &Snapshot| (s.dev, s.ino, s.mode, s.uid, s.gid);
            require(
                if directory {
                    same(&retained) == same(expected) && same(actual) == same(expected)
                } else {
                    retained == *expected && actual == expected
                },
                "installed path or original object changed",
            )?;
        }
        Ok(())
    }
}
