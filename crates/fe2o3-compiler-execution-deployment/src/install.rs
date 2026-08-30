use std::fmt;
use std::fs::File;
use std::io::Write as _;
use std::os::unix::fs::FileExt as _;
use std::path::Path;

use rustix::fs::{
    AtFlags, Gid, Mode, OFlags, RenameFlags, ResolveFlags, Uid, fchmod, fchown, fstat, mkdirat,
    openat, openat2, renameat_with, unlinkat,
};
use sha2::{Digest as _, Sha256};

use super::{
    AdmittedSourceV1, DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1,
    ManifestEntryV1, ObjectSnapshotV1, SealedDeploymentFileV1,
    VerifiedCompilerExecutionDeploymentV1, admit_source_file, changed, io_error, lower_hex,
    open_beneath, parse_manifest, snapshot, std_io_error, validate_build_info,
    validate_directory_mode, validate_sealed_file, validate_sealed_files, validate_sha256sums,
    verify_directory_children,
};

const INSTALL_PARENT_MODE_V1: u32 = 0o700;
const INSTALLED_DIRECTORY_MODE_V1: u32 = 0o755;
const INSTALL_ROOT_PREFIX_V1: &str = "compiler-execution-v1-";
const STAGING_PREFIX_V1: &str = ".compiler-execution-v1-staging-";
const COPY_BUFFER_BYTES_V1: usize = 64 * 1024;

const INSTALL_ROOT_CHILDREN_V1: &[&str] = &["usr"];
const INSTALL_USR_CHILDREN_V1: &[&str] = &["lib", "libexec", "share"];
const INSTALL_LIB_CHILDREN_V1: &[&str] = &["systemd", "sysusers.d", "tmpfiles.d"];
const INSTALL_SYSTEMD_CHILDREN_V1: &[&str] = &["system"];
const INSTALL_SYSTEMD_SYSTEM_CHILDREN_V1: &[&str] = &[
    "fe2o3-compiler-execution.service",
    "fe2o3-compiler-execution.socket",
];
const INSTALL_SYSUSERS_CHILDREN_V1: &[&str] = &["fe2o3-compiler-execution.conf"];
const INSTALL_TMPFILES_CHILDREN_V1: &[&str] = &["fe2o3-compiler-execution.conf"];
const INSTALL_LIBEXEC_CHILDREN_V1: &[&str] = &["fe2o3"];
const INSTALL_IMAGE_CHILDREN_V1: &[&str] = &[
    "fe2o3-compiler-execution-coordinator",
    "fe2o3-compiler-execution-issuer",
    "fe2o3-compiler-execution-provision",
    "fe2o3-compiler-execution-supervisor",
    "fe2o3-external-anchor-provisioning-helper",
    "fe2o3-external-anchor-service",
    "fe2o3-static-preexec-launcher",
];
const INSTALL_SHARE_CHILDREN_V1: &[&str] = &["fe2o3"];
const INSTALL_SHARE_FE2O3_CHILDREN_V1: &[&str] = &["compiler-execution"];
const INSTALL_IDENTITY_CHILDREN_V1: &[&str] = &["BUILD-INFO", "INSTALL-MANIFEST-V1", "SHA256SUMS"];

