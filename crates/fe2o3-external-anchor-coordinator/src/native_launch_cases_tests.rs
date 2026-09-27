fn prepared_fixture() -> (Fixture, Prepared) {
    let mut f = Fixture::new();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    reserve_fixture(&f, &mut b);
    let (p, c) = f.prepare_non_authoritative_same_owner_test(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    (f, p)
}

#[test]
fn native_staging_checks_final_objects_and_charges_both_full_images() {
    let (f, p) = prepared_fixture();
    let channels = Channels::new().unwrap();
    let floor = p.launch_input_storage(&f.supervisor, &f.policy).unwrap() + Channels::STORAGE;
    let q = p.staging_quota().unwrap();
    let mut w = Work::new(q.work());
    let mut b = Budget::new(&mut w, floor + q.scratch());
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (stage, charge) = p
        .stage_transfers::<false>(&f.supervisor, &f.policy, &channels, &mut b)
        .unwrap();
    b.reserve_storage(charge).unwrap();
    assert_eq!(b.storage(), floor + charge);
    assert_eq!(b.work(), q.work());
    assert!(b.peak_storage() <= floor + q.scratch());
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(
        charge,
        Stage::storage_for_sources(p.transfer_source_storage().unwrap()).unwrap()
    );
    assert!(b.peak_storage() >= floor + p.transfer_source_storage().unwrap() + charge);
    assert_eq!(
        identity(stage.executable()),
        (
            p.helper.object_identity().device(),
            p.helper.object_identity().inode()
        )
    );
    assert_eq!(
        identity(stage.binding(5).unwrap()),
        (
            p.daemon.object_identity().device(),
            p.daemon.object_identity().inode()
        )
    );
    for destination in DESTINATIONS {
        let file = stage.binding(destination).unwrap();
        assert!(
            rustix::io::fcntl_getfd(file)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
    assert_eq!(
        stage.spawn_work(63).unwrap(),
        Stage::spawn_work_for(DESTINATIONS.len(), 63).unwrap()
    );
    drop(stage);
    b.release_storage(charge).unwrap();
    assert_eq!(b.storage(), floor);
}

#[test]
fn every_final_native_transfer_rejects_descriptor_flag_substitution() {
    let (f, p) = prepared_fixture();
    let channels = Channels::new().unwrap();
    let floor = p.launch_input_storage(&f.supervisor, &f.policy).unwrap() + Channels::STORAGE;
    for destination in [0, 3, 4, 5, 6, 202, 220, 221, 222, 223] {
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        b.reserve_storage(floor).unwrap();
        let (stage, charge) = p
            .stage_transfers::<false>(&f.supervisor, &f.policy, &channels, &mut b)
            .unwrap();
        b.reserve_storage(charge).unwrap();
        let file = if destination == 0 {
            stage.executable()
        } else {
            stage.binding(destination).unwrap()
        };
        rustix::io::fcntl_setfd(file, rustix::io::FdFlags::empty()).unwrap();
        // FD flags do not mutate the original alias: failure must inspect the final File.
        p.revalidate_inner::<false>(&f.supervisor, &f.policy, &mut b)
            .unwrap();
        assert!(
            p.validate_staged::<false>(&stage, &f.supervisor, &f.policy, &channels, &mut b)
                .is_err(),
            "accepted changed final descriptor {destination}"
        );
        assert_eq!(b.storage(), floor + charge);
        drop(stage);
        b.release_storage(charge).unwrap();
    }
}

#[test]
fn native_staging_refuses_short_floor_work_scratch_and_retains_history() {
    let (f, p) = prepared_fixture();
    let channels = Channels::new().unwrap();
    let floor = p.launch_input_storage(&f.supervisor, &f.policy).unwrap() + Channels::STORAGE;
    for mode in 0..5 {
        let prepaid = floor - usize::from(mode == 0);
        let work = match mode {
            1 => ENTRY_WORK - 1,
            2 => launch::LOCAL_WORK - 1,
            _ => LIMIT,
        };
        let storage = if mode == 3 {
            prepaid + LAUNCH_FRAME - 1
        } else {
            LIMIT
        };
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, storage);
        b.reserve_storage(prepaid).unwrap();
        if mode == 4 {
            assert!(b.charge_work(LIMIT).is_ok());
            assert!(b.charge_work(1).is_err());
        }
        let denied = b.failed_work();
        let ledger = b.work_ledger_identity_v1();
        assert!(
            p.stage_transfers::<false>(&f.supervisor, &f.policy, &channels, &mut b)
                .is_err()
        );
        assert_eq!(b.storage(), prepaid);
        assert!(b.work_ledger_identity_v1() == ledger);
        if mode == 4 {
            assert_eq!(b.failed_work(), denied);
        }
    }
}

#[test]
fn native_staging_rejects_actual_context_and_unwind_keeps_the_input_floor() {
    let (f, p) = prepared_fixture();
    let channels = Channels::new().unwrap();
    let floor = p.launch_input_storage(&f.supervisor, &f.policy).unwrap() + Channels::STORAGE;
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(floor).unwrap();
    let wrong = make_policy(1, &mut b);
    let live = b.storage();
    assert!(
        p.stage_transfers::<false>(&f.supervisor, &wrong, &channels, &mut b)
            .is_err()
    );
    assert_eq!(b.storage(), live);
    let ledger = b.work_ledger_identity_v1();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _: LaunchResult<()> = b.with_prepaid_scope(live, 0, 0, 0, |b| {
            let (stage, charge) =
                p.stage_transfers::<false>(&f.supervisor, &f.policy, &channels, b)?;
            b.reserve_storage(charge)?;
            assert!(stage.binding(223).is_some());
            panic!("native staging unwind");
        });
    }));
    assert!(outcome.is_err());
    assert_eq!(b.storage(), live);
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn native_launch_quota_covers_staging_and_full_owner_overlap() {
    let (_, p) = prepared_fixture();
    let q = p.launch_quota().unwrap();
    let s = p.staging_quota().unwrap();
    assert!(q.work() > s.work() + Stage::spawn_work_for(9, 63).unwrap());
    assert!(q.scratch() > s.scratch() + Channels::STORAGE + Child::STORAGE + ADMISSION_STORAGE);
    assert!(Stage::spawn_work_for(0, 63).is_err());
    assert!(Stage::spawn_work_for(9, 64).is_err());
}

