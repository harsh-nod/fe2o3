use crate::artifact::PublishedHostArtifactV1;
use crate::digest::{Sha256Digest, sha256_bytes};
use crate::error::{HostLinkError, HostLinkErrorCodeV1, ResultContext};
use crate::model::{
    ArtifactProvenanceV1, ProducerArtifactSpecV1, ReleaseNonceV1, RootInputKindV1, TargetTripleV1,
    validate_ascii_token, validate_relative_path,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Component, Path, PathBuf};

const MAX_ROOT_ENTRIES: u64 = 100_000;
const MAX_ROOT_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ROOT_NAME_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 50_000;
const MAX_ROOT_DEPTH: usize = 64;
const SNAPSHOT_DOMAIN: &[u8] = b"fe2o3-host-link-fixed-root-v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RootIdentity {
    device: u64,
    inode: u64,
    mode: u32,
}

pub struct FixedRootV1 {
    label: String,
    diagnostic_path: PathBuf,
    locator_parent: File,
    locator_name: PathBuf,
    file: File,
    identity: RootIdentity,
    tree_digest: Sha256Digest,
    journal: MutationJournal,
}

impl FixedRootV1 {
    pub fn open(label: impl Into<String>, path: impl AsRef<Path>) -> Result<Self, HostLinkError> {
        let label = label.into();
        validate_ascii_token("fixed-root label", &label, 128)?;
        let path = path.as_ref();
        validate_absolute_root_path(path)?;
        let (locator_parent, locator_name, file) = open_root_locator(path)?;
        let identity = root_identity(&file)?;
        let journal = MutationJournal::new()?;
        let tree_digest = snapshot_root(&file, Some(&journal))?;
        let confirmation = snapshot_root(&file, None)?;
        journal.ensure_clean()?;
        if tree_digest != confirmation {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::RootMutation,
                format!("fixed root {label} changed while it was admitted"),
            ));
        }
        Ok(Self {
            label,
            diagnostic_path: path.to_path_buf(),
            locator_parent,
            locator_name,
            file,
            identity,
            tree_digest,
            journal,
        })
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn tree_digest(&self) -> Sha256Digest {
        self.tree_digest
    }

    pub fn diagnostic_path(&self) -> &Path {
        &self.diagnostic_path
    }

    pub(crate) fn identity_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(80);
        bytes.extend_from_slice(&self.identity.device.to_le_bytes());
        bytes.extend_from_slice(&self.identity.inode.to_le_bytes());
        bytes.extend_from_slice(&self.identity.mode.to_le_bytes());
        bytes.extend_from_slice(&self.journal.identity_bytes());
        bytes
    }

    pub fn revalidate(&self) -> Result<(), HostLinkError> {
        self.journal.ensure_clean()?;
        if root_identity(&self.file)? != self.identity {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::RootChanged,
                format!("retained fixed-root identity changed for {}", self.label),
            ));
        }
        let reopened =
            open_root_beneath(&self.locator_parent, &self.locator_name).map_err(|error| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::RootChanged,
                    format!(
                        "fixed-root locator {} no longer resolves to its retained object: {error}",
                        self.diagnostic_path.display()
                    ),
                )
            })?;
        if root_identity(&reopened)? != self.identity {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::RootChanged,
                format!("fixed-root locator was replaced for {}", self.label),
            ));
        }
        let observed = snapshot_root(&self.file, None)?;
        self.journal.ensure_clean()?;
        if observed != self.tree_digest {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::RootChanged,
                format!("fixed-root contents changed for {}", self.label),
            ));
        }
        Ok(())
    }

    pub(crate) fn revalidate_after_execution(&self) -> Result<(), HostLinkError> {
        self.journal.ensure_clean()?;
        if root_identity(&self.file)? != self.identity {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::RootChanged,
                format!("retained fixed-root identity changed for {}", self.label),
            ));
        }
        let reopened =
            open_root_beneath(&self.locator_parent, &self.locator_name).map_err(|error| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::RootChanged,
                    format!(
                        "fixed-root locator {} changed during execution: {error}",
                        self.diagnostic_path.display()
                    ),
                )
            })?;
        if root_identity(&reopened)? != self.identity {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::RootChanged,
                format!("fixed-root locator was replaced for {}", self.label),
            ));
        }
        self.journal.ensure_clean()
    }

    pub(crate) fn try_open_artifact(
        &self,
        relative_path: &[u8],
        kind: RootInputKindV1,
        release_nonce: ReleaseNonceV1,
        target: TargetTripleV1,
    ) -> Result<Option<PublishedHostArtifactV1>, HostLinkError> {
        validate_relative_path(relative_path)?;
        self.journal.ensure_clean()?;
        let relative_text = std::str::from_utf8(relative_path).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::InvalidPath,
                "fixed-root path is not UTF-8",
            )
        })?;
        let source = match open_beneath(&self.file, Path::new(relative_text)) {
            Ok(file) => file,
            Err(error) if error.code() == HostLinkErrorCodeV1::UnresolvedSearch => {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let label_digest = sha256_bytes(
            &[
                self.label.as_bytes(),
                b"\0",
                relative_path,
                b"\0",
                &[kind as u8],
            ]
            .concat(),
        );
        let spec = ProducerArtifactSpecV1::new(
            format!("root-{}", &label_digest.to_hex()[..24]),
            kind.artifact_kind(),
            ArtifactProvenanceV1::FixedRoot,
            release_nonce,
            target,
        )?;
        let artifact = PublishedHostArtifactV1::from_producer_fd(source, spec)?;
        self.journal.ensure_clean()?;
        Ok(Some(artifact))
    }
}