const INSTALL_DIRECTORY_SPECS_V1: &[(&str, &[&str])] = &[
    ("usr", INSTALL_USR_CHILDREN_V1),
    ("usr/lib", INSTALL_LIB_CHILDREN_V1),
    ("usr/lib/systemd", INSTALL_SYSTEMD_CHILDREN_V1),
    ("usr/lib/systemd/system", INSTALL_SYSTEMD_SYSTEM_CHILDREN_V1),
    ("usr/lib/sysusers.d", INSTALL_SYSUSERS_CHILDREN_V1),
    ("usr/lib/tmpfiles.d", INSTALL_TMPFILES_CHILDREN_V1),
    ("usr/libexec", INSTALL_LIBEXEC_CHILDREN_V1),
    ("usr/libexec/fe2o3", INSTALL_IMAGE_CHILDREN_V1),
    ("usr/share", INSTALL_SHARE_CHILDREN_V1),
    ("usr/share/fe2o3", INSTALL_SHARE_FE2O3_CHILDREN_V1),
    (
        "usr/share/fe2o3/compiler-execution",
        INSTALL_IDENTITY_CHILDREN_V1,
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InstallationFaultPointV1 {
    BeforeStagingCreate,
    StagingCreated,
    StagingMetadataSet,
    DirectoryCreated(usize),
    DirectoryMetadataSet(usize),
    FileCreated(usize),
    FileWritten(usize),
    FileModeSet(usize),
    FileSynced(usize),
    RootModeSet,
    RootVerified,
    DirectorySynced(usize),
    RootSynced,
    ParentPathVerified,
    RootRenamed,
    ParentSynced,
    PublishedRootVerified,
}

trait InstallationHooksV1 {
    fn checkpoint(
        &mut self,
        point: InstallationFaultPointV1,
    ) -> Result<(), DeploymentVerificationErrorV1>;
}

struct NoInstallationFaultV1;

impl InstallationHooksV1 for NoInstallationFaultV1 {
    fn checkpoint(
        &mut self,
        _point: InstallationFaultPointV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InstallParentReplacementPointV1 {
    DuringCopy,
    AfterPublication,
}

#[cfg(test)]
struct ReplaceInstallParentPathV1 {
    original: std::path::PathBuf,
    displaced: std::path::PathBuf,
    trigger: InstallationFaultPointV1,
    fired: bool,
}

#[cfg(test)]
impl InstallationHooksV1 for ReplaceInstallParentPathV1 {
    fn checkpoint(
        &mut self,
        point: InstallationFaultPointV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        if !self.fired && self.trigger == point {
            std::fs::rename(&self.original, &self.displaced).map_err(|source| {
                std_io_error("displace install-parent pathname during test", source)
            })?;
            std::fs::create_dir(&self.original).map_err(|source| {
                std_io_error("replace install-parent pathname during test", source)
            })?;
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(
                &self.original,
                std::fs::Permissions::from_mode(INSTALL_PARENT_MODE_V1),
            )
            .map_err(|source| {
                std_io_error("set replacement install-parent mode during test", source)
            })?;
            self.fired = true;
        }
        Ok(())
    }
}

#[cfg(test)]
struct InjectInstallationFaultV1 {
    point: InstallationFaultPointV1,
    fired: bool,
}

#[cfg(test)]
impl InstallationHooksV1 for InjectInstallationFaultV1 {
    fn checkpoint(
        &mut self,
        point: InstallationFaultPointV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        if !self.fired && self.point == point {
            self.fired = true;
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InjectedFailure,
                format!("injected installed-root interruption at {point:?}"),
            ));
        }
        Ok(())
    }
}

/// Whether an exact installed root was newly published or safely reacquired.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionInstalledRootPublicationV1 {
    /// This call published the complete root with one no-replace rename.
    Created,
    /// The content-addressed root already existed and passed complete revalidation.
    Reacquired,
}

/// Move-only, authority-free custody of one completely verified installed root.
///
/// The retained root descriptor is private. This value grants no service, compiler, signing,
/// publication, loading, launch, execution, or GPU authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_deployment::InstalledCompilerExecutionDeploymentV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<InstalledCompilerExecutionDeploymentV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_deployment::InstalledCompilerExecutionDeploymentV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<InstalledCompilerExecutionDeploymentV1>();
/// ```
pub struct InstalledCompilerExecutionDeploymentV1 {
    deployment: VerifiedCompilerExecutionDeploymentV1,
    root_name: String,
    publication: CompilerExecutionInstalledRootPublicationV1,
    root: File,
}

impl fmt::Debug for InstalledCompilerExecutionDeploymentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstalledCompilerExecutionDeploymentV1")
            .field("git_commit", &self.deployment.git_commit)
            .field("target", &self.deployment.target)
            .field(
                "manifest_sha256",
                &lower_hex(&self.deployment.manifest_sha256),
            )
            .field("root_name", &self.root_name)
            .field("publication", &self.publication)
            .field("authority", &"installed-root-custody-only")
            .finish_non_exhaustive()
    }
}

impl InstalledCompilerExecutionDeploymentV1 {
    /// Returns the exact source commit bound by the installed root.
    pub fn git_commit(&self) -> &str {
        &self.deployment.git_commit
    }

    /// Returns the exact target bound by the installed root.
    pub fn target(&self) -> &str {
        &self.deployment.target
    }

    /// Returns the manifest digest that deterministically names the installed root.
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.deployment.manifest_sha256
    }

    /// Returns the deterministic final name beneath the caller's retained install parent.
    pub fn root_name(&self) -> &str {
        &self.root_name
    }

    /// Returns whether this call created or reacquired the exact installed root.
    pub const fn publication(&self) -> CompilerExecutionInstalledRootPublicationV1 {
        self.publication
    }

    /// Returns the exact manifest-plus-content file count in the installed root.
    pub const fn file_count(&self) -> usize {
        14
    }

    /// Revalidates the complete root-owned tree against its retained sealed sources.
    ///
    /// This does not expose the retained root or source descriptors and grants no execution or
    /// service authority.
    pub fn revalidate(&self) -> Result<(), DeploymentVerificationErrorV1> {
        revalidate_installed_deployment(self, (0, 0))
    }
}

