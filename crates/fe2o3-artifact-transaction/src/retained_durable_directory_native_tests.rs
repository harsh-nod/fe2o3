struct AmbiguousCommitHook;

impl RetainedDurableDirectoryHooksV1 for AmbiguousCommitHook {
    fn record(
        &mut self,
        boundary: RetainedDurableRecordBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        if boundary == RetainedDurableRecordBoundaryV1::SyncCanonicalName
            && timing == RetainedDurableFaultTimingV1::Before
        {
            Err(io::Error::other("ambiguous canonical rename"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn native_record_commit_and_ambiguous_recovery_share_the_legacy_namespace() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    let directory = TestDirectory::new();
    let store = directory.store();
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 1024 * 1024);
    let floor = MeteredRetainedDurableDirectoryV2::DIRECTORY_STORAGE + 1024;
    budget.reserve_storage(floor).unwrap();
    let mut io = MeteredRetainedDurableDirectoryV2::new(&store, &mut budget);
    let mut hooks = NoRetainedDurableDirectoryHooksV1;
    io.commit_record("native.state", "native.redo", b"first", 64, &mut hooks)
        .unwrap();
    assert!(
        io.commit_record(
            "native.state",
            "native.redo",
            b"second",
            64,
            &mut AmbiguousCommitHook
        )
        .is_err()
    );
    // A visible rename is not acknowledged as durable; recovery cycles the
    // same canonical bytes through the distinct recovery namespace.
    assert_eq!(
        io.establish_recovered_record_durability(
            "native.state",
            "native.recovery",
            b"second",
            64,
            &mut hooks,
        )
        .unwrap(),
        b"second"
    );
    io.require_absent("native.redo").unwrap();
    io.require_absent("native.recovery").unwrap();
    assert_eq!(
        io.read_private("native.state", 64).unwrap().unwrap(),
        b"second"
    );
    drop(io);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > MeteredRetainedDurableDirectoryV2::FIXED_WORK);
    assert_eq!(
        store.read_private("native.state", 64).unwrap().unwrap(),
        b"second"
    );
}

#[test]
fn native_record_quota_refusal_precedes_io_and_preserves_existing_state() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    let directory = TestDirectory::new();
    let store = directory.store();
    let floor = MeteredRetainedDurableDirectoryV2::DIRECTORY_STORAGE + 3;
    let maximum = 64;
    let work_limit = 8 + MeteredRetainedDurableDirectoryV2::work_for(maximum).unwrap() - 1;
    for (work_limit, storage_limit) in [
        (work_limit, 1024 * 1024),
        (
            100_000_000,
            floor + MeteredRetainedDurableDirectoryV2::scratch_for(maximum).unwrap() - 1,
        ),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let mut io = MeteredRetainedDurableDirectoryV2::new(&store, &mut budget);
        assert!(matches!(
            io.commit_record(
                "native.state",
                "native.redo",
                b"new",
                maximum,
                &mut NoRetainedDurableDirectoryHooksV1
            ),
            Err(RetainedDurableDirectoryErrorV2::Resource(_))
        ));
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 0);
        drop(io);
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn native_record_rejects_oversized_and_unsafe_entries_without_following_them() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    let directory = TestDirectory::new();
    let store = directory.store();
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 1024 * 1024);
    budget
        .reserve_storage(MeteredRetainedDurableDirectoryV2::DIRECTORY_STORAGE + 1024)
        .unwrap();
    let mut io = MeteredRetainedDurableDirectoryV2::new(&store, &mut budget);
    assert!(matches!(
        io.require_absent(&"x".repeat(241)),
        Err(RetainedDurableDirectoryErrorV2::InvalidInput)
    ));
    std::os::unix::fs::symlink("missing", directory.path.join("unsafe.state")).unwrap();
    assert!(io.read_private("unsafe.state", 64).is_err());
    assert!(io.require_absent("unsafe.state").is_err());
    assert!(
        io.commit_record(
            "../escape",
            "native.redo",
            b"new",
            64,
            &mut NoRetainedDurableDirectoryHooksV1
        )
        .is_err()
    );
    assert!(!directory.path.join("native.redo").exists());
}