pub struct FixedRootSetV1 {
    roots: BTreeMap<String, FixedRootV1>,
}

impl FixedRootSetV1 {
    pub fn new(roots: Vec<FixedRootV1>) -> Result<Self, HostLinkError> {
        let mut map = BTreeMap::new();
        for root in roots {
            let label = root.label.clone();
            if map.insert(label.clone(), root).is_some() {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::DuplicateRecord,
                    format!("duplicate fixed-root label {label}"),
                ));
            }
        }
        Ok(Self { roots: map })
    }

    pub fn get(&self, label: &str) -> Option<&FixedRootV1> {
        self.roots.get(label)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &FixedRootV1)> {
        self.roots
            .iter()
            .map(|(label, root)| (label.as_str(), root))
    }

    pub fn revalidate(&self) -> Result<(), HostLinkError> {
        for root in self.roots.values() {
            root.revalidate()?;
        }
        Ok(())
    }
}

fn validate_absolute_root_path(path: &Path) -> Result<(), HostLinkError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidPath,
            "fixed-root locator must be an absolute path with only normal components",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_root_locator(path: &Path) -> Result<(File, PathBuf, File), HostLinkError> {
    let parent_path = path.parent().ok_or_else(|| {
        HostLinkError::new(
            HostLinkErrorCodeV1::InvalidPath,
            "fixed-root locator must name a child beneath a retained parent",
        )
    })?;
    let name = path.file_name().ok_or_else(|| {
        HostLinkError::new(
            HostLinkErrorCodeV1::InvalidPath,
            "the filesystem root cannot be a V1 fixed-root locator",
        )
    })?;
    let locator_name = PathBuf::from(name);
    let parent = open_directory_path(parent_path)?;
    let root = open_root_beneath(&parent, &locator_name)?;
    Ok((parent, locator_name, root))
}

#[cfg(not(target_os = "linux"))]
fn open_root_locator(_path: &Path) -> Result<(File, PathBuf, File), HostLinkError> {
    Err(HostLinkError::new(
        HostLinkErrorCodeV1::UnsupportedPlatform,
        "fixed-root opening requires Linux openat2",
    ))
}

#[cfg(target_os = "linux")]
fn open_directory_path(path: &Path) -> Result<File, HostLinkError> {
    use rustix::fs::{Mode, OFlags, ResolveFlags};
    rustix::fs::openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|error| {
        let code = if matches!(error, rustix::io::Errno::LOOP | rustix::io::Errno::XDEV) {
            HostLinkErrorCodeV1::Symlink
        } else {
            HostLinkErrorCodeV1::Io
        };
        HostLinkError::new(
            code,
            format!(
                "open fixed root without symlinks {}: {error}",
                path.display()
            ),
        )
    })
}