/// Derives the sole V1 final-root name from an admitted manifest SHA-256.
pub fn compiler_execution_install_root_name_v1(manifest_sha256: [u8; 32]) -> String {
    format!("{INSTALL_ROOT_PREFIX_V1}{}", lower_hex(&manifest_sha256))
}

/// Installs one verified deployment into a root-owned offline-root parent.
///
/// The process must have effective UID 0. `install_parent` must be a root-owned, root-group,
/// mode-`0700` directory without extended attributes. The final name is derived from the admitted
/// manifest digest and cannot be selected by the caller.
pub fn install_compiler_execution_deployment_v1(
    deployment: VerifiedCompilerExecutionDeploymentV1,
    install_parent: &Path,
) -> Result<InstalledCompilerExecutionDeploymentV1, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "compiler-execution installation requires effective UID 0",
        ));
    }
    install_for_owner(deployment, install_parent, (0, 0))
}

#[cfg(test)]
pub(super) fn install_compiler_execution_deployment_for_test_v1(
    deployment: VerifiedCompilerExecutionDeploymentV1,
    install_parent: &Path,
    owner: (u32, u32),
) -> Result<InstalledCompilerExecutionDeploymentV1, DeploymentVerificationErrorV1> {
    install_for_owner(deployment, install_parent, owner)
}

#[cfg(test)]
pub(super) fn install_compiler_execution_deployment_at_fault_for_test_v1(
    deployment: VerifiedCompilerExecutionDeploymentV1,
    install_parent: &Path,
    owner: (u32, u32),
    point: InstallationFaultPointV1,
) -> Result<InstalledCompilerExecutionDeploymentV1, DeploymentVerificationErrorV1> {
    let mut hooks = InjectInstallationFaultV1 {
        point,
        fired: false,
    };
    let result = install_for_owner_with_hooks(deployment, install_parent, owner, &mut hooks);
    assert!(
        hooks.fired,
        "requested installation fault point was not reached"
    );
    result
}

#[cfg(test)]
pub(super) fn install_compiler_execution_deployment_with_parent_replacement_for_test_v1(
    deployment: VerifiedCompilerExecutionDeploymentV1,
    install_parent: &Path,
    displaced_parent: &Path,
    owner: (u32, u32),
    point: InstallParentReplacementPointV1,
) -> Result<InstalledCompilerExecutionDeploymentV1, DeploymentVerificationErrorV1> {
    let trigger = match point {
        InstallParentReplacementPointV1::DuringCopy => InstallationFaultPointV1::FileWritten(0),
        InstallParentReplacementPointV1::AfterPublication => InstallationFaultPointV1::RootRenamed,
    };
    let mut hooks = ReplaceInstallParentPathV1 {
        original: install_parent.to_owned(),
        displaced: displaced_parent.to_owned(),
        trigger,
        fired: false,
    };
    let result = install_for_owner_with_hooks(deployment, install_parent, owner, &mut hooks);
    assert!(
        hooks.fired,
        "install-parent replacement checkpoint was not reached"
    );
    result
}

#[cfg(test)]
pub(super) fn installation_fault_points_for_test_v1() -> Vec<InstallationFaultPointV1> {
    let mut points = vec![
        InstallationFaultPointV1::BeforeStagingCreate,
        InstallationFaultPointV1::StagingCreated,
        InstallationFaultPointV1::StagingMetadataSet,
    ];
    for index in 0..INSTALL_DIRECTORY_SPECS_V1.len() {
        points.push(InstallationFaultPointV1::DirectoryCreated(index));
        points.push(InstallationFaultPointV1::DirectoryMetadataSet(index));
    }
    for index in 0..14 {
        points.push(InstallationFaultPointV1::FileCreated(index));
        points.push(InstallationFaultPointV1::FileWritten(index));
        points.push(InstallationFaultPointV1::FileModeSet(index));
        points.push(InstallationFaultPointV1::FileSynced(index));
    }
    points.extend([
        InstallationFaultPointV1::RootModeSet,
        InstallationFaultPointV1::RootVerified,
    ]);
    for index in (0..INSTALL_DIRECTORY_SPECS_V1.len()).rev() {
        points.push(InstallationFaultPointV1::DirectorySynced(index));
    }
    points.extend([
        InstallationFaultPointV1::RootSynced,
        InstallationFaultPointV1::ParentPathVerified,
        InstallationFaultPointV1::RootRenamed,
        InstallationFaultPointV1::ParentSynced,
        InstallationFaultPointV1::PublishedRootVerified,
    ]);
    points
}

