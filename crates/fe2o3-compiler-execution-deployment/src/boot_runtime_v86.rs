use super::*;
use std::path::{Component, Path, PathBuf};

#[path = "boot_image_v160.rs"]
mod image_v160;

const RUNTIME_DIRECTORY_V86: &str = "run/fe2o3";
const STAGING_CHILDREN_V86: &[&str] =
    &["base", "evidence", "root", "run", "state", "upper", "work"];

pub(super) fn checked_bind_source<'a>(
    path: &'a Path,
    identity: &QualificationMachineIdentityV1,
) -> Result<&'a str, DeploymentVerificationErrorV1> {
    let text = path
        .to_str()
        .ok_or_else(|| changed("machine bind source is not UTF-8"))?;
    let parent = path
        .parent()
        .ok_or_else(|| changed("machine bind source has no parent"))?;
    let staging = parent
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| changed("machine bind source has no staging name"))?;
    if !path.is_absolute()
        || text.len() > 4096
        || text
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b':' || byte == b'\\')
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        || path.file_name() != Some(std::ffi::OsStr::new("run"))
        || QualificationMachineIdentityV1::from_staging_name(staging)? != *identity
    {
        return Err(changed(
            "machine bind source is outside its exact staging identity",
        ));
    }
    Ok(text)
}

fn absolute_directory(path: &Path) -> Result<File, DeploymentVerificationErrorV1> {
    let slash =
        File::open("/").map_err(|source| std_io_error("open machine namespace root", source))?;
    let relative = path
        .strip_prefix("/")
        .map_err(|_| changed("machine staging path is not absolute"))?;
    if relative.as_os_str().is_empty() {
        return Err(changed("machine staging cannot be the filesystem root"));
    }
    openat2(
        &slash,
        relative,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|source| io_error("reopen machine staging in current namespace", source))
}

fn exact_snapshot(file: &File) -> Result<crate::ObjectSnapshotV1, DeploymentVerificationErrorV1> {
    fstat(file)
        .map(|stat| snapshot(&stat))
        .map_err(|source| io_error("inspect machine staging custody", source))
}

fn staging_snapshot(
    stage: &File,
    owner: (u32, u32),
) -> Result<crate::ObjectSnapshotV1, DeploymentVerificationErrorV1> {
    let before =
        super::super::validate_directory_mode(stage, Some(owner), 0o700, "machine staging")?;
    if before.links != 9 {
        return Err(changed("machine staging link count changed"));
    }
    super::super::verify_directory_children(stage, STAGING_CHILDREN_V86, "machine staging")?;
    if exact_snapshot(stage)? != before {
        return Err(changed("machine staging changed during admission"));
    }
    Ok(before)
}

fn alias_target(stage: &File, owner: (u32, u32)) -> Result<File, DeploymentVerificationErrorV1> {
    let target = super::super::open_beneath(stage, "run", true)?;
    let before =
        super::super::validate_directory_mode(&target, Some(owner), 0o700, "machine alias target")?;
    if before.links != 2 {
        return Err(changed("machine alias target link count changed"));
    }
    super::super::verify_directory_children(&target, &[], "machine alias target")?;
    if exact_snapshot(&target)? != before {
        return Err(changed("machine alias target changed during admission"));
    }
    Ok(target)
}

pub(super) struct MachineRuntimeSourceV86 {
    stage_path: PathBuf,
    stage: File,
    stage_snapshot: crate::ObjectSnapshotV1,
    root_snapshot: crate::ObjectSnapshotV1,
    target: File,
    target_snapshot: crate::ObjectSnapshotV1,
    runtime: MachineRuntimeDirectoryV86,
    image: image_v160::MachineImageSourceV160,
}