#[cfg(target_os = "linux")]
fn open_root_beneath(parent: &File, name: &Path) -> Result<File, HostLinkError> {
    use rustix::fs::{Mode, OFlags, ResolveFlags};
    rustix::fs::openat2(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|error| {
        let code = if matches!(error, rustix::io::Errno::LOOP | rustix::io::Errno::XDEV) {
            HostLinkErrorCodeV1::Symlink
        } else {
            HostLinkErrorCodeV1::Io
        };
        HostLinkError::new(
            code,
            format!(
                "open fixed root beneath retained locator parent {}: {error}",
                name.display()
            ),
        )
    })
}

#[cfg(target_os = "linux")]
fn open_beneath(directory: &File, path: &Path) -> Result<File, HostLinkError> {
    use rustix::fs::{Mode, OFlags, ResolveFlags};
    rustix::fs::openat2(
        directory,
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|error| {
        let code = if error == rustix::io::Errno::NOENT {
            HostLinkErrorCodeV1::UnresolvedSearch
        } else if matches!(error, rustix::io::Errno::LOOP | rustix::io::Errno::XDEV) {
            HostLinkErrorCodeV1::Symlink
        } else {
            HostLinkErrorCodeV1::Io
        };
        HostLinkError::new(
            code,
            format!(
                "open fixed-root input beneath retained descriptor {}: {error}",
                path.display()
            ),
        )
    })
}

#[cfg(not(target_os = "linux"))]
fn open_beneath(_directory: &File, _path: &Path) -> Result<File, HostLinkError> {
    Err(HostLinkError::new(
        HostLinkErrorCodeV1::UnsupportedPlatform,
        "fixed-root input opening requires Linux openat2",
    ))
}

#[cfg(unix)]
fn root_identity(file: &File) -> Result<RootIdentity, HostLinkError> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata().context(HostLinkErrorCodeV1::Io, || {
        "inspect retained fixed-root descriptor".to_owned()
    })?;
    if !metadata.file_type().is_dir() {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NotRegular,
            "fixed-root descriptor is not a directory",
        ));
    }
    Ok(RootIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode() & 0o7777,
    })
}

#[cfg(not(unix))]
fn root_identity(_file: &File) -> Result<RootIdentity, HostLinkError> {
    Err(HostLinkError::new(
        HostLinkErrorCodeV1::UnsupportedPlatform,
        "fixed-root identity requires Unix metadata",
    ))
}

#[cfg(target_os = "linux")]
struct MutationJournal {
    descriptor: std::os::fd::OwnedFd,
    procfs: ProcfsCapabilityV1,
}

#[cfg(target_os = "linux")]
const PROC_SUPER_MAGIC: u64 = 0x0000_9fa0;

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcObjectIdentityV1 {
    device: u64,
    inode: u64,
    mode: u32,
}

#[cfg(target_os = "linux")]
impl ProcObjectIdentityV1 {
    fn capture(file: &File, context: &str) -> Result<Self, HostLinkError> {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata().context(HostLinkErrorCodeV1::Io, || {
            format!("inspect {context} identity")
        })?;
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
        })
    }

    fn append_bytes(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.device.to_le_bytes());
        bytes.extend_from_slice(&self.inode.to_le_bytes());
        bytes.extend_from_slice(&self.mode.to_le_bytes());
    }
}

/// Authority for the trusted initial mount namespace's procfs paths used by inotify.
///
/// Construction assumes the embedding broker has already established its trusted initial mount
/// namespace. The capability then retains and revalidates kernel identities; it does not treat
/// procfs text or the inotify journal as fixed-root content authority.
#[cfg(target_os = "linux")]
struct ProcfsCapabilityV1 {
    root: File,
    root_identity: ProcObjectIdentityV1,
    mount_namespace: File,
    mount_namespace_identity: ProcObjectIdentityV1,
    fd_directory: File,
    fd_directory_identity: ProcObjectIdentityV1,
}

#[cfg(target_os = "linux")]
impl ProcfsCapabilityV1 {
    fn new() -> Result<Self, HostLinkError> {
        let root = open_directory_path(Path::new("/proc"))?;
        Self::from_root(root)
    }

