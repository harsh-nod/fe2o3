use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::File;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, ResolveFlags, fstat, mkdirat, openat, openat2, statat,
    unlinkat,
};

use super::install::set_owner_and_mode;
use super::qualification::{
    PreparedCompilerExecutionQualificationV1, open_qualification_parent_metadata,
    revalidate_prepared_qualification_with_parent_children,
};
use super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, ObjectSnapshotV1,
    canonical_directory_children, changed, io_error, lower_hex, random_staging_name,
    require_no_xattrs, snapshot, std_io_error, validate_directory_mode, verify_directory_children,
};

const QUALIFICATION_STAGING_PREFIX_V1: &str = ".compiler-execution-qualification-v1-";
const QUALIFICATION_STAGING_MODE_V1: u32 = 0o700;
const QUALIFICATION_STAGING_CHILDREN_V1: &[&str] =
    &["base", "evidence", "root", "run", "state", "upper", "work"];
const QUALIFICATION_RECOVERY_MAX_DEPTH_V1: usize = 64;
const QUALIFICATION_RECOVERY_MAX_ENTRIES_V1: usize = 131_072;

/// Result of descriptor-safe cleanup after a qualification worker exits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionQualificationRecoveryV1 {
    /// The retained qualification parent was already empty.
    AlreadyEmpty,
    /// One canonical incomplete staging transaction was completely removed.
    Recovered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QualificationStagingFaultPointV1 {
    BeforeRootCreate,
    RootCreated,
    RootMetadataSet,
    DirectoryCreated(usize),
    DirectoryMetadataSet(usize),
    TreeVerified,
    RootSynced,
    ParentPathVerified,
    ParentSynced,
}

trait QualificationStagingHooksV1 {
    fn checkpoint(
        &mut self,
        point: QualificationStagingFaultPointV1,
    ) -> Result<(), DeploymentVerificationErrorV1>;
}

struct NoQualificationStagingFaultV1;

impl QualificationStagingHooksV1 for NoQualificationStagingFaultV1 {
    fn checkpoint(
        &mut self,
        _point: QualificationStagingFaultPointV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        Ok(())
    }
}

#[cfg(test)]
struct InjectQualificationStagingFaultV1 {
    point: QualificationStagingFaultPointV1,
    fired: bool,
}

#[cfg(test)]
impl QualificationStagingHooksV1 for InjectQualificationStagingFaultV1 {
    fn checkpoint(
        &mut self,
        point: QualificationStagingFaultPointV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        if !self.fired && self.point == point {
            self.fired = true;
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InjectedFailure,
                format!("injected qualification staging interruption at {point:?}"),
            ));
        }
        Ok(())
    }
}

struct StagedDirectoryV1 {
    name: &'static str,
    file: File,
}

/// Move-only, authority-free custody of one exact empty qualification staging tree.
///
/// The private descriptors cover fixed mount points and disposable upper, work, run, state, and
/// evidence directories. No mount has been attached and no service can be launched from this
/// value. Explicit cleanup or `Drop` removes only this transaction's random child beneath the
/// retained qualification parent.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_deployment::StagedCompilerExecutionQualificationV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<StagedCompilerExecutionQualificationV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_deployment::StagedCompilerExecutionQualificationV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<StagedCompilerExecutionQualificationV1>();
/// ```
pub struct StagedCompilerExecutionQualificationV1 {
    prepared: PreparedCompilerExecutionQualificationV1,
    run_name: String,
    root: Option<File>,
    created_directories: Vec<&'static str>,
    directories: Vec<StagedDirectoryV1>,
    cleanup_required: bool,
}

impl fmt::Debug for StagedCompilerExecutionQualificationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StagedCompilerExecutionQualificationV1")
            .field("git_commit", &self.prepared.git_commit())
            .field(
                "manifest_sha256",
                &lower_hex(&self.prepared.manifest_sha256()),
            )
            .field(
                "base_image_sha256",
                &lower_hex(&self.prepared.base_image_sha256()),
            )
            .field("run_name", &self.run_name)
            .field("directory_count", &self.directory_count())
            .field("authority", &"empty-staging-custody-only")
            .finish_non_exhaustive()
    }
}