#[cfg(test)]
pub(super) fn installation_fault_is_after_publication_for_test_v1(
    point: InstallationFaultPointV1,
) -> bool {
    matches!(
        point,
        InstallationFaultPointV1::RootRenamed
            | InstallationFaultPointV1::ParentSynced
            | InstallationFaultPointV1::PublishedRootVerified
    )
}

fn install_for_owner(
    deployment: VerifiedCompilerExecutionDeploymentV1,
    install_parent: &Path,
    owner: (u32, u32),
) -> Result<InstalledCompilerExecutionDeploymentV1, DeploymentVerificationErrorV1> {
    install_for_owner_with_hooks(
        deployment,
        install_parent,
        owner,
        &mut NoInstallationFaultV1,
    )
}

fn install_for_owner_with_hooks(
    deployment: VerifiedCompilerExecutionDeploymentV1,
    install_parent: &Path,
    owner: (u32, u32),
    hooks: &mut impl InstallationHooksV1,
) -> Result<InstalledCompilerExecutionDeploymentV1, DeploymentVerificationErrorV1> {
    validate_sealed_file(&deployment.manifest)?;
    validate_sealed_files(&deployment.files)?;
    let parent = open_install_parent(install_parent, owner)?;
    let parent_snapshot =
        snapshot(&fstat(&parent).map_err(|source| io_error("inspect install parent", source))?);
    let root_name = compiler_execution_install_root_name_v1(deployment.manifest_sha256);

    if let Some(root) = open_named_root(&parent, &root_name)? {
        verify_installed_root(&root, owner, &deployment)?;
        verify_install_parent_path(install_parent, &parent, owner)?;
        return Ok(installed_result(
            deployment,
            root_name,
            CompilerExecutionInstalledRootPublicationV1::Reacquired,
            root,
        ));
    }

    let mut staging = StagingRootV1::create(&parent, parent_snapshot, owner, hooks)?;
    let prepared = prepare_staging_root(&mut staging, owner, &deployment, hooks);
    if let Err(error) = prepared {
        return Err(staging.cleanup_or(error));
    }
    if let Err(error) = verify_install_parent_path(install_parent, &parent, owner) {
        return Err(staging.cleanup_or(error));
    }
    if let Err(error) = hooks.checkpoint(InstallationFaultPointV1::ParentPathVerified) {
        return Err(staging.cleanup_or(error));
    }

    match renameat_with(
        &parent,
        staging.name.as_str(),
        &parent,
        root_name.as_str(),
        RenameFlags::NOREPLACE,
    ) {
        Ok(()) => {
            staging.cleanup_required = false;
            if let Err(error) = hooks.checkpoint(InstallationFaultPointV1::RootRenamed) {
                return Err(publication_ambiguous(error));
            }
        }
        Err(rustix::io::Errno::EXIST) => {
            let existing = open_named_root(&parent, &root_name)?.ok_or_else(|| {
                changed("installed root disappeared after no-replace publication conflict")
            })?;
            verify_installed_root(&existing, owner, &deployment)?;
            staging.cleanup()?;
            return Ok(installed_result(
                deployment,
                root_name,
                CompilerExecutionInstalledRootPublicationV1::Reacquired,
                existing,
            ));
        }
        Err(source) => {
            let error = io_error("atomically publish installed root", source);
            return Err(staging.cleanup_or(error));
        }
    }

    parent
        .sync_all()
        .map_err(|source| publication_ambiguous(format!("install-parent sync failed: {source}")))?;
    hooks
        .checkpoint(InstallationFaultPointV1::ParentSynced)
        .map_err(publication_ambiguous)?;
    verify_install_parent_path(install_parent, &parent, owner).map_err(publication_ambiguous)?;
    let published = open_named_root(&parent, &root_name)
        .map_err(publication_ambiguous)?
        .ok_or_else(|| publication_ambiguous("installed root disappeared after publication"))?;
    let retained_snapshot = snapshot(
        &fstat(
            staging
                .root
                .as_ref()
                .expect("active staging root retains its descriptor"),
        )
        .map_err(|source| {
            publication_ambiguous(format!("cannot inspect retained published root: {source}"))
        })?,
    );
    let published_snapshot = snapshot(&fstat(&published).map_err(|source| {
        publication_ambiguous(format!("cannot inspect reopened published root: {source}"))
    })?);
    if retained_snapshot.device != published_snapshot.device
        || retained_snapshot.inode != published_snapshot.inode
    {
        return Err(publication_ambiguous(
            "installed-root pathname does not name the prepared root",
        ));
    }
    verify_installed_root(&published, owner, &deployment).map_err(publication_ambiguous)?;
    hooks
        .checkpoint(InstallationFaultPointV1::PublishedRootVerified)
        .map_err(publication_ambiguous)?;
    let root = staging
        .root
        .take()
        .expect("published staging root retains its descriptor");
    Ok(installed_result(
        deployment,
        root_name,
        CompilerExecutionInstalledRootPublicationV1::Created,
        root,
    ))
}

