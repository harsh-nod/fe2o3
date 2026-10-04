use std::fmt;
use std::fs::File;
use std::os::fd::{AsFd, AsRawFd as _, OwnedFd};

use fe2o3_loop_device::{ReadOnlyAutoclearLoopDeviceV1, attach_sealed_read_only_loop_device_v1};
use rustix::fs::{
    AtFlags, Mode, OFlags, ResolveFlags, Statx, StatxFlags, fchmod, fgetxattr, flistxattr, fstat,
    fstatfs, mkdirat, openat2, statat, statx,
};
use rustix::mount::{
    FsMountFlags, FsOpenFlags, MountAttrFlags, MountPropagationFlags, MoveMountFlags, UnmountFlags,
    fsconfig_create, fsconfig_set_flag, fsconfig_set_string, fsmount, fsopen, mount_change,
    move_mount, unmount,
};

use super::fault::{NoQualificationFaultV1, QualificationFaultHooksV1};
use super::host::process_thread_count;
use super::install::verify_installed_projection;
use super::qualification::revalidate_prepared_qualification_with_parent_children;
use super::staging::StagedCompilerExecutionQualificationV1;
use super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, ObjectSnapshotV1,
    QualificationFaultPointV1, changed, io_error, lower_hex, snapshot, std_io_error,
    validate_directory_mode, verify_directory_children,
};

const SQUASHFS_MAGIC_V1: i64 = 0x7371_7368;
const OVERLAYFS_MAGIC_V1: i64 = 0x794c_7630;
const QUALIFICATION_STAGING_MODE_V1: u32 = 0o700;
const COMPOSED_ROOT_MODE_V1: u32 = 0o755;
// Linux 6.8 UAPI; rustix 1.1.4 forwards this flag but does not name it yet.
const STATX_MNT_ID_UNIQUE_V72: StatxFlags = StatxFlags::from_bits_retain(0x0000_4000);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct QualificationMountIdentityV72 {
    pub(super) device: u64,
    pub(super) inode: u64,
    pub(super) mount_id: u64,
}

fn mount_identity(
    file: impl AsFd,
) -> Result<QualificationMountIdentityV72, DeploymentVerificationErrorV1> {
    let observed = statx(
        file,
        "",
        AtFlags::EMPTY_PATH,
        STATX_MNT_ID_UNIQUE_V72 | StatxFlags::INO,
    )
    .map_err(|source| io_error("inspect unique qualification mount identity", source))?;
    mount_identity_from_statx(&observed)
}

fn mount_identity_from_statx(
    observed: &Statx,
) -> Result<QualificationMountIdentityV72, DeploymentVerificationErrorV1> {
    let required = (STATX_MNT_ID_UNIQUE_V72 | StatxFlags::INO).bits();
    if observed.stx_mask & required != required || observed.stx_mnt_id == 0 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationMount,
            "qualification cleanup requires a kernel-provided unique mount ID and inode",
        ));
    }
    Ok(QualificationMountIdentityV72 {
        device: rustix::fs::makedev(observed.stx_dev_major, observed.stx_dev_minor),
        inode: observed.stx_ino,
        mount_id: observed.stx_mnt_id,
    })
}

#[cfg(test)]
pub(super) fn mount_identity_for_test_v1(
    file: impl AsFd,
) -> Result<QualificationMountIdentityV72, DeploymentVerificationErrorV1> {
    mount_identity(file)
}
const MOUNTED_STAGING_CHILDREN_V1: &[&str] =
    &["base", "evidence", "root", "run", "state", "upper", "work"];
const EMPTY_MOUNTED_STAGING_CHILDREN_V1: &[&str] = &["evidence", "run", "state"];
/// Move-only evidence that this dedicated process entered a private mount namespace.
///
/// Creating this value irreversibly changes the calling process mount namespace. The caller must
/// be a single-threaded, dedicated root qualification worker. It grants no mount or service
/// authority by itself.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_deployment::PrivateQualificationMountNamespaceV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<PrivateQualificationMountNamespaceV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_deployment::PrivateQualificationMountNamespaceV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<PrivateQualificationMountNamespaceV1>();
/// ```
pub struct PrivateQualificationMountNamespaceV1 {
    namespace: File,
    device: u64,
    inode: u64,
}

impl fmt::Debug for PrivateQualificationMountNamespaceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateQualificationMountNamespaceV1")
            .field("namespace_device", &self.device)
            .field("namespace_inode", &self.inode)
            .field("authority", &"private-mount-namespace-evidence-only")
            .finish_non_exhaustive()
    }
}

impl PrivateQualificationMountNamespaceV1 {
    pub(super) fn revalidate(&self) -> Result<(), DeploymentVerificationErrorV1> {
        let retained = fstat(&self.namespace)
            .map_err(|source| io_error("inspect retained qualification mount namespace", source))?;
        if retained.st_dev != self.device || retained.st_ino != self.inode {
            return Err(changed(
                "retained qualification mount-namespace identity changed",
            ));
        }
        let current = open_mount_namespace()?;
        let current = fstat(&current)
            .map_err(|source| io_error("inspect current qualification mount namespace", source))?;
        if current.st_dev != self.device || current.st_ino != self.inode {
            return Err(changed(
                "calling thread left the retained qualification mount namespace",
            ));
        }
        Ok(())
    }
}

/// Move-only custody of the attached read-only base and disposable overlay root.
///
/// The installed deployment remains a lower layer and is revalidated against sealed source
/// evidence after composition. This value grants no systemd boot, service, compiler, signing,
/// publication, GPU, or execution authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_deployment::MountedCompilerExecutionQualificationV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<MountedCompilerExecutionQualificationV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_deployment::MountedCompilerExecutionQualificationV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<MountedCompilerExecutionQualificationV1>();
/// ```
pub struct MountedCompilerExecutionQualificationV1 {
    namespace: PrivateQualificationMountNamespaceV1,
    staged: Option<StagedCompilerExecutionQualificationV1>,
    loop_device: Option<ReadOnlyAutoclearLoopDeviceV1>,
    mounted_base: Option<File>,
    mounted_root: Option<File>,
    overlay_work: Option<File>,
    overlay_proc: Option<(File, ObjectSnapshotV1)>,
    base_identity: Option<QualificationMountIdentityV72>,
    root_identity: Option<QualificationMountIdentityV72>,
    base_attached: bool,
    root_attached: bool,
}

impl fmt::Debug for MountedCompilerExecutionQualificationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let staged = self
            .staged
            .as_ref()
            .expect("active mounted qualification retains staging custody");
        formatter
            .debug_struct("MountedCompilerExecutionQualificationV1")
            .field("git_commit", &staged.git_commit())
            .field("manifest_sha256", &lower_hex(&staged.manifest_sha256()))
            .field("base_image_sha256", &lower_hex(&staged.base_image_sha256()))
            .field("run_name", &staged.run_name())
            .field("authority", &"mounted-root-custody-only")
            .finish_non_exhaustive()
    }
}

