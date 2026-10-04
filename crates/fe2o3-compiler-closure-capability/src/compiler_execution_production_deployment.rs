//! Application-side installed deployment provenance under the trusted local root/kernel model.
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1, CompilerExecutionClientProfileV1,
    CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionSupervisorDeploymentV1,
};
use rustix::fs::{Mode, OFlags};

use crate::compiler_execution_client_profile::{
    TrustedFileSnapshot, validate_trusted_directory, validate_trusted_file,
};
use crate::{
    CompilerExecutionClientProfileCapabilityV1,
    CompilerExecutionExternalAnchorDeploymentCapabilityV1,
    CompilerExecutionSupervisorDeploymentCapabilityV1,
};

const DIRECTORIES: [&str; 3] = ["etc", "fe2o3", "compiler-execution"];
const FILES: [(&str, usize); 3] = [
    (
        "client-profile-v1",
        COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1,
    ),
    (
        "supervisor-deployment-v1",
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1,
    ),
    (
        "anchor-deployment-v1",
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1,
    ),
];
const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// Independently admitted fixed-path production configuration and its original file custody.
///
/// Only [`Self::open`] constructs this value. Caller-created sealed capabilities cannot be
/// promoted to installed provenance. Revalidation checks the current thread's namespaces,
/// credentials, root and every installed path, not only immutable copies. This trusts the
/// local root, kernel and compatible procfs; it does not resist malicious root or host rollback.
/// It grants no signing, service launch, compiler, verification or GPU authority.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::ProductionCompilerExecutionDeploymentV1;
/// fn cloneable<T: Clone>() {}
/// cloneable::<ProductionCompilerExecutionDeploymentV1>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::ProductionCompilerExecutionDeploymentV1;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<ProductionCompilerExecutionDeploymentV1>();
/// ```
pub struct ProductionCompilerExecutionDeploymentV1 {
    profile: CompilerExecutionClientProfileCapabilityV1,
    supervisor: CompilerExecutionSupervisorDeploymentCapabilityV1,
    anchor: CompilerExecutionExternalAnchorDeploymentCapabilityV1,
    tree: InstalledTree,
    namespace: NamespaceCustody,
}

impl fmt::Debug for ProductionCompilerExecutionDeploymentV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionCompilerExecutionDeploymentV1")
            .field("policy", &self.policy().identity())
            .finish_non_exhaustive()
    }
}

impl ProductionCompilerExecutionDeploymentV1 {
    /// Opens `/etc/fe2o3/compiler-execution/{client-profile,supervisor-deployment,anchor-deployment}-v1`.
    ///
    /// The caller must be unprivileged, distinct from both service UIDs, and in a namespace
    /// with full identity UID/GID mappings. Real-root private mount deployments remain supported.
    pub fn open() -> Result<Self, String> {
        Self::open_tree(Path::new("/"), 0, 0)
    }

