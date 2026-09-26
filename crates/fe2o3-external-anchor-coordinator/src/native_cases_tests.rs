use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::fs::{FlockOperation, Mode, OFlags};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    panic::{AssertUnwindSafe, catch_unwind},
};

const LIMIT: usize = 1 << 34;
fn public(seed: u8) -> [u8; 32] {
    ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes()
}
fn ids() -> (u32, u32) {
    let uid = rustix::process::geteuid().as_raw();
    let gid = rustix::process::getegid().as_raw();
    (
        if uid == 0 { 65534 } else { uid },
        if gid == 0 { 65534 } else { gid },
    )
}
fn make_policy(axis: usize, b: &mut Budget<'_>) -> PolicyCap {
    let (p, c) = Policy::new(
        7 + u64::from(axis == 1),
        Measurement::new([1 + u8::from(axis == 2); 32], 1).unwrap(),
        Measurement::new([2 + u8::from(axis == 3); 32], 2).unwrap(),
        public(3 + u8::from(axis == 4)),
        public(7 + u8::from(axis == 5)),
        b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (p, c) = PolicyCap::create(p, b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    p
}
fn make_supervisor(
    p: &PolicyCap,
    service: Service,
    axis: usize,
    b: &mut Budget<'_>,
) -> SupervisorCap {
    let (uid, gid) = ids();
    let service = Service::new(
        service.uid() + u32::from(axis == 3),
        service.gid() + u32::from(axis == 4),
    )
    .unwrap();
    let (s, c) = Supervisor::new(
        uid + 10 + u32::from(axis == 1),
        gid + 10 + u32::from(axis == 2),
        service,
        Measurement::new([4 + u8::from(axis == 5); 32], 4 + u64::from(axis == 6)).unwrap(),
        Measurement::new([5 + u8::from(axis == 7); 32], 5 + u64::from(axis == 8)).unwrap(),
        p.policy(),
        b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (s, c) = SupervisorCap::create(s, b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    s
}

struct Inputs {
    helper: File,
    daemon: File,
    root: File,
    lease: Lease,
    deployment: DeploymentCap,
    provisioning: ProvisioningCap,
    key: Key,
}
struct Fixture {
    dir: tempfile::TempDir,
    policy: PolicyCap,
    supervisor: SupervisorCap,
    inputs: Option<Inputs>,
}
impl Fixture {
    fn new() -> Self {
        Self::with_credentials(0, 0)
    }
    fn with_credentials(uid_delta: u32, gid_delta: u32) -> Self {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let root = dir.path().join("state");
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let (uid, gid) = ids();
        if rustix::process::geteuid().is_root() {
            rustix::fs::chown(
                &root,
                Some(rustix::process::Uid::from_raw(uid)),
                Some(rustix::process::Gid::from_raw(gid)),
            )
            .unwrap();
        }
        let lock = dir.path().join(
            std::path::Path::new(
                fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
            )
            .file_name()
            .unwrap(),
        );
        fs::write(&lock, []).unwrap();
        fs::set_permissions(&lock, fs::Permissions::from_mode(0o400)).unwrap();
        let bytes = crate::tests::static_pause_elf();
        let m = Measurement::new(Sha256::digest(&bytes).into(), bytes.len() as u64).unwrap();
        for name in ["helper", "daemon"] {
            fs::write(dir.path().join(name), &bytes).unwrap();
            fs::set_permissions(dir.path().join(name), fs::Permissions::from_mode(0o555)).unwrap();
        }
        let root = File::open(root).unwrap();
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        b.reserve_storage(Prepared::ROOT_STORAGE + Lease::FILE_STORAGE)
            .unwrap();
        let (lease, c) = Lease::admit_non_authoritative_same_owner_test(
            File::open(lock).unwrap(),
            &root,
            &mut b,
        )
        .unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let p = make_policy(0, &mut b);
        let service = Service::new(uid + uid_delta, gid + gid_delta).unwrap();
        let s = make_supervisor(&p, service, 0, &mut b);
        let (d, c) = Deployment::new(s.deployment(), p.policy(), m, &mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let (d, c) = DeploymentCap::create(d, &mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let (q, c) = Provisioning::new(d.deployment(), m, &mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let (q, c) = ProvisioningCap::create(q, &mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        b.reserve_storage(32).unwrap();
        let (key, c) = Key::create_and_zeroize(&mut [7; 32], d.deployment(), &mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        Self {
            inputs: Some(Inputs {
                helper: File::open(dir.path().join("helper")).unwrap(),
                daemon: File::open(dir.path().join("daemon")).unwrap(),
                root,
                lease,
                deployment: d,
                provisioning: q,
                key,
            }),
            dir,
            policy: p,
            supervisor: s,
        }
    }
    fn input(&self) -> &Inputs {
        self.inputs.as_ref().unwrap()
    }
    fn floor(&self) -> usize {
        let i = self.input();
        Prepared::prepare_input_storage(
            &i.lease,
            &i.deployment,
            &i.provisioning,
            &i.key,
            &self.supervisor,
            &self.policy,
        )
        .unwrap()
    }
    fn quota(&self) -> Quota {
        Prepared::preparation_quota(&self.input().deployment, &self.input().provisioning).unwrap()
    }
    fn context_storage(&self) -> usize {
        self.policy.retained_storage() + self.supervisor.retained_storage()
    }
    fn prepare_non_authoritative_same_owner_test(
        &mut self,
        b: &mut Budget<'_>,
    ) -> Result<(Prepared, Storage)> {
        let i = self.inputs.take().unwrap();
        Prepared::prepare_inner::<false>(
            i.helper,
            i.daemon,
            i.root,
            i.lease,
            i.deployment,
            i.provisioning,
            i.key,
            &self.supervisor,
            &self.policy,
            b,
        )
    }
    fn production(&mut self, b: &mut Budget<'_>) -> Result<(Prepared, Storage)> {
        let i = self.inputs.take().unwrap();
        Prepared::prepare(
            i.helper,
            i.daemon,
            i.root,
            i.lease,
            i.deployment,
            i.provisioning,
            i.key,
            &self.supervisor,
            &self.policy,
            b,
        )
    }
    fn lock(&self) -> File {
        File::open(
            self.dir.path().join(
                std::path::Path::new(
                    fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
                )
                .file_name()
                .unwrap(),
            ),
        )
        .unwrap()
    }
}
fn identity(f: &File) -> (u64, u64) {
    let m = f.metadata().unwrap();
    (m.dev(), m.ino())
}
fn refs(id: (u64, u64)) -> usize {
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|e| fs::metadata(e.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == id)
        .count()
}
fn retire(p: Prepared, b: &mut Budget<'_>) {
    let storage = p.retained_storage();
    drop(p);
    b.release_storage(storage).unwrap();
}
fn reserve_fixture(f: &Fixture, b: &mut Budget<'_>) {
    b.reserve_storage(f.floor()).unwrap();
}

#[test]
fn exact_preparation_and_revalidation_preserve_images_lock_and_full_charge() {
    let mut f = Fixture::new();
    let floor = f.floor();
    let q = f.quota();
    let i = f.input();
    assert_eq!(
        floor,
        Prepared::ROOT_STORAGE
            + i.lease.retained_storage()
            + i.deployment.retained_storage()
            + i.provisioning.retained_storage()
            + i.key.retained_storage()
            + f.context_storage()
            + 2 * size_of::<(File, ImageStorage)>()
            + i.helper.metadata().unwrap().len() as usize
            + i.daemon.metadata().unwrap().len() as usize
    );
    let sources = [identity(&f.input().helper), identity(&f.input().daemon)];
    let mut w = Work::new(q.work());
    let mut b = Budget::new(&mut w, floor + q.scratch());
    reserve_fixture(&f, &mut b);
    let ledger = b.work_ledger_identity_v1();
    let (p, c) = f.prepare_non_authoritative_same_owner_test(&mut b).unwrap();
    assert_eq!(b.work(), q.work());
    assert_eq!(b.storage(), floor);
    assert_eq!(b.peak_storage(), floor + q.scratch());
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(c.additional_storage(), Prepared::GROWTH_STORAGE);
    assert_eq!(
        p.retained_storage() + f.context_storage(),
        floor + c.additional_storage()
    );
    b.reserve_storage(c.additional_storage()).unwrap();
    for (image, source) in [&p.helper, &p.daemon].into_iter().zip(sources) {
        let id = image.object_identity();
        assert_ne!((id.device(), id.inode()), source);
        assert_eq!((id.uid(), id.gid()), ids());
        assert_eq!(refs(source), 0);
    }
    assert_eq!(
        rustix::fs::flock(f.lock(), FlockOperation::NonBlockingLockExclusive),
        Err(rustix::io::Errno::WOULDBLOCK)
    );
    let rq = p.revalidation_quota().unwrap();
    let live = p.retained_storage() + f.context_storage();
    let mut rw = Work::new(rq.work());
    let mut rb = Budget::new(&mut rw, live + rq.scratch());
    rb.reserve_storage(live).unwrap();
    p.revalidate_inner::<false>(&f.supervisor, &f.policy, &mut rb)
        .unwrap();
    assert_eq!(rb.work(), rq.work());
    assert_eq!(rb.storage(), live);
    assert_eq!(rb.peak_storage(), live + rq.scratch());
    retire(p, &mut b);
    assert_eq!(b.storage(), f.context_storage());
    rustix::fs::flock(f.lock(), FlockOperation::NonBlockingLockExclusive).unwrap();
}

#[test]
fn consuming_refusals_check_floor_entry_local_nested_work_and_scratch() {
    for mode in 0..6 {
        let mut f = Fixture::new();
        let floor = f.floor();
        let q = f.quota();
        let ids = [
            identity(&f.input().root),
            identity(&f.input().helper),
            identity(&f.input().daemon),
        ];
        let prepaid = floor - usize::from(mode == 0);
        let work = match mode {
            1 => ENTRY_WORK - 1,
            2 => LOCAL_WORK - 1,
            3 => q.work() - 1,
            _ => q.work(),
        };
        let scratch = match mode {
            4 => Prepared::FRAME_STORAGE - 1,
            5 => q.scratch() - 1,
            _ => q.scratch(),
        };
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, prepaid + scratch);
        b.reserve_storage(prepaid).unwrap();
        let e = f
            .prepare_non_authoritative_same_owner_test(&mut b)
            .unwrap_err();
        match mode {
            0 => assert!(matches!(e, Error::Resource(Resource::Accounting))),
            1 | 2 => assert!(matches!(e, Error::Resource(Resource::Work(_)))),
            3 => assert!(matches!(
                e,
                Error::Executable(ImageError::Resource(Resource::Work(_)))
            )),
            4 => assert!(matches!(e, Error::Resource(Resource::Storage(_)))),
            5 => assert!(b.failed_storage().is_some(), "{e:?}"),
            _ => unreachable!(),
        }
        assert_eq!(b.storage(), prepaid);
        for id in ids {
            assert_eq!(refs(id), 0);
        }
        rustix::fs::flock(f.lock(), FlockOperation::NonBlockingLockExclusive).unwrap();
    }
}

#[test]
fn revalidation_short_floor_work_scratch_keeps_all_owners() {
    let mut f = Fixture::new();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    reserve_fixture(&f, &mut b);
    let (p, c) = f.prepare_non_authoritative_same_owner_test(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let live = p.retained_storage() + f.context_storage();
    let q = p.revalidation_quota().unwrap();
    for mode in 0..3 {
        let floor = live - usize::from(mode == 0);
        let mut w = Work::new(q.work() - usize::from(mode == 1));
        let mut rb = Budget::new(&mut w, floor + q.scratch() - usize::from(mode == 2));
        rb.reserve_storage(floor).unwrap();
        assert!(
            p.revalidate_inner::<false>(&f.supervisor, &f.policy, &mut rb)
                .is_err()
        );
        assert_eq!(rb.storage(), floor);
        assert_eq!(
            rustix::fs::flock(f.lock(), FlockOperation::NonBlockingLockExclusive),
            Err(rustix::io::Errno::WOULDBLOCK)
        );
    }
    p.revalidate_inner::<false>(&f.supervisor, &f.policy, &mut b)
        .unwrap();
    retire(p, &mut b);
}

#[test]
fn original_ledger_prefix_and_first_denials_survive_success_and_unwind() {
    let mut f = Fixture::new();
    let floor = f.floor();
    let q = f.quota();
    let mut w = Work::new(13 + q.work());
    let mut b = Budget::new(&mut w, floor + q.scratch());
    reserve_fixture(&f, &mut b);
    b.charge_work(13).unwrap();
    assert!(b.charge_work(14 + q.work()).is_err());
    assert!(b.reserve_storage(q.scratch() + 1).is_err());
    let denied_work = b.failed_work();
    let denied_storage = b.failed_storage();
    let ledger = b.work_ledger_identity_v1();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        b.with_prepaid_scope(floor, 0, 0, 0, |b| -> Result<()> {
            let (_p, c) = f.prepare_non_authoritative_same_owner_test(b)?;
            b.reserve_storage(c.additional_storage())?;
            panic!("caller unwind while native owner remains fully charged")
        })
    }));
    assert!(panic.is_err());
    assert_eq!(b.work(), 13 + q.work());
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(b.failed_work(), denied_work);
    assert_eq!(b.failed_storage(), denied_storage);
    assert_eq!(b.storage(), floor);
    b.release_storage(floor - f.context_storage()).unwrap();
    assert_eq!(b.storage(), f.context_storage());
    rustix::fs::flock(f.lock(), FlockOperation::NonBlockingLockExclusive).unwrap();
}

#[test]
fn arithmetic_and_live_ledger_overflows_do_not_wrap_or_reset() {
    assert!(matches!(
        sum(&[usize::MAX, 1]),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    for storage_overflow in [false, true] {
        let mut f = Fixture::new();
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let live = if storage_overflow {
            usize::MAX
        } else {
            f.floor()
        };
        b.reserve_storage(live).unwrap();
        if !storage_overflow {
            b.charge_work(usize::MAX - ENTRY_WORK + 1).unwrap();
        }
        assert!(f.prepare_non_authoritative_same_owner_test(&mut b).is_err());
        assert_eq!(b.storage(), live);
        assert_eq!(
            if storage_overflow {
                b.failed_storage()
            } else {
                b.failed_work()
            },
            Some(usize::MAX)
        );
    }
}

#[test]
fn every_policy_axis_and_complete_supervisor_identity_are_required() {
    for axis in 1..=13 {
        let mut f = Fixture::new();
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        reserve_fixture(&f, &mut b);
        if axis <= 5 {
            let p = make_policy(axis, &mut b);
            let old = std::mem::replace(&mut f.policy, p);
            let charge = old.retained_storage();
            drop(old);
            b.release_storage(charge).unwrap();
        } else {
            let s = make_supervisor(
                &f.policy,
                f.input().deployment.deployment().service(),
                axis - 5,
                &mut b,
            );
            let old = std::mem::replace(&mut f.supervisor, s);
            let charge = old.retained_storage();
            drop(old);
            b.release_storage(charge).unwrap();
        }
        let live = b.storage();
        assert!(matches!(
            f.prepare_non_authoritative_same_owner_test(&mut b),
            Err(Error::Invalid(Failure::ContextMismatch))
        ));
        assert_eq!(b.storage(), live);
    }
}

#[test]
fn provisioning_and_key_bind_full_actual_deployment_not_just_key_bytes() {
    for change_key in [false, true] {
        let mut f = Fixture::new();
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        reserve_fixture(&f, &mut b);
        let (d, c) = Deployment::new(
            f.supervisor.deployment(),
            f.policy.policy(),
            Measurement::new([9; 32], 9).unwrap(),
            &mut b,
        )
        .unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let i = f.inputs.as_mut().unwrap();
        if change_key {
            b.reserve_storage(32).unwrap();
            let (key, c) = Key::create_and_zeroize(&mut [7; 32], &d, &mut b).unwrap();
            b.reserve_storage(c.additional_storage()).unwrap();
            b.release_storage(32).unwrap();
            let old = std::mem::replace(&mut i.key, key);
            let charge = old.retained_storage();
            drop(old);
            b.release_storage(charge).unwrap();
        } else {
            let (q, c) =
                Provisioning::new(&d, i.provisioning.provisioning().helper(), &mut b).unwrap();
            b.reserve_storage(c.additional_storage()).unwrap();
            let (q, c) = ProvisioningCap::create(q, &mut b).unwrap();
            b.reserve_storage(c.additional_storage()).unwrap();
            let old = std::mem::replace(&mut i.provisioning, q);
            let charge = old.retained_storage();
            drop(old);
            b.release_storage(charge).unwrap();
        }
        let live = b.storage();
        let result = f.prepare_non_authoritative_same_owner_test(&mut b);
        if change_key {
            assert!(matches!(result, Err(Error::Capability(_))));
        } else {
            assert!(matches!(
                result,
                Err(Error::Invalid(Failure::ProvisioningMismatch))
            ));
        }
        assert_eq!(b.storage(), live);
    }
}

#[test]
fn exact_root_credentials_are_required_by_public_entrypoints() {
    let mut f = Fixture::new();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    reserve_fixture(&f, &mut b);
    let floor = b.storage();
    match fe2o3_protected_service_spawn::require_exact_root_identity_v1() {
        Ok(()) => {
            let (p, c) = f.production(&mut b).unwrap();
            b.reserve_storage(c.additional_storage()).unwrap();
            p.revalidate(&f.supervisor, &f.policy, &mut b).unwrap();
            retire(p, &mut b);
        }
        Err(_) => {
            assert!(matches!(
                f.production(&mut b),
                Err(Error::Invalid(Failure::RootRequired))
            ));
            assert_eq!(b.storage(), floor);
            let mut f = Fixture::new();
            reserve_fixture(&f, &mut b);
            let (p, c) = f.prepare_non_authoritative_same_owner_test(&mut b).unwrap();
            b.reserve_storage(c.additional_storage()).unwrap();
            assert!(matches!(
                p.revalidate(&f.supervisor, &f.policy, &mut b),
                Err(Error::Invalid(Failure::RootRequired))
            ));
            retire(p, &mut b);
        }
    }
}

#[test]
fn root_credentials_flags_mode_type_and_pin_refuse() {
    for mode in 0..7 {
        let mut f = match mode {
            0 => Fixture::with_credentials(1, 0),
            1 => Fixture::with_credentials(0, 1),
            _ => Fixture::new(),
        };
        let i = f.inputs.as_mut().unwrap();
        match mode {
            2 => rustix::fs::fchmod(&i.root, Mode::from_raw_mode(0o755)).unwrap(),
            3 => rustix::io::fcntl_setfd(&i.root, rustix::io::FdFlags::empty()).unwrap(),
            4 => {
                let flags = rustix::fs::fcntl_getfl(&i.root).unwrap();
                rustix::fs::fcntl_setfl(&i.root, flags | OFlags::APPEND).unwrap();
            }
            5 => i.root = File::open(f.dir.path().join("helper")).unwrap(),
            _ => {}
        }
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        reserve_fixture(&f, &mut b);
        if mode == 6 {
            let (mut p, c) = f.prepare_non_authoritative_same_owner_test(&mut b).unwrap();
            b.reserve_storage(c.additional_storage()).unwrap();
            let other = f.dir.path().join("other");
            fs::create_dir(&other).unwrap();
            fs::set_permissions(&other, fs::Permissions::from_mode(0o700)).unwrap();
            if rustix::process::geteuid().is_root() {
                let (uid, gid) = ids();
                rustix::fs::chown(
                    &other,
                    Some(rustix::process::Uid::from_raw(uid)),
                    Some(rustix::process::Gid::from_raw(gid)),
                )
                .unwrap();
            }
            p.root = File::open(other).unwrap();
            assert!(matches!(
                p.revalidate_inner::<false>(&f.supervisor, &f.policy, &mut b),
                Err(Error::Invalid(Failure::StateRootChanged))
            ));
            retire(p, &mut b);
        } else {
            assert!(matches!(
                f.prepare_non_authoritative_same_owner_test(&mut b),
                Err(Error::Invalid(Failure::InvalidStateRoot))
            ));
        }
    }
}

#[test]
fn source_digest_length_mode_and_key_metadata_refuse_without_retiring_input_charges() {
    for mode in 0..7 {
        let mut f = Fixture::new();
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        reserve_fixture(&f, &mut b);
        if mode == 6 {
            let (file, c) = f.input().key.try_clone_for_transfer(&mut b).unwrap();
            b.reserve_storage(c.additional_storage()).unwrap();
            rustix::fs::fchmod(&file, Mode::from_raw_mode(0o444)).unwrap();
            drop(file);
            b.release_storage(c.additional_storage()).unwrap();
        } else {
            let name = if mode < 3 { "helper" } else { "daemon" };
            let path = f.dir.path().join(name);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            match mode % 3 {
                0 => {
                    let mut bytes = fs::read(&path).unwrap();
                    let last = bytes.len() - 1;
                    bytes[last] ^= 1;
                    fs::write(&path, bytes).unwrap();
                }
                1 => {
                    let mut bytes = fs::read(&path).unwrap();
                    bytes.push(0);
                    fs::write(&path, bytes).unwrap();
                }
                _ => fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap(),
            }
            if mode % 3 != 2 {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
            }
        }
        let floor = b.storage();
        let e = f
            .prepare_non_authoritative_same_owner_test(&mut b)
            .unwrap_err();
        if mode == 6 {
            assert!(matches!(e, Error::Capability(_)));
        } else {
            assert!(matches!(e, Error::Executable(_)));
        }
        assert_eq!(b.storage(), floor);
        rustix::fs::flock(f.lock(), FlockOperation::NonBlockingLockExclusive).unwrap();
    }
}

#[test]
fn revalidation_requires_actual_context_and_rechecks_mutable_image_metadata() {
    let mut f = Fixture::new();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    reserve_fixture(&f, &mut b);
    let (p, c) = f.prepare_non_authoritative_same_owner_test(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let other = make_policy(1, &mut b);
    assert!(matches!(
        p.revalidate_inner::<false>(&f.supervisor, &other, &mut b),
        Err(Error::Invalid(Failure::ContextMismatch))
    ));
    let (file, c) = p.helper.try_clone_for_exec(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    // F_SEAL_EXEC locks execute bits, but read/write permissions remain mutable.
    assert_eq!(
        rustix::fs::fchmod(&file, Mode::from_raw_mode(0o500)),
        Err(rustix::io::Errno::PERM)
    );
    p.revalidate_inner::<false>(&f.supervisor, &f.policy, &mut b)
        .unwrap();
    rustix::fs::fchmod(&file, Mode::from_raw_mode(0o755)).unwrap();
    let live = b.storage();
    assert!(matches!(
        p.revalidate_inner::<false>(&f.supervisor, &f.policy, &mut b),
        Err(Error::Executable(_))
    ));
    assert_eq!(b.storage(), live);
    drop(file);
    b.release_storage(c.additional_storage()).unwrap();
    retire(p, &mut b);
}

#[test]
fn exact_public_capability_objects_and_preparing_pid_are_rechecked() {
    for target in 0..5 {
        let mut f = Fixture::new();
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        reserve_fixture(&f, &mut b);
        let (mut p, c) = f.prepare_non_authoritative_same_owner_test(&mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        if target == 4 {
            p.prepared_by = rustix::process::Pid::from_raw(p.prepared_by.as_raw_pid() + 1).unwrap();
            assert!(matches!(
                p.revalidate_inner::<false>(&f.supervisor, &f.policy, &mut b),
                Err(Error::Invalid(Failure::CoordinatorChanged))
            ));
        } else {
            let (file, c) = match target {
                0 => f.policy.try_clone_for_transfer(&mut b),
                1 => f.supervisor.try_clone_for_transfer(&mut b),
                2 => p.deployment.try_clone_for_transfer(&mut b),
                _ => p.provisioning.try_clone_for_transfer(&mut b),
            }
            .unwrap();
            b.reserve_storage(c.additional_storage()).unwrap();
            rustix::fs::fchmod(&file, Mode::from_raw_mode(0o444)).unwrap();
            let live = b.storage();
            assert!(matches!(
                p.revalidate_inner::<false>(&f.supervisor, &f.policy, &mut b),
                Err(Error::Capability(_))
            ));
            assert_eq!(b.storage(), live);
            drop(file);
            b.release_storage(c.additional_storage()).unwrap();
        }
        retire(p, &mut b);
    }
}

#[test]
fn entry_denial_closes_all_consumed_descriptor_owners_without_retiring_their_charges() {
    let mut f = Fixture::new();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    reserve_fixture(&f, &mut b);
    let floor = b.storage();
    let mut objects = vec![
        identity(&f.input().helper),
        identity(&f.input().daemon),
        identity(&f.input().root),
    ];
    let parent = fs::metadata(f.dir.path()).unwrap();
    objects.push((parent.dev(), parent.ino()));
    for target in 0..4 {
        let (file, storage) = match target {
            0 => f
                .input()
                .deployment
                .try_clone_for_transfer(&mut b)
                .map(|(f, c)| (f, c.additional_storage()))
                .unwrap(),
            1 => f
                .input()
                .provisioning
                .try_clone_for_transfer(&mut b)
                .map(|(f, c)| (f, c.additional_storage()))
                .unwrap(),
            2 => f
                .input()
                .key
                .try_clone_for_transfer(&mut b)
                .map(|(f, c)| (f, c.additional_storage()))
                .unwrap(),
            _ => f
                .input()
                .lease
                .try_clone_for_transfer(&mut b)
                .map(|(f, c)| (f, c.additional_storage()))
                .unwrap(),
        };
        b.reserve_storage(storage).unwrap();
        objects.push(identity(&file));
        drop(file);
        b.release_storage(storage).unwrap();
    }
    b.charge_work(LIMIT - b.work() - ENTRY_WORK + 1).unwrap();
    assert!(matches!(
        f.prepare_non_authoritative_same_owner_test(&mut b),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(b.storage(), floor);
    for id in objects {
        assert_eq!(refs(id), 0, "consumed object still open: {id:?}");
    }
    b.release_storage(floor - f.context_storage()).unwrap();
    assert_eq!(b.storage(), f.context_storage());
    rustix::fs::flock(f.lock(), FlockOperation::NonBlockingLockExclusive).unwrap();
}