fn installed_result(
    deployment: VerifiedCompilerExecutionDeploymentV1,
    root_name: String,
    publication: CompilerExecutionInstalledRootPublicationV1,
    root: File,
) -> InstalledCompilerExecutionDeploymentV1 {
    InstalledCompilerExecutionDeploymentV1 {
        deployment,
        root_name,
        publication,
        root,
    }
}

#[cfg(test)]
pub(super) fn revalidate_installed_deployment_for_test_v1(
    installed: &InstalledCompilerExecutionDeploymentV1,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    revalidate_installed_deployment(installed, owner)
}

pub(super) fn revalidate_installed_deployment(
    installed: &InstalledCompilerExecutionDeploymentV1,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    validate_sealed_file(&installed.deployment.manifest)?;
    validate_sealed_files(&installed.deployment.files)?;
    verify_installed_root(&installed.root, owner, &installed.deployment)
}

fn open_install_parent(
    path: &Path,
    owner: (u32, u32),
) -> Result<File, DeploymentVerificationErrorV1> {
    let parent = openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|source| io_error("open compiler-execution install parent", source))?;
    validate_directory_mode(
        &parent,
        Some(owner),
        INSTALL_PARENT_MODE_V1,
        "compiler-execution install parent",
    )?;
    Ok(parent)
}

fn verify_install_parent_path(
    path: &Path,
    retained: &File,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    let reopened = open_install_parent(path, owner)?;
    let retained = snapshot(
        &fstat(retained).map_err(|source| io_error("reinspect retained install parent", source))?,
    );
    let reopened = snapshot(
        &fstat(&reopened)
            .map_err(|source| io_error("reinspect canonical install parent", source))?,
    );
    if retained.device != reopened.device
        || retained.inode != reopened.inode
        || retained.mode != reopened.mode
        || retained.uid != reopened.uid
        || retained.gid != reopened.gid
    {
        return Err(changed(
            "install-parent pathname changed during installed-root publication",
        ));
    }
    Ok(())
}

fn publication_ambiguous(error: impl fmt::Display) -> DeploymentVerificationErrorV1 {
    super::invalid(
        DeploymentVerificationErrorKindV1::PublicationAmbiguous,
        format!("installed root was published but final validation failed: {error}"),
    )
}

fn open_named_root(
    parent: &File,
    name: &str,
) -> Result<Option<File>, DeploymentVerificationErrorV1> {
    match openat2(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    ) {
        Ok(descriptor) => Ok(Some(File::from(descriptor))),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(source) => Err(io_error("open content-addressed installed root", source)),
    }
}

fn prepare_staging_root(
    staging: &mut StagingRootV1,
    owner: (u32, u32),
    deployment: &VerifiedCompilerExecutionDeploymentV1,
    hooks: &mut impl InstallationHooksV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    for (index, &(path, _)) in INSTALL_DIRECTORY_SPECS_V1.iter().enumerate() {
        staging.create_directory(path, owner, index, hooks)?;
    }
    for (index, source) in std::iter::once(&deployment.manifest)
        .chain(&deployment.files)
        .enumerate()
    {
        staging.copy_source(source, owner, index, hooks)?;
    }
    let root = staging
        .root
        .as_ref()
        .expect("active staging root retains its descriptor");
    fchmod(root, Mode::from_raw_mode(INSTALLED_DIRECTORY_MODE_V1))
        .map_err(|source| io_error("set installed-root mode", source))?;
    hooks.checkpoint(InstallationFaultPointV1::RootModeSet)?;
    verify_installed_root(root, owner, deployment)?;
    hooks.checkpoint(InstallationFaultPointV1::RootVerified)?;
    for (index, &(path, _)) in INSTALL_DIRECTORY_SPECS_V1.iter().enumerate().rev() {
        open_beneath(root, path, true)?
            .sync_all()
            .map_err(|source| std_io_error("sync installed-root directory", source))?;
        hooks.checkpoint(InstallationFaultPointV1::DirectorySynced(index))?;
    }
    root.sync_all()
        .map_err(|source| std_io_error("sync complete installed root", source))?;
    hooks.checkpoint(InstallationFaultPointV1::RootSynced)
}