    fn open_tree(root: &Path, owner_uid: u32, owner_gid: u32) -> Result<Self, String> {
        let namespace = NamespaceCustody::open()?;
        let (tree, bytes) = InstalledTree::open(root, owner_uid, owner_gid)?;
        let profile = CompilerExecutionClientProfileCapabilityV1::create(
            CompilerExecutionClientProfileV1::decode(&bytes[0]).map_err(|e| e.to_string())?,
        )?;
        let supervisor = CompilerExecutionSupervisorDeploymentCapabilityV1::create(
            CompilerExecutionSupervisorDeploymentV1::decode(&bytes[1])
                .map_err(|e| e.to_string())?,
        )?;
        let anchor = CompilerExecutionExternalAnchorDeploymentCapabilityV1::create(
            CompilerExecutionExternalAnchorDeploymentV1::decode(&bytes[2])
                .map_err(|e| e.to_string())?,
        )?;
        let admitted = Self {
            profile,
            supervisor,
            anchor,
            tree,
            namespace,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    pub fn policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
        self.profile.profile().policy()
    }

    /// Rechecks original ownership, installed paths, exact bytes, namespace and policy links.
    pub fn revalidate(&self) -> Result<(), String> {
        self.namespace.revalidate()?;
        self.profile.revalidate()?;
        self.supervisor.revalidate()?;
        self.anchor.revalidate()?;
        let profile = self.profile.profile();
        let supervisor = self.supervisor.deployment();
        let anchor = self.anchor.deployment();
        if profile.supervisor_uid() != supervisor.service_uid()
            || profile.supervisor_gid() != supervisor.service_gid()
            || profile.external_anchor_service() != supervisor.external_anchor_service()
            || !supervisor.matches_policy(profile.policy())
            || !anchor.matches_supervisor_and_policy(supervisor, profile.policy())
            || self.namespace.credentials.uid == supervisor.service_uid()
            || self.namespace.credentials.uid == anchor.service().uid()
        {
            return Err(
                "production compiler deployment policy or service credentials mismatch".into(),
            );
        }
        self.tree.revalidate(&[
            profile.canonical_bytes().as_slice(),
            supervisor.canonical_bytes().as_slice(),
            anchor.canonical_bytes().as_slice(),
        ])?;
        self.namespace.revalidate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
}

fn identity(file: &File) -> Result<Identity, String> {
    let stat = file.metadata().map_err(|e| e.to_string())?;
    Ok(Identity {
        device: stat.dev(),
        inode: stat.ino(),
    })
}

struct InstalledFile {
    file: File,
    snapshot: TrustedFileSnapshot,
}
struct InstalledTree {
    root_path: PathBuf,
    directories: Vec<File>,
    files: Vec<InstalledFile>,
    owner_uid: u32,
    owner_gid: u32,
}

impl InstalledTree {
    fn open(root: &Path, owner_uid: u32, owner_gid: u32) -> Result<(Self, Vec<Vec<u8>>), String> {
        let mut directories = vec![open_directory(root)?];
        validate_trusted_directory(&directories[0], owner_uid, owner_gid, "root")?;
        for component in DIRECTORIES {
            let next = open_directory_at(directories.last().unwrap(), component)?;
            validate_trusted_directory(&next, owner_uid, owner_gid, component)?;
            directories.push(next);
        }
        let mut files = Vec::with_capacity(FILES.len());
        let mut bytes = Vec::with_capacity(FILES.len());
        for (name, len) in FILES {
            let file = open_file_at(directories.last().unwrap(), name)?;
            let snapshot = validate_trusted_file(&file, owner_uid, owner_gid, name, len)?;
            bytes.push(read_exact(&file, len)?);
            if validate_trusted_file(&file, owner_uid, owner_gid, name, len)? != snapshot {
                return Err("installed compiler configuration changed during admission".into());
            }
            files.push(InstalledFile { file, snapshot });
        }
        Ok((
            Self {
                root_path: root.to_owned(),
                directories,
                files,
                owner_uid,
                owner_gid,
            },
            bytes,
        ))
    }

    fn revalidate(&self, expected: &[&[u8]; 3]) -> Result<(), String> {
        self.revalidate_with(expected, || {})
    }

    fn revalidate_with(
        &self,
        expected: &[&[u8]; 3],
        after_reads: impl FnOnce(),
    ) -> Result<(), String> {
        let current = self.revalidate_directories()?;
        for (index, ((name, len), retained)) in FILES.iter().zip(&self.files).enumerate() {
            let installed = open_file_at(&current, name)?;
            for file in [&retained.file, &installed] {
                if validate_trusted_file(file, self.owner_uid, self.owner_gid, name, *len)?
                    != retained.snapshot
                    || read_exact(file, *len)? != expected[index]
                    || validate_trusted_file(file, self.owner_uid, self.owner_gid, name, *len)?
                        != retained.snapshot
                {
                    return Err(format!("installed compiler configuration {name} changed"));
                }
            }
        }
        // Detect ancestor or file replacement during the reads, not just before them.
        after_reads();
        let current = self.revalidate_directories()?;
        for ((name, len), retained) in FILES.iter().zip(&self.files) {
            let installed = open_file_at(&current, name)?;
            if validate_trusted_file(&installed, self.owner_uid, self.owner_gid, name, *len)?
                != retained.snapshot
            {
                return Err(format!(
                    "installed compiler configuration {name} changed after read"
                ));
            }
        }
        Ok(())
    }

    fn revalidate_directories(&self) -> Result<File, String> {
        // Reopen the ambient root as well: chroot can change resolution without changing mntns.
        let mut current = open_directory(&self.root_path)?;
        for (index, retained) in self.directories.iter().enumerate() {
            validate_trusted_directory(retained, self.owner_uid, self.owner_gid, "retained")?;
            validate_trusted_directory(&current, self.owner_uid, self.owner_gid, "current")?;
            if identity(retained)? != identity(&current)? {
                return Err("installed compiler configuration directory was replaced".into());
            }
            if let Some(component) = DIRECTORIES.get(index) {
                current = open_directory_at(&current, component)?;
            }
        }
        Ok(current)
    }
}

fn open_directory(path: &Path) -> Result<File, String> {
    rustix::fs::open(path, DIRECTORY_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|e| e.to_string())
}
fn open_directory_at(parent: &File, name: &str) -> Result<File, String> {
    rustix::fs::openat(parent, name, DIRECTORY_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|e| e.to_string())
}
fn open_file_at(parent: &File, name: &str) -> Result<File, String> {
    rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|e| e.to_string())
}
fn read_exact(file: &File, length: usize) -> Result<Vec<u8>, String> {
    let mut bytes = vec![0; length];
    file.read_exact_at(&mut bytes, 0)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Credentials {
    uid: u32,
    gid: u32,
}

impl Credentials {
    fn current() -> Result<Self, String> {
        let (mut uid, mut euid, mut suid) = (0, 0, 0);
        let (mut gid, mut egid, mut sgid) = (0, 0, 0);
        // SAFETY: both calls write three distinct initialized scalar outputs.
        if unsafe { libc::getresuid(&mut uid, &mut euid, &mut suid) } != 0
            || unsafe { libc::getresgid(&mut gid, &mut egid, &mut sgid) } != 0
            || uid == 0
            || uid != euid
            || uid != suid
            || gid != egid
            || gid != sgid
            // SAFETY: an invalid ID queries the current filesystem ID without changing it.
            || unsafe { libc::syscall(libc::SYS_setfsuid, u32::MAX) } != libc::c_long::from(uid)
            || unsafe { libc::syscall(libc::SYS_setfsgid, u32::MAX) } != libc::c_long::from(gid)
            || !capabilities_empty()?
        {
            return Err(
                "production compiler configuration requires stable unprivileged credentials".into(),
            );
        }
        Ok(Self { uid, gid })
    }
}

fn capabilities_empty() -> Result<bool, String> {
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Data {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    let mut header = Header {
        version: 0x2008_0522,
        pid: 0,
    };
    let mut data = [Data {
        effective: u32::MAX,
        permitted: u32::MAX,
        inheritable: u32::MAX,
    }; 2];
    // SAFETY: capability ABI v3 writes exactly these two repr(C) records for the calling thread.
    if unsafe { libc::syscall(libc::SYS_capget, &raw mut header, data.as_mut_ptr()) } != 0 {
        return Err("cannot inspect application capabilities".into());
    }
    // Ambient capabilities are a subset of permitted and inheritable; supplementary GPU groups
    // and the capability bounding set do not confer current authority and remain allowed.
    Ok(data
        .iter()
        .all(|v| v.effective == 0 && v.permitted == 0 && v.inheritable == 0))
}

struct NamespaceCustody {
    procfs: File,
    mount: File,
    user: File,
    credentials: Credentials,
}

impl NamespaceCustody {
    fn open() -> Result<Self, String> {
        let procfs = open_directory(Path::new("/proc"))?;
        require_filesystem(&procfs, libc::PROC_SUPER_MAGIC as _, "procfs")?;
        let mount = open_namespace(&procfs, "thread-self/ns/mnt")?;
        let user = open_namespace(&procfs, "thread-self/ns/user")?;
        let admitted = Self {
            procfs,
            mount,
            user,
            credentials: Credentials::current()?,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    fn revalidate(&self) -> Result<(), String> {
        let current_proc = open_directory(Path::new("/proc"))?;
        require_filesystem(&current_proc, libc::PROC_SUPER_MAGIC as _, "procfs")?;
        if identity(&current_proc)? != identity(&self.procfs)?
            || Credentials::current()? != self.credentials
        {
            return Err("production compiler configuration procfs or credentials changed".into());
        }
        for procfs in [&self.procfs, &current_proc] {
            for (retained, name) in [
                (&self.mount, "thread-self/ns/mnt"),
                (&self.user, "thread-self/ns/user"),
            ] {
                if identity(retained)? != identity(&open_namespace(procfs, name)?)? {
                    return Err("production compiler configuration namespace changed".into());
                }
            }
            for name in ["thread-self/uid_map", "thread-self/gid_map"] {
                let mut bytes = Vec::new();
                open_proc_map(procfs, name)?
                    .take(4097)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() > 4096 || !full_identity_mapping(&bytes) {
                    return Err(
                        "production compiler configuration rejects remapped user namespaces".into(),
                    );
                }
            }
        }
        Ok(())
    }
}

fn open_proc_map(procfs: &File, name: &str) -> Result<File, String> {
    let file = rustix::fs::openat2(
        procfs,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        rustix::fs::ResolveFlags::BENEATH
            | rustix::fs::ResolveFlags::NO_XDEV
            | rustix::fs::ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|e| e.to_string())?;
    require_filesystem(&file, libc::PROC_SUPER_MAGIC as _, "namespace mapping")?;
    Ok(file)
}

fn full_identity_mapping(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut fields = text.split_ascii_whitespace().map(str::parse::<u64>);
    matches!(
        (fields.next(), fields.next(), fields.next(), fields.next()),
        (Some(Ok(0)), Some(Ok(0)), Some(Ok(4_294_967_295)), None)
    )
}

fn require_filesystem(file: &File, magic: i64, label: &str) -> Result<(), String> {
    let stat = rustix::fs::fstatfs(file).map_err(|e| e.to_string())?;
    if stat.f_type as i64 != magic {
        return Err(format!("unexpected {label} filesystem"));
    }
    Ok(())
}

fn open_namespace(procfs: &File, name: &str) -> Result<File, String> {
    // Procfs namespace magic links must be followed; ordinary files are rejected by NSFS type.
    let file = rustix::fs::openat(
        procfs,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|e| e.to_string())?;
    require_filesystem(&file, libc::NSFS_MAGIC as _, "namespace")?;
    Ok(file)
}

#[cfg(test)]
mod tests;