    fn from_root(root: File) -> Result<Self, HostLinkError> {
        verify_proc_filesystem(&root)?;
        let root_identity = ProcObjectIdentityV1::capture(&root, "retained procfs root")?;
        let mount_namespace = open_proc_relative(&root, Path::new("thread-self/ns/mnt"), false)?;
        let mount_namespace_identity =
            ProcObjectIdentityV1::capture(&mount_namespace, "retained mount namespace")?;
        let fd_directory = open_proc_relative(&root, Path::new("self/fd"), true)?;
        let fd_directory_identity =
            ProcObjectIdentityV1::capture(&fd_directory, "retained procfs fd directory")?;
        let capability = Self {
            root,
            root_identity,
            mount_namespace,
            mount_namespace_identity,
            fd_directory,
            fd_directory_identity,
        };
        capability.revalidate()?;
        Ok(capability)
    }

    fn revalidate(&self) -> Result<(), HostLinkError> {
        verify_proc_filesystem(&self.root)?;
        if ProcObjectIdentityV1::capture(&self.root, "retained procfs root")? != self.root_identity
            || ProcObjectIdentityV1::capture(&self.mount_namespace, "retained mount namespace")?
                != self.mount_namespace_identity
            || ProcObjectIdentityV1::capture(&self.fd_directory, "retained procfs fd directory")?
                != self.fd_directory_identity
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::RootChanged,
                "retained procfs or mount-namespace identity changed",
            ));
        }
        let ambient_root = open_directory_path(Path::new("/proc")).map_err(|error| {
            HostLinkError::new(
                HostLinkErrorCodeV1::RootChanged,
                format!("ambient procfs root cannot be authenticated: {error}"),
            )
        })?;
        verify_proc_filesystem(&ambient_root)?;
        let current_mount_namespace =
            open_proc_relative(&self.root, Path::new("thread-self/ns/mnt"), false)?;
        let current_fd_directory = open_proc_relative(&self.root, Path::new("self/fd"), true)?;
        let ambient_fd_directory = open_proc_relative(&ambient_root, Path::new("self/fd"), true)?;
        if ProcObjectIdentityV1::capture(&ambient_root, "ambient procfs root")?
            != self.root_identity
            || ProcObjectIdentityV1::capture(&current_mount_namespace, "current mount namespace")?
                != self.mount_namespace_identity
            || ProcObjectIdentityV1::capture(&current_fd_directory, "current procfs fd directory")?
                != self.fd_directory_identity
            || ProcObjectIdentityV1::capture(&ambient_fd_directory, "ambient procfs fd directory")?
                != self.fd_directory_identity
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::RootChanged,
                "ambient procfs path or trusted mount namespace was substituted",
            ));
        }
        Ok(())
    }

    fn verify_watch_target(&self, directory: &File) -> Result<(), HostLinkError> {
        use std::os::fd::AsRawFd;
        self.revalidate()?;
        let path = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        let through_proc = open_proc_watch_target(&path)?;
        if root_identity(&through_proc)? != root_identity(directory)? {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::RootChanged,
                "procfs watch path did not resolve to the retained fixed-root directory",
            ));
        }
        Ok(())
    }

    fn identity_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(60);
        self.root_identity.append_bytes(&mut bytes);
        self.mount_namespace_identity.append_bytes(&mut bytes);
        self.fd_directory_identity.append_bytes(&mut bytes);
        bytes
    }
}

