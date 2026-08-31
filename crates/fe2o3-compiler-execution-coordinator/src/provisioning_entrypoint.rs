use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io::{self, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1, COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1,
    CompilerExecutionIssuerMeasurementV1,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
    MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
};
use fe2o3_protected_service_spawn::{ProtectedServiceSpawnErrorV1, require_exact_root_identity_v1};
use fe2o3_runtime_protocol::{
    SealedStaticApplicationErrorV1, sealed_static_application_identity_v1,
};
use rustix::fs::{
    AtFlags, FileType, FlockOperation, Mode, OFlags, RenameFlags, flock, fstat, fsync, open,
    openat, renameat_with, unlinkat,
};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use crate::inherited::{RootFileSnapshotV1, validate_provisioned_file};
use crate::lifecycle::{CompilerExecutionLifecycleLeaseModeV1, CompilerExecutionLifecycleLeaseV1};
use crate::{
    CompilerExecutionCoordinatorErrorV1, CompilerExecutionProvisioningBundleV1,
    CompilerExecutionProvisioningErrorV1, CompilerExecutionProvisioningInputsV1,
};

const CONFIG_DIRECTORY_V1: &str = "/etc/fe2o3/compiler-execution";
const SUPERVISOR_PATH_V1: &str = "/usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor";
const LAUNCHER_PATH_V1: &str = "/usr/libexec/fe2o3/fe2o3-static-preexec-launcher";
const ISSUER_PATH_V1: &str = "/usr/libexec/fe2o3/fe2o3-compiler-execution-issuer";
const ANCHOR_HELPER_PATH_V1: &str = "/usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper";
const ANCHOR_DAEMON_PATH_V1: &str = "/usr/libexec/fe2o3/fe2o3-external-anchor-service";

const SUPERVISOR_DEPLOYMENT_FILE_V1: &str = "supervisor-deployment-v1";
const ISSUER_POLICY_FILE_V1: &str = "issuer-policy-v1";
const ANCHOR_DEPLOYMENT_FILE_V1: &str = "anchor-deployment-v1";
const ANCHOR_PROVISIONING_FILE_V1: &str = "anchor-provisioning-v1";
const CLIENT_PROFILE_FILE_V1: &str = "client-profile-v1";
const ISSUER_SEED_FILE_V1: &str = "issuer-signing-key-seed-v1";
const ANCHOR_SEED_FILE_V1: &str = "anchor-signing-key-seed-v1";

const COMPILER_USER_V1: &[u8] = b"fe2o3-compiler\0";
const COMPILER_GROUP_V1: &[u8] = b"fe2o3-compiler\0";
const ANCHOR_USER_V1: &[u8] = b"fe2o3-anchor\0";
const ANCHOR_GROUP_V1: &[u8] = b"fe2o3-anchor\0";

const ROOT_ID_V1: u32 = 0;
const CONFIG_DIRECTORY_MODE_V1: u32 = 0o755;
const LIFECYCLE_PARENT_MODE_V1: u32 = 0o755;
const EXECUTABLE_MODE_V1: u32 = 0o555;
const PUBLIC_RECORD_MODE_V1: u32 = 0o444;
const SECRET_SEED_MODE_V1: u32 = 0o400;
const KEY_SEED_BYTES_V1: usize = 32;
const ACCOUNT_BUFFER_MAX_V1: usize = 1024 * 1024;

/// Provisions the fixed same-host reference compiler-execution deployment.
///
/// The only argument is one canonical nonzero decimal policy generation. Production paths,
/// service account names, image roles, record names, modes, and ownership are fixed. Existing
/// matching files are accepted; an existing mismatch is never replaced.
pub fn run_compiler_execution_reference_provisioner_v1()
-> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
    let generation = parse_generation_arguments()?;
    require_single_threaded_entrypoint()?;
    clear_environment()?;
    require_exact_root_identity_v1()
        .map_err(CompilerExecutionProvisioningInstallErrorV1::RootIdentity)?;
    let compiler = lookup_service_identity(COMPILER_USER_V1, COMPILER_GROUP_V1)?;
    let anchor = lookup_service_identity(ANCHOR_USER_V1, ANCHOR_GROUP_V1)?;
    provision_layout(
        &ProvisioningLayoutV1::production(),
        generation,
        compiler,
        anchor,
        ROOT_ID_V1,
        ROOT_ID_V1,
    )
}

fn parse_generation_arguments() -> Result<u64, CompilerExecutionProvisioningInstallErrorV1> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let generation = arguments
        .next()
        .ok_or(CompilerExecutionProvisioningInstallErrorV1::InvalidArguments)?;
    if arguments.next().is_some() {
        return Err(CompilerExecutionProvisioningInstallErrorV1::InvalidArguments);
    }
    parse_generation(&generation)
}

fn parse_generation(
    generation: &OsStr,
) -> Result<u64, CompilerExecutionProvisioningInstallErrorV1> {
    let text = generation
        .to_str()
        .ok_or(CompilerExecutionProvisioningInstallErrorV1::InvalidArguments)?;
    let value = text
        .parse::<u64>()
        .ok()
        .filter(|value| *value != 0 && value.to_string() == text)
        .ok_or(CompilerExecutionProvisioningInstallErrorV1::InvalidArguments)?;
    Ok(value)
}

fn clear_environment() -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
    // SAFETY: this entrypoint requires one thread and performs no environment access afterwards.
    if unsafe { libc::clearenv() } != 0 {
        return Err(CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "clear provisioning environment",
            source: io::Error::last_os_error(),
        });
    }
    Ok(())
}

