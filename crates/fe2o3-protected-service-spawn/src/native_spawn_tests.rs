use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::os::fd::{AsFd, AsRawFd};

const SOURCE: usize = 4096;
const LIMIT: usize = 10_000_000;
type Stage = StagedProtectedServiceExecV2;

fn fixture() -> (File, File, File) {
    let (r, w) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    (
        File::open("/dev/null").unwrap(),
        File::from(r),
        File::from(w),
    )
}
fn run(
    bindings: usize,
    duplicate: bool,
    work: usize,
    storage: usize,
    reserved: usize,
) -> (
    Result<(Stage, ProtectedServiceSpawnStorageV2)>,
    usize,
    usize,
    usize,
) {
    let (file, read, write) = fixture();
    let bindings: Vec<_> = (0..bindings)
        .map(|i| Binding::new(file.as_fd(), if duplicate { 3 } else { 3 + i as i32 }).unwrap())
        .collect();
    let mut w = Work::new(work);
    let mut b = Budget::new(&mut w, storage);
    b.reserve_storage(reserved).unwrap();
    // Inert descriptor mechanics only: SOURCE conservatively covers these handles
    // and bindings; /dev/null and pipes have no executable/image payload.
    let result = unsafe {
        Stage::stage(
            &file,
            &bindings,
            write.as_fd(),
            read.as_fd(),
            write.as_fd(),
            SOURCE,
            &mut b,
        )
    };
    (result, b.work(), b.storage(), b.peak_storage())
}

#[test]
fn stage_exact_limits_preserve_final_file_identity_flags_and_full_overlap() {
    let (file, read, write) = fixture();
    let bindings = [
        Binding::new(file.as_fd(), 3).unwrap(),
        Binding::new(read.as_fd(), 202).unwrap(),
    ];
    let retained = Stage::storage_for_sources(SOURCE).unwrap();
    let peak = SOURCE + Stage::STAGING_SCRATCH + retained;
    let mut w = Work::new(Stage::STAGING_WORK + 7);
    let mut b = Budget::new(&mut w, peak);
    b.charge_work(7).unwrap();
    b.reserve_storage(SOURCE).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (staged, charge) = unsafe {
        Stage::stage(
            &file,
            &bindings,
            write.as_fd(),
            read.as_fd(),
            write.as_fd(),
            SOURCE,
            &mut b,
        )
    }
    .unwrap();
    assert_eq!(b.work(), Stage::STAGING_WORK + 7);
    assert_eq!(b.storage(), SOURCE);
    assert_eq!(b.peak_storage(), peak);
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(charge.additional_storage(), retained);
    assert_eq!(staged.retained_storage(), retained);
    b.reserve_storage(retained).unwrap();
    for (a, original) in [
        (staged.executable(), &file),
        (staged.binding(3).unwrap(), &file),
        (staged.binding(202).unwrap(), &read),
    ] {
        assert!(a.as_raw_fd() >= PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1);
        assert!(
            rustix::io::fcntl_getfd(a)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let a = rustix::fs::fstat(a).unwrap();
        let o = rustix::fs::fstat(original).unwrap();
        assert_eq!((a.st_dev, a.st_ino), (o.st_dev, o.st_ino));
    }
    assert!(staged.binding(99).is_none());
    let max_work = staged.spawn_work(63).unwrap();
    assert_eq!(
        max_work,
        Stage::SPAWN_WORK + native_work::child_work(2, 63).unwrap() + Cleanup::RESERVATION_WORK
    );
    assert!(staged.spawn_work(64).is_err());
    let before = rustix::fs::fstat(&file).unwrap();
    drop(staged);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), SOURCE);
    assert_eq!(rustix::fs::fstat(&file).unwrap().st_ino, before.st_ino);
}

#[test]
fn stage_refuses_short_entry_floor_work_and_full_image_overlap() {
    let peak = SOURCE + Stage::STAGING_SCRATCH + Stage::storage_for_sources(SOURCE).unwrap();
    for (work, storage, reserve, category) in [
        (ENTRY - 1, peak, SOURCE, 0),
        (Stage::STAGING_WORK - 1, peak, SOURCE, 0),
        (Stage::STAGING_WORK, peak, SOURCE - 1, 1),
        (Stage::STAGING_WORK, peak - 1, SOURCE, 2),
    ] {
        let (result, accepted, live, _) = run(1, false, work, storage, reserve);
        let error = result.unwrap_err();
        assert_eq!(live, reserve);
        assert!(accepted <= work);
        match category {
            0 => assert!(matches!(
                error,
                ProtectedServiceSpawnErrorV2::Resource(Resource::Work(_))
            )),
            1 => assert!(matches!(
                error,
                ProtectedServiceSpawnErrorV2::Resource(Resource::Accounting)
            )),
            _ => assert!(matches!(
                error,
                ProtectedServiceSpawnErrorV2::Resource(Resource::Storage(_))
            )),
        }
    }
}

