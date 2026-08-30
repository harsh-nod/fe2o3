use std::fs::File;
use std::os::fd::AsFd;
use std::path::Path;

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1, COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
};
use rustix::fs::{FileType, FlockOperation, Mode, OFlags, flock, openat};

use crate::CompilerExecutionCoordinatorErrorV1;
use crate::inherited::{RootFileSnapshotV1, validate_provisioned_file};

const LIFECYCLE_LOCK_ROLE_V1: &str = "compiler-execution lifecycle lock";
const LIFECYCLE_PARENT_MODE_V1: u32 = 0o755;
const ROOT_ID_V1: u32 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompilerExecutionLifecycleLeaseModeV1 {
    SharedService,
    ExclusiveProvisioning,
}

pub(crate) struct CompilerExecutionLifecycleLeaseV1 {
    file: File,
    snapshot: RootFileSnapshotV1,
    mode: CompilerExecutionLifecycleLeaseModeV1,
    expected_uid: u32,
    expected_gid: u32,
}

impl std::fmt::Debug for CompilerExecutionLifecycleLeaseV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompilerExecutionLifecycleLeaseV1")
            .field("authority", &"deployment-lifecycle-only")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl CompilerExecutionLifecycleLeaseV1 {
    pub(crate) fn admit_service_from_root(
        root: &impl AsFd,
    ) -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        Self::admit_service_from_root_for_owner(root, ROOT_ID_V1, ROOT_ID_V1)
    }

    fn admit_service_from_root_for_owner(
        root: &impl AsFd,
        expected_uid: u32,
        expected_gid: u32,
    ) -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        let parent = openat(
            root,
            "..",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|source| CompilerExecutionCoordinatorErrorV1::Io {
            operation: "derive lifecycle parent from supervisor root",
            source: source.into(),
        })?;
        validate_parent(&parent, expected_uid, expected_gid)?;
        let lock_name = Path::new(COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1)
            .file_name()
            .ok_or(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
                role: LIFECYCLE_LOCK_ROLE_V1,
                reason: "canonical lifecycle-lock path has no file name",
            })?;
        let file = openat(
            &parent,
            lock_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|source| CompilerExecutionCoordinatorErrorV1::Io {
            operation: "open lifecycle lock beside supervisor root",
            source: source.into(),
        })?;
        Self::admit(
            File::from(file),
            CompilerExecutionLifecycleLeaseModeV1::SharedService,
            expected_uid,
            expected_gid,
        )
    }

    pub(crate) fn admit(
        file: File,
        mode: CompilerExecutionLifecycleLeaseModeV1,
        expected_uid: u32,
        expected_gid: u32,
    ) -> Result<Self, CompilerExecutionCoordinatorErrorV1> {
        let snapshot = validate_lifecycle_file(&file, expected_uid, expected_gid)?;
        acquire(&file, mode)?;
        let admitted = Self {
            file,
            snapshot,
            mode,
            expected_uid,
            expected_gid,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    pub(crate) fn revalidate(&self) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        if validate_lifecycle_file(&self.file, self.expected_uid, self.expected_gid)?
            != self.snapshot
        {
            return Err(CompilerExecutionCoordinatorErrorV1::LifecycleChanged);
        }
        acquire(&self.file, self.mode)?;
        if validate_lifecycle_file(&self.file, self.expected_uid, self.expected_gid)?
            != self.snapshot
        {
            return Err(CompilerExecutionCoordinatorErrorV1::LifecycleChanged);
        }
        Ok(())
    }

    pub(crate) fn revalidate_alias(
        &self,
        alias: File,
    ) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
        self.revalidate()?;
        if validate_lifecycle_file(&alias, self.expected_uid, self.expected_gid)? != self.snapshot {
            return Err(CompilerExecutionCoordinatorErrorV1::LifecycleChanged);
        }
        self.revalidate()
    }
}

impl Drop for CompilerExecutionLifecycleLeaseV1 {
    fn drop(&mut self) {
        let _ = flock(&self.file, FlockOperation::Unlock);
    }
}

fn validate_parent(
    parent: &impl AsFd,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(parent).map_err(|source| {
        CompilerExecutionCoordinatorErrorV1::Io {
            operation: "inspect lifecycle parent descriptor flags",
            source: source.into(),
        }
    })?;
    let status = rustix::fs::fcntl_getfl(parent).map_err(|source| {
        CompilerExecutionCoordinatorErrorV1::Io {
            operation: "inspect lifecycle parent status flags",
            source: source.into(),
        }
    })?;
    let stat =
        rustix::fs::fstat(parent).map_err(|source| CompilerExecutionCoordinatorErrorV1::Io {
            operation: "inspect lifecycle parent",
            source: source.into(),
        })?;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_mode & 0o7777 != LIFECYCLE_PARENT_MODE_V1
        || stat.st_uid != expected_uid
        || stat.st_gid != expected_gid
        || stat.st_nlink == 0
    {
        return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
            role: "lifecycle parent",
            reason: "type, access, owner, group, mode, or links is not exact",
        });
    }
    for attribute in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        let mut byte = 0_u8;
        match rustix::fs::fgetxattr(parent, attribute, std::slice::from_mut(&mut byte)) {
            Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => {}
            Ok(_) | Err(rustix::io::Errno::RANGE) => {
                return Err(CompilerExecutionCoordinatorErrorV1::ProvisionedInput {
                    role: "lifecycle parent",
                    reason: "directory has a forbidden capability or POSIX ACL",
                });
            }
            Err(source) => {
                return Err(CompilerExecutionCoordinatorErrorV1::Io {
                    operation: "inspect lifecycle parent extended attributes",
                    source: source.into(),
                });
            }
        }
    }
    Ok(())
}