fn require_single_threaded_entrypoint() -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
    let tasks = std::fs::read_dir("/proc/self/task").map_err(|source| {
        CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "inspect provisioning thread set",
            source,
        }
    })?;
    if tasks.take(2).count() != 1 {
        return Err(CompilerExecutionProvisioningInstallErrorV1::MultipleThreads);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ServiceIdentityV1 {
    uid: u32,
    gid: u32,
}

fn lookup_service_identity(
    user_name: &'static [u8],
    group_name: &'static [u8],
) -> Result<ServiceIdentityV1, CompilerExecutionProvisioningInstallErrorV1> {
    let (uid, primary_gid) = lookup_user(user_name)?;
    let gid = lookup_group(group_name)?;
    if primary_gid != gid {
        return Err(
            CompilerExecutionProvisioningInstallErrorV1::AccountPrimaryGroupMismatch {
                uid,
                primary_gid,
                named_gid: gid,
            },
        );
    }
    Ok(ServiceIdentityV1 { uid, gid })
}

fn lookup_user(
    name: &'static [u8],
) -> Result<(u32, u32), CompilerExecutionProvisioningInstallErrorV1> {
    debug_assert_eq!(name.last(), Some(&0));
    let mut size = 16 * 1024;
    loop {
        let mut buffer = vec![0_u8; size];
        let mut entry = MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        // SAFETY: name is NUL-terminated, buffer is writable for its exact length, and both output
        // pointers remain valid for the duration of this reentrant libc lookup.
        let status = unsafe {
            libc::getpwnam_r(
                name.as_ptr().cast(),
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE && size < ACCOUNT_BUFFER_MAX_V1 {
            size = (size * 2).min(ACCOUNT_BUFFER_MAX_V1);
            continue;
        }
        if status != 0 {
            return Err(CompilerExecutionProvisioningInstallErrorV1::Io {
                operation: "look up compiler-execution service user",
                source: io::Error::from_raw_os_error(status),
            });
        }
        if result.is_null() {
            return Err(CompilerExecutionProvisioningInstallErrorV1::AccountMissing);
        }
        // SAFETY: a non-null result from getpwnam_r points to the initialized entry supplied above.
        let entry = unsafe { entry.assume_init() };
        return Ok((entry.pw_uid, entry.pw_gid));
    }
}

fn lookup_group(name: &'static [u8]) -> Result<u32, CompilerExecutionProvisioningInstallErrorV1> {
    debug_assert_eq!(name.last(), Some(&0));
    let mut size = 16 * 1024;
    loop {
        let mut buffer = vec![0_u8; size];
        let mut entry = MaybeUninit::<libc::group>::uninit();
        let mut result = std::ptr::null_mut();
        // SAFETY: name is NUL-terminated, buffer is writable for its exact length, and both output
        // pointers remain valid for the duration of this reentrant libc lookup.
        let status = unsafe {
            libc::getgrnam_r(
                name.as_ptr().cast(),
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE && size < ACCOUNT_BUFFER_MAX_V1 {
            size = (size * 2).min(ACCOUNT_BUFFER_MAX_V1);
            continue;
        }
        if status != 0 {
            return Err(CompilerExecutionProvisioningInstallErrorV1::Io {
                operation: "look up compiler-execution service group",
                source: io::Error::from_raw_os_error(status),
            });
        }
        if result.is_null() {
            return Err(CompilerExecutionProvisioningInstallErrorV1::AccountMissing);
        }
        // SAFETY: a non-null result from getgrnam_r points to the initialized entry supplied above.
        let entry = unsafe { entry.assume_init() };
        return Ok(entry.gr_gid);
    }
}

struct ProvisioningLayoutV1 {
    config_directory: PathBuf,
    lifecycle_lock: PathBuf,
    listener: PathBuf,
    supervisor: PathBuf,
    launcher: PathBuf,
    issuer: PathBuf,
    anchor_helper: PathBuf,
    anchor_daemon: PathBuf,
}

impl ProvisioningLayoutV1 {
    fn production() -> Self {
        Self {
            config_directory: CONFIG_DIRECTORY_V1.into(),
            lifecycle_lock: COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1.into(),
            listener: COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1.into(),
            supervisor: SUPERVISOR_PATH_V1.into(),
            launcher: LAUNCHER_PATH_V1.into(),
            issuer: ISSUER_PATH_V1.into(),
            anchor_helper: ANCHOR_HELPER_PATH_V1.into(),
            anchor_daemon: ANCHOR_DAEMON_PATH_V1.into(),
        }
    }
}

fn provision_layout(
    layout: &ProvisioningLayoutV1,
    generation: u64,
    compiler: ServiceIdentityV1,
    anchor: ServiceIdentityV1,
    expected_file_uid: u32,
    expected_file_gid: u32,
) -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
    let lifecycle = RetainedProvisioningLifecycleLeaseV1::admit(
        &layout.lifecycle_lock,
        expected_file_uid,
        expected_file_gid,
    )?;
    require_listener_absent(&layout.listener)?;
    let (directory, directory_snapshot) = open_and_lock_directory(
        &layout.config_directory,
        expected_file_uid,
        expected_file_gid,
    )?;
    lifecycle.revalidate()?;
    require_listener_absent(&layout.listener)?;
    let supervisor = measure_static_image(
        &layout.supervisor,
        "protected supervisor",
        MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
        expected_file_uid,
        expected_file_gid,
    )?;
    let launcher = measure_static_image(
        &layout.launcher,
        "static pre-exec launcher",
        MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
        expected_file_uid,
        expected_file_gid,
    )?;
    let issuer = measure_static_image(
        &layout.issuer,
        "compiler-execution issuer",
        MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
        expected_file_uid,
        expected_file_gid,
    )?;
    let anchor_helper = measure_static_image(
        &layout.anchor_helper,
        "external-anchor provisioning helper",
        MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
        expected_file_uid,
        expected_file_gid,
    )?;
    let anchor_daemon = measure_static_image(
        &layout.anchor_daemon,
        "external-anchor daemon",
        MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
        expected_file_uid,
        expected_file_gid,
    )?;

    let issuer_seed = load_or_create_seed(
        &directory,
        ISSUER_SEED_FILE_V1,
        "issuer signing-key seed",
        expected_file_uid,
        expected_file_gid,
    )?;
    let anchor_seed = load_or_create_seed(
        &directory,
        ANCHOR_SEED_FILE_V1,
        "external-anchor signing-key seed",
        expected_file_uid,
        expected_file_gid,
    )?;
    let issuer_verifying_key = SigningKey::from_bytes(issuer_seed.as_bytes())
        .verifying_key()
        .to_bytes();
    let anchor_verifying_key = SigningKey::from_bytes(anchor_seed.as_bytes())
        .verifying_key()
        .to_bytes();
    let inputs = CompilerExecutionProvisioningInputsV1::new(
        generation,
        compiler.uid,
        compiler.gid,
        anchor.uid,
        anchor.gid,
        supervisor.measurement,
        launcher.measurement,
        issuer.measurement,
        anchor_helper.measurement,
        anchor_daemon.measurement,
        issuer_verifying_key,
        anchor_verifying_key,
    )
    .map_err(CompilerExecutionProvisioningInstallErrorV1::Provisioning)?;
    let bundle = CompilerExecutionProvisioningBundleV1::new(inputs)
        .map_err(CompilerExecutionProvisioningInstallErrorV1::Provisioning)?;
    drop(issuer_seed);
    drop(anchor_seed);

    lifecycle.revalidate()?;
    require_listener_absent(&layout.listener)?;
    publish_or_verify(
        &directory,
        ISSUER_POLICY_FILE_V1,
        "issuer policy",
        bundle.policy().canonical_bytes(),
        expected_file_uid,
        expected_file_gid,
    )?;
    publish_or_verify(
        &directory,
        SUPERVISOR_DEPLOYMENT_FILE_V1,
        "supervisor deployment",
        bundle.supervisor().canonical_bytes(),
        expected_file_uid,
        expected_file_gid,
    )?;
    publish_or_verify(
        &directory,
        ANCHOR_DEPLOYMENT_FILE_V1,
        "external-anchor deployment",
        bundle.anchor_deployment().canonical_bytes(),
        expected_file_uid,
        expected_file_gid,
    )?;
    publish_or_verify(
        &directory,
        ANCHOR_PROVISIONING_FILE_V1,
        "external-anchor provisioning",
        bundle.anchor_provisioning().canonical_bytes(),
        expected_file_uid,
        expected_file_gid,
    )?;
    publish_or_verify(
        &directory,
        CLIENT_PROFILE_FILE_V1,
        "compiler-execution client profile",
        bundle.client_profile().canonical_bytes(),
        expected_file_uid,
        expected_file_gid,
    )?;
    supervisor.revalidate(expected_file_uid, expected_file_gid)?;
    launcher.revalidate(expected_file_uid, expected_file_gid)?;
    issuer.revalidate(expected_file_uid, expected_file_gid)?;
    anchor_helper.revalidate(expected_file_uid, expected_file_gid)?;
    anchor_daemon.revalidate(expected_file_uid, expected_file_gid)?;
    revalidate_directory_path(
        &layout.config_directory,
        &directory,
        directory_snapshot,
        expected_file_uid,
        expected_file_gid,
    )?;
    lifecycle.revalidate()?;
    require_listener_absent(&layout.listener)?;
    Ok(())
}

fn require_listener_absent(path: &Path) -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(CompilerExecutionProvisioningInstallErrorV1::ListenerActive),
        Err(source) => Err(CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "inspect compiler-execution listener",
            source,
        }),
    }
}

struct RetainedProvisioningLifecycleLeaseV1 {
    parent_path: PathBuf,
    lock_name: PathBuf,
    parent: OwnedFd,
    parent_snapshot: DirectorySnapshotV1,
    parent_uid: u32,
    parent_gid: u32,
    lease: CompilerExecutionLifecycleLeaseV1,
}

impl RetainedProvisioningLifecycleLeaseV1 {
    fn admit(
        lock_path: &Path,
        parent_uid: u32,
        parent_gid: u32,
    ) -> Result<Self, CompilerExecutionProvisioningInstallErrorV1> {
        let parent_path = lock_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or(CompilerExecutionProvisioningInstallErrorV1::InvalidLifecycleParent)?;
        let lock_name = lock_path
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or(CompilerExecutionProvisioningInstallErrorV1::InvalidLifecycleParent)?;
        let parent = open_directory(parent_path, "open compiler-execution lifecycle parent")?;
        let parent_snapshot = inspect_lifecycle_parent(&parent, parent_uid, parent_gid)?;
        let lock = openat(
            &parent,
            lock_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "open compiler-execution lifecycle lock",
            source: source.into(),
        })?;
        let lease = CompilerExecutionLifecycleLeaseV1::admit(
            File::from(lock),
            CompilerExecutionLifecycleLeaseModeV1::ExclusiveProvisioning,
            parent_uid,
            parent_gid,
        )
        .map_err(CompilerExecutionProvisioningInstallErrorV1::Coordinator)?;
        let admitted = Self {
            parent_path: parent_path.to_path_buf(),
            lock_name: lock_name.into(),
            parent,
            parent_snapshot,
            parent_uid,
            parent_gid,
            lease,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    fn revalidate(&self) -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
        self.lease
            .revalidate()
            .map_err(CompilerExecutionProvisioningInstallErrorV1::Coordinator)?;
        if inspect_lifecycle_parent(&self.parent, self.parent_uid, self.parent_gid)?
            != self.parent_snapshot
        {
            return Err(CompilerExecutionProvisioningInstallErrorV1::LifecycleParentChanged);
        }
        let reopened_parent = open_directory(
            &self.parent_path,
            "reopen compiler-execution lifecycle parent",
        )?;
        if inspect_lifecycle_parent(&reopened_parent, self.parent_uid, self.parent_gid)?
            != self.parent_snapshot
        {
            return Err(CompilerExecutionProvisioningInstallErrorV1::LifecycleParentChanged);
        }
        for parent in [&self.parent, &reopened_parent] {
            let alias = openat(
                parent,
                &self.lock_name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
                operation: "reopen compiler-execution lifecycle lock",
                source: source.into(),
            })?;
            self.lease
                .revalidate_alias(File::from(alias))
                .map_err(CompilerExecutionProvisioningInstallErrorV1::Coordinator)?;
        }
        self.lease
            .revalidate()
            .map_err(CompilerExecutionProvisioningInstallErrorV1::Coordinator)
    }
}