impl MountedCompilerExecutionQualificationV1 {
    /// Returns the exact deployment commit visible through the composed root.
    pub fn git_commit(&self) -> &str {
        self.staged
            .as_ref()
            .expect("active mounted qualification retains staging custody")
            .git_commit()
    }

    /// Returns the exact deployment-manifest digest visible through the composed root.
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.staged
            .as_ref()
            .expect("active mounted qualification retains staging custody")
            .manifest_sha256()
    }

    /// Returns the independently pinned base-image digest mounted read-only.
    pub fn base_image_sha256(&self) -> [u8; 32] {
        self.staged
            .as_ref()
            .expect("active mounted qualification retains staging custody")
            .base_image_sha256()
    }

    /// Returns the random private staging name beneath the retained qualification parent.
    pub fn run_name(&self) -> &str {
        self.staged
            .as_ref()
            .expect("active mounted qualification retains staging custody")
            .run_name()
    }

    /// Revalidates namespace custody, both mount identities, and every installed deployment file.
    pub fn revalidate(&self) -> Result<(), DeploymentVerificationErrorV1> {
        revalidate_mounted_qualification(self, (0, 0), MountedRootStateV1::Pristine)
    }

    pub(super) fn revalidate_systemd_preflight_state(
        &self,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        revalidate_mounted_qualification(self, (0, 0), MountedRootStateV1::SystemdPreflight)
    }

    pub(super) fn inherit_composed_root_descriptor(
        &self,
    ) -> Result<OwnedFd, DeploymentVerificationErrorV1> {
        self.revalidate()?;
        let root = self
            .mounted_root
            .as_ref()
            .ok_or_else(|| changed("mounted qualification root descriptor was released"))?;
        duplicate_exact_mount_descriptor(root, "composed root")
    }

    pub(super) fn inherit_systemd_preflight_root_descriptor(
        &self,
    ) -> Result<OwnedFd, DeploymentVerificationErrorV1> {
        self.revalidate_systemd_preflight_state()?;
        let root = self
            .mounted_root
            .as_ref()
            .ok_or_else(|| changed("mounted qualification root descriptor was released"))?;
        duplicate_exact_mount_descriptor(root, "composed root")
    }

    pub(super) fn inherit_systemd_machine_descriptors(
        &self,
    ) -> Result<(OwnedFd, OwnedFd), DeploymentVerificationErrorV1> {
        self.revalidate_systemd_preflight_state()?;
        let base = self
            .mounted_base
            .as_ref()
            .ok_or_else(|| changed("mounted qualification base descriptor was released"))?;
        let root = self
            .mounted_root
            .as_ref()
            .ok_or_else(|| changed("mounted qualification root descriptor was released"))?;
        Ok((
            duplicate_exact_mount_descriptor(base, "pinned base")?,
            duplicate_exact_mount_descriptor(root, "composed root")?,
        ))
    }

    /// Unmounts overlay then SquashFS, releases the autoclear loop device, and removes staging.
    pub fn cleanup(mut self) -> Result<(), DeploymentVerificationErrorV1> {
        self.cleanup_internal()
    }

    pub(super) fn cleanup_with_hooks(
        mut self,
        hooks: &mut impl QualificationFaultHooksV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        self.cleanup_internal_with_hooks(hooks)
    }

    fn cleanup_or(
        &mut self,
        error: DeploymentVerificationErrorV1,
    ) -> DeploymentVerificationErrorV1 {
        match self.cleanup_internal() {
            Ok(()) => error,
            Err(cleanup) => super::invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                format!("qualification mount failed and cleanup also failed: {cleanup}"),
            ),
        }
    }

    fn cleanup_internal(&mut self) -> Result<(), DeploymentVerificationErrorV1> {
        self.cleanup_internal_with_hooks(&mut NoQualificationFaultV1)
    }

    fn cleanup_internal_with_hooks(
        &mut self,
        hooks: &mut impl QualificationFaultHooksV1,
    ) -> Result<(), DeploymentVerificationErrorV1> {
        if self.staged.is_none() {
            return Ok(());
        }
        self.namespace.revalidate()?;
        let mut deferred = None;
        if self.root_attached {
            let staged = self
                .staged
                .as_ref()
                .expect("active mounted qualification retains staging custody");
            let identity = self
                .root_identity
                .ok_or_else(|| changed("qualification overlay cleanup identity was released"))?;
            if let Err(error) = unmount_retained_child(
                staged.root_descriptor(),
                staged.directory_descriptor("root"),
                "root",
                &mut self.mounted_root,
                identity,
            ) {
                return Err(combine_cleanup_error(deferred, error));
            }
            self.root_attached = false;
            self.root_identity = None;
            defer_checkpoint(
                &mut deferred,
                hooks.checkpoint(QualificationFaultPointV1::OverlayUnmounted),
            );
        }
        if self.base_attached {
            let staged = self
                .staged
                .as_ref()
                .expect("active mounted qualification retains staging custody");
            let identity = self
                .base_identity
                .ok_or_else(|| changed("qualification base cleanup identity was released"))?;
            if let Err(error) = unmount_retained_child(
                staged.root_descriptor(),
                staged.directory_descriptor("base"),
                "base",
                &mut self.mounted_base,
                identity,
            ) {
                return Err(combine_cleanup_error(deferred, error));
            }
            self.base_attached = false;
            self.base_identity = None;
            defer_checkpoint(
                &mut deferred,
                hooks.checkpoint(QualificationFaultPointV1::BaseUnmounted),
            );
        }
        self.loop_device.take();
        self.overlay_work.take();
        self.overlay_proc.take();
        defer_checkpoint(
            &mut deferred,
            hooks.checkpoint(QualificationFaultPointV1::LoopReleased),
        );
        let cleanup = self
            .staged
            .take()
            .expect("active mounted qualification retains staging custody")
            .cleanup();
        if let Err(error) = cleanup {
            return Err(combine_cleanup_error(deferred, error));
        }
        defer_checkpoint(
            &mut deferred,
            hooks.checkpoint(QualificationFaultPointV1::StagingCleaned),
        );
        match deferred {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn unmount_retained_child(
    staging_root: &File,
    target: &File,
    name: &str,
    retained: &mut Option<File>,
    identity: QualificationMountIdentityV72,
) -> Result<(), DeploymentVerificationErrorV1> {
    let reopened = open_mounted_child(staging_root, name)?;
    for file in std::iter::once(&reopened).chain(retained.iter()) {
        let current = mount_identity(file)?;
        if current != identity {
            return Err(changed("qualification cleanup mount identity changed"));
        }
    }
    // Open files pin the mount. Keep its identity and attachment state, not a busy FD.
    drop(reopened);
    retained.take();
    unmount(descriptor_path(target), UnmountFlags::empty())
        .map_err(|source| io_error("unmount retained qualification child", source))
}

#[cfg(test)]
pub(super) fn unmount_retained_child_for_test_v1(
    staging_root: &File,
    target: &File,
    name: &str,
    retained: &mut Option<File>,
    identity: QualificationMountIdentityV72,
) -> Result<(), DeploymentVerificationErrorV1> {
    unmount_retained_child(staging_root, target, name, retained, identity)
}

fn duplicate_exact_mount_descriptor(
    original: &File,
    name: &'static str,
) -> Result<OwnedFd, DeploymentVerificationErrorV1> {
    let inherited = rustix::io::dup(original)
        .map_err(|source| io_error("duplicate qualification mount for child execution", source))?;
    let inherited_flags = rustix::io::fcntl_getfd(&inherited)
        .map_err(|source| io_error("inspect inherited qualification mount flags", source))?;
    let original_stat = fstat(original)
        .map_err(|source| io_error("inspect retained qualification mount", source))?;
    let duplicate_stat = fstat(&inherited)
        .map_err(|source| io_error("inspect inherited qualification mount", source))?;
    if !inherited_flags.is_empty()
        || (original_stat.st_dev, original_stat.st_ino)
            != (duplicate_stat.st_dev, duplicate_stat.st_ino)
    {
        return Err(changed(format!(
            "inherited {name} descriptor does not retain exact executable custody"
        )));
    }
    Ok(inherited)
}

fn defer_checkpoint(
    deferred: &mut Option<DeploymentVerificationErrorV1>,
    checkpoint: Result<(), DeploymentVerificationErrorV1>,
) {
    if deferred.is_none() {
        *deferred = checkpoint.err();
    }
}

fn combine_cleanup_error(
    deferred: Option<DeploymentVerificationErrorV1>,
    cleanup: DeploymentVerificationErrorV1,
) -> DeploymentVerificationErrorV1 {
    match deferred {
        Some(primary) => super::invalid(
            DeploymentVerificationErrorKindV1::CleanupFailed,
            format!("{primary}; dependent cleanup also failed: {cleanup}"),
        ),
        None => cleanup,
    }
}

impl Drop for MountedCompilerExecutionQualificationV1 {
    fn drop(&mut self) {
        let _ = self.cleanup_internal();
    }
}

/// Enters a new recursive-private mount namespace for one dedicated qualification worker.
///
/// The operation requires effective UID zero and exactly one task in the current process. It is
/// irreversible; callers must not invoke it from a reusable application process.
pub fn enter_private_qualification_mount_namespace_v1()
-> Result<PrivateQualificationMountNamespaceV1, DeploymentVerificationErrorV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InsufficientPrivilege,
            "qualification mount-namespace isolation requires effective UID 0",
        ));
    }
    if process_thread_count()? != 1 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification mount namespace requires a single-threaded dedicated process",
        ));
    }
    let original = open_mount_namespace()?;
    let original = fstat(&original)
        .map_err(|source| io_error("inspect original qualification mount namespace", source))?;
    #[allow(deprecated)]
    rustix::thread::unshare(rustix::thread::UnshareFlags::NEWNS).map_err(|source| {
        io_error(
            "unshare private compiler-execution qualification mount namespace",
            source,
        )
    })?;
    mount_change(
        "/",
        MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
    )
    .map_err(|source| io_error("make qualification mount propagation private", source))?;
    let namespace = open_mount_namespace()?;
    let current = fstat(&namespace)
        .map_err(|source| io_error("inspect private qualification mount namespace", source))?;
    if (current.st_dev, current.st_ino) == (original.st_dev, original.st_ino) {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationIsolation,
            "qualification mount namespace did not change after unshare",
        ));
    }
    let retained = PrivateQualificationMountNamespaceV1 {
        namespace,
        device: current.st_dev,
        inode: current.st_ino,
    };
    retained.revalidate()?;
    Ok(retained)
}