#[cfg(target_os = "linux")]
fn verify_proc_filesystem(root: &File) -> Result<(), HostLinkError> {
    let filesystem = rustix::fs::fstatfs(root).context(HostLinkErrorCodeV1::Io, || {
        "inspect retained procfs filesystem type".to_owned()
    })?;
    if filesystem.f_type as u64 != PROC_SUPER_MAGIC {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::RootChanged,
            "fixed-root mutation journal path is not on an authentic procfs",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_proc_relative(root: &File, path: &Path, directory: bool) -> Result<File, HostLinkError> {
    let mut flags = rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC;
    if directory {
        flags |= rustix::fs::OFlags::DIRECTORY;
    }
    rustix::fs::openat(root, path, flags, rustix::fs::Mode::empty())
        .map(File::from)
        .context(HostLinkErrorCodeV1::RootChanged, || {
            format!("open authenticated procfs component {}", path.display())
        })
}

#[cfg(target_os = "linux")]
fn open_proc_watch_target(path: &Path) -> Result<File, HostLinkError> {
    rustix::fs::openat(
        rustix::fs::CWD,
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .context(HostLinkErrorCodeV1::RootChanged, || {
        "open authenticated procfs inotify target".to_owned()
    })
}

#[cfg(target_os = "linux")]
impl MutationJournal {
    fn new() -> Result<Self, HostLinkError> {
        let procfs = ProcfsCapabilityV1::new()?;
        let descriptor = rustix::fs::inotify::init(
            rustix::fs::inotify::CreateFlags::CLOEXEC | rustix::fs::inotify::CreateFlags::NONBLOCK,
        )
        .context(HostLinkErrorCodeV1::Io, || {
            "create fixed-root mutation journal".to_owned()
        })?;
        Ok(Self { descriptor, procfs })
    }

    fn watch(&self, directory: &File) -> Result<(), HostLinkError> {
        use std::os::fd::AsRawFd;
        self.procfs.verify_watch_target(directory)?;
        let path = format!("/proc/self/fd/{}", directory.as_raw_fd());
        let flags = rustix::fs::inotify::WatchFlags::ATTRIB
            | rustix::fs::inotify::WatchFlags::CLOSE_WRITE
            | rustix::fs::inotify::WatchFlags::CREATE
            | rustix::fs::inotify::WatchFlags::DELETE
            | rustix::fs::inotify::WatchFlags::DELETE_SELF
            | rustix::fs::inotify::WatchFlags::MODIFY
            | rustix::fs::inotify::WatchFlags::MOVE_SELF
            | rustix::fs::inotify::WatchFlags::MOVED_FROM
            | rustix::fs::inotify::WatchFlags::MOVED_TO
            | rustix::fs::inotify::WatchFlags::ONLYDIR;
        rustix::fs::inotify::add_watch(&self.descriptor, path, flags)
            .context(HostLinkErrorCodeV1::Io, || {
                "install fixed-root mutation watch".to_owned()
            })?;
        self.procfs.verify_watch_target(directory)?;
        Ok(())
    }

    fn ensure_clean(&self) -> Result<(), HostLinkError> {
        use std::mem::MaybeUninit;
        self.procfs.revalidate()?;
        let mut storage = [MaybeUninit::uninit(); 16 * 1024];
        let mut reader = rustix::fs::inotify::Reader::new(&self.descriptor, &mut storage);
        match reader.next() {
            Err(rustix::io::Errno::AGAIN) => self.procfs.revalidate(),
            Err(error) => Err(HostLinkError::new(
                HostLinkErrorCodeV1::Io,
                format!("read fixed-root mutation journal: {error}"),
            )),
            Ok(event) => Err(HostLinkError::new(
                HostLinkErrorCodeV1::RootMutation,
                format!(
                    "fixed-root mutation journal recorded watch {} flags {:?}",
                    event.wd(),
                    event.events()
                ),
            )),
        }
    }

    fn identity_bytes(&self) -> Vec<u8> {
        self.procfs.identity_bytes()
    }
}

#[cfg(not(target_os = "linux"))]
struct MutationJournal;

#[cfg(not(target_os = "linux"))]
impl MutationJournal {
    fn new() -> Result<Self, HostLinkError> {
        Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedPlatform,
            "fixed-root mutation journaling requires Linux",
        ))
    }
    fn watch(&self, _directory: &File) -> Result<(), HostLinkError> {
        Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedPlatform,
            "fixed-root mutation journaling requires Linux",
        ))
    }
    fn ensure_clean(&self) -> Result<(), HostLinkError> {
        Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedPlatform,
            "fixed-root mutation journaling requires Linux",
        ))
    }
    fn identity_bytes(&self) -> Vec<u8> {
        Vec::new()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct ObjectSnapshot {
    device: u64,
    inode: u64,
    link_count: u64,
    mode: u32,
    size: i64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

#[cfg(target_os = "linux")]
impl ObjectSnapshot {
    fn from_stat(stat: rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            link_count: stat.st_nlink,
            mode: stat.st_mode,
            size: stat.st_size,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        }
    }
}

struct SnapshotState {
    entries: u64,
    bytes: u64,
    name_bytes: u64,
}

#[cfg(target_os = "linux")]
fn snapshot_root(
    directory: &File,
    journal: Option<&MutationJournal>,
) -> Result<Sha256Digest, HostLinkError> {
    let root = rustix::fs::openat(
        directory,
        ".",
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .context(HostLinkErrorCodeV1::Io, || {
        "retain fixed-root scan descriptor".to_owned()
    })?;
    let mut digest = Sha256::new();
    digest.update(SNAPSHOT_DOMAIN);
    let mut state = SnapshotState {
        entries: 0,
        bytes: 0,
        name_bytes: 0,
    };
    snapshot_directory(&root, &mut digest, &mut state, 0, journal)?;
    Ok(Sha256Digest::from_bytes(digest.finalize().into()))
}

#[cfg(not(target_os = "linux"))]
fn snapshot_root(
    _directory: &File,
    _journal: Option<&MutationJournal>,
) -> Result<Sha256Digest, HostLinkError> {
    Err(HostLinkError::new(
        HostLinkErrorCodeV1::UnsupportedPlatform,
        "fixed-root snapshots require Linux",
    ))
}

#[cfg(target_os = "linux")]
fn snapshot_directory(
    directory: &File,
    digest: &mut Sha256,
    state: &mut SnapshotState,
    depth: usize,
    journal: Option<&MutationJournal>,
) -> Result<(), HostLinkError> {
    use rustix::fs::{FileType, Mode, OFlags, fstat, openat};
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStrExt;

    if let Some(journal) = journal {
        journal.watch(directory)?;
    }
    let names: Vec<OsString> = sorted_names(directory, state)?;
    digest.update(b"directory\0");
    for name in names {
        hash_field(digest, name.as_bytes());
        let descriptor = openat(
            directory,
            Path::new(&name),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            let code = if error == rustix::io::Errno::LOOP {
                HostLinkErrorCodeV1::Symlink
            } else {
                HostLinkErrorCodeV1::Io
            };
            HostLinkError::new(
                code,
                format!("open fixed-root snapshot entry {name:?}: {error}"),
            )
        })?;
        let initial = fstat(&descriptor)
            .map(ObjectSnapshot::from_stat)
            .context(HostLinkErrorCodeV1::Io, || {
                format!("inspect fixed-root snapshot entry {name:?}")
            })?;
        match FileType::from_raw_mode(initial.mode) {
            FileType::RegularFile => {
                if initial.link_count != 1 {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::RootChanged,
                        format!("fixed-root regular file has external hardlinks: {name:?}"),
                    ));
                }
                digest.update(b"file\0");
                digest.update(initial.device.to_le_bytes());
                digest.update(initial.inode.to_le_bytes());
                digest.update(initial.link_count.to_le_bytes());
                digest.update((initial.mode & 0o7777).to_le_bytes());
                digest.update(initial.modified_seconds.to_le_bytes());
                digest.update(initial.modified_nanoseconds.to_le_bytes());
                digest.update(initial.changed_seconds.to_le_bytes());
                digest.update(initial.changed_nanoseconds.to_le_bytes());
                let size = u64::try_from(initial.size).map_err(|_| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::RootChanged,
                        format!("fixed-root entry has negative size: {name:?}"),
                    )
                })?;
                state.bytes = state
                    .bytes
                    .checked_add(size)
                    .filter(|bytes| *bytes <= MAX_ROOT_BYTES)
                    .ok_or_else(|| {
                        HostLinkError::new(
                            HostLinkErrorCodeV1::ArtifactTooLarge,
                            "fixed-root content exceeds its byte bound",
                        )
                    })?;
                digest.update(size.to_le_bytes());
                let file = File::from(descriptor);
                let mut remaining = size;
                let mut offset = 0_u64;
                let mut buffer = [0_u8; 64 * 1024];
                while remaining != 0 {
                    let limit = usize::try_from(remaining.min(buffer.len() as u64))
                        .expect("bounded root read fits usize");
                    let count = rustix::io::pread(&file, &mut buffer[..limit], offset)
                        .context(HostLinkErrorCodeV1::Io, || {
                            format!("hash fixed-root entry without changing its offset {name:?}")
                        })?;
                    if count == 0 {
                        return Err(HostLinkError::new(
                            HostLinkErrorCodeV1::RootChanged,
                            format!("fixed-root entry shortened while hashing: {name:?}"),
                        ));
                    }
                    digest.update(&buffer[..count]);
                    remaining -= count as u64;
                    offset += count as u64;
                }
                let mut extra = [0_u8; 1];
                if rustix::io::pread(&file, &mut extra, size)
                    .context(HostLinkErrorCodeV1::Io, || {
                        format!("bound fixed-root entry without changing its offset {name:?}")
                    })?
                    != 0
                {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::RootChanged,
                        format!("fixed-root entry grew while hashing: {name:?}"),
                    ));
                }
                let final_snapshot = fstat(&file)
                    .map(ObjectSnapshot::from_stat)
                    .context(HostLinkErrorCodeV1::Io, || {
                        format!("reinspect fixed-root entry {name:?}")
                    })?;
                if final_snapshot != initial {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::RootChanged,
                        format!("fixed-root entry changed while hashing: {name:?}"),
                    ));
                }
            }
            FileType::Directory => {
                digest.update(b"subdirectory\0");
                digest.update((initial.mode & 0o7777).to_le_bytes());
                let next_depth = depth
                    .checked_add(1)
                    .filter(|value| *value <= MAX_ROOT_DEPTH)
                    .ok_or_else(|| {
                        HostLinkError::new(
                            HostLinkErrorCodeV1::FieldTooLarge,
                            "fixed-root depth exceeds its bound",
                        )
                    })?;
                let child = File::from(descriptor);
                snapshot_directory(&child, digest, state, next_depth, journal)?;
                let final_snapshot = fstat(&child)
                    .map(ObjectSnapshot::from_stat)
                    .context(HostLinkErrorCodeV1::Io, || {
                        format!("reinspect fixed-root directory {name:?}")
                    })?;
                if final_snapshot != initial {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::RootChanged,
                        format!("fixed-root directory changed while hashing: {name:?}"),
                    ));
                }
            }
            _ => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::Symlink,
                    format!("fixed-root entry is not a regular file or directory: {name:?}"),
                ));
            }
        }
    }
    digest.update(b"end-directory\0");
    Ok(())
}