#[test]
fn stage_validates_entire_bounded_destination_table() {
    for (n, duplicate, accepted) in [
        (0, false, false),
        (1, false, true),
        (32, false, true),
        (33, false, false),
        (2, true, false),
    ] {
        let (result, work, storage, _) = run(n, duplicate, LIMIT, LIMIT, SOURCE);
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(work, Stage::STAGING_WORK);
        assert_eq!(storage, SOURCE);
    }
    assert!(matches!(
        Stage::storage_for_sources(usize::MAX),
        Err(ProtectedServiceSpawnErrorV2::Resource(Resource::Arithmetic))
    ));
}

#[test]
fn native_spawn_nonroot_refuses_before_cleanup_reservation_or_clone() {
    if rustix::process::geteuid().is_root() {
        return;
    }
    let (result, _, _, _) = run(1, false, LIMIT, LIMIT, SOURCE);
    let (stage, charge) = result.unwrap();
    let mut cleanup =
        crate::process_reaper::isolated_cleanup(Account::new(Work::new(LIMIT), Cleanup::STORAGE));
    let before = cleanup.report().unwrap();
    let credentials = Credentials::new(
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap();
    let mut w = Work::new(Stage::SPAWN_WORK);
    let mut b = Budget::new(&mut w, charge.additional_storage() + Stage::SPAWN_SCRATCH);
    b.reserve_storage(charge.additional_storage()).unwrap();
    // Same private implementation, with mandatory root check intact. These inert
    // inputs cannot reach clone and do not establish protected executable admission.
    assert!(matches!(
        stage.spawn_inner(credentials, &mut cleanup, &mut b),
        Err(ProtectedServiceSpawnErrorV2::State(
            "native protected-service spawn requires exact root"
        ))
    ));
    assert_eq!(b.work(), Stage::SPAWN_WORK);
    assert_eq!(b.storage(), charge.additional_storage());
    assert_eq!(cleanup.report().unwrap(), before);
    cleanup.shutdown().unwrap();
}

#[test]
fn staging_restores_storage_and_keeps_prefix_denials_during_unwind() {
    let (file, read, write) = fixture();
    let bindings = [Binding::new(file.as_fd(), 3).unwrap()];
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.charge_work(5).unwrap();
    b.reserve_storage(SOURCE).unwrap();
    assert!(b.charge_work(LIMIT).is_err());
    assert!(b.reserve_storage(LIMIT).is_err());
    let failed_work = b.failed_work();
    let failed_storage = b.failed_storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        b.with_prepaid_scope::<_, ProtectedServiceSpawnErrorV2>(SOURCE, 0, 0, 0, |b| {
            let (stage, charge) = unsafe {
                Stage::stage(
                    &file,
                    &bindings,
                    write.as_fd(),
                    read.as_fd(),
                    write.as_fd(),
                    SOURCE,
                    b,
                )
            }?;
            b.reserve_storage(charge.additional_storage())?;
            let _owned = stage;
            panic!("private staging unwind fixture");
            #[allow(unreachable_code)]
            Ok(())
        })
    }));
    assert!(result.is_err());
    assert_eq!(b.storage(), SOURCE);
    assert_eq!(b.work(), 5 + Stage::STAGING_WORK);
    assert_eq!(b.failed_work(), failed_work);
    assert_eq!(b.failed_storage(), failed_storage);
}

#[test]
fn retaining_spawn_requires_complete_inputs_and_preserves_root_refusal() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Input(Arc<AtomicUsize>);
    impl Drop for Input {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (stage, _) = run(1, false, LIMIT, LIMIT, SOURCE).0.unwrap();
    let mut cleanup =
        crate::process_reaper::isolated_cleanup(Account::new(Work::new(LIMIT), LIMIT));
    let before = cleanup.report().unwrap();
    let credentials = Credentials::new(65534, 65534).unwrap();
    let work = stage.spawn_retaining_work::<Input>(63, 64).unwrap();
    assert_eq!(
        work,
        Stage::SPAWN_WORK
            + native_work::child_work(1, 63).unwrap()
            + Cleanup::retained_launch_work::<Input>(64).unwrap()
    );
    let scratch = Stage::spawn_retaining_scratch::<Input>(64).unwrap();
    for short in [true, false] {
        if !short && rustix::process::geteuid().is_root() {
            continue;
        }
        let floor = stage.retained_storage() + 64 - usize::from(short);
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, floor + scratch);
        b.reserve_storage(floor).unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        // The short owner floor or exact-root gate forbids clone. This only
        // exercises consuming refusal, never admitted deployment or child evidence.
        let result = unsafe {
            stage.spawn_retaining(credentials, Input(drops.clone()), 64, &mut cleanup, &mut b)
        };
        if short {
            assert!(matches!(
                result,
                Err(ProtectedServiceSpawnErrorV2::Resource(Resource::Accounting))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProtectedServiceSpawnErrorV2::State(
                    "native protected-service spawn requires exact root"
                ))
            ));
        }
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(b.storage(), floor);
        assert_eq!(cleanup.report().unwrap(), before);
    }
}