/// Attaches the sealed SquashFS base and one disposable overlay inside a private namespace.
///
/// The installed root is the top read-only lower layer and the sealed base is the second lower
/// layer. Upper and work directories come only from the exact staged transaction. The composed
/// deployment projection is verified against retained sealed source custody before return.
pub fn attach_compiler_execution_qualification_mounts_v1(
    namespace: PrivateQualificationMountNamespaceV1,
    staged: StagedCompilerExecutionQualificationV1,
) -> Result<MountedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    attach_compiler_execution_qualification_mounts_with_hooks_v1(
        namespace,
        staged,
        &mut NoQualificationFaultV1,
    )
}

pub(super) fn attach_compiler_execution_qualification_mounts_with_hooks_v1(
    namespace: PrivateQualificationMountNamespaceV1,
    mut staged: StagedCompilerExecutionQualificationV1,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<MountedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    namespace.revalidate()?;
    staged.refresh_mount_namespace_descriptors()?;
    namespace.revalidate()?;
    for name in ["upper", "work"] {
        refuse_overlay_backed_state(staged.directory_descriptor(name), name)?;
    }
    let loop_device = attach_sealed_read_only_loop_device_v1(staged.prepared().sealed_base_image())
        .map_err(|source| {
            super::invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationMount,
                format!("attach sealed qualification image to loop device: {source}"),
            )
        })?;
    let mut mounted = MountedCompilerExecutionQualificationV1 {
        namespace,
        staged: Some(staged),
        loop_device: Some(loop_device),
        mounted_base: None,
        mounted_root: None,
        overlay_work: None,
        overlay_proc: None,
        base_identity: None,
        root_identity: None,
        base_attached: false,
        root_attached: false,
    };
    if let Err(error) = hooks.checkpoint(QualificationFaultPointV1::LoopAttached) {
        return Err(mounted.cleanup_or(error));
    }
    if let Err(error) = attach_base(&mut mounted, hooks) {
        return Err(mounted.cleanup_or(error));
    }
    if let Err(error) = attach_overlay(&mut mounted, hooks) {
        return Err(mounted.cleanup_or(error));
    }
    if let Err(error) =
        revalidate_mounted_qualification(&mounted, (0, 0), MountedRootStateV1::Pristine)
    {
        return Err(mounted.cleanup_or(error));
    }
    if let Err(error) = hooks.checkpoint(QualificationFaultPointV1::ProjectionRevalidated) {
        return Err(mounted.cleanup_or(error));
    }
    Ok(mounted)
}

fn refuse_overlay_backed_state(
    directory: &File,
    role: &'static str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let observed = fstatfs(directory).map_err(|source| {
        io_error(
            "inspect qualification overlay state backing filesystem",
            source,
        )
    })?;
    if observed.f_type == OVERLAYFS_MAGIC_V1 {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationMount,
            format!(
                "qualification {role} directory is backed by OverlayFS, which cannot provide an overlay upper or work directory"
            ),
        ));
    }
    Ok(())
}

