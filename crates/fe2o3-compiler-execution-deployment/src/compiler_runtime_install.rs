//! Initial offline installation; immutable files are published by directory rename.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    path::Path,
};

use fe2o3_build_authority::CompilerApprovalPolicyV2;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3 as PROFILE_BYTES,
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3 as PROFILE_STORAGE,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3 as PROFILE_WORK,
    CompilerExecutionClientProfileV3 as Profile,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::fs::{
    AtFlags, FileType, Gid, IFlags, Mode, OFlags, RenameFlags, SealFlags, Uid, fchmod, fchown,
    fcntl_get_seals, fstat, ioctl_getflags, ioctl_setflags, mkdirat, openat, renameat_with, statat,
};
use sha2::{Digest, Sha256};

use super::{
    DeploymentVerificationErrorKindV1 as Kind, DeploymentVerificationErrorV1 as Error,
    ObjectSnapshotV1, VerifiedCompilerRuntimeDeploymentV1, changed, invalid, io_error,
    open_beneath, random_staging_name, require_no_xattrs, snapshot, std_io_error,
};

const RUNTIME_PARENT: &str = "opt/fe2o3";
const APPROVAL_PARENT: &str = "etc/fe2o3";
const RUNTIME_NAME: &str = "compiler-runtime-v1";
const APPROVAL_NAME: &str = "build-authority";
const PROFILE_PATH: &str = "etc/fe2o3/compiler-execution/client-profile-v3";
const DIRECTORY_MODE: u32 = 0o755;
const BUFFER_BYTES: usize = 64 * 1024;
type Result<T> = std::result::Result<T, Error>;

/// Inert record of initial installation, not approval, execution or launch authority.
/// The caller must keep the destination offline throughout installation. This record
/// does not attest ELF dependency completeness or exclude a privileged writer.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_deployment::InstalledCompilerRuntimeDeploymentV1;
/// fn clone<T: Clone>() {}
/// clone::<InstalledCompilerRuntimeDeploymentV1>();
/// ```
#[derive(Debug)]
pub struct InstalledCompilerRuntimeDeploymentV1 {
    policy_sha256: [u8; 32],
    manifest_sha256: [u8; 32],
    file_count: usize,
}

impl InstalledCompilerRuntimeDeploymentV1 {
    /// SHA-256 of the installed canonical policy bytes, not an approval identity.
    pub const fn policy_sha256(&self) -> [u8; 32] {
        self.policy_sha256
    }
    /// SHA-256 of the installed canonical inventory bytes, not execution evidence.
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }
    /// Number of runtime code files, excluding the policy and inventory.
    pub const fn file_count(&self) -> usize {
        self.file_count
    }
}

/// Installs a verified bundle beneath a private, root-owned mode0700 offline root.
///
/// The root must already contain protected `opt/fe2o3` and `etc/fe2o3` directories
/// and a genuinely provisioned `etc/fe2o3/compiler-execution/client-profile-v3`.
/// Both destination directories must be absent. The running host root is refused.
/// The caller is responsible for excluding services and privileged concurrent
/// writers; root ownership does not prove that the destination is offline.
///
/// Files receive exact modes, root ownership and real FS_IMMUTABLE before runtime
/// directory publication, followed by approval directory publication. These are
/// two no-replace renames, not an atomic pair. Once staging begins, failure reports
/// the retained paths and publication stage. Nothing is rolled back or reused;
/// use a fresh offline root or explicitly recover it outside this API. No service
/// is started, existing state overwritten or production admission bypassed.
pub fn install_compiler_runtime_deployment_v1(
    bundle: VerifiedCompilerRuntimeDeploymentV1,
    offline_root: &Path,
) -> Result<InstalledCompilerRuntimeDeploymentV1> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(invalid(
            Kind::InsufficientPrivilege,
            "compiler runtime installation requires effective UID 0",
        ));
    }
    install_using(
        bundle,
        offline_root,
        (0, 0),
        Immutability {
            install: make_immutable,
            verify: require_immutable,
        },
        &mut |_| Ok(()),
    )
}

#[derive(Clone, Copy)]
struct Immutability {
    install: fn(&File) -> Result<()>,
    verify: fn(&File) -> Result<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Staging,
    Prepared,
    RuntimePublished,
    ApprovalPublished,
}

