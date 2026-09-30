//! Disposable state around genuine fixed provisioning, never synthetic trust.
//! The operator must isolate /tmp and /run/fe2o3 and exclude administrative
//! writers to the installed records, keys, profile, images and compiler runtime.
//! Records/keys/images are read only. This module does not invoke a provisioner.
//! Runner prerequisites remain procfs, native clone3/ptrace, root identity, and
//! CHOWN/FOWNER/SETUID/SETGID/SETPCAP/SYS_PTRACE/KILL plus DAC_READ_SEARCH or
//! DAC_OVERRIDE for lifecycle traversal. Use no network/GPU and bounded outer
//! CPU/memory/PIDs/deadline cleanup. Read-only mounts do not replace FS_IMMUTABLE.
use super::*;
use crate::runtime_listener::RuntimeListener;
use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1 as LOCK;
use fe2o3_compiler_execution_supervisor::{
    IssuerServiceCredentialProfileV1 as Credentials,
    ProvisionedProtectedIssuerServiceInputsV2 as Inputs,
};
use fe2o3_external_anchor_coordinator::PreparedExternalAnchorOccurrenceV3 as Anchor;
use rustix::process::{Gid, Uid};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[path = "native_root_request_provisioned_inputs_tests.rs"]
mod inputs;

pub(super) fn require_environment() {
    assert_eq!(
        std::env::var("FE2O3_RUN_PROVISIONED_ROOT_REQUEST").as_deref(),
        Ok("isolated-disposable-root")
    );
    fe2o3_protected_service_spawn::require_exact_root_identity_v1().unwrap();
    assert_eq!(
        std::env::var("FE2O3_NATIVE_ROOT_SCRATCH").as_deref(),
        Ok("/tmp")
    );
    assert_eq!(
        std::env::var("FE2O3_NATIVE_ROOT_RUNTIME").as_deref(),
        Ok("/run/fe2o3")
    );
    let m = disk::symlink_metadata("/run/fe2o3").unwrap();
    assert!(m.is_dir());
    assert_eq!((m.uid(), m.gid(), m.mode() & 0o7777), (0, 0, 0o755));
}

pub(super) struct Fixture {
    // Retain the original cleanup guard until AFTER Native has drained/dropped.
    listener: Option<RuntimeListener>,
    pub dir: tempfile::TempDir,
    service_root: File,
    anchor_root: File,
    lock: PathBuf,
}

impl Fixture {
    // Diagnostic/path/file ownership, outside the exact production Prepared charge.
    const STORAGE: usize = 64 * 1024 + root::LISTENER_GROWTH + root::FILE_STORAGE;

    fn new(issuer: (u32, u32), anchor: (u32, u32)) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("provisioned-root-request-")
            .tempdir_in("/tmp")
            .unwrap();
        mode(dir.path(), 0o755);
        let service = dir.path().join("issuer");
        let anchor_path = dir.path().join("anchor");
        for path in [&service, &anchor_path] {
            disk::create_dir(path).unwrap();
            mode(path, 0o700);
        }
        let service_root = File::open(&service).unwrap();
        let anchor_root = File::open(&anchor_path).unwrap();
        // Only these TempDir-owned state directories change ownership.
        owner(&service_root, issuer);
        owner(&anchor_root, anchor);
        let lock = dir.path().join(Path::new(LOCK).file_name().unwrap());
        File::create_new(&lock).unwrap();
        mode(&lock, 0o400);
        Self {
            listener: None,
            dir,
            service_root,
            anchor_root,
            lock,
        }
    }

    pub(super) fn assert_unlocked(&self) {
        let lock = File::open(&self.lock).unwrap();
        fs::flock(&lock, fs::FlockOperation::NonBlockingLockExclusive)
            .expect("original lifecycle aliases must retire through cleanup");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for root in [&self.service_root, &self.anchor_root] {
            owner(root, (0, 0));
        }
        // RuntimeListener removes only its own recorded inode, not a replacement.
        drop(self.listener.take());
    }
}