fn attach_base(
    mounted: &mut MountedCompilerExecutionQualificationV1,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    let loop_device = mounted
        .loop_device
        .as_ref()
        .expect("mount attachment retains loop custody");
    loop_device.revalidate().map_err(|source| {
        super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationMount,
            format!("revalidate qualification loop device: {source}"),
        )
    })?;
    let context = fsopen("squashfs", FsOpenFlags::FSOPEN_CLOEXEC)
        .map_err(|source| io_error("open SquashFS mount context", source))?;
    // Superblock creation opens the device before fsmount applies mount attributes.
    fsconfig_set_flag(&context, "ro").map_err(|source| {
        io_error(
            "request read-only qualification SquashFS superblock",
            source,
        )
    })?;
    fsconfig_set_string(&context, "source", loop_device.device_path())
        .map_err(|source| io_error("bind loop device to SquashFS context", source))?;
    fsconfig_create(&context)
        .map_err(|source| io_error("create qualification SquashFS superblock", source))?;
    let detached = fsmount(
        &context,
        FsMountFlags::FSMOUNT_CLOEXEC,
        MountAttrFlags::MOUNT_ATTR_RDONLY
            | MountAttrFlags::MOUNT_ATTR_NODEV
            | MountAttrFlags::MOUNT_ATTR_NOSUID,
    )
    .map_err(|source| io_error("create detached qualification SquashFS mount", source))?;
    let staged = mounted
        .staged
        .as_ref()
        .expect("mount attachment retains staging custody");
    let identity = mount_identity(&detached)?;
    move_mount(
        &detached,
        "",
        staged.directory_descriptor("base"),
        "",
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
    )
    .map_err(|source| io_error("attach qualification SquashFS mount", source))?;
    mounted.base_attached = true;
    mounted.base_identity = Some(identity);
    hooks.checkpoint(QualificationFaultPointV1::BaseMounted)?;
    let base = open_mounted_child(staged.root_descriptor(), "base")?;
    require_filesystem(&base, SQUASHFS_MAGIC_V1, "qualification SquashFS base")?;
    mounted.mounted_base = Some(base);
    Ok(())
}

fn attach_overlay(
    mounted: &mut MountedCompilerExecutionQualificationV1,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    let staged = mounted
        .staged
        .as_ref()
        .expect("mount attachment retains staging custody");
    let base = mounted
        .mounted_base
        .as_ref()
        .expect("overlay attachment follows base attachment");
    let lowerdirs = overlay_lowerdirs(staged.prepared().installed().retained_root(), base);
    prepare_overlay_upper(staged.directory_descriptor("upper"), (0, 0))?;
    validate_sealed_base_proc_target(base, staged.prepared().installed().retained_root())?;
    mounted.overlay_proc = Some(prepare_overlay_proc_target(
        staged.directory_descriptor("upper"),
        (0, 0),
    )?);
    let context = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC)
        .map_err(|source| io_error("open overlay mount context", source))?;
    configure_overlay_profile(&context)?;
    fsconfig_set_string(&context, "lowerdir", lowerdirs)
        .map_err(|source| io_error("set qualification overlay lower directories", source))?;
    fsconfig_set_string(
        &context,
        "upperdir",
        descriptor_path(staged.directory_descriptor("upper")),
    )
    .map_err(|source| io_error("set qualification overlay upper directory", source))?;
    fsconfig_set_string(
        &context,
        "workdir",
        descriptor_path(staged.directory_descriptor("work")),
    )
    .map_err(|source| io_error("set qualification overlay work directory", source))?;
    fsconfig_create(&context)
        .map_err(|source| io_error("create qualification overlay superblock", source))?;
    let detached = fsmount(
        &context,
        FsMountFlags::FSMOUNT_CLOEXEC,
        MountAttrFlags::MOUNT_ATTR_NODEV | MountAttrFlags::MOUNT_ATTR_NOSUID,
    )
    .map_err(|source| io_error("create detached qualification overlay mount", source))?;
    mounted.overlay_work = Some(open_overlay_work(
        staged.directory_descriptor("work"),
        (0, 0),
    )?);
    let identity = mount_identity(&detached)?;
    move_mount(
        &detached,
        "",
        staged.directory_descriptor("root"),
        "",
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
    )
    .map_err(|source| io_error("attach qualification overlay root", source))?;
    mounted.root_attached = true;
    mounted.root_identity = Some(identity);
    hooks.checkpoint(QualificationFaultPointV1::OverlayMounted)?;
    let root = open_mounted_child(staged.root_descriptor(), "root")?;
    require_filesystem(&root, OVERLAYFS_MAGIC_V1, "qualification overlay root")?;
    mounted.mounted_root = Some(root);
    Ok(())
}

#[derive(Clone, Copy)]
enum MountedRootStateV1 {
    Pristine,
    SystemdPreflight,
}

fn configure_overlay_profile(context: impl AsFd) -> Result<(), DeploymentVerificationErrorV1> {
    // This disposable overlay neither exports persistent handles nor retains an inode index.
    fsconfig_set_string(&context, "uuid", "null")
        .map_err(|source| io_error("set qualification overlay UUID profile", source))?;
    fsconfig_set_string(&context, "index", "off")
        .map_err(|source| io_error("set qualification overlay index profile", source))
}

fn prepare_overlay_upper(
    upper: &File,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    let before = validate_directory_mode(
        upper,
        Some(owner),
        QUALIFICATION_STAGING_MODE_V1,
        "pristine qualification upper",
    )?;
    verify_directory_children(upper, &[], "pristine qualification upper")?;
    // OverlayFS derives the composed root mode from this exact upper inode.
    fchmod(upper, Mode::from_raw_mode(COMPOSED_ROOT_MODE_V1))
        .map_err(|source| io_error("set composed qualification upper mode", source))?;
    let after = validate_directory_mode(
        upper,
        Some(owner),
        COMPOSED_ROOT_MODE_V1,
        "composed qualification upper",
    )?;
    if before.device != after.device || before.inode != after.inode {
        return Err(changed(
            "qualification upper identity changed during mode transition",
        ));
    }
    verify_directory_children(upper, &[], "composed qualification upper")
}