impl StagedCompilerExecutionQualificationV1 {
    /// Returns the exact deployment commit retained by this staging transaction.
    pub fn git_commit(&self) -> &str {
        self.prepared.git_commit()
    }

    /// Returns the exact deployment-manifest digest retained by this transaction.
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.prepared.manifest_sha256()
    }

    /// Returns the independently pinned base-image digest retained by this transaction.
    pub const fn base_image_sha256(&self) -> [u8; 32] {
        self.prepared.base_image_sha256()
    }

    /// Returns the random private child name beneath the caller's qualification parent.
    pub fn run_name(&self) -> &str {
        &self.run_name
    }

    /// Returns the run root plus its seven fixed empty child directories.
    pub const fn directory_count(&self) -> usize {
        1 + QUALIFICATION_STAGING_CHILDREN_V1.len()
    }

    /// Revalidates all retained evidence and the exact empty root-owned staging tree.
    pub fn revalidate(&self) -> Result<(), DeploymentVerificationErrorV1> {
        revalidate_staged_qualification(self, (0, 0))
    }

    /// Removes this transaction's exact empty staging tree and synchronizes its parent.
    pub fn cleanup(mut self) -> Result<(), DeploymentVerificationErrorV1> {
        self.cleanup_internal()
    }

    pub(super) fn prepared(&self) -> &PreparedCompilerExecutionQualificationV1 {
        &self.prepared
    }

    pub(super) fn root_descriptor(&self) -> &File {
        self.root
            .as_ref()
            .expect("active qualification staging retains its root descriptor")
    }

    pub(super) fn directory_descriptor(&self, name: &str) -> &File {
        &self
            .directories
            .iter()
            .find(|directory| directory.name == name)
            .expect("V1 staging directory name is fixed")
            .file
    }

    pub(super) fn prepared_parent_children(&self) -> [&str; 1] {
        [self.run_name.as_str()]
    }

    fn cleanup_or(
        &mut self,
        error: DeploymentVerificationErrorV1,
    ) -> DeploymentVerificationErrorV1 {
        match self.cleanup_internal() {
            Ok(()) => error,
            Err(cleanup) => super::invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                format!("qualification staging failed and cleanup also failed: {cleanup}"),
            ),
        }
    }

    fn cleanup_internal(&mut self) -> Result<(), DeploymentVerificationErrorV1> {
        if !self.cleanup_required {
            return Ok(());
        }
        if let Some(root) = &self.root {
            for name in self.created_directories.iter().rev() {
                match unlinkat(root, *name, AtFlags::REMOVEDIR) {
                    Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                    Err(source) => {
                        return Err(io_error("remove qualification staging directory", source));
                    }
                }
            }
        }
        self.directories.clear();
        self.created_directories.clear();
        self.root.take();
        match unlinkat(
            self.prepared.qualification_parent(),
            self.run_name.as_str(),
            AtFlags::REMOVEDIR,
        ) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => {}
            Err(source) => return Err(io_error("remove qualification staging root", source)),
        }
        self.prepared
            .qualification_parent()
            .sync_all()
            .map_err(|source| std_io_error("sync qualification parent after cleanup", source))?;
        self.cleanup_required = false;
        Ok(())
    }
}

impl Drop for StagedCompilerExecutionQualificationV1 {
    fn drop(&mut self) {
        let _ = self.cleanup_internal();
    }
}

/// Creates one exact empty staging tree for a prepared disposable-root qualification.
///
/// The process must have effective UID 0. The prepared parent must still be empty and satisfy its
/// root-owned policy. The returned value grants no mount, namespace, service, or execution
/// authority.
pub fn stage_compiler_execution_qualification_v1(
    prepared: PreparedCompilerExecutionQualificationV1,
) -> Result<StagedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "compiler-execution qualification staging requires effective UID 0",
        ));
    }
    stage_for_owner(prepared, (0, 0))
}