fn open_directory(
    path: &Path,
    operation: &'static str,
) -> Result<OwnedFd, CompilerExecutionProvisioningInstallErrorV1> {
    open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
        operation,
        source: source.into(),
    })
}

fn inspect_lifecycle_parent(
    directory: &OwnedFd,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<DirectorySnapshotV1, CompilerExecutionProvisioningInstallErrorV1> {
    let stat =
        fstat(directory).map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "inspect compiler-execution lifecycle parent",
            source: source.into(),
        })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_mode & 0o7777 != LIFECYCLE_PARENT_MODE_V1
        || stat.st_uid != expected_uid
        || stat.st_gid != expected_gid
        || stat.st_nlink == 0
    {
        return Err(CompilerExecutionProvisioningInstallErrorV1::InvalidLifecycleParent);
    }
    reject_forbidden_metadata(directory, "lifecycle parent")?;
    Ok(DirectorySnapshotV1::from_stat(&stat))
}

fn open_and_lock_directory(
    path: &Path,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<(OwnedFd, DirectorySnapshotV1), CompilerExecutionProvisioningInstallErrorV1> {
    let directory = open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
        operation: "open compiler-execution configuration directory",
        source: source.into(),
    })?;
    let stat =
        fstat(&directory).map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "inspect compiler-execution configuration directory",
            source: source.into(),
        })?;
    let snapshot = DirectorySnapshotV1::from_stat(&stat);
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_mode & 0o7777 != CONFIG_DIRECTORY_MODE_V1
        || stat.st_uid != expected_uid
        || stat.st_gid != expected_gid
        || stat.st_nlink == 0
    {
        return Err(CompilerExecutionProvisioningInstallErrorV1::InvalidDirectory);
    }
    reject_forbidden_metadata(&directory, "configuration directory")?;
    flock(&directory, FlockOperation::NonBlockingLockExclusive).map_err(|source| {
        if source == rustix::io::Errno::WOULDBLOCK {
            CompilerExecutionProvisioningInstallErrorV1::DirectoryBusy
        } else {
            CompilerExecutionProvisioningInstallErrorV1::Io {
                operation: "lock compiler-execution configuration directory",
                source: source.into(),
            }
        }
    })?;
    Ok((directory, snapshot))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectorySnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
}