fn validate_overlay_upper(
    upper: &File,
    owner: (u32, u32),
    state: MountedRootStateV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    super::validate_directory_metadata(
        upper,
        Some(owner),
        COMPOSED_ROOT_MODE_V1,
        "composed qualification upper",
    )?;
    if matches!(state, MountedRootStateV1::Pristine) {
        super::require_no_xattrs(upper, "pristine qualification upper")?;
        return verify_directory_children(upper, &["proc"], "pristine qualification upper");
    }
    // A completed copy-up marks its parent impure; no other backing xattr is admitted.
    const IMPURE: &[u8] = b"trusted.overlay.impure\0";
    let mut names = [0u8; IMPURE.len()];
    let length = flistxattr(upper, &mut names)
        .map_err(|source| io_error("inspect qualification upper xattr profile", source))?;
    if length == 0 {
        return Ok(());
    }
    if length != IMPURE.len() || names.as_slice() != IMPURE {
        return Err(changed(
            "qualification upper has an unexpected backing xattr",
        ));
    }
    let mut value = [0u8; 2];
    let length = fgetxattr(upper, "trusted.overlay.impure", &mut value)
        .map_err(|source| io_error("inspect qualification upper impure marker", source))?;
    if length != 1 || value[0] != b'y' {
        return Err(changed("qualification upper has an invalid impure marker"));
    }
    let length = flistxattr(upper, &mut names)
        .map_err(|source| io_error("reinspect qualification upper xattr profile", source))?;
    if length != IMPURE.len() || names.as_slice() != IMPURE {
        return Err(changed("qualification upper backing xattrs changed"));
    }
    Ok(())
}

fn validate_sealed_base_proc_target(
    base: &File,
    installed: &File,
) -> Result<(), DeploymentVerificationErrorV1> {
    require_filesystem(base, SQUASHFS_MAGIC_V1, "qualification SquashFS base")?;
    if !rustix::fs::fstatvfs(base)
        .map_err(|source| io_error("inspect sealed base mount flags", source))?
        .f_flag
        .contains(rustix::fs::StatVfsMountFlags::RDONLY)
    {
        return Err(changed("qualification base is no longer read-only"));
    }
    match statat(installed, "proc", AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => (),
        Ok(_) => {
            return Err(changed(
                "installed deployment overrides the base proc target",
            ));
        }
        Err(source) => return Err(io_error("inspect installed proc target absence", source)),
    }
    let target = super::open_beneath(base, "proc", true)?;
    // This exact base was sealed only after its no-xattr-table image profile was checked.
    // SquashFS reports EOPNOTSUPP for that profile. No xattr error is accepted here:
    // the lower target is checked for shape, then a fresh inspectable upper target is made.
    let expected = super::validate_directory_metadata(
        &target,
        Some((0, 0)),
        COMPOSED_ROOT_MODE_V1,
        "sealed base proc target",
    )?;
    verify_directory_children(&target, &[], "sealed base proc target")?;
    let reopened = super::open_beneath(base, "proc", true)?;
    if snapshot(&fstat(&reopened).map_err(|source| io_error("reinspect base proc target", source))?)
        != expected
    {
        return Err(changed("sealed base proc target changed"));
    }
    Ok(())
}

fn prepare_overlay_proc_target(
    upper: &File,
    owner: (u32, u32),
) -> Result<(File, ObjectSnapshotV1), DeploymentVerificationErrorV1> {
    let (target, before) = prepare_overlay_proc_directory(upper, owner)?;
    // The sealed lower proc is empty. Do not merge it or admit variable origin handles.
    rustix::fs::fsetxattr(
        &target,
        "trusted.overlay.opaque",
        b"y",
        rustix::fs::XattrFlags::CREATE,
    )
    .map_err(|source| io_error("mark native preflight proc target opaque", source))?;
    let after = super::validate_directory_metadata(
        &target,
        Some(owner),
        COMPOSED_ROOT_MODE_V1,
        "opaque preflight proc target",
    )?;
    let mut expected = before;
    expected.changed_seconds = after.changed_seconds;
    expected.changed_nanoseconds = after.changed_nanoseconds;
    if expected != after {
        return Err(changed(
            "native proc identity changed during opaque marking",
        ));
    }
    let retained = (target, after);
    revalidate_overlay_proc_target(upper, &retained, owner, OverlayProcProfileV82::Opaque)?;
    Ok(retained)
}

fn prepare_overlay_proc_directory(
    upper: &File,
    owner: (u32, u32),
) -> Result<(File, ObjectSnapshotV1), DeploymentVerificationErrorV1> {
    validate_directory_mode(upper, Some(owner), COMPOSED_ROOT_MODE_V1, "prepared upper")?;
    verify_directory_children(upper, &[], "prepared upper")?;
    // Create before OverlayFS attachment; never alter backing directories of a live overlay.
    mkdirat(upper, "proc", Mode::from_raw_mode(COMPOSED_ROOT_MODE_V1))
        .map_err(|source| io_error("create native preflight proc target", source))?;
    let target = super::open_beneath(upper, "proc", true)?;
    fchmod(&target, Mode::from_raw_mode(COMPOSED_ROOT_MODE_V1))
        .map_err(|source| io_error("set native preflight proc target mode", source))?;
    let expected = validate_directory_mode(
        &target,
        Some(owner),
        COMPOSED_ROOT_MODE_V1,
        "native preflight proc target",
    )?;
    verify_directory_children(&target, &[], "native preflight proc target")?;
    let retained = (target, expected);
    revalidate_overlay_proc_target(upper, &retained, owner, OverlayProcProfileV82::Unmarked)?;
    Ok(retained)
}

#[derive(Clone, Copy)]
enum OverlayProcProfileV82 {
    Unmarked,
    Opaque,
}

const PROC_OPAQUE_NAME_V82: &[u8] = b"trusted.overlay.opaque\0";

fn proc_opaque_marker_is_exact(names: &[u8], value: &[u8]) -> bool {
    names == PROC_OPAQUE_NAME_V82 && value == b"y"
}

fn require_proc_opaque_marker(target: &File) -> Result<(), DeploymentVerificationErrorV1> {
    let refused = || {
        super::invalid(
            DeploymentVerificationErrorKindV1::ForbiddenAttributes,
            "native preflight proc target must carry only its exact opaque marker",
        )
    };
    let mut names = [0u8; PROC_OPAQUE_NAME_V82.len()];
    let length = match flistxattr(target, &mut names) {
        Ok(length) if length == names.len() => length,
        Ok(_) | Err(rustix::io::Errno::RANGE) => return Err(refused()),
        Err(source) => return Err(io_error("inspect native proc xattr names", source)),
    };
    if names.as_slice() != PROC_OPAQUE_NAME_V82 {
        return Err(refused());
    }
    let mut value = [0u8; 2];
    let value_length = match fgetxattr(target, "trusted.overlay.opaque", &mut value) {
        Ok(length) if length <= value.len() => length,
        Ok(_) | Err(rustix::io::Errno::RANGE) => return Err(refused()),
        Err(source) => return Err(io_error("inspect native proc opaque marker", source)),
    };
    if !proc_opaque_marker_is_exact(&names[..length], &value[..value_length]) {
        return Err(refused());
    }
    Ok(())
}