/// Recovers zero or one canonical staging transaction left by a terminated root worker.
///
/// The qualification parent must retain the production root-owned mode-`0700` policy. Recovery
/// rejects unknown or multiple top-level children, follows no symlink, crosses no mount, and
/// bounds both tree depth and entry count before deleting any disposable content.
pub fn recover_compiler_execution_qualification_parent_v1(
    qualification_parent: &std::path::Path,
) -> Result<CompilerExecutionQualificationRecoveryV1, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "qualification staging recovery requires effective UID 0",
        ));
    }
    recover_qualification_parent_for_owner(qualification_parent, (0, 0))
}

#[cfg(test)]
fn recover_qualification_parent_for_test_v1(
    qualification_parent: &std::path::Path,
    owner: (u32, u32),
) -> Result<CompilerExecutionQualificationRecoveryV1, DeploymentVerificationErrorV1> {
    recover_qualification_parent_for_owner(qualification_parent, owner)
}

fn recover_qualification_parent_for_owner(
    qualification_parent: &std::path::Path,
    owner: (u32, u32),
) -> Result<CompilerExecutionQualificationRecoveryV1, DeploymentVerificationErrorV1> {
    let parent = open_qualification_parent_metadata(qualification_parent, owner)?;
    let parent_snapshot = snapshot(
        &fstat(&parent)
            .map_err(|source| io_error("inspect qualification recovery parent", source))?,
    );
    let children = canonical_directory_children(&parent, "qualification recovery parent")?;
    if children.is_empty() {
        revalidate_recovery_parent(qualification_parent, &parent, parent_snapshot, owner)?;
        return Ok(CompilerExecutionQualificationRecoveryV1::AlreadyEmpty);
    }
    if children.len() != 1 || !canonical_recovery_name(&children[0]) {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidInventory,
            "qualification recovery parent does not contain exactly one canonical transaction",
        ));
    }
    let run_name = &children[0];
    let root = open_recovery_directory(&parent, run_name)?;
    let root_snapshot = validate_recovery_root(&root, owner)?;
    if root_snapshot.device != parent_snapshot.device {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            "qualification recovery staging root crosses a filesystem boundary",
        ));
    }
    let staging_children =
        canonical_directory_children(&root, "qualification recovery staging root")?;
    if staging_children.iter().any(|name| {
        !QUALIFICATION_STAGING_CHILDREN_V1
            .iter()
            .any(|expected| name == OsStr::new(expected))
    }) {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidInventory,
            "qualification recovery staging root contains an unknown top-level entry",
        ));
    }

    let mut preflight_budget = 0_usize;
    for name in &staging_children {
        let directory = open_recovery_directory(&root, name)?;
        require_recovery_device(&directory, parent_snapshot.device)?;
        validate_recovery_tree(&directory, parent_snapshot.device, 0, &mut preflight_budget)?;
    }

    let mut removal_budget = 0_usize;
    for name in staging_children.iter().rev() {
        let directory = open_recovery_directory(&root, name)?;
        require_recovery_device(&directory, parent_snapshot.device)?;
        remove_recovery_tree(&directory, parent_snapshot.device, 0, &mut removal_budget)?;
        directory
            .sync_all()
            .map_err(|source| std_io_error("sync recovered staging directory", source))?;
        drop(directory);
        unlinkat(&root, name, AtFlags::REMOVEDIR)
            .map_err(|source| io_error("remove recovered staging directory", source))?;
    }
    root.sync_all()
        .map_err(|source| std_io_error("sync recovered staging root", source))?;
    drop(root);
    unlinkat(&parent, run_name, AtFlags::REMOVEDIR)
        .map_err(|source| io_error("remove recovered staging root", source))?;
    parent
        .sync_all()
        .map_err(|source| std_io_error("sync qualification parent after recovery", source))?;
    revalidate_recovery_parent(qualification_parent, &parent, parent_snapshot, owner)?;
    Ok(CompilerExecutionQualificationRecoveryV1::Recovered)
}

fn canonical_recovery_name(name: &OsStr) -> bool {
    let bytes = name.as_bytes();
    bytes
        .strip_prefix(QUALIFICATION_STAGING_PREFIX_V1.as_bytes())
        .is_some_and(|suffix| {
            suffix.len() == 32
                && suffix
                    .iter()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        })
}