impl DirectorySnapshotV1 {
    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            uid: stat.st_uid,
            gid: stat.st_gid,
            links: stat.st_nlink,
        }
    }
}

fn revalidate_directory_path(
    path: &Path,
    retained: &OwnedFd,
    expected: DirectorySnapshotV1,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
    let retained_stat =
        fstat(retained).map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "reinspect retained provisioning directory",
            source: source.into(),
        })?;
    let reopened = open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
        operation: "reopen provisioning directory path",
        source: source.into(),
    })?;
    let reopened_stat =
        fstat(&reopened).map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "reinspect provisioning directory path",
            source: source.into(),
        })?;
    if DirectorySnapshotV1::from_stat(&retained_stat) != expected
        || DirectorySnapshotV1::from_stat(&reopened_stat) != expected
        || reopened_stat.st_mode & 0o7777 != CONFIG_DIRECTORY_MODE_V1
        || reopened_stat.st_uid != expected_uid
        || reopened_stat.st_gid != expected_gid
        || reopened_stat.st_nlink == 0
    {
        return Err(CompilerExecutionProvisioningInstallErrorV1::DirectoryChanged);
    }
    reject_forbidden_metadata(&reopened, "configuration directory")
}

fn measure_static_image(
    path: &Path,
    role: &'static str,
    maximum_byte_len: u64,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<MeasuredStaticImageV1, CompilerExecutionProvisioningInstallErrorV1> {
    let descriptor = open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
        operation: "open provisioned static image",
        source: source.into(),
    })?;
    let file = File::from(descriptor);
    let before = validate_provisioned_file(
        &file,
        role,
        EXECUTABLE_MODE_V1,
        None,
        expected_uid,
        expected_gid,
    )
    .map_err(CompilerExecutionProvisioningInstallErrorV1::Coordinator)?;
    let byte_len = usize::try_from(before.byte_len).ok().filter(|byte_len| {
        u64::try_from(*byte_len).is_ok_and(|byte_len| byte_len <= maximum_byte_len)
    });
    let byte_len =
        byte_len.ok_or(CompilerExecutionProvisioningInstallErrorV1::ImageTooLarge { role })?;
    let mut first = vec![0_u8; byte_len];
    let mut second = vec![0_u8; byte_len];
    file.read_exact_at(&mut first, 0).map_err(|source| {
        CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "read provisioned static image",
            source,
        }
    })?;
    file.read_exact_at(&mut second, 0).map_err(|source| {
        CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "reread provisioned static image",
            source,
        }
    })?;
    let after = validate_provisioned_file(
        &file,
        role,
        EXECUTABLE_MODE_V1,
        None,
        expected_uid,
        expected_gid,
    )
    .map_err(CompilerExecutionProvisioningInstallErrorV1::Coordinator)?;
    if before != after || first != second {
        return Err(CompilerExecutionProvisioningInstallErrorV1::ImageChanged { role });
    }
    sealed_static_application_identity_v1(&first).map_err(|source| {
        CompilerExecutionProvisioningInstallErrorV1::InvalidStaticImage { role, source }
    })?;
    let measurement = CompilerExecutionIssuerMeasurementV1::new(
        Sha256::digest(&first).into(),
        u64::try_from(first.len()).expect("bounded image length fits u64"),
    )
    .map_err(|_| CompilerExecutionProvisioningInstallErrorV1::ImageTooLarge { role })?;
    Ok(MeasuredStaticImageV1 {
        path: path.to_path_buf(),
        role,
        file,
        snapshot: before,
        measurement,
    })
}

struct MeasuredStaticImageV1 {
    path: PathBuf,
    role: &'static str,
    file: File,
    snapshot: RootFileSnapshotV1,
    measurement: CompilerExecutionIssuerMeasurementV1,
}

impl MeasuredStaticImageV1 {
    fn revalidate(
        &self,
        expected_uid: u32,
        expected_gid: u32,
    ) -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
        let retained = validate_provisioned_file(
            &self.file,
            self.role,
            EXECUTABLE_MODE_V1,
            None,
            expected_uid,
            expected_gid,
        )
        .map_err(CompilerExecutionProvisioningInstallErrorV1::Coordinator)?;
        let reopened = open(
            &self.path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "reopen provisioned static image",
            source: source.into(),
        })?;
        let reopened = validate_provisioned_file(
            &File::from(reopened),
            self.role,
            EXECUTABLE_MODE_V1,
            None,
            expected_uid,
            expected_gid,
        )
        .map_err(CompilerExecutionProvisioningInstallErrorV1::Coordinator)?;
        if retained != self.snapshot || reopened != self.snapshot {
            return Err(CompilerExecutionProvisioningInstallErrorV1::ImageChanged {
                role: self.role,
            });
        }
        Ok(())
    }
}

struct SecretSeedV1([u8; KEY_SEED_BYTES_V1]);

impl SecretSeedV1 {
    fn as_bytes(&self) -> &[u8; KEY_SEED_BYTES_V1] {
        &self.0
    }
}

impl Drop for SecretSeedV1 {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

fn load_or_create_seed(
    directory: &OwnedFd,
    name: &'static str,
    role: &'static str,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<SecretSeedV1, CompilerExecutionProvisioningInstallErrorV1> {
    match openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(descriptor) => {
            let bytes = read_stable_exact::<KEY_SEED_BYTES_V1>(
                &File::from(descriptor),
                role,
                SECRET_SEED_MODE_V1,
                expected_uid,
                expected_gid,
            )?;
            Ok(SecretSeedV1(bytes))
        }
        Err(rustix::io::Errno::NOENT) => {
            let mut seed = SecretSeedV1([0; KEY_SEED_BYTES_V1]);
            fill_random(seed.0.as_mut_slice())?;
            publish_new_file(
                directory,
                name,
                role,
                seed.as_bytes(),
                SECRET_SEED_MODE_V1,
                expected_uid,
                expected_gid,
            )?;
            Ok(seed)
        }
        Err(source) => Err(CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "open signing-key seed",
            source: source.into(),
        }),
    }
}

fn publish_or_verify<const N: usize>(
    directory: &OwnedFd,
    name: &'static str,
    role: &'static str,
    expected: &[u8; N],
    expected_uid: u32,
    expected_gid: u32,
) -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
    match openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(descriptor) => {
            let actual = read_stable_exact::<N>(
                &File::from(descriptor),
                role,
                PUBLIC_RECORD_MODE_V1,
                expected_uid,
                expected_gid,
            )?;
            if actual != *expected {
                return Err(
                    CompilerExecutionProvisioningInstallErrorV1::ExistingFileMismatch { role },
                );
            }
            Ok(())
        }
        Err(rustix::io::Errno::NOENT) => publish_new_file(
            directory,
            name,
            role,
            expected,
            PUBLIC_RECORD_MODE_V1,
            expected_uid,
            expected_gid,
        ),
        Err(source) => Err(CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "open canonical provisioning record",
            source: source.into(),
        }),
    }
}