fn install_using(
    bundle: VerifiedCompilerRuntimeDeploymentV1,
    path: &Path,
    owner: (u32, u32),
    protection: Immutability,
    checkpoint: &mut impl FnMut(Stage) -> Result<()>,
) -> Result<InstalledCompilerRuntimeDeploymentV1> {
    let root = super::install::open_install_parent(path, owner)?;
    let host = super::open_bundle_root(Path::new("/"))?;
    if same_inode(&root, &host)? {
        return Err(invalid(
            Kind::InvalidMetadata,
            "live host root is not an offline install root",
        ));
    }
    let parents = open_parents(&root, owner)?;
    let profile = read_profile(&root, owner, &bundle.policy)?;
    require_absent(&parents.0, RUNTIME_NAME)?;
    require_absent(&parents.1, APPROVAL_NAME)?;
    let runtime_stage =
        random_staging_name(".compiler-runtime-", "name runtime staging directory")?;
    let approval_stage =
        random_staging_name(".compiler-approval-", "name approval staging directory")?;
    let mut stage = Stage::Staging;
    let root_identity = fstat(&root).map_err(|e| io_error("retain offline root identity", e))?;
    let mut runtime_created = false;
    let mut approval_created = false;
    let result = (|| {
        let runtime = create_staging(&parents.0, &runtime_stage, owner, &mut runtime_created)?;
        let approval = create_staging(&parents.1, &approval_stage, owner, &mut approval_created)?;
        checkpoint(stage)?;
        let directories = runtime_directories(&bundle);
        for directory in &directories {
            create_directory(&runtime, directory, owner, DIRECTORY_MODE)?;
        }
        if bundle.files.len() != bundle.manifest.entries().len() {
            return Err(invalid(
                Kind::InvalidInventory,
                "sealed runtime file count differs",
            ));
        }
        let mut runtime_files = Vec::with_capacity(bundle.files.len());
        for (source, entry) in bundle.files.iter().zip(bundle.manifest.entries()) {
            runtime_files.push(copy_file(
                source,
                &runtime,
                entry.path,
                entry.length,
                entry.sha256,
                entry.role.protected_mode(),
                owner,
                protection,
            )?);
        }
        let policy_digest = Sha256::digest(bundle.policy.canonical_bytes()).into();
        let manifest_digest = Sha256::digest(bundle.manifest.canonical_bytes()).into();
        let approval_files = [
            copy_file(
                &bundle.policy_file,
                &approval,
                "policy-v2",
                bundle.policy.canonical_bytes().len() as u64,
                policy_digest,
                0o444,
                owner,
                protection,
            )?,
            copy_file(
                &bundle.manifest_file,
                &approval,
                "compiler-runtime-manifest-v1",
                bundle.manifest.canonical_bytes().len() as u64,
                manifest_digest,
                0o444,
                owner,
                protection,
            )?,
        ];
        for directory in directories.iter().rev() {
            sync(&open_beneath(&runtime, directory, true)?)?;
        }
        for directory in [&runtime, &approval] {
            fchmod(directory, Mode::from_raw_mode(DIRECTORY_MODE))
                .map_err(|e| io_error("set completed staging directory mode", e))?;
            sync(directory)?;
        }
        stage = Stage::Prepared;
        checkpoint(stage)?;
        revalidate_origins(path, &root, owner, &parents, &profile, &bundle.policy)?;
        verify_tree(&runtime, &runtime_files, owner, protection.verify)?;
        verify_tree(&approval, &approval_files, owner, protection.verify)?;
        require_named_inode(&parents.0, &runtime_stage, &runtime)?;
        require_named_inode(&parents.1, &approval_stage, &approval)?;
        // Publish code first. No policy may name a partially populated runtime.
        publish(&parents.0, &runtime_stage, RUNTIME_NAME)?;
        stage = Stage::RuntimePublished;
        checkpoint(stage)?;
        sync(&parents.0)?;
        require_named_inode(&parents.0, RUNTIME_NAME, &runtime)?;
        revalidate_origins(path, &root, owner, &parents, &profile, &bundle.policy)?;
        verify_tree(&runtime, &runtime_files, owner, protection.verify)?;
        verify_tree(&approval, &approval_files, owner, protection.verify)?;
        require_named_inode(&parents.1, &approval_stage, &approval)?;
        publish(&parents.1, &approval_stage, APPROVAL_NAME)?;
        stage = Stage::ApprovalPublished;
        checkpoint(stage)?;
        sync(&parents.1)?;
        require_named_inode(&parents.1, APPROVAL_NAME, &approval)?;
        require_named_inode(&parents.0, RUNTIME_NAME, &runtime)?;
        revalidate_origins(path, &root, owner, &parents, &profile, &bundle.policy)?;
        verify_tree(&runtime, &runtime_files, owner, protection.verify)?;
        verify_tree(&approval, &approval_files, owner, protection.verify)?;
        Ok(InstalledCompilerRuntimeDeploymentV1 {
            policy_sha256: policy_digest,
            manifest_sha256: manifest_digest,
            file_count: bundle.files.len(),
        })
    })();
    result.map_err(|error| invalid(Kind::IncompleteRuntimeInstallation, format!(
        "compiler runtime install stopped at {stage:?}: {error}; no rollback performed; planned paths beneath {} (retained root device={} inode={}): {RUNTIME_PARENT}/{} (created={runtime_created}), {APPROVAL_PARENT}/{} (created={approval_created}); paths may have been displaced; recover explicitly or use a fresh offline root",
        path.display(), root_identity.st_dev, root_identity.st_ino,
        if matches!(stage, Stage::RuntimePublished | Stage::ApprovalPublished) { RUNTIME_NAME } else { &runtime_stage },
        if stage == Stage::ApprovalPublished { APPROVAL_NAME } else { &approval_stage },
    )))
}