fn verify_installed_root(
    root: &File,
    owner: (u32, u32),
    deployment: &VerifiedCompilerExecutionDeploymentV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    let root_snapshot = validate_directory_mode(
        root,
        Some(owner),
        INSTALLED_DIRECTORY_MODE_V1,
        "installed root",
    )?;
    verify_directory_children(root, INSTALL_ROOT_CHILDREN_V1, "installed root")?;
    for &(path, children) in INSTALL_DIRECTORY_SPECS_V1 {
        let directory = open_beneath(root, path, true)?;
        let initial = validate_directory_mode(
            &directory,
            Some(owner),
            INSTALLED_DIRECTORY_MODE_V1,
            "installed-root directory",
        )?;
        if initial.device != root_snapshot.device {
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InvalidMetadata,
                "installed-root directory crosses a filesystem boundary",
            ));
        }
        verify_directory_children(&directory, children, "installed-root directory")?;
        if snapshot(
            &fstat(&directory)
                .map_err(|source| io_error("reinspect installed-root directory", source))?,
        ) != initial
        {
            return Err(changed(
                "installed-root directory changed during enumeration",
            ));
        }
    }

    let manifest = admit_installed_file(root, root_snapshot, &deployment.manifest.entry)?;
    let parsed = parse_manifest(&manifest.bytes, &deployment.git_commit)?;
    let expected_entries: Vec<_> = deployment
        .files
        .iter()
        .map(|source| source.entry.clone())
        .collect();
    if parsed.target != deployment.target || parsed.entries != expected_entries {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::ContentMismatch,
            "installed manifest differs from sealed source custody",
        ));
    }

    let mut build_info = None;
    let mut sums = None;
    for (index, source) in deployment.files.iter().enumerate() {
        let admitted = admit_installed_file(root, root_snapshot, &source.entry)?;
        match index {
            0 => build_info = Some(admitted.bytes),
            1 => sums = Some(admitted.bytes),
            _ => {}
        }
    }
    validate_build_info(
        build_info
            .as_deref()
            .expect("V1 inventory begins with BUILD-INFO"),
        &deployment.git_commit,
        &deployment.target,
    )?;
    validate_sha256sums(
        sums.as_deref()
            .expect("V1 inventory contains canonical SHA256SUMS second"),
        &expected_entries,
    )?;
    verify_directory_children(root, INSTALL_ROOT_CHILDREN_V1, "installed root")?;
    if snapshot(&fstat(root).map_err(|source| io_error("reinspect installed root", source))?)
        != root_snapshot
    {
        return Err(changed("installed root changed during complete admission"));
    }
    Ok(())
}

fn admit_installed_file(
    root: &File,
    root_snapshot: ObjectSnapshotV1,
    expected: &ManifestEntryV1,
) -> Result<AdmittedSourceV1, DeploymentVerificationErrorV1> {
    let relative = expected
        .spec
        .install
        .strip_prefix('/')
        .expect("V1 install paths are absolute");
    let installed_spec = super::FileSpecV1 {
        source: relative,
        ..expected.spec
    };
    let mut installed_entry = expected.clone();
    installed_entry.spec = installed_spec;
    admit_source_file(root, root_snapshot, installed_spec, Some(&installed_entry))
}

struct StagingRootV1<'a> {
    parent: &'a File,
    name: String,
    root: Option<File>,
    created_files: Vec<&'static str>,
    created_directories: Vec<&'static str>,
    cleanup_required: bool,
}