fn publish_new_file(
    directory: &OwnedFd,
    name: &'static str,
    role: &'static str,
    bytes: &[u8],
    final_mode: u32,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
    let temporary_name = random_temporary_name()?;
    let mut pending = PendingNameV1 {
        directory,
        name: temporary_name,
        published: false,
    };
    let descriptor = openat(
        directory,
        pending.name.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
        operation: "create provisioning temporary file",
        source: source.into(),
    })?;
    let mut file = File::from(descriptor);
    file.write_all(bytes)
        .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "write provisioning temporary file",
            source,
        })?;
    rustix::fs::fchmod(&file, Mode::from_raw_mode(final_mode)).map_err(|source| {
        CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "set provisioning file mode",
            source: source.into(),
        }
    })?;
    file.sync_all()
        .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "sync provisioning temporary file",
            source,
        })?;
    let stat = fstat(&file).map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
        operation: "inspect provisioning temporary file",
        source: source.into(),
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_mode & 0o7777 != final_mode
        || stat.st_uid != expected_uid
        || stat.st_gid != expected_gid
        || stat.st_nlink != 1
        || usize::try_from(stat.st_size).ok() != Some(bytes.len())
    {
        return Err(CompilerExecutionProvisioningInstallErrorV1::TemporaryFileChanged { role });
    }
    reject_forbidden_metadata(&file, role)?;
    renameat_with(
        directory,
        pending.name.as_str(),
        directory,
        name,
        RenameFlags::NOREPLACE,
    )
    .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
        operation: "publish provisioning file",
        source: source.into(),
    })?;
    pending.published = true;
    fsync(directory).map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
        operation: "sync provisioning directory",
        source: source.into(),
    })?;
    let published = openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
        operation: "reopen published provisioning file",
        source: source.into(),
    })?;
    let mut actual = read_stable_dynamic(
        &File::from(published),
        role,
        final_mode,
        bytes.len(),
        expected_uid,
        expected_gid,
    )?;
    let matches = bool::from(actual.ct_eq(bytes));
    actual.zeroize();
    if !matches {
        return Err(CompilerExecutionProvisioningInstallErrorV1::PublishedFileChanged { role });
    }
    Ok(())
}

struct PendingNameV1<'a> {
    directory: &'a OwnedFd,
    name: String,
    published: bool,
}

impl Drop for PendingNameV1<'_> {
    fn drop(&mut self) {
        if !self.published {
            let _ = unlinkat(self.directory, self.name.as_str(), AtFlags::empty());
        }
    }
}

fn read_stable_exact<const N: usize>(
    file: &File,
    role: &'static str,
    mode: u32,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<[u8; N], CompilerExecutionProvisioningInstallErrorV1> {
    let bytes = read_stable_dynamic(file, role, mode, N, expected_uid, expected_gid)?;
    Ok(bytes
        .try_into()
        .expect("stable exact read has fixed length"))
}

fn read_stable_dynamic(
    file: &File,
    role: &'static str,
    mode: u32,
    length: usize,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<Vec<u8>, CompilerExecutionProvisioningInstallErrorV1> {
    let before =
        validate_provisioned_file(file, role, mode, Some(length), expected_uid, expected_gid)
            .map_err(CompilerExecutionProvisioningInstallErrorV1::Coordinator)?;
    let mut first = vec![0_u8; length];
    let mut second = vec![0_u8; length];
    if let Err(source) = file.read_exact_at(&mut first, 0) {
        first.zeroize();
        second.zeroize();
        return Err(CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "read provisioning file",
            source,
        });
    }
    if let Err(source) = file.read_exact_at(&mut second, 0) {
        first.zeroize();
        second.zeroize();
        return Err(CompilerExecutionProvisioningInstallErrorV1::Io {
            operation: "reread provisioning file",
            source,
        });
    }
    let after =
        match validate_provisioned_file(file, role, mode, Some(length), expected_uid, expected_gid)
        {
            Ok(after) => after,
            Err(error) => {
                first.zeroize();
                second.zeroize();
                return Err(CompilerExecutionProvisioningInstallErrorV1::Coordinator(
                    error,
                ));
            }
        };
    if before != after || !bool::from(first.ct_eq(&second)) {
        first.zeroize();
        second.zeroize();
        return Err(CompilerExecutionProvisioningInstallErrorV1::PublishedFileChanged { role });
    }
    second.zeroize();
    Ok(first)
}

fn reject_forbidden_metadata(
    file: &impl AsFd,
    role: &'static str,
) -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
    for attribute in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        let mut byte = 0_u8;
        match rustix::fs::fgetxattr(file, attribute, std::slice::from_mut(&mut byte)) {
            Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => {}
            Ok(_) | Err(rustix::io::Errno::RANGE) => {
                return Err(
                    CompilerExecutionProvisioningInstallErrorV1::ForbiddenMetadata { role },
                );
            }
            Err(source) => {
                return Err(CompilerExecutionProvisioningInstallErrorV1::Io {
                    operation: "inspect provisioning metadata",
                    source: source.into(),
                });
            }
        }
    }
    Ok(())
}