fn validate_recovery_root(
    root: &File,
    owner: (u32, u32),
) -> Result<ObjectSnapshotV1, DeploymentVerificationErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(root)
        .map_err(|source| io_error("inspect recovery-root descriptor flags", source))?;
    let status = rustix::fs::fcntl_getfl(root)
        .map_err(|source| io_error("inspect recovery-root status flags", source))?;
    let observed = snapshot(
        &fstat(root).map_err(|source| io_error("inspect qualification recovery root", source))?,
    );
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || FileType::from_raw_mode(observed.mode) != FileType::Directory
        || observed.mode & 0o7777 & !QUALIFICATION_STAGING_MODE_V1 != 0
        || observed.links == 0
        || (observed.uid, observed.gid) != owner
    {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            "qualification recovery root is not one root-owned staging-mode subset",
        ));
    }
    require_no_xattrs(root, "qualification recovery root")?;
    Ok(observed)
}

fn revalidate_recovery_parent(
    path: &std::path::Path,
    retained: &File,
    expected: ObjectSnapshotV1,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    verify_directory_children(retained, &[], "recovered qualification parent")?;
    let observed = snapshot(
        &fstat(retained)
            .map_err(|source| io_error("reinspect retained qualification parent", source))?,
    );
    let reopened = open_qualification_parent_metadata(path, owner)?;
    verify_directory_children(&reopened, &[], "canonical recovered qualification parent")?;
    let reopened = snapshot(
        &fstat(&reopened)
            .map_err(|source| io_error("reinspect canonical qualification parent", source))?,
    );
    if (
        observed.device,
        observed.inode,
        observed.mode,
        observed.uid,
        observed.gid,
    ) != (
        expected.device,
        expected.inode,
        expected.mode,
        expected.uid,
        expected.gid,
    ) || (
        reopened.device,
        reopened.inode,
        reopened.mode,
        reopened.uid,
        reopened.gid,
    ) != (
        expected.device,
        expected.inode,
        expected.mode,
        expected.uid,
        expected.gid,
    ) {
        return Err(changed(
            "qualification-parent identity changed during staging recovery",
        ));
    }
    Ok(())
}

fn require_recovery_device(
    directory: &File,
    expected_device: u64,
) -> Result<(), DeploymentVerificationErrorV1> {
    let stat = fstat(directory)
        .map_err(|source| io_error("inspect qualification recovery directory", source))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_dev != expected_device
    {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            "qualification recovery directory crosses a filesystem boundary",
        ));
    }
    Ok(())
}

fn validate_recovery_tree(
    directory: &File,
    expected_device: u64,
    depth: usize,
    budget: &mut usize,
) -> Result<(), DeploymentVerificationErrorV1> {
    let children = raw_directory_children(directory)?;
    consume_recovery_budget(depth, children.len(), budget)?;
    for name in children {
        let stat = statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|source| io_error("inspect qualification recovery entry", source))?;
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            let child = open_recovery_directory(directory, &name)?;
            require_recovery_device(&child, expected_device)?;
            validate_recovery_tree(&child, expected_device, depth + 1, budget)?;
        }
    }
    Ok(())
}

fn remove_recovery_tree(
    directory: &File,
    expected_device: u64,
    depth: usize,
    budget: &mut usize,
) -> Result<(), DeploymentVerificationErrorV1> {
    let children = raw_directory_children(directory)?;
    consume_recovery_budget(depth, children.len(), budget)?;
    for name in children {
        let stat = statat(directory, &name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|source| io_error("reinspect qualification recovery entry", source))?;
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            let child = open_recovery_directory(directory, &name)?;
            require_recovery_device(&child, expected_device)?;
            remove_recovery_tree(&child, expected_device, depth + 1, budget)?;
            drop(child);
            unlinkat(directory, &name, AtFlags::REMOVEDIR)
                .map_err(|source| io_error("remove recovered nested directory", source))?;
        } else {
            unlinkat(directory, &name, AtFlags::empty())
                .map_err(|source| io_error("remove recovered staging entry", source))?;
        }
    }
    Ok(())
}