fn open_parents(root: &File, owner: (u32, u32)) -> Result<(File, File)> {
    for path in [
        "etc",
        APPROVAL_PARENT,
        "etc/fe2o3/compiler-execution",
        "opt",
        RUNTIME_PARENT,
    ] {
        protected_directory(&open_beneath(root, path, true)?, owner)?;
    }
    Ok((
        open_beneath(root, RUNTIME_PARENT, true)?,
        open_beneath(root, APPROVAL_PARENT, true)?,
    ))
}

fn protected_directory(file: &File, owner: (u32, u32)) -> Result<()> {
    let value =
        snapshot(&fstat(file).map_err(|e| io_error("inspect protected install directory", e))?);
    if FileType::from_raw_mode(value.mode) != FileType::Directory
        || value.links < 2
        || value.mode & 0o7022 != 0
        || value.mode & 0o500 != 0o500
        || (value.uid, value.gid) != owner
    {
        return Err(invalid(
            Kind::InvalidMetadata,
            "unprotected runtime install directory",
        ));
    }
    require_no_xattrs(file, "runtime install directory")
}

struct ProfileSnapshot {
    file: File,
    metadata: ObjectSnapshotV1,
    bytes: [u8; PROFILE_BYTES],
}

fn read_profile(
    root: &File,
    owner: (u32, u32),
    policy: &CompilerApprovalPolicyV2,
) -> Result<ProfileSnapshot> {
    let file = open_beneath(root, PROFILE_PATH, false)?;
    let before = regular_file(&file, PROFILE_BYTES as u64, 0o444, owner)?;
    let mut bytes = [0; PROFILE_BYTES];
    read_at(&file, &mut bytes, 0)?;
    if regular_file(&file, PROFILE_BYTES as u64, 0o444, owner)? != before {
        return Err(changed("V3 profile changed during read"));
    }
    let mut work = Work::new(PROFILE_WORK + 64);
    let mut budget = Budget::new(&mut work, PROFILE_STORAGE + PROFILE_BYTES);
    budget
        .reserve_storage(PROFILE_BYTES)
        .map_err(|e| invalid(Kind::InvalidManifest, e.to_string()))?;
    let (profile, charge) = Profile::decode(&bytes, &mut budget)
        .map_err(|e| invalid(Kind::InvalidManifest, format!("installed V3 profile: {e}")))?;
    budget
        .reserve_storage(charge.additional_storage())
        .map_err(|e| invalid(Kind::InvalidManifest, e.to_string()))?;
    budget
        .charge_work(64)
        .map_err(|e| invalid(Kind::InvalidManifest, e.to_string()))?;
    let anchor = profile.external_anchor_service();
    if policy.client_profile_identity() != profile.identity().as_bytes()
        || [profile.supervisor_uid(), anchor.uid()].contains(&policy.proof_helper_uid())
        || [profile.supervisor_gid(), anchor.gid()].contains(&policy.proof_helper_gid())
    {
        return Err(invalid(
            Kind::ContentMismatch,
            "runtime policy differs from the V3 profile or aliases its service credentials",
        ));
    }
    Ok(ProfileSnapshot {
        file,
        metadata: before,
        bytes,
    })
}