impl MachineRuntimeSourceV86 {
    pub(super) fn capture(
        root: &File,
        staging_name: &str,
        owner: (u32, u32),
    ) -> Result<Self, DeploymentVerificationErrorV1> {
        let identity = QualificationMachineIdentityV1::from_staging_name(staging_name)?;
        let root_path = std::fs::canonicalize(format!("/proc/self/fd/{}", root.as_raw_fd()))
            .map_err(|source| std_io_error("resolve retained machine root", source))?;
        if root_path.file_name() != Some(std::ffi::OsStr::new("root")) {
            return Err(changed("machine root has no canonical staging position"));
        }
        let stage_path = root_path
            .parent()
            .ok_or_else(|| changed("machine root has no parent"))?
            .to_owned();
        checked_bind_source(&stage_path.join("run"), &identity)?;
        let stage = absolute_directory(&stage_path)?;
        let stage_snapshot = staging_snapshot(&stage, owner)?;
        let root_snapshot = exact_snapshot(root)?;
        if exact_snapshot(&absolute_directory(&root_path)?)? != root_snapshot {
            return Err(changed("machine root pathname lost retained custody"));
        }
        let target = alias_target(&stage, owner)?;
        let target_snapshot = exact_snapshot(&target)?;
        let runtime = MachineRuntimeDirectoryV86::open(root, owner)?;
        let image = image_v160::MachineImageSourceV160::capture(&stage, owner)?;
        Ok(Self {
            stage_path,
            stage,
            stage_snapshot,
            root_snapshot,
            target,
            target_snapshot,
            runtime,
            image,
        })
    }

    // Caller first enters a dedicated private NEWNS. Reopen every mount-relative
    // path there, retaining and comparing the original descriptors until joined.
    pub(super) fn attach(
        self,
        original_base: &File,
        original_root: &File,
        owner: (u32, u32),
    ) -> Result<MachineRuntimeAliasV86, DeploymentVerificationErrorV1> {
        let stage = absolute_directory(&self.stage_path)?;
        let base_snapshot = exact_snapshot(original_base)?;
        let base = absolute_directory(&self.stage_path.join("base"))?;
        let root = absolute_directory(&self.stage_path.join("root"))?;
        let target = alias_target(&stage, owner)?;
        for actual in [
            staging_snapshot(&stage, owner)?,
            staging_snapshot(&self.stage, owner)?,
        ] {
            if actual != self.stage_snapshot {
                return Err(changed("machine staging changed across namespace entry"));
            }
        }
        if exact_snapshot(&base)? != base_snapshot
            || exact_snapshot(original_base)? != base_snapshot
            || exact_snapshot(&root)? != self.root_snapshot
            || exact_snapshot(original_root)? != self.root_snapshot
            || exact_snapshot(&target)? != self.target_snapshot
            || exact_snapshot(&self.target)? != self.target_snapshot
        {
            return Err(changed("machine alias source or target custody changed"));
        }
        self.runtime.revalidate(original_root, owner)?;
        let runtime = MachineRuntimeDirectoryV86::open(&root, owner)?;
        if runtime.snapshot != self.runtime.snapshot {
            return Err(changed("machine runtime changed across namespace entry"));
        }
        rustix::mount::mount_bind(
            format!("/proc/self/fd/{}", runtime.directory.as_raw_fd()),
            format!("/proc/self/fd/{}", target.as_raw_fd()),
        )
        .map_err(|source| io_error("attach child-private machine runtime alias", source))?;
        let path = self.stage_path.join("run");
        let mounted = absolute_directory(&path)?;
        let image = self.image.attach(&self.stage_path, &root, owner)?;
        let alias = MachineRuntimeAliasV86 {
            path,
            stage,
            stage_snapshot: self.stage_snapshot,
            mounted,
            base,
            base_snapshot,
            root,
            root_snapshot: self.root_snapshot,
            runtime_snapshot: runtime.snapshot,
            owner,
            image,
        };
        alias.revalidate()?;
        Ok(alias)
    }
}

// This mount exists only in the exec helper's private namespace and its nspawn
// descendants. Parent shutdown/reaping tears down that namespace on every path.
pub(super) struct MachineRuntimeAliasV86 {
    path: PathBuf,
    stage: File,
    stage_snapshot: crate::ObjectSnapshotV1,
    mounted: File,
    pub(super) base: File,
    base_snapshot: crate::ObjectSnapshotV1,
    pub(super) root: File,
    root_snapshot: crate::ObjectSnapshotV1,
    runtime_snapshot: crate::ObjectSnapshotV1,
    owner: (u32, u32),
    image: image_v160::MachineImageAliasV160,
}