fn consume_recovery_budget(
    depth: usize,
    entries: usize,
    budget: &mut usize,
) -> Result<(), DeploymentVerificationErrorV1> {
    if depth >= QUALIFICATION_RECOVERY_MAX_DEPTH_V1 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::CleanupFailed,
            "qualification recovery tree exceeds the depth bound",
        ));
    }
    *budget = budget.checked_add(entries).ok_or_else(|| {
        super::invalid(
            DeploymentVerificationErrorKindV1::CleanupFailed,
            "qualification recovery entry count overflowed",
        )
    })?;
    if *budget > QUALIFICATION_RECOVERY_MAX_ENTRIES_V1 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::CleanupFailed,
            "qualification recovery tree exceeds the entry bound",
        ));
    }
    Ok(())
}

fn raw_directory_children(
    directory: &File,
) -> Result<Vec<OsString>, DeploymentVerificationErrorV1> {
    let scan = openat(
        directory,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|source| io_error("retain qualification recovery directory", source))?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|source| io_error("enumerate qualification recovery directory", source))?;
    let mut observed = Vec::new();
    for entry in &mut entries {
        let entry =
            entry.map_err(|source| io_error("read qualification recovery entry", source))?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        if bytes.is_empty() || bytes.contains(&0) {
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InvalidInventory,
                "qualification recovery contains an invalid entry name",
            ));
        }
        observed.push(OsString::from_vec(bytes.to_vec()));
    }
    observed.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    Ok(observed)
}

fn open_recovery_directory(
    parent: &File,
    name: &OsStr,
) -> Result<File, DeploymentVerificationErrorV1> {
    openat2(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|source| io_error("open qualification recovery directory", source))
}

#[cfg(test)]
pub(super) fn stage_compiler_execution_qualification_for_test_v1(
    prepared: PreparedCompilerExecutionQualificationV1,
    owner: (u32, u32),
) -> Result<StagedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    stage_for_owner(prepared, owner)
}

#[cfg(test)]
pub(super) fn stage_compiler_execution_qualification_at_fault_for_test_v1(
    prepared: PreparedCompilerExecutionQualificationV1,
    owner: (u32, u32),
    point: QualificationStagingFaultPointV1,
) -> Result<StagedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    let mut hooks = InjectQualificationStagingFaultV1 {
        point,
        fired: false,
    };
    let result = stage_for_owner_with_hooks(prepared, owner, &mut hooks);
    assert!(hooks.fired, "requested staging fault point was not reached");
    result
}

#[cfg(test)]
pub(super) fn qualification_staging_fault_points_for_test_v1()
-> Vec<QualificationStagingFaultPointV1> {
    let mut points = vec![
        QualificationStagingFaultPointV1::BeforeRootCreate,
        QualificationStagingFaultPointV1::RootCreated,
        QualificationStagingFaultPointV1::RootMetadataSet,
    ];
    for index in 0..QUALIFICATION_STAGING_CHILDREN_V1.len() {
        points.push(QualificationStagingFaultPointV1::DirectoryCreated(index));
        points.push(QualificationStagingFaultPointV1::DirectoryMetadataSet(
            index,
        ));
    }
    points.extend([
        QualificationStagingFaultPointV1::TreeVerified,
        QualificationStagingFaultPointV1::RootSynced,
        QualificationStagingFaultPointV1::ParentPathVerified,
        QualificationStagingFaultPointV1::ParentSynced,
    ]);
    points
}

#[cfg(test)]
pub(super) fn revalidate_staged_qualification_for_test_v1(
    staged: &StagedCompilerExecutionQualificationV1,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    revalidate_staged_qualification(staged, owner)
}

fn stage_for_owner(
    prepared: PreparedCompilerExecutionQualificationV1,
    owner: (u32, u32),
) -> Result<StagedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    stage_for_owner_with_hooks(prepared, owner, &mut NoQualificationStagingFaultV1)
}

