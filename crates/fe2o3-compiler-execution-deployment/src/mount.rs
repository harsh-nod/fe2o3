use std::fmt;
use std::fs::File;
use std::os::fd::{AsRawFd as _, OwnedFd};

use fe2o3_loop_device::{ReadOnlyAutoclearLoopDeviceV1, attach_sealed_read_only_loop_device_v1};
use rustix::fs::{Mode, OFlags, ResolveFlags, fstat, fstatfs, openat2};
use rustix::mount::{
    FsMountFlags, FsOpenFlags, MountAttrFlags, MountPropagationFlags, MoveMountFlags, UnmountFlags,
    fsconfig_create, fsconfig_set_string, fsmount, fsopen, mount_change, move_mount, unmount,
};

use super::fault::{NoQualificationFaultV1, QualificationFaultHooksV1};
use super::host::process_thread_count;
use super::install::verify_installed_projection;
use super::qualification::revalidate_prepared_qualification_with_parent_children;
use super::staging::StagedCompilerExecutionQualificationV1;
use super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, QualificationFaultPointV1,
    changed, io_error, lower_hex, snapshot, std_io_error, validate_directory_mode,
    verify_directory_children,
};

const SQUASHFS_MAGIC_V1: i64 = 0x7371_7368;
const OVERLAYFS_MAGIC_V1: i64 = 0x794c_7630;
const QUALIFICATION_STAGING_MODE_V1: u32 = 0o700;
const COMPOSED_ROOT_MODE_V1: u32 = 0o755;
const MOUNTED_STAGING_CHILDREN_V1: &[&str] =
    &["base", "evidence", "root", "run", "state", "upper", "work"];
const UNMOUNTED_STAGING_CHILDREN_V1: &[&str] = &["evidence", "run", "state", "upper", "work"];
const PREFLIGHT_EMPTY_STAGING_CHILDREN_V1: &[&str] = &["evidence", "run", "state"];
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
    fn revalidate(&self) -> Result<(), DeploymentVerificationErrorV1> {
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
            let path = descriptor_path(
                self.staged
                    .as_ref()
                    .expect("active mounted qualification retains staging custody")
                    .directory_descriptor("root"),
            );
            if let Err(source) = unmount(path, UnmountFlags::empty()) {
                return Err(combine_cleanup_error(
                    deferred,
                    io_error("unmount qualification overlay root", source),
                ));
            }
            self.root_attached = false;
            self.mounted_root.take();
            defer_checkpoint(
                &mut deferred,
                hooks.checkpoint(QualificationFaultPointV1::OverlayUnmounted),
            );
        }
        if self.base_attached {
            let path = descriptor_path(
                self.staged
                    .as_ref()
                    .expect("active mounted qualification retains staging custody")
                    .directory_descriptor("base"),
            );
            if let Err(source) = unmount(path, UnmountFlags::empty()) {
                return Err(combine_cleanup_error(
                    deferred,
                    io_error("unmount qualification SquashFS base", source),
                ));
            }
            self.base_attached = false;
            self.mounted_base.take();
            defer_checkpoint(
                &mut deferred,
                hooks.checkpoint(QualificationFaultPointV1::BaseUnmounted),
            );
        }
        self.loop_device.take();
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
    staged: StagedCompilerExecutionQualificationV1,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<MountedCompilerExecutionQualificationV1, DeploymentVerificationErrorV1> {
    namespace.revalidate()?;
    staged.revalidate()?;
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
    move_mount(
        &detached,
        "",
        staged.directory_descriptor("base"),
        "",
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
    )
    .map_err(|source| io_error("attach qualification SquashFS mount", source))?;
    mounted.base_attached = true;
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
    let context = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC)
        .map_err(|source| io_error("open overlay mount context", source))?;
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
    move_mount(
        &detached,
        "",
        staged.directory_descriptor("root"),
        "",
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
    )
    .map_err(|source| io_error("attach qualification overlay root", source))?;
    mounted.root_attached = true;
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
    for name in UNMOUNTED_STAGING_CHILDREN_V1 {
        let directory = staged.directory_descriptor(name);
        validate_directory_mode(
            directory,
            Some(owner),
            QUALIFICATION_STAGING_MODE_V1,
            "unmounted qualification staging directory",
        )?;
        if matches!(state, MountedRootStateV1::Pristine)
            || PREFLIGHT_EMPTY_STAGING_CHILDREN_V1.contains(name)
        {
            verify_directory_children(directory, &[], "unmounted qualification staging directory")?;
        }
    }
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