fn validate_lifecycle_file(
    file: &File,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<RootFileSnapshotV1, CompilerExecutionCoordinatorErrorV1> {
    validate_provisioned_file(
        file,
        LIFECYCLE_LOCK_ROLE_V1,
        COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1,
        Some(0),
        expected_uid,
        expected_gid,
    )
}

fn acquire(
    file: &File,
    mode: CompilerExecutionLifecycleLeaseModeV1,
) -> Result<(), CompilerExecutionCoordinatorErrorV1> {
    let operation = match mode {
        CompilerExecutionLifecycleLeaseModeV1::SharedService => {
            FlockOperation::NonBlockingLockShared
        }
        CompilerExecutionLifecycleLeaseModeV1::ExclusiveProvisioning => {
            FlockOperation::NonBlockingLockExclusive
        }
    };
    flock(file, operation).map_err(|source| {
        if source == rustix::io::Errno::WOULDBLOCK {
            CompilerExecutionCoordinatorErrorV1::LifecycleBusy
        } else {
            CompilerExecutionCoordinatorErrorV1::Io {
                operation: "acquire compiler-execution lifecycle lease",
                source: source.into(),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn owner_drop_unlocks_even_when_a_fork_style_duplicate_survives() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("lifecycle");
        std::fs::write(&path, []).unwrap();
        std::fs::set_permissions(
            &path,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let shared = CompilerExecutionLifecycleLeaseV1::admit(
            File::open(&path).unwrap(),
            CompilerExecutionLifecycleLeaseModeV1::SharedService,
            uid,
            gid,
        )
        .unwrap();
        let duplicate = shared.file.try_clone().unwrap();

        assert!(matches!(
            CompilerExecutionLifecycleLeaseV1::admit(
                File::open(&path).unwrap(),
                CompilerExecutionLifecycleLeaseModeV1::ExclusiveProvisioning,
                uid,
                gid,
            ),
            Err(CompilerExecutionCoordinatorErrorV1::LifecycleBusy)
        ));
        drop(shared);
        let exclusive = CompilerExecutionLifecycleLeaseV1::admit(
            File::open(&path).unwrap(),
            CompilerExecutionLifecycleLeaseModeV1::ExclusiveProvisioning,
            uid,
            gid,
        )
        .unwrap();
        exclusive.revalidate().unwrap();
        drop(duplicate);
    }

    #[test]
    fn dedicated_lifecycle_lock_coexists_with_state_root_singleton() {
        let fixture = tempfile::tempdir().unwrap();
        let lifecycle_path = fixture.path().join("lifecycle");
        let state_root = fixture.path().join("state-root");
        std::fs::write(&lifecycle_path, []).unwrap();
        std::fs::set_permissions(
            &lifecycle_path,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        std::fs::create_dir(&state_root).unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let lifecycle = CompilerExecutionLifecycleLeaseV1::admit(
            File::open(&lifecycle_path).unwrap(),
            CompilerExecutionLifecycleLeaseModeV1::SharedService,
            uid,
            gid,
        )
        .unwrap();

        let singleton = File::open(&state_root).unwrap();
        flock(&singleton, FlockOperation::NonBlockingLockExclusive).unwrap();
        lifecycle.revalidate().unwrap();
    }

    #[test]
    fn service_lease_is_derived_from_the_supervisor_root_parent() {
        let fixture = tempfile::tempdir().unwrap();
        std::fs::set_permissions(
            fixture.path(),
            std::fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
        )
        .unwrap();
        let state_root = fixture.path().join("state-root");
        std::fs::create_dir(&state_root).unwrap();
        let lock_name = Path::new(COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1)
            .file_name()
            .unwrap();
        let lock_path = fixture.path().join(lock_name);
        std::fs::write(&lock_path, []).unwrap();
        std::fs::set_permissions(
            &lock_path,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();

        let service = CompilerExecutionLifecycleLeaseV1::admit_service_from_root_for_owner(
            &File::open(&state_root).unwrap(),
            uid,
            gid,
        )
        .unwrap();
        assert!(matches!(
            CompilerExecutionLifecycleLeaseV1::admit(
                File::open(&lock_path).unwrap(),
                CompilerExecutionLifecycleLeaseModeV1::ExclusiveProvisioning,
                uid,
                gid,
            ),
            Err(CompilerExecutionCoordinatorErrorV1::LifecycleBusy)
        ));
        service.revalidate().unwrap();
    }

    #[test]
    fn pathname_alias_substitution_is_rejected() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("lifecycle");
        std::fs::write(&path, []).unwrap();
        std::fs::set_permissions(
            &path,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let lease = CompilerExecutionLifecycleLeaseV1::admit(
            File::open(&path).unwrap(),
            CompilerExecutionLifecycleLeaseModeV1::ExclusiveProvisioning,
            uid,
            gid,
        )
        .unwrap();

        std::fs::rename(&path, fixture.path().join("displaced")).unwrap();
        std::fs::write(&path, []).unwrap();
        std::fs::set_permissions(
            &path,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        assert!(matches!(
            lease.revalidate_alias(File::open(&path).unwrap()),
            Err(CompilerExecutionCoordinatorErrorV1::LifecycleChanged)
        ));
    }
}