fn stage_for_owner_with_hooks(
    prepared: PreparedCompilerExecutionQualificationV1,
    owner: (u32, u32),
    hooks: &mut impl QualificationStagingHooksV1,
) -> Result<StagedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    revalidate_prepared_qualification_with_parent_children(&prepared, owner, &[])?;
    let parent_snapshot = snapshot(
        &fstat(prepared.qualification_parent())
            .map_err(|source| io_error("inspect qualification staging parent", source))?,
    );

    for _ in 0..16 {
        hooks.checkpoint(QualificationStagingFaultPointV1::BeforeRootCreate)?;
        let run_name = random_staging_name(
            QUALIFICATION_STAGING_PREFIX_V1,
            "generate qualification staging randomness",
        )?;
        match mkdirat(
            prepared.qualification_parent(),
            run_name.as_str(),
            Mode::from_raw_mode(QUALIFICATION_STAGING_MODE_V1),
        ) {
            Ok(()) => {
                return finish_staging_create(prepared, run_name, parent_snapshot, owner, hooks);
            }
            Err(rustix::io::Errno::EXIST) => continue,
            Err(source) => return Err(io_error("create qualification staging root", source)),
        }
    }
    Err(super::invalid(
        DeploymentVerificationErrorKindV1::Io,
        "could not allocate a unique qualification staging-root name",
    ))
}

fn finish_staging_create(
    prepared: PreparedCompilerExecutionQualificationV1,
    run_name: String,
    parent_snapshot: ObjectSnapshotV1,
    owner: (u32, u32),
    hooks: &mut impl QualificationStagingHooksV1,
) -> Result<StagedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    let mut staged = StagedCompilerExecutionQualificationV1 {
        prepared,
        run_name,
        root: None,
        created_directories: Vec::with_capacity(QUALIFICATION_STAGING_CHILDREN_V1.len()),
        directories: Vec::with_capacity(QUALIFICATION_STAGING_CHILDREN_V1.len()),
        cleanup_required: true,
    };
    if let Err(error) = hooks.checkpoint(QualificationStagingFaultPointV1::RootCreated) {
        return Err(staged.cleanup_or(error));
    }
    let root = match open_staged_directory(
        staged.prepared.qualification_parent(),
        staged.run_name.as_str(),
    ) {
        Ok(root) => root,
        Err(error) => return Err(staged.cleanup_or(error)),
    };
    staged.root = Some(root);
    let root = staged
        .root
        .as_ref()
        .expect("new qualification staging root retains its descriptor");
    if let Err(error) = set_owner_and_mode(root, owner, QUALIFICATION_STAGING_MODE_V1) {
        return Err(staged.cleanup_or(error));
    }
    let root_snapshot = match validate_directory_mode(
        root,
        Some(owner),
        QUALIFICATION_STAGING_MODE_V1,
        "qualification staging root",
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => return Err(staged.cleanup_or(error)),
    };
    if root_snapshot.device != parent_snapshot.device {
        let error = super::invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            "qualification staging root crosses a filesystem boundary",
        );
        return Err(staged.cleanup_or(error));
    }
    if let Err(error) = hooks.checkpoint(QualificationStagingFaultPointV1::RootMetadataSet) {
        return Err(staged.cleanup_or(error));
    }

    for (index, &name) in QUALIFICATION_STAGING_CHILDREN_V1.iter().enumerate() {
        if let Err(source) = mkdirat(
            root,
            name,
            Mode::from_raw_mode(QUALIFICATION_STAGING_MODE_V1),
        ) {
            let error = io_error("create qualification staging directory", source);
            return Err(staged.cleanup_or(error));
        }
        staged.created_directories.push(name);
        if let Err(error) =
            hooks.checkpoint(QualificationStagingFaultPointV1::DirectoryCreated(index))
        {
            return Err(staged.cleanup_or(error));
        }
        let directory = match open_staged_directory(root, name) {
            Ok(directory) => directory,
            Err(error) => return Err(staged.cleanup_or(error)),
        };
        staged.directories.push(StagedDirectoryV1 {
            name,
            file: directory,
        });
        let directory = &staged
            .directories
            .last()
            .expect("created staging directory is retained")
            .file;
        if let Err(error) = set_owner_and_mode(directory, owner, QUALIFICATION_STAGING_MODE_V1) {
            return Err(staged.cleanup_or(error));
        }
        let directory_snapshot = match validate_directory_mode(
            directory,
            Some(owner),
            QUALIFICATION_STAGING_MODE_V1,
            "qualification staging directory",
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => return Err(staged.cleanup_or(error)),
        };
        if directory_snapshot.device != parent_snapshot.device {
            let error = super::invalid(
                DeploymentVerificationErrorKindV1::InvalidMetadata,
                "qualification staging directory crosses a filesystem boundary",
            );
            return Err(staged.cleanup_or(error));
        }
        if let Err(error) = hooks.checkpoint(
            QualificationStagingFaultPointV1::DirectoryMetadataSet(index),
        ) {
            return Err(staged.cleanup_or(error));
        }
    }

    if let Err(error) = revalidate_staged_qualification(&staged, owner) {
        return Err(staged.cleanup_or(error));
    }
    if let Err(error) = hooks.checkpoint(QualificationStagingFaultPointV1::TreeVerified) {
        return Err(staged.cleanup_or(error));
    }
    if let Err(source) = staged
        .root
        .as_ref()
        .expect("completed staging root retains its descriptor")
        .sync_all()
    {
        let error = std_io_error("sync qualification staging root", source);
        return Err(staged.cleanup_or(error));
    }
    if let Err(error) = hooks.checkpoint(QualificationStagingFaultPointV1::RootSynced) {
        return Err(staged.cleanup_or(error));
    }
    if let Err(error) = revalidate_prepared_qualification_with_parent_children(
        &staged.prepared,
        owner,
        &[staged.run_name.as_str()],
    ) {
        return Err(staged.cleanup_or(error));
    }
    if let Err(error) = hooks.checkpoint(QualificationStagingFaultPointV1::ParentPathVerified) {
        return Err(staged.cleanup_or(error));
    }
    if let Err(source) = staged.prepared.qualification_parent().sync_all() {
        let error = std_io_error("sync qualification parent after staging", source);
        return Err(staged.cleanup_or(error));
    }
    if let Err(error) = hooks.checkpoint(QualificationStagingFaultPointV1::ParentSynced) {
        return Err(staged.cleanup_or(error));
    }
    Ok(staged)
}