impl<'a> StagingRootV1<'a> {
    fn create(
        parent: &'a File,
        parent_snapshot: ObjectSnapshotV1,
        owner: (u32, u32),
        hooks: &mut impl InstallationHooksV1,
    ) -> Result<Self, DeploymentVerificationErrorV1> {
        for _ in 0..16 {
            hooks.checkpoint(InstallationFaultPointV1::BeforeStagingCreate)?;
            let name = random_staging_name()?;
            match mkdirat(
                parent,
                name.as_str(),
                Mode::from_raw_mode(INSTALL_PARENT_MODE_V1),
            ) {
                Ok(()) => {
                    let mut staging = Self {
                        parent,
                        name,
                        root: None,
                        created_files: Vec::new(),
                        created_directories: Vec::new(),
                        cleanup_required: true,
                    };
                    if let Err(error) = hooks.checkpoint(InstallationFaultPointV1::StagingCreated) {
                        return Err(staging.cleanup_or(error));
                    }
                    let root = match open_named_root(parent, &staging.name) {
                        Ok(Some(root)) => root,
                        Ok(None) => {
                            let error =
                                changed("newly created staging root disappeared before admission");
                            return Err(staging.cleanup_or(error));
                        }
                        Err(error) => return Err(staging.cleanup_or(error)),
                    };
                    staging.root = Some(root);
                    let root = staging
                        .root
                        .as_ref()
                        .expect("new staging root retains its descriptor");
                    if let Err(error) = set_owner_and_mode(root, owner, INSTALL_PARENT_MODE_V1) {
                        return Err(staging.cleanup_or(error));
                    }
                    let staging_snapshot = validate_directory_mode(
                        root,
                        Some(owner),
                        INSTALL_PARENT_MODE_V1,
                        "staging install root",
                    );
                    let staging_snapshot = match staging_snapshot {
                        Ok(snapshot) => snapshot,
                        Err(error) => return Err(staging.cleanup_or(error)),
                    };
                    if staging_snapshot.device != parent_snapshot.device {
                        let error = super::invalid(
                            DeploymentVerificationErrorKindV1::InvalidMetadata,
                            "staging install root is not on the install-parent filesystem",
                        );
                        return Err(staging.cleanup_or(error));
                    }
                    if let Err(error) =
                        hooks.checkpoint(InstallationFaultPointV1::StagingMetadataSet)
                    {
                        return Err(staging.cleanup_or(error));
                    }
                    return Ok(staging);
                }
                Err(rustix::io::Errno::EXIST) => continue,
                Err(source) => return Err(io_error("create private staging install root", source)),
            }
        }
        Err(super::invalid(
            DeploymentVerificationErrorKindV1::Io,
            "could not allocate a unique staging install-root name",
        ))
    }

    fn create_directory(
        &mut self,
        path: &'static str,
        owner: (u32, u32),
        index: usize,
        hooks: &mut impl InstallationHooksV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        let root = self
            .root
            .as_ref()
            .expect("active staging root retains its descriptor");
        let (parent_path, name) = split_relative_path(path);
        let parent = open_relative_directory(root, parent_path)?;
        mkdirat(
            &parent,
            name,
            Mode::from_raw_mode(INSTALLED_DIRECTORY_MODE_V1),
        )
        .map_err(|source| io_error("create installed-root directory", source))?;
        self.created_directories.push(path);
        hooks.checkpoint(InstallationFaultPointV1::DirectoryCreated(index))?;
        let directory = open_beneath(root, path, true)?;
        set_owner_and_mode(&directory, owner, INSTALLED_DIRECTORY_MODE_V1)?;
        let root_device = fstat(root)
            .map_err(|source| io_error("inspect staging-root filesystem", source))?
            .st_dev;
        let directory_snapshot = validate_directory_mode(
            &directory,
            Some(owner),
            INSTALLED_DIRECTORY_MODE_V1,
            "new installed-root directory",
        )?;
        if directory_snapshot.device != root_device {
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InvalidMetadata,
                "new installed-root directory crosses a filesystem boundary",
            ));
        }
        hooks.checkpoint(InstallationFaultPointV1::DirectoryMetadataSet(index))?;
        Ok(())
    }

    fn copy_source(
        &mut self,
        source: &SealedDeploymentFileV1,
        owner: (u32, u32),
        index: usize,
        hooks: &mut impl InstallationHooksV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        validate_sealed_file(source)?;
        let relative = source
            .entry
            .spec
            .install
            .strip_prefix('/')
            .expect("V1 install paths are absolute");
        let (parent_path, name) = split_relative_path(relative);
        let root = self
            .root
            .as_ref()
            .expect("active staging root retains its descriptor");
        let parent = open_relative_directory(root, parent_path)?;
        let descriptor = openat(
            &parent,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|error| io_error("create installed-root file", error))?;
        self.created_files.push(relative);
        hooks.checkpoint(InstallationFaultPointV1::FileCreated(index))?;
        let mut destination = File::from(descriptor);
        set_owner_if_needed(&destination, owner)?;
        copy_exact_source(source, &mut destination)?;
        hooks.checkpoint(InstallationFaultPointV1::FileWritten(index))?;
        fchmod(&destination, Mode::from_raw_mode(source.entry.spec.mode))
            .map_err(|error| io_error("set installed-root file mode", error))?;
        hooks.checkpoint(InstallationFaultPointV1::FileModeSet(index))?;
        destination
            .sync_all()
            .map_err(|error| std_io_error("sync installed-root file", error))?;
        hooks.checkpoint(InstallationFaultPointV1::FileSynced(index))
    }

    fn cleanup_or(
        &mut self,
        error: DeploymentVerificationErrorV1,
    ) -> DeploymentVerificationErrorV1 {
        match self.cleanup() {
            Ok(()) => error,
            Err(cleanup) => super::invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                format!("installation failed and staging cleanup also failed: {cleanup}"),
            ),
        }
    }

    fn cleanup(&mut self) -> Result<(), DeploymentVerificationErrorV1> {
        if !self.cleanup_required {
            return Ok(());
        }
        if let Some(root) = &self.root {
            for path in self.created_files.iter().rev() {
                let (parent_path, name) = split_relative_path(path);
                let parent = open_relative_directory(root, parent_path)?;
                match unlinkat(&parent, name, AtFlags::empty()) {
                    Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                    Err(source) => return Err(io_error("remove staging install file", source)),
                }
            }
            for path in self.created_directories.iter().rev() {
                let (parent_path, name) = split_relative_path(path);
                let parent = open_relative_directory(root, parent_path)?;
                match unlinkat(&parent, name, AtFlags::REMOVEDIR) {
                    Ok(()) | Err(rustix::io::Errno::NOENT) => {}
                    Err(source) => {
                        return Err(io_error("remove staging install directory", source));
                    }
                }
            }
        }
        self.root.take();
        match unlinkat(self.parent, self.name.as_str(), AtFlags::REMOVEDIR) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => {}
            Err(source) => return Err(io_error("remove staging install root", source)),
        }
        self.parent
            .sync_all()
            .map_err(|source| std_io_error("sync install parent after cleanup", source))?;
        self.cleanup_required = false;
        Ok(())
    }
}

