use super::*;

const RUNTIME_DIRECTORY_V86: &str = "run/fe2o3";

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