fn revalidate_staged_qualification(
    staged: &StagedCompilerExecutionQualificationV1,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    if !staged.cleanup_required || staged.root.is_none() {
        return Err(changed("qualification staging custody is no longer active"));
    }
    revalidate_prepared_qualification_with_parent_children(
        &staged.prepared,
        owner,
        &[staged.run_name.as_str()],
    )?;
    let parent_snapshot = snapshot(
        &fstat(staged.prepared.qualification_parent())
            .map_err(|source| io_error("reinspect qualification staging parent", source))?,
    );
    let root = staged
        .root
        .as_ref()
        .expect("active staging custody retains its root descriptor");
    let root_snapshot = validate_directory_mode(
        root,
        Some(owner),
        QUALIFICATION_STAGING_MODE_V1,
        "qualification staging root",
    )?;
    if root_snapshot.device != parent_snapshot.device {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            "qualification staging root changed filesystem",
        ));
    }
    verify_directory_children(
        root,
        QUALIFICATION_STAGING_CHILDREN_V1,
        "qualification staging root",
    )?;
    if staged.created_directories != QUALIFICATION_STAGING_CHILDREN_V1
        || staged.directories.len() != QUALIFICATION_STAGING_CHILDREN_V1.len()
    {
        return Err(changed(
            "qualification staging descriptor inventory changed",
        ));
    }
    for (retained, expected_name) in staged
        .directories
        .iter()
        .zip(QUALIFICATION_STAGING_CHILDREN_V1)
    {
        if retained.name != *expected_name {
            return Err(changed("qualification staging descriptor order changed"));
        }
        let retained_snapshot = validate_directory_mode(
            &retained.file,
            Some(owner),
            QUALIFICATION_STAGING_MODE_V1,
            "qualification staging directory",
        )?;
        if retained_snapshot.device != parent_snapshot.device {
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InvalidMetadata,
                "qualification staging directory changed filesystem",
            ));
        }
        verify_directory_children(&retained.file, &[], "qualification staging directory")?;
        let reopened = open_staged_directory(root, expected_name)?;
        if snapshot(
            &fstat(&reopened)
                .map_err(|source| io_error("reinspect qualification staging pathname", source))?,
        ) != retained_snapshot
        {
            return Err(changed("qualification staging pathname changed"));
        }
    }
    let reopened_root = open_staged_directory(
        staged.prepared.qualification_parent(),
        staged.run_name.as_str(),
    )?;
    if snapshot(
        &fstat(&reopened_root)
            .map_err(|source| io_error("reinspect qualification staging root path", source))?,
    ) != root_snapshot
    {
        return Err(changed("qualification staging-root pathname changed"));
    }
    verify_directory_children(
        root,
        QUALIFICATION_STAGING_CHILDREN_V1,
        "qualification staging root",
    )
}

