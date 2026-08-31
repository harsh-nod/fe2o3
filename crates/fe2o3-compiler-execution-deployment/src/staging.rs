use std::fmt;
use std::fs::File;

use rustix::fs::{AtFlags, Mode, OFlags, ResolveFlags, fstat, mkdirat, openat2, unlinkat};

use super::install::set_owner_and_mode;
use super::qualification::{
    PreparedCompilerExecutionQualificationV1,
    revalidate_prepared_qualification_with_parent_children,
};
use super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, ObjectSnapshotV1, changed,
    io_error, lower_hex, random_staging_name, snapshot, std_io_error, validate_directory_mode,
    verify_directory_children,
};

const QUALIFICATION_STAGING_PREFIX_V1: &str = ".compiler-execution-qualification-v1-";
const QUALIFICATION_STAGING_MODE_V1: u32 = 0o700;
const QUALIFICATION_STAGING_CHILDREN_V1: &[&str] =
    &["base", "evidence", "root", "run", "state", "upper", "work"];

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