#[cfg(target_os = "linux")]
fn sorted_names(
    directory: &File,
    state: &mut SnapshotState,
) -> Result<Vec<std::ffi::OsString>, HostLinkError> {
    use rustix::fs::{Mode, OFlags, openat};
    use std::ffi::OsString;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let scan = openat(
        directory,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .context(HostLinkErrorCodeV1::Io, || {
        "open fixed-root directory scan".to_owned()
    })?;
    let mut entries = rustix::fs::Dir::read_from(&scan).context(HostLinkErrorCodeV1::Io, || {
        "enumerate fixed-root directory".to_owned()
    })?;
    let mut names = Vec::new();
    let mut directory_entries = 0_usize;
    for entry in &mut entries {
        let entry = entry.context(HostLinkErrorCodeV1::Io, || {
            "enumerate fixed-root entry".to_owned()
        })?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        directory_entries = directory_entries
            .checked_add(1)
            .filter(|count| *count <= MAX_DIRECTORY_ENTRIES)
            .ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::FieldTooLarge,
                    "fixed-root directory exceeds its entry bound",
                )
            })?;
        state.entries = state
            .entries
            .checked_add(1)
            .filter(|entries| *entries <= MAX_ROOT_ENTRIES)
            .ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::FieldTooLarge,
                    "fixed-root exceeds its entry bound",
                )
            })?;
        state.name_bytes = state
            .name_bytes
            .checked_add(bytes.len() as u64)
            .filter(|bytes| *bytes <= MAX_ROOT_NAME_BYTES)
            .ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::FieldTooLarge,
                    "fixed-root exceeds its name-byte bound",
                )
            })?;
        names.push(OsString::from_vec(bytes.to_vec()));
    }
    names.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    Ok(names)
}

fn hash_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn authentic_procfs_and_mount_namespace_revalidate() {
        let procfs = ProcfsCapabilityV1::new().unwrap();
        assert_eq!(procfs.identity_bytes().len(), 60);
        procfs.revalidate().unwrap();
    }

    #[test]
    fn ordinary_directory_cannot_substitute_for_procfs() {
        let fake = tempfile::tempdir().unwrap();
        let fake_root = open_directory_path(fake.path()).unwrap();
        assert_eq!(
            ProcfsCapabilityV1::from_root(fake_root)
                .err()
                .expect("non-proc filesystem must not become path authority")
                .code(),
            HostLinkErrorCodeV1::RootChanged
        );
    }
}