fn revalidate_overlay_proc_target(
    upper: &File,
    retained: &(File, ObjectSnapshotV1),
    owner: (u32, u32),
    profile: OverlayProcProfileV82,
) -> Result<(), DeploymentVerificationErrorV1> {
    let reopened = super::open_beneath(upper, "proc", true)?;
    for target in [&retained.0, &reopened, &retained.0] {
        let observed = super::validate_directory_metadata(
            target,
            Some(owner),
            COMPOSED_ROOT_MODE_V1,
            "retained native preflight proc target",
        )?;
        match profile {
            OverlayProcProfileV82::Unmarked => {
                super::require_no_xattrs(target, "unmarked native proc target")?;
            }
            OverlayProcProfileV82::Opaque => require_proc_opaque_marker(target)?,
        }
        if observed != retained.1 {
            return Err(changed(
                "native preflight proc target lost retained identity",
            ));
        }
        verify_directory_children(target, &[], "retained native preflight proc target")?;
    }
    Ok(())
}

fn open_overlay_work(
    parent: &File,
    owner: (u32, u32),
) -> Result<File, DeploymentVerificationErrorV1> {
    let parent_stat = validate_directory_mode(
        parent,
        Some(owner),
        QUALIFICATION_STAGING_MODE_V1,
        "qualification work parent",
    )?;
    verify_directory_children(parent, &["work"], "qualification work parent")?;
    let work = openat2(
        parent,
        "work",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|source| io_error("retain kernel qualification work directory", source))?;
    let work_stat =
        validate_directory_mode(&work, Some(owner), 0, "kernel qualification work directory")?;
    if work_stat.device != parent_stat.device {
        return Err(changed(
            "kernel qualification work directory changed filesystem",
        ));
    }
    verify_directory_children(&work, &[], "kernel qualification work directory")?;
    Ok(work)
}

fn revalidate_overlay_work(
    parent: &File,
    retained: &File,
    owner: (u32, u32),
) -> Result<(), DeploymentVerificationErrorV1> {
    let reopened = open_overlay_work(parent, owner)?;
    let retained =
        validate_directory_mode(retained, Some(owner), 0, "retained kernel work directory")?;
    let reopened = snapshot(
        &fstat(&reopened)
            .map_err(|source| io_error("reinspect kernel qualification work directory", source))?,
    );
    if reopened != retained {
        return Err(changed(
            "kernel qualification work directory differs from retained custody",
        ));
    }
    Ok(())
}

fn revalidate_mounted_qualification(
    mounted: &MountedCompilerExecutionQualificationV1,
    owner: (u32, u32),
    state: MountedRootStateV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    mounted.namespace.revalidate()?;
    if !mounted.base_attached || !mounted.root_attached {
        return Err(changed("qualification mount custody is incomplete"));
    }
    let staged = mounted
        .staged
        .as_ref()
        .ok_or_else(|| changed("qualification staging custody was released"))?;
    let parent_children = staged.prepared_parent_children();
    revalidate_prepared_qualification_with_parent_children(
        staged.prepared(),
        owner,
        &parent_children,
    )?;
    let staging_root = staged.root_descriptor();
    validate_directory_mode(
        staging_root,
        Some(owner),
        QUALIFICATION_STAGING_MODE_V1,
        "mounted qualification staging root",
    )?;
    verify_directory_children(
        staging_root,
        MOUNTED_STAGING_CHILDREN_V1,
        "mounted qualification staging root",
    )?;
    for name in EMPTY_MOUNTED_STAGING_CHILDREN_V1 {
        let directory = staged.directory_descriptor(name);
        validate_directory_mode(
            directory,
            Some(owner),
            QUALIFICATION_STAGING_MODE_V1,
            "unmounted qualification staging directory",
        )?;
        verify_directory_children(directory, &[], "unmounted qualification staging directory")?;
    }
    validate_overlay_upper(staged.directory_descriptor("upper"), owner, state)?;
    revalidate_overlay_proc_target(
        staged.directory_descriptor("upper"),
        mounted
            .overlay_proc
            .as_ref()
            .ok_or_else(|| changed("native preflight proc target custody was released"))?,
        owner,
        OverlayProcProfileV82::Opaque,
    )?;
    revalidate_overlay_work(
        staged.directory_descriptor("work"),
        mounted
            .overlay_work
            .as_ref()
            .ok_or_else(|| changed("kernel work directory custody was released"))?,
        owner,
    )?;
    let base = mounted
        .mounted_base
        .as_ref()
        .ok_or_else(|| changed("mounted qualification base descriptor was released"))?;
    mounted
        .loop_device
        .as_ref()
        .ok_or_else(|| changed("qualification loop custody was released"))?
        .revalidate()
        .map_err(|source| {
            super::invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationMount,
                format!("revalidate qualification loop device: {source}"),
            )
        })?;
    require_filesystem(base, SQUASHFS_MAGIC_V1, "qualification SquashFS base")?;
    require_path_identity(staging_root, "base", base)?;
    let root = mounted
        .mounted_root
        .as_ref()
        .ok_or_else(|| changed("mounted qualification root descriptor was released"))?;
    require_filesystem(root, OVERLAYFS_MAGIC_V1, "qualification overlay root")?;
    require_path_identity(staging_root, "root", root)?;
    validate_directory_mode(
        root,
        Some(owner),
        COMPOSED_ROOT_MODE_V1,
        "composed qualification root",
    )?;
    verify_installed_projection(root, staged.prepared().installed(), owner)
}

fn open_mount_namespace() -> Result<File, DeploymentVerificationErrorV1> {
    File::open("/proc/self/ns/mnt")
        .map_err(|source| std_io_error("open current qualification mount namespace", source))
}

fn descriptor_path(file: &File) -> String {
    format!("/proc/self/fd/{}", file.as_raw_fd())
}

fn overlay_lowerdirs(installed: &File, base: &File) -> String {
    format!("{}:{}", descriptor_path(installed), descriptor_path(base))
}

fn open_mounted_child(
    staging_root: &File,
    name: &str,
) -> Result<File, DeploymentVerificationErrorV1> {
    openat2(
        staging_root,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|source| io_error("open attached qualification mount", source))
}

fn require_path_identity(
    staging_root: &File,
    name: &str,
    retained: &File,
) -> Result<(), DeploymentVerificationErrorV1> {
    let reopened = open_mounted_child(staging_root, name)?;
    let reopened = snapshot(
        &fstat(&reopened)
            .map_err(|source| io_error("inspect reopened qualification mount", source))?,
    );
    let retained = snapshot(
        &fstat(retained)
            .map_err(|source| io_error("inspect retained qualification mount", source))?,
    );
    if reopened.device != retained.device || reopened.inode != retained.inode {
        return Err(changed(
            "qualification mount pathname differs from retained mount custody",
        ));
    }
    Ok(())
}