fn revalidate_origins(
    path: &Path,
    root: &File,
    owner: (u32, u32),
    parents: &(File, File),
    profile: &ProfileSnapshot,
    policy: &CompilerApprovalPolicyV2,
) -> Result<()> {
    let mut retained_bytes = [0; PROFILE_BYTES];
    read_at(&profile.file, &mut retained_bytes, 0)?;
    if regular_file(&profile.file, PROFILE_BYTES as u64, 0o444, owner)? != profile.metadata
        || retained_bytes != profile.bytes
    {
        return Err(changed(
            "retained V3 profile changed during runtime installation",
        ));
    }
    super::install::verify_install_parent_path(path, root, owner)?;
    let current = open_parents(root, owner)?;
    if !same_inode(&parents.0, &current.0)? || !same_inode(&parents.1, &current.1)? {
        return Err(changed("runtime installation parent path changed"));
    }
    let current = read_profile(root, owner, policy)?;
    if current.metadata != profile.metadata || current.bytes != profile.bytes {
        return Err(changed(
            "installed V3 profile changed during runtime installation",
        ));
    }
    Ok(())
}

fn runtime_directories(bundle: &VerifiedCompilerRuntimeDeploymentV1) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for entry in bundle.manifest.entries() {
        for (index, _) in entry.path.match_indices('/') {
            result.insert(entry.path[..index].to_owned());
        }
    }
    result
}

fn create_directory(parent: &File, path: &str, owner: (u32, u32), mode: u32) -> Result<File> {
    // Ancestors were created and opened beneath this retained, private directory.
    let (base, name) = match path.rsplit_once('/') {
        Some((base, name)) => (open_beneath(parent, base, true)?, name),
        None => (open_beneath(parent, ".", true)?, path),
    };
    mkdirat(&base, name, Mode::from_raw_mode(0o700))
        .map_err(|e| io_error("create private runtime staging directory", e))?;
    let directory = open_beneath(&base, name, true)?;
    set_owner(&directory, owner)?;
    fchmod(&directory, Mode::from_raw_mode(mode))
        .map_err(|e| io_error("set runtime staging directory mode", e))?;
    protected_directory(&directory, owner)?;
    Ok(directory)
}

fn create_staging(
    parent: &File,
    name: &str,
    owner: (u32, u32),
    created: &mut bool,
) -> Result<File> {
    mkdirat(parent, name, Mode::from_raw_mode(0o700))
        .map_err(|e| io_error("create runtime installation staging root", e))?;
    *created = true;
    let directory = open_beneath(parent, name, true)?;
    set_owner(&directory, owner)?;
    fchmod(&directory, Mode::from_raw_mode(0o700))
        .map_err(|e| io_error("set runtime installation staging root mode", e))?;
    protected_directory(&directory, owner)?;
    Ok(directory)
}

fn set_owner(file: &File, (uid, gid): (u32, u32)) -> Result<()> {
    fchown(file, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid)))
        .map_err(|e| io_error("set installed runtime owner", e))
}