#[test]
fn native_launch_refusals_run_with_an_actual_cleanup_account() {
    use std::process::{Command, Stdio};
    let completion = tempfile::NamedTempFile::new().unwrap();
    let name = format!(
        "{}::native_launch_refusal_subprocess",
        module_path!().split_once("::").unwrap().1
    );
    let mut process = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .env("FE2O3_NATIVE_ANCHOR_REFUSAL_PROBE", "1")
        .env("FE2O3_NATIVE_ANCHOR_REFUSAL_COMPLETION", completion.path())
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = process.try_wait().unwrap() {
            assert!(status.success(), "native root refusal subprocess failed");
            assert_eq!(std::fs::read(completion.path()).unwrap(), b"complete");
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = process.kill();
            let _ = process.wait();
            panic!("native root refusal subprocess timed out");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn native_launch_refusal_subprocess() {
    if std::env::var_os("FE2O3_NATIVE_ANCHOR_REFUSAL_PROBE").is_none() {
        return;
    }
    use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account;
    let mut cleanup = Cleanup::admit(Account::new(Work::new(LIMIT), Cleanup::STORAGE)).unwrap();
    for mode in 0..3 {
        let (f, p) = prepared_fixture();
        let floor = p.launch_input_storage(&f.supervisor, &f.policy).unwrap();
        let q = p.launch_quota().unwrap();
        let mut w = Work::new(if mode == 0 { ENTRY_WORK - 1 } else { q.work() });
        let mut b = Budget::new(&mut w, floor + q.scratch());
        b.reserve_storage(floor).unwrap();
        let before = cleanup.report().unwrap();
        let timeout = if mode == 1 {
            Duration::ZERO
        } else {
            Duration::from_secs(1)
        };
        if mode == 2 && rustix::process::geteuid().is_root() {
            continue;
        }
        let err = p
            .launch(&f.supervisor, &f.policy, timeout, &mut cleanup, &mut b)
            .unwrap_err();
        match mode {
            0 => assert!(matches!(err, LaunchError::Resource(Resource::Work(_)))),
            1 => assert!(matches!(
                err,
                LaunchError::Invalid("invalid native anchor launch timeout")
            )),
            _ => assert!(matches!(
                err,
                LaunchError::Preparation(Error::Invalid(Failure::RootRequired))
            )),
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(cleanup.report().unwrap(), before);
    }
    native_root_guard_schedules(&mut cleanup);
    std::fs::write(
        std::env::var_os("FE2O3_NATIVE_ANCHOR_REFUSAL_COMPLETION").unwrap(),
        b"complete",
    )
    .unwrap();
}

fn native_root_guard_schedules(cleanup: &mut Cleanup) {
    let (f, mut p) = prepared_fixture();
    let (other, mut unrelated) = prepared_fixture();
    let floor = p.launch_input_storage(&f.supervisor, &f.policy).unwrap();
    let unrelated_floor = unrelated
        .launch_input_storage(&other.supervisor, &other.policy)
        .unwrap();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(floor + unrelated_floor).unwrap();
    let ledger = b.work_ledger_identity_v1();

    // Both leases are valid independently; only the actual root binding is sufficient.
    std::mem::swap(&mut p.lifecycle, &mut unrelated.lifecycle);
    assert!(matches!(
        p.revalidate_inner::<false>(&f.supervisor, &f.policy, &mut b),
        Err(Error::Lifecycle(_))
    ));
    std::mem::swap(&mut p.lifecycle, &mut unrelated.lifecycle);
    assert!(matches!(
        p.cleanup_guard::<false>(&f.supervisor, &f.policy, cleanup, false, &mut b),
        Err(LaunchError::Cleanup(
            fe2o3_protected_service_spawn::ProtectedServiceCleanupErrorV2::State
        ))
    ));

    let quota = p.cleanup_guard_quota(true).unwrap();
    let mut short_work = Work::new(quota.work() - 1);
    let mut short = Budget::new(&mut short_work, floor + quota.scratch());
    short.reserve_storage(floor).unwrap();
    assert!(
        p.cleanup_guard::<false>(&f.supervisor, &f.policy, cleanup, true, &mut short)
            .is_err()
    );
    assert_eq!(short.storage(), floor);
    assert!(matches!(
        cleanup.try_clone_deployment_guard(&mut b),
        Err(fe2o3_protected_service_spawn::ProtectedServiceCleanupErrorV2::State)
    ));

    let mut exact_work = Work::new(quota.work());
    let mut exact = Budget::new(&mut exact_work, floor + quota.scratch());
    exact.reserve_storage(floor).unwrap();
    p.cleanup_guard::<false>(&f.supervisor, &f.policy, cleanup, true, &mut exact)
        .unwrap();
    assert_eq!(exact.work(), quota.work());
    assert_eq!(exact.storage(), floor);
    assert!(exact.peak_storage() <= floor + quota.scratch());
    assert!(
        p.cleanup_guard::<false>(&f.supervisor, &f.policy, cleanup, true, &mut b)
            .is_err()
    );
    assert!(matches!(
        unrelated.cleanup_guard::<false>(&other.supervisor, &other.policy, cleanup, false, &mut b),
        Err(LaunchError::Preparation(Error::Lifecycle(_)))
    ));

    let quota = p.cleanup_guard_quota(false).unwrap();
    let mut exact_work = Work::new(quota.work());
    let mut exact = Budget::new(&mut exact_work, floor + quota.scratch());
    exact.reserve_storage(floor).unwrap();
    p.cleanup_guard::<false>(&f.supervisor, &f.policy, cleanup, false, &mut exact)
        .unwrap();
    assert_eq!(exact.work(), quota.work());
    assert_eq!(exact.storage(), floor);
    let before = cleanup.report().unwrap();
    assert_eq!(before.storage, Cleanup::STORAGE);
    assert!(before.admission_open);

    let lock_name = std::path::Path::new(
        fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
    )
    .file_name()
    .unwrap();
    let observer = File::open(f.dir.path().join(lock_name)).unwrap();
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        p.cleanup_guard::<false>(&f.supervisor, &f.policy, cleanup, false, &mut b)
            .unwrap();
        drop(p);
        panic!("root guard must survive prepared-owner unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(
        rustix::fs::flock(&observer, FlockOperation::NonBlockingLockExclusive),
        Err(rustix::io::Errno::AGAIN)
    );
    assert_eq!(b.storage(), floor + unrelated_floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    cleanup.shutdown().unwrap();
    rustix::fs::flock(&observer, FlockOperation::NonBlockingLockExclusive).unwrap();
    rustix::fs::flock(&observer, FlockOperation::Unlock).unwrap();
}