impl Drop for StagingRootV1<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn split_relative_path(path: &'static str) -> (&'static str, &'static str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

fn open_relative_directory(root: &File, path: &str) -> Result<File, DeploymentVerificationErrorV1> {
    if path.is_empty() {
        root.try_clone()
            .map_err(|source| std_io_error("clone staging-root descriptor", source))
    } else {
        open_beneath(root, path, true)
    }
}

fn set_owner_and_mode(
    file: &File,
    owner: (u32, u32),
    mode: u32,
) -> Result<(), DeploymentVerificationErrorV1> {
    set_owner_if_needed(file, owner)?;
    fchmod(file, Mode::from_raw_mode(mode))
        .map_err(|source| io_error("set installed-root directory mode", source))
}

fn set_owner_if_needed(
    file: &File,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    let stat = fstat(file).map_err(|source| io_error("inspect new installed object", source))?;
    if (stat.st_uid, stat.st_gid) != owner {
        fchown(
            file,
            Some(Uid::from_raw(owner.0)),
            Some(Gid::from_raw(owner.1)),
        )
        .map_err(|source| io_error("set installed-root object owner", source))?;
    }
    Ok(())
}

fn copy_exact_source(
    source: &SealedDeploymentFileV1,
    destination: &mut File,
) -> Result<(), DeploymentVerificationErrorV1> {
    let mut offset = 0_u64;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; COPY_BUFFER_BYTES_V1];
    while offset < source.entry.byte_len {
        let remaining = usize::try_from(source.entry.byte_len - offset)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let count = source
            .file
            .read_at(&mut buffer[..remaining], offset)
            .map_err(|error| std_io_error("read sealed deployment source", error))?;
        if count == 0 {
            return Err(changed(
                "sealed deployment source ended during installation",
            ));
        }
        destination
            .write_all(&buffer[..count])
            .map_err(|error| std_io_error("write installed-root file", error))?;
        digest.update(&buffer[..count]);
        offset += count as u64;
    }
    let mut trailing = [0_u8; 1];
    if source
        .file
        .read_at(&mut trailing, source.entry.byte_len)
        .map_err(|error| std_io_error("check sealed deployment source length", error))?
        != 0
        || <[u8; 32]>::from(digest.finalize()) != source.entry.sha256
    {
        return Err(changed(
            "sealed deployment source changed while installing its bytes",
        ));
    }
    Ok(())
}

fn random_staging_name() -> Result<String, DeploymentVerificationErrorV1> {
    let mut random = [0_u8; 16];
    let mut filled = 0;
    while filled < random.len() {
        let count =
            rustix::rand::getrandom(&mut random[filled..], rustix::rand::GetRandomFlags::empty())
                .map_err(|source| io_error("generate install-root staging randomness", source))?;
        if count == 0 {
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::Io,
                "Linux getrandom returned no staging-name bytes",
            ));
        }
        filled += count;
    }
    Ok(format!("{STAGING_PREFIX_V1}{}", lower_hex(&random)))
}