fn regular_file(
    file: &File,
    length: u64,
    mode: u32,
    owner: (u32, u32),
) -> Result<ObjectSnapshotV1> {
    let value = snapshot(&fstat(file).map_err(|e| io_error("inspect installed runtime file", e))?);
    if FileType::from_raw_mode(value.mode) != FileType::RegularFile
        || value.links != 1
        || value.byte_len != length
        || value.mode & 0o7777 != mode
        || (value.uid, value.gid) != owner
    {
        return Err(invalid(
            Kind::InvalidMetadata,
            "installed runtime file metadata differs",
        ));
    }
    require_no_xattrs(file, "installed runtime file")?;
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
fn copy_file(
    source: &File,
    root: &File,
    path: &str,
    length: u64,
    expected: [u8; 32],
    mode: u32,
    owner: (u32, u32),
    protection: Immutability,
) -> Result<InstalledFile> {
    let seals = SealFlags::SEAL | SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE;
    if !fcntl_get_seals(source)
        .map_err(|e| io_error("inspect retained runtime seals", e))?
        .contains(seals)
        || snapshot(&fstat(source).map_err(|e| io_error("inspect sealed runtime length", e))?)
            .byte_len
            != length
    {
        return Err(invalid(
            Kind::InvalidMetadata,
            "retained runtime is not sealed at its exact length",
        ));
    }
    let (parent, name) = match path.rsplit_once('/') {
        Some((parent, name)) => (open_beneath(root, parent, true)?, name),
        None => (open_beneath(root, ".", true)?, path),
    };
    let output = File::from(
        openat(
            &parent,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|e| io_error("create installed runtime file", e))?,
    );
    let mut buffer = [0; BUFFER_BYTES];
    let mut digest = Sha256::new();
    let mut offset = 0;
    while offset < length {
        let count = (length - offset).min(BUFFER_BYTES as u64) as usize;
        read_at(source, &mut buffer[..count], offset)?;
        digest.update(&buffer[..count]);
        let written = rustix::io::write(&output, &buffer[..count])
            .map_err(|e| io_error("write installed runtime content", e))?;
        if written != count {
            return Err(invalid(Kind::Io, "short installed runtime write"));
        }
        offset += count as u64;
    }
    if <[u8; 32]>::from(digest.finalize()) != expected {
        return Err(invalid(
            Kind::ContentMismatch,
            "sealed runtime digest differs",
        ));
    }
    set_owner(&output, owner)?;
    fchmod(&output, Mode::from_raw_mode(mode))
        .map_err(|e| io_error("set protected runtime file mode", e))?;
    sync(&output)?;
    (protection.install)(&output)?;
    (protection.verify)(&output)?;
    sync(&output)?;
    let before = regular_file(&output, length, mode, owner)?;
    let installed = open_beneath(root, path, false)?;
    if regular_file(&installed, length, mode, owner)? != before {
        return Err(changed(
            "installed runtime pathname differs from created file",
        ));
    }
    let mut digest = Sha256::new();
    offset = 0;
    while offset < length {
        let count = (length - offset).min(BUFFER_BYTES as u64) as usize;
        read_at(&installed, &mut buffer[..count], offset)?;
        digest.update(&buffer[..count]);
        offset += count as u64;
    }
    if <[u8; 32]>::from(digest.finalize()) != expected
        || regular_file(&installed, length, mode, owner)? != before
    {
        return Err(changed(
            "installed runtime changed during content verification",
        ));
    }
    Ok(InstalledFile {
        path: path.to_owned(),
        metadata: before,
        file: installed,
        sha256: expected,
    })
}

fn make_immutable(file: &File) -> Result<()> {
    let flags = ioctl_getflags(file).map_err(|e| io_error("inspect runtime inode flags", e))?;
    ioctl_setflags(file, flags | IFlags::IMMUTABLE)
        .map_err(|e| io_error("set immutable runtime inode", e))?;
    require_immutable(file)
}

fn require_immutable(file: &File) -> Result<()> {
    if !ioctl_getflags(file)
        .map_err(|e| io_error("reinspect runtime inode flags", e))?
        .contains(IFlags::IMMUTABLE)
    {
        return Err(invalid(
            Kind::InvalidMetadata,
            "runtime inode is not immutable",
        ));
    }
    Ok(())
}

struct InstalledFile {
    path: String,
    metadata: ObjectSnapshotV1,
    file: File,
    sha256: [u8; 32],
}

fn verify_tree(
    root: &File,
    files: &[InstalledFile],
    owner: (u32, u32),
    immutable: fn(&File) -> Result<()>,
) -> Result<()> {
    let mut roster: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for file in files {
        if regular_file(
            &file.file,
            file.metadata.byte_len,
            file.metadata.mode & 0o7777,
            owner,
        )? != file.metadata
        {
            return Err(changed("retained installed runtime file changed"));
        }
        immutable(&file.file)?;
        let mut offset = 0;
        let mut buffer = [0; BUFFER_BYTES];
        let mut digest = Sha256::new();
        while offset < file.metadata.byte_len {
            let count = (file.metadata.byte_len - offset).min(BUFFER_BYTES as u64) as usize;
            read_at(&file.file, &mut buffer[..count], offset)?;
            digest.update(&buffer[..count]);
            offset += count as u64;
        }
        if <[u8; 32]>::from(digest.finalize()) != file.sha256 {
            return Err(changed("retained installed runtime digest changed"));
        }
        let mut base = "";
        for (index, _) in file.path.match_indices('/') {
            roster
                .entry(base)
                .or_default()
                .insert(&file.path[if base.is_empty() { 0 } else { base.len() + 1 }..index]);
            base = &file.path[..index];
        }
        roster
            .entry(base)
            .or_default()
            .insert(&file.path[if base.is_empty() { 0 } else { base.len() + 1 }..]);
        let current = open_beneath(root, &file.path, false)?;
        if regular_file(
            &current,
            file.metadata.byte_len,
            file.metadata.mode & 0o7777,
            owner,
        )? != file.metadata
        {
            return Err(changed("completed runtime file was replaced or changed"));
        }
        immutable(&current)?;
    }
    for (path, mut children) in roster {
        let directory = open_beneath(root, if path.is_empty() { "." } else { path }, true)?;
        protected_directory(&directory, owner)?;
        let before = snapshot(
            &fstat(&directory).map_err(|e| io_error("inspect completed runtime directory", e))?,
        );
        if before.mode & 0o7777 != DIRECTORY_MODE {
            return Err(invalid(
                Kind::InvalidMetadata,
                "completed runtime directory mode differs",
            ));
        }
        let mut scan = rustix::fs::Dir::read_from(&directory)
            .map_err(|e| io_error("enumerate completed runtime directory", e))?;
        let max_entries = children.len() + 2;
        for (index, entry) in (&mut scan).enumerate() {
            if index >= max_entries {
                return Err(invalid(
                    Kind::InvalidInventory,
                    "completed runtime directory exceeds its roster bound",
                ));
            }
            let entry = entry.map_err(|e| io_error("read completed runtime directory", e))?;
            let bytes = entry.file_name().to_bytes();
            if matches!(bytes, b"." | b"..") {
                continue;
            }
            if !std::str::from_utf8(bytes).is_ok_and(|name| children.remove(name)) {
                return Err(invalid(
                    Kind::InvalidInventory,
                    "completed runtime contains an extra or substituted path",
                ));
            }
        }
        if !children.is_empty() {
            return Err(invalid(
                Kind::InvalidInventory,
                "completed runtime is missing a path",
            ));
        }
        if snapshot(
            &fstat(&directory).map_err(|e| io_error("reinspect completed runtime directory", e))?,
        ) != before
        {
            return Err(changed(
                "completed runtime directory changed during enumeration",
            ));
        }
    }
    Ok(())
}

fn require_absent(parent: &File, name: &str) -> Result<()> {
    match statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(error) => Err(io_error("inspect initial runtime destination", error)),
        Ok(_) => Err(invalid(
            Kind::InvalidInventory,
            "runtime install destination already exists; no overwrite or reuse",
        )),
    }
}