impl MachineRuntimeAliasV86 {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn image_root(&self) -> &File {
        self.image.root()
    }

    pub(super) fn revalidate(&self) -> Result<(), DeploymentVerificationErrorV1> {
        self.image.revalidate()?;
        let named = absolute_directory(&self.path)?;
        let stage = absolute_directory(
            self.path
                .parent()
                .ok_or_else(|| changed("machine alias lost parent"))?,
        )?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| changed("machine alias lost parent"))?;
        if staging_snapshot(&stage, self.owner)? != self.stage_snapshot
            || staging_snapshot(&self.stage, self.owner)? != self.stage_snapshot
            || exact_snapshot(&self.base)? != self.base_snapshot
            || exact_snapshot(&absolute_directory(&parent.join("base"))?)? != self.base_snapshot
            || exact_snapshot(&self.root)? != self.root_snapshot
            || exact_snapshot(&absolute_directory(&parent.join("root"))?)? != self.root_snapshot
            || validate_runtime_directory(&named, self.owner)? != self.runtime_snapshot
            || validate_runtime_directory(&self.mounted, self.owner)? != self.runtime_snapshot
        {
            return Err(changed("machine runtime alias changed before nspawn exec"));
        }
        Ok(())
    }
}

pub(super) struct MachineRuntimeDirectoryV86 {
    pub(super) directory: File,
    snapshot: crate::ObjectSnapshotV1,
}

impl MachineRuntimeDirectoryV86 {
    pub(super) fn open(
        root: &File,
        owner: (u32, u32),
    ) -> Result<Self, DeploymentVerificationErrorV1> {
        let directory = open_runtime_directory(root)?;
        let snapshot = validate_runtime_directory(&directory, owner)?;
        let retained = Self {
            directory,
            snapshot,
        };
        retained.revalidate(root, owner)?;
        Ok(retained)
    }

    pub(super) fn revalidate(
        &self,
        root: &File,
        owner: (u32, u32),
    ) -> Result<(), DeploymentVerificationErrorV1> {
        let named = open_runtime_directory(root)?;
        if validate_runtime_directory(&self.directory, owner)? != self.snapshot
            || validate_runtime_directory(&named, owner)? != self.snapshot
        {
            return Err(changed("systemd machine runtime directory custody changed"));
        }
        Ok(())
    }
}

fn open_runtime_directory(root: &File) -> Result<File, DeploymentVerificationErrorV1> {
    openat2(
        root,
        RUNTIME_DIRECTORY_V86,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|source| io_error("open composed-root systemd runtime directory", source))
}

fn validate_runtime_directory(
    directory: &File,
    owner: (u32, u32),
) -> Result<crate::ObjectSnapshotV1, DeploymentVerificationErrorV1> {
    let before = snapshot(
        &fstat(directory)
            .map_err(|source| io_error("inspect systemd runtime directory", source))?,
    );
    if FileType::from_raw_mode(before.mode) != FileType::Directory
        || before.mode & 0o7777 != 0o755
        || (before.uid, before.gid) != owner
        || before.links != 2
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
            "systemd machine runtime directory metadata is not canonical",
        ));
    }
    require_no_xattrs(directory, "systemd machine runtime directory")?;
    // Stop at the first child: boot must not inherit any preexisting socket/report.
    let mut entries = rustix::fs::Dir::read_from(directory)
        .map_err(|source| io_error("enumerate systemd runtime directory", source))?;
    for entry in &mut entries {
        let entry = entry.map_err(|source| io_error("read systemd runtime directory", source))?;
        if !matches!(entry.file_name().to_bytes(), b"." | b"..") {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
                "systemd machine runtime directory is not empty before boot",
            ));
        }
    }
    if snapshot(
        &fstat(directory)
            .map_err(|source| io_error("reinspect systemd runtime directory", source))?,
    ) != before
    {
        return Err(changed(
            "systemd runtime directory changed during admission",
        ));
    }
    Ok(before)
}

#[cfg(test)]
#[path = "boot_runtime_v86_tests.rs"]
mod tests;
