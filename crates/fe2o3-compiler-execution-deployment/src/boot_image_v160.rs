use super::*;
use rustix::mount::{
    FsMountFlags, FsOpenFlags, MountAttrFlags, MoveMountFlags, fsconfig_create,
    fsconfig_set_string, fsmount, fsopen, mount_bind, move_mount,
};

const IMAGE_STATE_BYTES: u64 = 64 * 1024;
const IMAGE_STATE_INODES: u64 = 16;

fn mount_namespace() -> Result<File, DeploymentVerificationErrorV1> {
    File::open("/proc/thread-self/ns/mnt")
        .map_err(|source| std_io_error("retain machine image mount namespace", source))
}

fn same_object(left: &File, right: &File) -> Result<bool, DeploymentVerificationErrorV1> {
    let left = exact_snapshot(left)?;
    let right = exact_snapshot(right)?;
    Ok((left.device, left.inode) == (right.device, right.inode))
}

fn empty_state(stage: &File, owner: (u32, u32)) -> Result<File, DeploymentVerificationErrorV1> {
    let state = super::super::super::open_beneath(stage, "state", true)?;
    let before = super::super::super::validate_directory_mode(
        &state,
        Some(owner),
        0o700,
        "machine image state target",
    )?;
    super::super::super::verify_directory_children(&state, &[], "machine image state target")?;
    if before.links != 2 || exact_snapshot(&state)? != before {
        return Err(changed("machine image state changed during admission"));
    }
    Ok(state)
}

pub(super) struct MachineImageSourceV160 {
    state: File,
    snapshot: crate::ObjectSnapshotV1,
    namespace: File,
}

impl MachineImageSourceV160 {
    pub(super) fn capture(
        stage: &File,
        owner: (u32, u32),
    ) -> Result<Self, DeploymentVerificationErrorV1> {
        let state = empty_state(stage, owner)?;
        Ok(Self {
            snapshot: exact_snapshot(&state)?,
            state,
            namespace: mount_namespace()?,
        })
    }

    pub(super) fn attach(
        self,
        stage_path: &Path,
        root: &File,
        owner: (u32, u32),
    ) -> Result<MachineImageAliasV160, DeploymentVerificationErrorV1> {
        let namespace = mount_namespace()?;
        if same_object(&namespace, &self.namespace)? {
            return Err(changed(
                "machine image alias requires a new private namespace",
            ));
        }
        rustix::mount::mount_change(
            "/",
            rustix::mount::MountPropagationFlags::PRIVATE
                | rustix::mount::MountPropagationFlags::REC,
        )
        .map_err(|source| io_error("isolate private machine image mount propagation", source))?;
        let stage = absolute_directory(stage_path)?;
        let target = empty_state(&stage, owner)?;
        if exact_snapshot(&target)? != self.snapshot
            || exact_snapshot(&self.state)? != self.snapshot
        {
            return Err(changed("machine image state target lost retained custody"));
        }
        let root_snapshot = exact_snapshot(root)?;
        let context = fsopen("tmpfs", FsOpenFlags::FSOPEN_CLOEXEC)
            .map_err(|source| io_error("create private machine image state", source))?;
        for (name, value) in [("size", "64k"), ("nr_inodes", "16"), ("mode", "0700")] {
            fsconfig_set_string(&context, name, value)
                .map_err(|source| io_error("bound private machine image state", source))?;
        }
        fsconfig_create(&context)
            .map_err(|source| io_error("instantiate private machine image state", source))?;
        let detached = fsmount(
            &context,
            FsMountFlags::FSMOUNT_CLOEXEC,
            MountAttrFlags::MOUNT_ATTR_NODEV
                | MountAttrFlags::MOUNT_ATTR_NOSUID
                | MountAttrFlags::MOUNT_ATTR_NOEXEC,
        )
        .map_err(|source| io_error("prepare private machine image state mount", source))?;
        move_mount(
            &detached,
            "",
            &target,
            "",
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
        )
        .map_err(|source| io_error("attach private machine image state mount", source))?;
        let path = stage_path.join("state");
        let state = absolute_directory(&path)?;
        rustix::fs::mkdirat(&state, "root", Mode::from_raw_mode(0o700))
            .map_err(|source| io_error("create private machine image alias target", source))?;
        let image = super::super::super::open_beneath(&state, "root", true)?;
        mount_bind(
            format!("/proc/self/fd/{}", root.as_raw_fd()),
            format!("/proc/self/fd/{}", image.as_raw_fd()),
        )
        .map_err(|source| io_error("attach retained machine image root alias", source))?;
        let image = absolute_directory(&path.join("root"))?;
        let alias = MachineImageAliasV160 {
            path,
            state_snapshot: exact_snapshot(&state)?,
            state,
            image,
            root_snapshot,
            original_state: self.state,
            original_state_snapshot: self.snapshot,
            namespace,
            owner,
        };
        alias.revalidate()?;
        Ok(alias)
    }
}

// nspawn's adjacent image lock belongs to this child-private tmpfs, not the
// parent's closed staging inventory. Normal locking is preserved. Reaping the
// helper and its descendants destroys both private mounts on every exit path.
pub(super) struct MachineImageAliasV160 {
    path: PathBuf,
    state: File,
    state_snapshot: crate::ObjectSnapshotV1,
    image: File,
    root_snapshot: crate::ObjectSnapshotV1,
    original_state: File,
    original_state_snapshot: crate::ObjectSnapshotV1,
    namespace: File,
    owner: (u32, u32),
}

impl MachineImageAliasV160 {
    pub(super) fn root(&self) -> &File {
        &self.image
    }

    pub(super) fn revalidate(&self) -> Result<(), DeploymentVerificationErrorV1> {
        let state = absolute_directory(&self.path)?;
        let image = absolute_directory(&self.path.join("root"))?;
        let filesystem = fstatfs(&state)
            .map_err(|source| io_error("inspect private machine image filesystem", source))?;
        let usage = rustix::fs::fstatvfs(&state)
            .map_err(|source| io_error("inspect private machine image bounds", source))?;
        let flags = rustix::fs::StatVfsMountFlags::NODEV
            | rustix::fs::StatVfsMountFlags::NOSUID
            | rustix::fs::StatVfsMountFlags::NOEXEC;
        let current = super::super::super::validate_directory_mode(
            &state,
            Some(self.owner),
            0o700,
            "private machine image state",
        )?;
        super::super::super::verify_directory_children(
            &state,
            &["root"],
            "private machine image state",
        )?;
        if !same_object(&mount_namespace()?, &self.namespace)?
            || filesystem.f_type != 0x0102_1994
            || usage.f_blocks.checked_mul(usage.f_frsize) != Some(IMAGE_STATE_BYTES)
            || usage.f_files != IMAGE_STATE_INODES
            || !usage.f_flag.contains(flags)
            || current.links != 3
            || current != self.state_snapshot
            || exact_snapshot(&self.state)? != self.state_snapshot
            || exact_snapshot(&self.original_state)? != self.original_state_snapshot
            || exact_snapshot(&image)? != self.root_snapshot
            || exact_snapshot(&self.image)? != self.root_snapshot
        {
            return Err(changed(
                "private machine image alias changed before nspawn exec",
            ));
        }
        Ok(())
    }
}