fn open_staged_directory(parent: &File, name: &str) -> Result<File, DeploymentVerificationErrorV1> {
    openat2(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|source| io_error("open qualification staging directory", source))
}

#[cfg(test)]
mod recovery_tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    use std::path::{Path, PathBuf};

    use super::*;

    const RUN_A: &str = ".compiler-execution-qualification-v1-0123456789abcdef0123456789abcdef";
    const RUN_B: &str = ".compiler-execution-qualification-v1-fedcba9876543210fedcba9876543210";

    fn owner() -> (u32, u32) {
        (
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
    }

    fn qualification_parent() -> (tempfile::TempDir, PathBuf) {
        let temporary = tempfile::tempdir().unwrap();
        let parent = temporary.path().join("qualification");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        (temporary, parent)
    }

    fn staging_root(parent: &Path, name: &str) -> PathBuf {
        let root = parent.join(name);
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        root
    }

    #[test]
    fn recovery_removes_bounded_partial_and_non_utf8_disposable_content() {
        let (_temporary, parent) = qualification_parent();
        let root = staging_root(&parent, RUN_A);
        let upper = root.join("upper");
        let work = root.join("work");
        fs::create_dir(&upper).unwrap();
        fs::create_dir(&work).unwrap();
        fs::create_dir_all(upper.join("nested/deeper")).unwrap();
        fs::write(upper.join("nested/deeper/file"), b"disposable").unwrap();
        symlink("/must-not-be-followed", upper.join("link")).unwrap();
        let mut non_utf8 = work.clone();
        non_utf8.push(OsString::from_vec(vec![0xff, 0xfe]));
        fs::write(non_utf8, b"disposable").unwrap();

        assert_eq!(
            recover_qualification_parent_for_test_v1(&parent, owner()).unwrap(),
            CompilerExecutionQualificationRecoveryV1::Recovered
        );
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
        assert_eq!(
            recover_qualification_parent_for_test_v1(&parent, owner()).unwrap(),
            CompilerExecutionQualificationRecoveryV1::AlreadyEmpty
        );
    }

    #[test]
    fn recovery_rejects_unknown_or_multiple_transactions_without_deleting_them() {
        let (_temporary, parent) = qualification_parent();
        fs::create_dir(parent.join("unknown")).unwrap();
        assert_eq!(
            recover_qualification_parent_for_test_v1(&parent, owner())
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InvalidInventory
        );
        assert!(parent.join("unknown").is_dir());

        fs::remove_dir(parent.join("unknown")).unwrap();
        staging_root(&parent, RUN_A);
        staging_root(&parent, RUN_B);
        assert_eq!(
            recover_qualification_parent_for_test_v1(&parent, owner())
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InvalidInventory
        );
        assert!(parent.join(RUN_A).is_dir());
        assert!(parent.join(RUN_B).is_dir());
    }

    #[test]
    fn recovery_rejects_unknown_staging_children_before_deletion() {
        let (_temporary, parent) = qualification_parent();
        let root = staging_root(&parent, RUN_A);
        fs::create_dir(root.join("upper")).unwrap();
        fs::create_dir(root.join("rogue")).unwrap();
        assert_eq!(
            recover_qualification_parent_for_test_v1(&parent, owner())
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InvalidInventory
        );
        assert!(root.join("upper").is_dir());
        assert!(root.join("rogue").is_dir());
    }

    #[test]
    fn recovery_identity_accepts_a_pre_metadata_mode_subset() {
        let (_temporary, parent) = qualification_parent();
        let root = staging_root(&parent, RUN_A);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).unwrap();
        let retained = File::open(&root).unwrap();
        assert_eq!(
            validate_recovery_root(&retained, owner()).unwrap().mode & 0o7777,
            0o500
        );
    }
}