fn random_temporary_name() -> Result<String, CompilerExecutionProvisioningInstallErrorV1> {
    let mut random = [0_u8; 16];
    fill_random(&mut random)?;
    let mut name = String::from(".provisioning-");
    for byte in random {
        use std::fmt::Write as _;
        write!(&mut name, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(name)
}

fn fill_random(bytes: &mut [u8]) -> Result<(), CompilerExecutionProvisioningInstallErrorV1> {
    let mut filled = 0;
    while filled < bytes.len() {
        let count =
            rustix::rand::getrandom(&mut bytes[filled..], rustix::rand::GetRandomFlags::empty())
                .map_err(|source| CompilerExecutionProvisioningInstallErrorV1::Io {
                    operation: "generate provisioning randomness",
                    source: source.into(),
                })?;
        if count == 0 {
            return Err(CompilerExecutionProvisioningInstallErrorV1::RandomnessUnavailable);
        }
        filled += count;
    }
    Ok(())
}

/// Stable failure while installing the fixed compiler-execution reference deployment.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionProvisioningInstallErrorV1 {
    /// The command did not receive exactly one canonical nonzero decimal generation.
    InvalidArguments,
    /// The process does not have the exact root identity required for provisioning.
    RootIdentity(ProtectedServiceSpawnErrorV1),
    /// The provisioning process created or inherited more than one thread.
    MultipleThreads,
    /// A required service user or group does not exist.
    AccountMissing,
    /// A service user's primary group differs from its same-named deployment group.
    AccountPrimaryGroupMismatch {
        /// Service UID.
        uid: u32,
        /// User database primary GID.
        primary_gid: u32,
        /// Same-named group database GID.
        named_gid: u32,
    },
    /// The root-owned parent of the lifecycle lock has an invalid policy.
    InvalidLifecycleParent,
    /// The root-owned parent of the lifecycle lock changed during provisioning.
    LifecycleParentChanged,
    /// The protected service listener still exists and may activate the deployment.
    ListenerActive,
    /// The configuration directory has the wrong type, owner, group, or mode.
    InvalidDirectory,
    /// Another provisioning occurrence owns the configuration directory lock.
    DirectoryBusy,
    /// The retained configuration directory no longer matches its fixed pathname.
    DirectoryChanged,
    /// An executable exceeds its role's fixed size bound.
    ImageTooLarge {
        /// Fixed executable role.
        role: &'static str,
    },
    /// An executable changed while it was measured.
    ImageChanged {
        /// Fixed executable role.
        role: &'static str,
    },
    /// An executable is outside the loader-independent static ELF profile.
    InvalidStaticImage {
        /// Fixed executable role.
        role: &'static str,
        /// Exact static-image rejection.
        source: SealedStaticApplicationErrorV1,
    },
    /// A provisioned source failed the shared immutable-file policy.
    Coordinator(CompilerExecutionCoordinatorErrorV1),
    /// The canonical public record graph could not be constructed.
    Provisioning(CompilerExecutionProvisioningErrorV1),
    /// A temporary file changed before publication.
    TemporaryFileChanged {
        /// Fixed file role.
        role: &'static str,
    },
    /// A published file changed or did not retain the exact expected bytes.
    PublishedFileChanged {
        /// Fixed file role.
        role: &'static str,
    },
    /// An existing record differs from the newly derived canonical bytes.
    ExistingFileMismatch {
        /// Fixed record role.
        role: &'static str,
    },
    /// A file or directory carries a forbidden ACL or file capability.
    ForbiddenMetadata {
        /// Fixed file or directory role.
        role: &'static str,
    },
    /// The kernel random source returned no bytes.
    RandomnessUnavailable,
    /// A bounded operating-system operation failed.
    Io {
        /// Stable operation description.
        operation: &'static str,
        /// Operating-system failure.
        source: io::Error,
    },
}

impl fmt::Display for CompilerExecutionProvisioningInstallErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments => formatter
                .write_str("expected exactly one canonical nonzero decimal policy generation"),
            Self::RootIdentity(error) => write!(formatter, "root identity required: {error}"),
            Self::MultipleThreads => {
                formatter.write_str("provisioning entrypoint is not single-threaded")
            }
            Self::AccountMissing => {
                formatter.write_str("required compiler-execution service account is missing")
            }
            Self::AccountPrimaryGroupMismatch {
                uid,
                primary_gid,
                named_gid,
            } => write!(
                formatter,
                "service UID {uid} has primary GID {primary_gid}, expected {named_gid}"
            ),
            Self::InvalidLifecycleParent => {
                formatter.write_str("invalid compiler-execution lifecycle-lock parent directory")
            }
            Self::LifecycleParentChanged => {
                formatter.write_str("compiler-execution lifecycle-lock parent directory changed")
            }
            Self::ListenerActive => {
                formatter.write_str("compiler-execution listener must be stopped for provisioning")
            }
            Self::InvalidDirectory => {
                formatter.write_str("invalid compiler-execution configuration directory")
            }
            Self::DirectoryBusy => {
                formatter.write_str("compiler-execution configuration directory is busy")
            }
            Self::DirectoryChanged => {
                formatter.write_str("compiler-execution configuration directory changed")
            }
            Self::ImageTooLarge { role } => write!(formatter, "{role} has an invalid length"),
            Self::ImageChanged { role } => write!(formatter, "{role} changed while measured"),
            Self::InvalidStaticImage { role, source } => {
                write!(formatter, "{role} is not a sealed-static image: {source}")
            }
            Self::Coordinator(error) => write!(formatter, "invalid provisioned source: {error}"),
            Self::Provisioning(error) => write!(formatter, "invalid provisioning graph: {error}"),
            Self::TemporaryFileChanged { role } => {
                write!(formatter, "temporary {role} changed before publication")
            }
            Self::PublishedFileChanged { role } => {
                write!(formatter, "published {role} changed during verification")
            }
            Self::ExistingFileMismatch { role } => {
                write!(
                    formatter,
                    "existing {role} differs from canonical provisioning"
                )
            }
            Self::ForbiddenMetadata { role } => {
                write!(
                    formatter,
                    "{role} carries a forbidden ACL or file capability"
                )
            }
            Self::RandomnessUnavailable => {
                formatter.write_str("kernel provisioning randomness is unavailable")
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for CompilerExecutionProvisioningInstallErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RootIdentity(error) => Some(error),
            Self::InvalidStaticImage { source, .. } => Some(source),
            Self::Coordinator(error) => Some(error),
            Self::Provisioning(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1,
        COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1, COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1, CompilerExecutionClientProfileV1,
        CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorProvisioningV1,
        CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
    };

    use super::*;

    #[test]
    fn generation_is_nonzero_canonical_decimal() {
        assert_eq!(parse_generation(OsStr::new("1")).unwrap(), 1);
        assert_eq!(
            parse_generation(OsStr::new("18446744073709551615")).unwrap(),
            u64::MAX
        );
        for value in [
            "",
            "0",
            "01",
            "+1",
            "-1",
            " 1",
            "1 ",
            "18446744073709551616",
        ] {
            assert!(parse_generation(OsStr::new(value)).is_err(), "{value:?}");
        }
    }

    #[test]
    fn provisioning_is_idempotent_and_generation_substitution_fails_closed() {
        let fixture = tempfile::tempdir().unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let compiler = ServiceIdentityV1 {
            uid: nonroot_id(uid, 1_001),
            gid: nonroot_id(gid, 1_002),
        };
        let config = fixture.path().join("config");
        let images = fixture.path().join("images");
        let lifecycle_lock = fixture.path().join("lifecycle-lock");
        std::fs::create_dir(&config).unwrap();
        std::fs::create_dir(&images).unwrap();
        std::fs::write(&lifecycle_lock, []).unwrap();
        std::fs::set_permissions(
            fixture.path(),
            std::fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
        )
        .unwrap();
        std::fs::set_permissions(
            &config,
            std::fs::Permissions::from_mode(CONFIG_DIRECTORY_MODE_V1),
        )
        .unwrap();
        std::fs::set_permissions(
            &lifecycle_lock,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        let paths: Vec<_> = (0_u8..5)
            .map(|index| {
                let path = images.join(format!("image-{index}"));
                std::fs::write(&path, static_pause_elf(index)).unwrap();
                std::fs::set_permissions(
                    &path,
                    std::fs::Permissions::from_mode(EXECUTABLE_MODE_V1),
                )
                .unwrap();
                path
            })
            .collect();
        let layout = ProvisioningLayoutV1 {
            config_directory: config.clone(),
            lifecycle_lock: lifecycle_lock.clone(),
            listener: fixture.path().join("absent-listener"),
            supervisor: paths[0].clone(),
            launcher: paths[1].clone(),
            issuer: paths[2].clone(),
            anchor_helper: paths[3].clone(),
            anchor_daemon: paths[4].clone(),
        };
        let anchor = ServiceIdentityV1 {
            uid: distinct_nonroot_id(compiler.uid, 2_001),
            gid: nonroot_id(gid, 2_002),
        };

        let active_service = File::open(&lifecycle_lock).unwrap();
        flock(&active_service, FlockOperation::NonBlockingLockShared).unwrap();
        assert!(matches!(
            provision_layout(&layout, 7, compiler, anchor, uid, gid),
            Err(CompilerExecutionProvisioningInstallErrorV1::Coordinator(
                CompilerExecutionCoordinatorErrorV1::LifecycleBusy
            ))
        ));
        assert!(!config.join(ISSUER_SEED_FILE_V1).exists());
        assert!(!config.join(ISSUER_POLICY_FILE_V1).exists());
        drop(active_service);

        provision_layout(&layout, 7, compiler, anchor, uid, gid).unwrap();
        let before = read_records(&config);
        provision_layout(&layout, 7, compiler, anchor, uid, gid).unwrap();
        assert_eq!(read_records(&config), before);

        std::fs::remove_file(config.join(ANCHOR_PROVISIONING_FILE_V1)).unwrap();
        provision_layout(&layout, 7, compiler, anchor, uid, gid).unwrap();
        assert_eq!(read_records(&config), before);

        std::fs::write(&layout.listener, b"active").unwrap();
        assert!(matches!(
            provision_layout(&layout, 7, compiler, anchor, uid, gid),
            Err(CompilerExecutionProvisioningInstallErrorV1::ListenerActive)
        ));
        std::fs::remove_file(&layout.listener).unwrap();

        let (lock, _) = open_and_lock_directory(&config, uid, gid).unwrap();
        assert!(matches!(
            provision_layout(&layout, 7, compiler, anchor, uid, gid),
            Err(CompilerExecutionProvisioningInstallErrorV1::DirectoryBusy)
        ));
        drop(lock);

        let issuer_seed = config.join(ISSUER_SEED_FILE_V1);
        std::fs::set_permissions(&issuer_seed, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            provision_layout(&layout, 7, compiler, anchor, uid, gid),
            Err(CompilerExecutionProvisioningInstallErrorV1::Coordinator(_))
        ));
        std::fs::set_permissions(
            &issuer_seed,
            std::fs::Permissions::from_mode(SECRET_SEED_MODE_V1),
        )
        .unwrap();

        assert!(matches!(
            provision_layout(&layout, 8, compiler, anchor, uid, gid),
            Err(
                CompilerExecutionProvisioningInstallErrorV1::ExistingFileMismatch {
                    role: "issuer policy"
                }
            )
        ));
        assert_eq!(read_records(&config), before);

        let policy = CompilerExecutionIssuerPolicyV1::decode(&before[0]).unwrap();
        let supervisor = CompilerExecutionSupervisorDeploymentV1::decode(&before[1]).unwrap();
        let anchor = CompilerExecutionExternalAnchorDeploymentV1::decode(&before[2]).unwrap();
        let provisioning =
            CompilerExecutionExternalAnchorProvisioningV1::decode(&before[3]).unwrap();
        let client_profile = CompilerExecutionClientProfileV1::decode(&before[4]).unwrap();
        assert!(supervisor.matches_policy(&policy));
        assert!(anchor.matches_supervisor_and_policy(&supervisor, &policy));
        assert!(provisioning.matches_deployment(&anchor));
        assert_eq!(client_profile.policy(), &policy);
        assert_eq!(client_profile.supervisor_uid(), supervisor.service_uid());
        assert_eq!(client_profile.supervisor_gid(), supervisor.service_gid());
        assert_eq!(
            client_profile.external_anchor_service(),
            supervisor.external_anchor_service()
        );
        for (name, mode, length) in [
            (ISSUER_SEED_FILE_V1, SECRET_SEED_MODE_V1, KEY_SEED_BYTES_V1),
            (ANCHOR_SEED_FILE_V1, SECRET_SEED_MODE_V1, KEY_SEED_BYTES_V1),
            (
                ISSUER_POLICY_FILE_V1,
                PUBLIC_RECORD_MODE_V1,
                COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1,
            ),
            (
                SUPERVISOR_DEPLOYMENT_FILE_V1,
                PUBLIC_RECORD_MODE_V1,
                COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1,
            ),
            (
                ANCHOR_DEPLOYMENT_FILE_V1,
                PUBLIC_RECORD_MODE_V1,
                COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1,
            ),
            (
                ANCHOR_PROVISIONING_FILE_V1,
                PUBLIC_RECORD_MODE_V1,
                COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1,
            ),
            (
                CLIENT_PROFILE_FILE_V1,
                PUBLIC_RECORD_MODE_V1,
                COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1,
            ),
        ] {
            let metadata = std::fs::metadata(config.join(name)).unwrap();
            assert_eq!(metadata.permissions().mode() & 0o7777, mode);
            assert_eq!(metadata.len(), length as u64);
        }
    }

    #[test]
    fn retained_image_and_directory_paths_cannot_be_replaced() {
        let fixture = tempfile::tempdir().unwrap();
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let image = fixture.path().join("image");
        std::fs::write(&image, static_pause_elf(1)).unwrap();
        std::fs::set_permissions(&image, std::fs::Permissions::from_mode(EXECUTABLE_MODE_V1))
            .unwrap();
        let measured = measure_static_image(
            &image,
            "test image",
            MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
            uid,
            gid,
        )
        .unwrap();
        std::fs::rename(&image, fixture.path().join("displaced-image")).unwrap();
        std::fs::write(&image, static_pause_elf(2)).unwrap();
        std::fs::set_permissions(&image, std::fs::Permissions::from_mode(EXECUTABLE_MODE_V1))
            .unwrap();
        assert!(matches!(
            measured.revalidate(uid, gid),
            Err(CompilerExecutionProvisioningInstallErrorV1::ImageChanged { role: "test image" })
        ));

        let directory = fixture.path().join("config");
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(
            &directory,
            std::fs::Permissions::from_mode(CONFIG_DIRECTORY_MODE_V1),
        )
        .unwrap();
        let (retained, snapshot) = open_and_lock_directory(&directory, uid, gid).unwrap();
        std::fs::rename(&directory, fixture.path().join("displaced-config")).unwrap();
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(
            &directory,
            std::fs::Permissions::from_mode(CONFIG_DIRECTORY_MODE_V1),
        )
        .unwrap();
        assert!(matches!(
            revalidate_directory_path(&directory, &retained, snapshot, uid, gid),
            Err(CompilerExecutionProvisioningInstallErrorV1::DirectoryChanged)
        ));

        let lifecycle_parent = fixture.path().join("lifecycle-parent");
        let lifecycle_lock = lifecycle_parent.join("lifecycle-lock");
        std::fs::create_dir(&lifecycle_parent).unwrap();
        std::fs::set_permissions(
            &lifecycle_parent,
            std::fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
        )
        .unwrap();
        std::fs::write(&lifecycle_lock, []).unwrap();
        std::fs::set_permissions(
            &lifecycle_lock,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        let lifecycle =
            RetainedProvisioningLifecycleLeaseV1::admit(&lifecycle_lock, uid, gid).unwrap();
        std::fs::rename(
            &lifecycle_lock,
            lifecycle_parent.join("displaced-lifecycle-lock"),
        )
        .unwrap();
        std::fs::write(&lifecycle_lock, []).unwrap();
        std::fs::set_permissions(
            &lifecycle_lock,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        assert!(matches!(
            lifecycle.revalidate(),
            Err(CompilerExecutionProvisioningInstallErrorV1::Coordinator(
                CompilerExecutionCoordinatorErrorV1::LifecycleChanged
            ))
        ));

        let parent = fixture.path().join("replaceable-parent");
        let lock = parent.join("lifecycle-lock");
        std::fs::create_dir(&parent).unwrap();
        std::fs::set_permissions(
            &parent,
            std::fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
        )
        .unwrap();
        std::fs::write(&lock, []).unwrap();
        std::fs::set_permissions(
            &lock,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        let lifecycle = RetainedProvisioningLifecycleLeaseV1::admit(&lock, uid, gid).unwrap();
        std::fs::rename(&parent, fixture.path().join("displaced-parent")).unwrap();
        std::fs::create_dir(&parent).unwrap();
        std::fs::set_permissions(
            &parent,
            std::fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
        )
        .unwrap();
        std::fs::write(&lock, []).unwrap();
        std::fs::set_permissions(
            &lock,
            std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        assert!(matches!(
            lifecycle.revalidate(),
            Err(CompilerExecutionProvisioningInstallErrorV1::LifecycleParentChanged)
        ));
    }

    #[test]
    fn dynamic_image_is_rejected_before_measurement() {
        let fixture = tempfile::tempdir().unwrap();
        let image = fixture.path().join("dynamic-image");
        std::fs::write(&image, b"not an ELF image").unwrap();
        std::fs::set_permissions(&image, std::fs::Permissions::from_mode(EXECUTABLE_MODE_V1))
            .unwrap();
        assert!(matches!(
            measure_static_image(
                &image,
                "dynamic image",
                MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
                rustix::process::geteuid().as_raw(),
                rustix::process::getegid().as_raw(),
            ),
            Err(
                CompilerExecutionProvisioningInstallErrorV1::InvalidStaticImage {
                    role: "dynamic image",
                    ..
                }
            )
        ));
    }

    fn read_records(directory: &Path) -> [Vec<u8>; 5] {
        [
            std::fs::read(directory.join(ISSUER_POLICY_FILE_V1)).unwrap(),
            std::fs::read(directory.join(SUPERVISOR_DEPLOYMENT_FILE_V1)).unwrap(),
            std::fs::read(directory.join(ANCHOR_DEPLOYMENT_FILE_V1)).unwrap(),
            std::fs::read(directory.join(ANCHOR_PROVISIONING_FILE_V1)).unwrap(),
            std::fs::read(directory.join(CLIENT_PROFILE_FILE_V1)).unwrap(),
        ]
    }

    fn nonroot_id(actual: u32, fallback: u32) -> u32 {
        if actual == 0 || actual == u32::MAX {
            fallback
        } else {
            actual
        }
    }

    fn distinct_nonroot_id(first: u32, fallback: u32) -> u32 {
        if first != fallback {
            fallback
        } else {
            fallback + 1
        }
    }

    fn static_pause_elf(discriminator: u8) -> Vec<u8> {
        const HEADER: usize = 64;
        const PROGRAM: usize = 56;
        const PROGRAMS: usize = 4;
        const CODE_OFFSET: usize = 0x1000;
        let code = [0xb8, 0x22 + discriminator, 0, 0, 0, 0x0f, 0x05, 0xeb, 0xf7];
        let mut bytes = vec![0_u8; CODE_OFFSET + code.len()];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
        bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
        bytes[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
        bytes[32..40].copy_from_slice(&(HEADER as u64).to_le_bytes());
        bytes[52..54].copy_from_slice(&(HEADER as u16).to_le_bytes());
        bytes[54..56].copy_from_slice(&(PROGRAM as u16).to_le_bytes());
        bytes[56..58].copy_from_slice(&(PROGRAMS as u16).to_le_bytes());
        let table_size = (PROGRAM * PROGRAMS) as u64;
        write_program(
            &mut bytes,
            0,
            6,
            4,
            HEADER as u64,
            0x400040,
            table_size,
            table_size,
            8,
        );
        write_program(
            &mut bytes,
            1,
            1,
            4,
            0,
            0x400000,
            HEADER as u64 + table_size,
            HEADER as u64 + table_size,
            0x1000,
        );
        write_program(
            &mut bytes,
            2,
            1,
            5,
            CODE_OFFSET as u64,
            0x401000,
            code.len() as u64,
            code.len() as u64,
            0x1000,
        );
        write_program(&mut bytes, 3, 0x6474_e551, 6, 0, 0, 0, 0, 16);
        bytes[CODE_OFFSET..].copy_from_slice(&code);
        bytes
    }

    #[allow(clippy::too_many_arguments)]
    fn write_program(
        bytes: &mut [u8],
        index: usize,
        kind: u32,
        flags: u32,
        offset: u64,
        virtual_address: u64,
        file_size: u64,
        memory_size: u64,
        alignment: u64,
    ) {
        const HEADER: usize = 64;
        const PROGRAM: usize = 56;
        let start = HEADER + index * PROGRAM;
        bytes[start..start + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&flags.to_le_bytes());
        bytes[start + 8..start + 16].copy_from_slice(&offset.to_le_bytes());
        bytes[start + 16..start + 24].copy_from_slice(&virtual_address.to_le_bytes());
        bytes[start + 32..start + 40].copy_from_slice(&file_size.to_le_bytes());
        bytes[start + 40..start + 48].copy_from_slice(&memory_size.to_le_bytes());
        bytes[start + 48..start + 56].copy_from_slice(&alignment.to_le_bytes());
    }
}