// Mirrors the existing production composition through its actual lower-level
// admission APIs. The unsafe 14-FD main-thread entry is NOT called from libtest.
// The original account funds all full inputs and nested work. On success only
// dead source reservations retire; both returned owners are reserved immediately.
pub(super) fn prepare(pool: &mut Cleanup, b: &mut Budget<'_>) -> (Fixture, Prepared) {
    let floor = b.storage();
    let (fixture, prepared) = b
        .with_prepaid_scope(0, 8, root::LOCAL_WORK, HARNESS, |b| -> root::Result<_> {
            let inputs::Provisioned {
                trust,
                anchor,
                provisioning,
                anchor_key,
                programs,
                helper,
                daemon,
            } = inputs::Provisioned::read(b);
            let deployment = trust.deployment().deployment();
            let credentials = Credentials::new(deployment.service_uid(), deployment.service_gid())?;
            let service = deployment.external_anchor_service();
            let mut fixture = Fixture::new(
                (credentials.uid(), credentials.gid()),
                (service.uid(), service.gid()),
            );
            b.reserve_storage(Fixture::STORAGE)?;

            b.reserve_storage(Anchor::ROOT_STORAGE)?;
            let anchor_root = fixture.anchor_root.try_clone().unwrap();
            let (anchor_lease, charge) = Lease::open(&anchor_root, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (supervisor_lease, charge) = Lease::open(&fixture.service_root, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (root_lease, charge) = Lease::open(&fixture.service_root, b)?;
            b.reserve_storage(charge.additional_storage())?;
            anchor_lease.revalidate_for_root(&fixture.service_root, b)?;
            root_lease.revalidate_for_root(&anchor_root, b)?;

            let (anchor, charge) = Anchor::prepare(
                helper,
                daemon,
                anchor_root,
                anchor_lease,
                anchor,
                provisioning,
                anchor_key,
                trust.deployment(),
                trust.policy(),
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            anchor.retain_cleanup_guard(trust.deployment(), trust.policy(), pool, b)?;
            let (anchor, charge) =
                anchor.launch(trust.deployment(), trust.policy(), TIMEOUT, pool, b)?;
            b.reserve_storage(charge.additional_storage())?;
            anchor.validate_continuity(trust.deployment(), trust.policy(), b)?;

            b.reserve_storage(Inputs::PAIR_STORAGE + root::FILE_STORAGE)?;
            let runtime = fs::open(
                "/run/fe2o3",
                fs::OFlags::RDONLY
                    | fs::OFlags::DIRECTORY
                    | fs::OFlags::CLOEXEC
                    | fs::OFlags::NOFOLLOW,
                fs::Mode::empty(),
            )
            .unwrap();
            let (mut listener, growth) = root::listener(runtime, credentials.gid(), b)?;
            b.reserve_storage(growth)?;
            let fd = listener.take_descriptor()?;
            fixture.listener = Some(listener);
            let (inputs, charge) = Inputs::admit(
                fd,
                fixture.service_root.try_clone().unwrap(),
                credentials,
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            let (prepared, charge) = Prepared::prepare(
                programs,
                trust,
                inputs,
                supervisor_lease,
                root_lease,
                anchor,
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            prepared.validate_cleanup_guard(pool, b)?;
            Ok((fixture, prepared))
        })
        .expect("real provisioned records, key owners, images and original anchor must admit");
    b.reserve_storage(Fixture::STORAGE + prepared.retained_storage())
        .unwrap();
    assert_eq!(
        b.storage(),
        floor + Fixture::STORAGE + prepared.retained_storage()
    );
    assert!(b.failed_work().is_none() && b.failed_storage().is_none());
    eprintln!("ROOT_REQUEST_PROVISIONED");
    (fixture, prepared)
}

fn mode(path: &Path, mode: u32) {
    disk::set_permissions(path, disk::Permissions::from_mode(mode)).unwrap();
}
fn owner(file: &File, (uid, gid): (u32, u32)) {
    fs::fchown(file, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid))).unwrap();
}