fn same_inode(left: &File, right: &File) -> Result<bool> {
    let left = fstat(left).map_err(|e| io_error("inspect retained install inode", e))?;
    let right = fstat(right).map_err(|e| io_error("inspect current install inode", e))?;
    Ok(left.st_dev == right.st_dev && left.st_ino == right.st_ino)
}

fn require_named_inode(parent: &File, name: &str, retained: &File) -> Result<()> {
    if !same_inode(&open_beneath(parent, name, true)?, retained)? {
        return Err(changed("published runtime directory was replaced"));
    }
    Ok(())
}

fn publish(parent: &File, staging: &str, destination: &str) -> Result<()> {
    renameat_with(parent, staging, parent, destination, RenameFlags::NOREPLACE)
        .map_err(|e| io_error("publish immutable runtime directory without replacement", e))
}

fn sync(file: &File) -> Result<()> {
    file.sync_all()
        .map_err(|e| std_io_error("sync compiler runtime installation", e))
}

fn read_at(file: &File, bytes: &mut [u8], offset: u64) -> Result<()> {
    let expected = bytes.len();
    let read = rustix::io::pread(file, bytes, offset)
        .map_err(|e| io_error("read exact runtime installation content", e))?;
    if read != expected {
        return Err(invalid(Kind::Io, "short runtime installation read"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "compiler_runtime_install_tests.rs"]
mod tests;