fn require_filesystem(
    file: &File,
    expected_magic: i64,
    role: &'static str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let observed = fstatfs(file)
        .map_err(|source| io_error("inspect qualification mounted filesystem", source))?;
    if observed.f_type != expected_magic {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationMount,
            format!(
                "{role} has unexpected filesystem type {:#x}",
                observed.f_type
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    include!("mount_overlay_state_v76_tests.rs");
    include!("mount_proc_target_v81_tests.rs");

    #[test]
    fn overlay_state_backing_check_preserves_retained_descriptor() {
        let directory = tempfile::tempdir().unwrap();
        let file = File::open(directory.path()).unwrap();
        let before = snapshot(&fstat(&file).unwrap());
        let fd = file.as_raw_fd();
        for role in ["upper", "work"] {
            let result = refuse_overlay_backed_state(&file, role);
            if fstatfs(&file).unwrap().f_type == 0x794c_7630 {
                let error = result.unwrap_err();
                assert_eq!(
                    error.kind(),
                    DeploymentVerificationErrorKindV1::InvalidQualificationMount
                );
                assert_eq!(
                    error.to_string(),
                    format!(
                        "qualification {role} directory is backed by OverlayFS, which cannot provide an overlay upper or work directory"
                    )
                );
            } else {
                result.unwrap();
            }
            assert_eq!(file.as_raw_fd(), fd);
            assert_eq!(snapshot(&fstat(&file).unwrap()), before);
        }
    }

    #[test]
    fn overlay_state_backing_is_checked_before_any_loop_or_mount() {
        let body = include_str!("mount.rs")
            .split_once(
                "pub(super) fn attach_compiler_execution_qualification_mounts_with_hooks_v1(",
            )
            .unwrap()
            .1
            .split_once("fn refuse_overlay_backed_state(")
            .unwrap()
            .0;
        let refresh = body
            .find("staged.refresh_mount_namespace_descriptors()?;")
            .unwrap();
        let check = body
            .find("refuse_overlay_backed_state(staged.directory_descriptor(name), name)?;")
            .unwrap();
        let attach = body
            .find("attach_sealed_read_only_loop_device_v1(")
            .unwrap();
        assert!(refresh < check && check < attach);
        assert!(body[refresh..check].contains("for name in [\"upper\", \"work\"]"));
        assert!(body[refresh..check].contains("namespace.revalidate()?;"));
    }

    #[test]
    #[ignore = "requires a dedicated disposable mount/PID namespace with CAP_SYS_ADMIN and OverlayFS"]
    fn qualification_overlay_backing_refuses_stacked_upper_and_accepts_native_tmpfs() {
        assert_eq!(
            std::env::var("FE2O3_PRIVATE_MOUNT_REGRESSION_V72").as_deref(),
            Ok("1")
        );
        assert_eq!(rustix::process::geteuid().as_raw(), 0);
        rustix::thread::unshare(rustix::thread::UnshareFlags::NEWNS).unwrap();
        mount_change(
            "/",
            MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
        )
        .unwrap();
        let scratch = tempfile::tempdir().unwrap();
        struct Mounts(Vec<std::path::PathBuf>);
        impl Drop for Mounts {
            fn drop(&mut self) {
                for path in self.0.iter().rev() {
                    let _ = unmount(path, UnmountFlags::DETACH);
                }
            }
        }
        let mut cleanup = Mounts(Vec::new());
        let attributes = MountAttrFlags::MOUNT_ATTR_NODEV | MountAttrFlags::MOUNT_ATTR_NOSUID;
        let flags =
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH;
        let context = fsopen("tmpfs", FsOpenFlags::FSOPEN_CLOEXEC).unwrap();
        fsconfig_set_string(&context, "size", "16m").unwrap();
        fsconfig_create(&context).unwrap();
        let detached = fsmount(&context, FsMountFlags::FSMOUNT_CLOEXEC, attributes).unwrap();
        let target = File::open(scratch.path()).unwrap();
        move_mount(&detached, "", &target, "", flags).unwrap();
        cleanup.0.push(scratch.path().to_owned());
        drop((detached, context, target));
        for name in [
            "lower",
            "upper",
            "work",
            "root",
            "native-upper",
            "native-work",
        ] {
            std::fs::create_dir(scratch.path().join(name)).unwrap();
        }
        let lower = File::open(scratch.path().join("lower")).unwrap();
        let upper = File::open(scratch.path().join("upper")).unwrap();
        let work = File::open(scratch.path().join("work")).unwrap();
        let context = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC).unwrap();
        fsconfig_set_string(&context, "lowerdir", descriptor_path(&lower)).unwrap();
        fsconfig_set_string(&context, "upperdir", descriptor_path(&upper)).unwrap();
        fsconfig_set_string(&context, "workdir", descriptor_path(&work)).unwrap();
        fsconfig_create(&context).unwrap();
        let detached = fsmount(&context, FsMountFlags::FSMOUNT_CLOEXEC, attributes).unwrap();
        let root_path = scratch.path().join("root");
        let target = File::open(&root_path).unwrap();
        move_mount(&detached, "", &target, "", flags).unwrap();
        cleanup.0.push(root_path.clone());
        drop((detached, context, target, upper, work));
        for role in ["upper", "work"] {
            let path = root_path.join(role);
            std::fs::create_dir(&path).unwrap();
            let directory = File::open(&path).unwrap();
            let before = snapshot(&fstat(&directory).unwrap());
            let fd = directory.as_raw_fd();
            assert_eq!(fstatfs(&directory).unwrap().f_type, 0x794c_7630);
            let error = refuse_overlay_backed_state(&directory, role).unwrap_err();
            assert_eq!(
                error.kind(),
                DeploymentVerificationErrorKindV1::InvalidQualificationMount
            );
            assert!(error.to_string().contains(&format!(
                "qualification {role} directory is backed by OverlayFS"
            )));
            assert_eq!(directory.as_raw_fd(), fd);
            assert_eq!(snapshot(&fstat(&directory).unwrap()), before);
            let context = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC).unwrap();
            fsconfig_set_string(&context, "lowerdir", descriptor_path(&lower)).unwrap();
            assert_eq!(
                fsconfig_set_string(&context, format!("{role}dir"), descriptor_path(&directory)),
                Err(rustix::io::Errno::INVAL)
            );
        }
        let upper = File::open(scratch.path().join("native-upper")).unwrap();
        let work = File::open(scratch.path().join("native-work")).unwrap();
        for (directory, role) in [(&upper, "upper"), (&work, "work")] {
            assert_eq!(fstatfs(directory).unwrap().f_type, 0x0102_1994);
            refuse_overlay_backed_state(directory, role).unwrap();
        }
        let context = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC).unwrap();
        fsconfig_set_string(&context, "lowerdir", descriptor_path(&lower)).unwrap();
        fsconfig_set_string(&context, "upperdir", descriptor_path(&upper)).unwrap();
        fsconfig_set_string(&context, "workdir", descriptor_path(&work)).unwrap();
        fsconfig_create(&context).unwrap();
        let detached = fsmount(&context, FsMountFlags::FSMOUNT_CLOEXEC, attributes).unwrap();
        drop((detached, context, lower, upper, work));
        unmount(&root_path, UnmountFlags::empty()).unwrap();
        cleanup.0.pop();
        unmount(scratch.path(), UnmountFlags::empty()).unwrap();
        cleanup.0.pop();
        assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
    }

    #[test]
    fn qualification_mount_identity_requires_unique_returned_mask_and_nonzero_id() {
        let file = File::open(".").unwrap();
        let mut observed = statx(&file, "", AtFlags::EMPTY_PATH, StatxFlags::BASIC_STATS).unwrap();
        // Inert decoder fixture: ordinary tests do not claim kernel unique-ID support.
        observed.stx_mask = 0x4100;
        observed.stx_mnt_id = 0x1_0000_0042;
        let expected = mount_identity_from_statx(&observed).unwrap();
        assert_eq!(expected.mount_id, 0x1_0000_0042);
        assert_eq!(expected.inode, observed.stx_ino);
        for mask in [0, 0x100, 0x1100, 0x4000] {
            let mut absent = observed;
            absent.stx_mask = mask;
            assert_eq!(
                mount_identity_from_statx(&absent).unwrap_err().kind(),
                DeploymentVerificationErrorKindV1::InvalidQualificationMount,
                "mask {mask:#x}"
            );
        }
        observed.stx_mnt_id = 0;
        assert_eq!(
            mount_identity_from_statx(&observed).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationMount
        );
    }

    #[test]
    fn unique_mount_identity_is_required_before_attachment() {
        let source = include_str!("mount.rs");
        for (start, end) in [
            ("fn attach_base(", "fn attach_overlay("),
            ("fn attach_overlay(", "enum MountedRootStateV1"),
        ] {
            let body = source
                .split_once(start)
                .unwrap()
                .1
                .split_once(end)
                .unwrap()
                .0;
            assert!(
                body.find("let identity = mount_identity(&detached)?;")
                    .unwrap()
                    < body.find("move_mount(").unwrap()
            );
        }
    }

    #[test]
    fn cleanup_mount_identity_refusal_keeps_retained_descriptor() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("base")).unwrap();
        let parent = File::open(root.path()).unwrap();
        let target = File::open(root.path().join("base")).unwrap();
        let mut retained = Some(target.try_clone().unwrap());
        let fd = retained.as_ref().unwrap().as_raw_fd();
        let mut wrong = match mount_identity(&target) {
            Ok(identity) => identity,
            Err(error) => {
                assert_eq!(
                    error.kind(),
                    DeploymentVerificationErrorKindV1::InvalidQualificationMount
                );
                assert!(
                    error
                        .to_string()
                        .contains("kernel-provided unique mount ID")
                );
                assert_eq!(retained.as_ref().unwrap().as_raw_fd(), fd);
                return;
            }
        };
        wrong.inode ^= 1;
        assert_eq!(
            unmount_retained_child(&parent, &target, "base", &mut retained, wrong)
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InputChanged
        );
        assert_eq!(retained.as_ref().unwrap().as_raw_fd(), fd);
        assert!(root.path().join("base").is_dir());
    }

    #[test]
    fn cleanup_releases_mount_fds_before_ordinary_unmount_and_keeps_failure_state() {
        let source = include_str!("mount.rs");
        let helper = source
            .split_once("fn unmount_retained_child(")
            .unwrap()
            .1
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        let identity = helper.find("current != identity").unwrap();
        let reopened = helper.find("drop(reopened)").unwrap();
        let retained = helper.find("retained.take()").unwrap();
        let unmount = helper
            .find("unmount(descriptor_path(target), UnmountFlags::empty())")
            .unwrap();
        assert!(identity < reopened && reopened < retained && retained < unmount);
        assert!(!helper.contains("DETACH"));
        let cleanup = source
            .split_once("fn cleanup_internal_with_hooks(")
            .unwrap()
            .1
            .split_once("fn unmount_retained_child(")
            .unwrap()
            .0;
        for name in ["root", "base"] {
            let branch = cleanup
                .split_once(&format!("if self.{name}_attached {{"))
                .unwrap()
                .1;
            let failure = branch.find("return Err(").unwrap();
            let clear = branch
                .find(&format!("self.{name}_attached = false;"))
                .unwrap();
            let identity = branch
                .find(&format!("self.{name}_identity = None;"))
                .unwrap();
            assert!(failure < clear && clear < identity, "{name}");
        }
    }

    #[test]
    fn squashfs_superblock_is_readonly_before_creation() {
        let body = include_str!("mount.rs")
            .split_once("fn attach_base(")
            .unwrap()
            .1
            .split_once("fn attach_overlay(")
            .unwrap()
            .0;
        let ordered = [
            "fsopen(\"squashfs\", FsOpenFlags::FSOPEN_CLOEXEC)",
            "fsconfig_set_flag(&context, \"ro\")",
            "fsconfig_set_string(&context, \"source\", loop_device.device_path())",
            "fsconfig_create(&context)",
            "let detached = fsmount(",
        ];
        let positions = ordered.map(|call| {
            assert_eq!(body.matches(call).count(), 1, "{call}");
            body.find(call).unwrap()
        });
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        let readonly_step: String = body[positions[1]..positions[2]]
            .chars()
            .filter(|value| !value.is_ascii_whitespace())
            .collect();
        assert_eq!(
            readonly_step,
            "fsconfig_set_flag(&context,\"ro\").map_err(|source|{io_error(\"requestread-onlyqualificationSquashFSsuperblock\",source,)})?;"
        );
        for flag in ["MOUNT_ATTR_RDONLY", "MOUNT_ATTR_NODEV", "MOUNT_ATTR_NOSUID"] {
            assert!(body[positions[4]..].contains(flag), "{flag}");
        }
        assert!(!body.contains("\"rw\""));
    }

    #[test]
    fn descriptor_mount_paths_and_lower_order_are_canonical() {
        let installed = File::open(".").unwrap();
        let base = File::open(".").unwrap();
        assert_eq!(
            descriptor_path(&installed),
            format!("/proc/self/fd/{}", installed.as_raw_fd())
        );
        assert_eq!(
            overlay_lowerdirs(&installed, &base),
            format!(
                "/proc/self/fd/{}:/proc/self/fd/{}",
                installed.as_raw_fd(),
                base.as_raw_fd()
            )
        );
    }

    #[test]
    fn production_mount_namespace_requires_effective_root() {
        if rustix::process::geteuid().as_raw() == 0 {
            return;
        }
        assert_eq!(
            enter_private_qualification_mount_namespace_v1()
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InsufficientPrivilege
        );
    }
}
