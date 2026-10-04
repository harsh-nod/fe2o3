// Native observation and actual V5 lock custody over an inert handoff. Run only
// through the isolated parent below; the existing ten startup cases are unchanged.
use super::*;
use fe2o3_artifact_transaction::{
    MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5 as REQUEST_STORAGE,
    enable_same_mount_namespace_artifact_path_guard_v1,
    try_acquire_artifact_process_spawn_lease_v1 as spawn_lease, with_artifact_process_spawn_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as PublicationAccount;
use std::{
    os::fd::AsRawFd,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    path::Path,
    sync::atomic::Ordering,
};

const PUBLICATION_CASE: &str = "FE2O3_NATIVE_ROOT_PUBLICATION_CASE";
const PUBLICATION_CASES: [&str; 7] = [
    "drop",
    "issuer-removed",
    "unwind",
    "outer-accounting",
    "invocation-mismatch",
    "short-ceiling",
    "large-root",
];
// Logical ownership shape only, not allocated bytes or an approved runtime.
const EXTERNAL_RUNTIME_SHAPE: usize = 352_457_184;
type Prepublished = (
    fixtures::Fixture,
    publication_fixture::Publication,
    compiler::PublicationInputs,
    usize,
);

pub(super) fn matrix() {
    fixtures::require_environment();
    let helper = test_name("native_root_publication_case");
    for case in PUBLICATION_CASES {
        let runtime = tempfile::Builder::new()
            .prefix("publication-runtime-")
            .tempdir_in(fixtures::path("FE2O3_NATIVE_ROOT_SCRATCH"))
            .unwrap();
        fixtures::mode(runtime.path(), 0o700);
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                &helper,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(PUBLICATION_CASE, case)
            .env("XDG_RUNTIME_DIR", runtime.path())
            .env_remove("FE2O3_ARTIFACT_PATH_GUARD_DIR")
            .env_remove("FE2O3_ARTIFACT_PATH_GUARD_DIR_IDENTITY");
        let output = with_artifact_process_spawn_v1(|| command.output()).unwrap();
        require_output(
            output,
            &format!("NATIVE_ROOT_PUBLICATION_V3_OK case={case} "),
        );
    }
}

pub(super) fn case() {
    fixtures::require_environment();
    let case = std::env::var(PUBLICATION_CASE).expect("publication matrix subprocess case");
    assert!(PUBLICATION_CASES.contains(&case.as_str()));
    let runtime = fixtures::path("XDG_RUNTIME_DIR");
    let metadata = disk::symlink_metadata(&runtime).unwrap();
    assert!(runtime.starts_with("/tmp") && metadata.is_dir());
    assert_eq!(
        (metadata.uid(), metadata.gid(), metadata.mode() & 0o7777),
        (0, 0, 0o700)
    );
    enable_same_mount_namespace_artifact_path_guard_v1();
    // Inert fixture publication is a prior writer transaction, not a request
    // reset or a bypass of legacy publication's strict whole-account cap.
    let prepublished = (case == "large-root").then(|| {
        let f = fixtures::Fixture::new(false);
        let mut work = Work::new(WORK);
        let mut writer = Budget::new(&mut work, REQUEST_STORAGE);
        let (publication, inputs) = publication_fixture::Publication::new(&f, false, &mut writer);
        (f, publication, inputs, writer.storage())
    });
    let external = if prepublished.is_some() {
        EXTERNAL_RUNTIME_SHAPE
    } else {
        0
    };
    let total = external + REQUEST_STORAGE;
    // This is the ORIGINAL account used by preparation, clone, session and
    // publication. Never replace it or raise the artifact schema's 256 MiB cap.
    let mut account = PublicationAccount::new(Work::new(WORK), total);
    account.with_budget(|b| {
    b.reserve_storage(external).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let address = b as *const Budget<'_> as usize;
    let result = catch_unwind(AssertUnwindSafe(|| run(&case, prepublished, b)));
    eprintln!(
        "NATIVE_ROOT_PUBLICATION_ACCOUNT case={case} limit={} work={} storage={} peak={} failed_work={:?} failed_storage={:?}",
        b.storage_limit(),
        b.work(),
        b.storage(),
        b.peak_storage(),
        b.failed_work(),
        b.failed_storage()
    );
    assert_eq!(b.storage_limit(), total);
    assert!(b.storage() >= external);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(b as *const Budget<'_> as usize, address);
    let (used, quote, cleanup_peak) = match result {
        Ok(stats) => stats,
        Err(panic) => resume_unwind(panic),
    };
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
    eprintln!(
        "NATIVE_ROOT_PUBLICATION_V3_OK case={case} work={used}/{quote} peak={} cleanup_peak={cleanup_peak}",
        b.peak_storage()
    );
    });
}

#[allow(unsafe_code)]
fn run(
    case: &str,
    prepublished: Option<Prepublished>,
    b: &mut Budget<'_>,
) -> (usize, usize, usize) {
    let (mut f, prepublished) = match prepublished {
        Some((f, p, inputs, storage)) => {
            b.reserve_storage(storage).unwrap();
            (f, Some((p, inputs)))
        }
        None => (fixtures::Fixture::new(false), None),
    };
    // Paths outlive the original cleanup controller even when any assertion
    // unwinds. The controller itself outlives every foreground attempt below.
    let publication;
    let mut cleanup = cleanup::Drain::new();
    report("before-preparation", b);
    let prepared = preparation::prepare(&mut f, &mut cleanup.pool, b);
    report("after-preparation", b);
    let (p, inputs) = prepublished.unwrap_or_else(|| {
        publication_fixture::Publication::new(&f, case == "invocation-mismatch", b)
    });
    publication = p;
    report("after-inert-publication", b);
    let (trace, exit, drops) = compiler::confirmed_publication(&f, inputs, &mut cleanup.pool, b);
    let base_cleanup = prepared
        .issuer_cleanup_quota(&trace, 3 * TURNS + 3)
        .unwrap();
    let late = fe2o3_broker_authority_service::RootPublicationCustodyV3::observation_cleanup_quota(
        publication.handoff_limit,
    )
    .unwrap();
    let cleanup_report = cleanup.pool.report().unwrap();
    let extra_work = late
        .work()
        .checked_add(TURNS.checked_mul(late.retirement_work()).unwrap())
        .unwrap();
    assert!(
        cleanup_report
            .work
            .checked_add(base_cleanup.work())
            .unwrap()
            .checked_add(extra_work)
            .unwrap()
            <= cleanup_report.work_limit
    );
    assert!(
        cleanup_report
            .storage
            .checked_add(base_cleanup.additional_storage())
            .unwrap()
            .checked_add(late.persistent_storage())
            .unwrap()
            <= STORAGE
    );
    let consumed = prepared.retained_storage() + trace.retained_storage();
    report("before-attempt", b);
    // SAFETY: the unchanged confirmed native-clone fixture supplies the original
    // held first exec and authenticated peer. No compiler instruction resumes.
    let (mut attempt, growth) =
        unsafe { prepared.launch_root_attempt(trace, TIMEOUT, &mut cleanup.pool, b) }
            .expect("genuine original trace/session startup within original request cap");
    b.reserve_storage(growth.additional_storage()).unwrap();
    assert_eq!(
        attempt.retained_storage(),
        consumed + growth.additional_storage()
    );
    attempt.validate_ready(b).unwrap();
    assert!(attempt.poll_compiler(b).unwrap().is_exec());
    assert!(!kernel_locked(publication.directory.path()));
    if case == "issuer-removed" {
        let retained = attempt.retained_storage();
        assert_ne!(attempt.cancel_issuer(), Some(CleanupPoll::Quarantined));
        assert_eq!(attempt.issuer_pid(), None);
        assert_eq!(attempt.retained_storage(), retained);
        attempt.validate_original(b).unwrap();
    }
    let floor = b.storage();
    let retained = attempt.retained_storage();
    let maximum = publication.handoff_limit - usize::from(case == "short-ceiling");
    let quota = NativeAttempt::<compiler::Backing>::publication_observation_quota(maximum).unwrap();
    let mut used = 0;
    let mut blocked = None;
    report("before-observation", b);
    match case {
        "short-ceiling" | "invocation-mismatch" => {
            let error =
                observe(attempt, &mut cleanup.pool, maximum, quota, &mut used, b).unwrap_err();
            assert_eq!(
                b.storage(),
                floor,
                "consumed reservation remains caller-owned"
            );
            if case == "invocation-mismatch" {
                assert!(
                    error
                        .to_string()
                        .contains("native compiler occurrence mismatch"),
                    "{error:?}"
                );
                // This error is after BOTH actual owners install, not an invented
                // success view. Immediate terminal cleanup is also legal.
                let locked = kernel_locked(publication.directory.path());
                assert!(locked || terminal(exit.as_fd()));
                eprintln!(
                    "NATIVE_ROOT_PUBLICATION_POST_ERROR locked={locked} compiler_terminal={}",
                    terminal(exit.as_fd())
                );
            } else {
                assert!(
                    !kernel_locked(publication.directory.path()),
                    "ceiling refusal must precede lock acquisition"
                );
            }
        }
        "outer-accounting" => {
            let result: Result<NativeAttempt<'_, compiler::Backing>> =
                b.with_prepaid_scope(floor, 0, 0, 1, |b| {
                    let entry = b.storage();
                    let (attempt, charge) =
                        observe(attempt, &mut cleanup.pool, maximum, quota, &mut used, b)?;
                    b.reserve_storage(charge.additional_storage())?;
                    assert_publication(&attempt, &publication, b);
                    blocked = Some(spawn_lease().unwrap());
                    // Refuse the OUTER scope only after real observation completed.
                    b.release_storage(b.storage() - entry + 1)?;
                    Ok(attempt)
                });
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            assert_eq!(b.storage(), floor);
        }
        _ => {
            let (mut attempt, charge) =
                observe(attempt, &mut cleanup.pool, maximum, quota, &mut used, b)
                    .expect("actual original-trace observation and lease/token acquisition");
            assert_eq!(b.storage(), floor);
            assert_eq!(
                attempt.retained_storage(),
                retained + charge.additional_storage()
            );
            b.reserve_storage(charge.additional_storage()).unwrap();
            assert_publication(&attempt, &publication, b);
            if case == "drop" {
                let (retained, paid) = (attempt.retained_storage(), b.storage());
                assert_ne!(attempt.cancel_issuer(), Some(CleanupPoll::Quarantined));
                assert_eq!(attempt.issuer_pid(), None);
                assert_eq!(attempt.retained_storage(), retained);
                assert_eq!(b.storage(), paid);
                assert_publication(&attempt, &publication, b);
            }
            blocked = Some(spawn_lease().unwrap());
            if case == "unwind" {
                assert!(
                    catch_unwind(AssertUnwindSafe(move || {
                        let _attempt = attempt;
                        panic!("intentional post-observation foreground unwind");
                    }))
                    .is_err()
                );
            } else {
                drop(attempt);
            }
        }
    }
    report("after-foreground-drop", b);
    if blocked.is_some() {
        assert!(kernel_locked(publication.directory.path()));
    }
    cleanup::wait_exit(&mut cleanup.pool, exit.as_fd());
    if blocked.is_some() {
        // The child is terminal but its slot must retain both locks while the
        // genuine spawn coordinator cannot admit a retirement barrier.
        assert!(terminal(exit.as_fd()));
        assert!(kernel_locked(publication.directory.path()));
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert!(matches!(
            cleanup.pool.shutdown(),
            Err(fe2o3_protected_service_spawn::ProtectedServiceCleanupErrorV2::Busy)
        ));
        for _ in 0..32 {
            cleanup
                .pool
                .pump(fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_PROCESSES_V2)
                .unwrap();
            assert!(kernel_locked(publication.directory.path()));
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let report = cleanup.pool.report().unwrap();
    eprintln!(
        "NATIVE_ROOT_PUBLICATION_CLEANUP work={} peak={} retained={} failed_work={:?}",
        report.work, report.peak_storage, report.storage, report.failed_work
    );
    drop(blocked);
    cleanup.finish();
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "original immutable compiler backing drops once"
    );
    assert!(!kernel_locked(publication.directory.path()));
    f.assert_unlocked();
    assert!(
        matches!(
            rustix::process::waitid(
                rustix::process::WaitId::PidFd(exit.as_fd()),
                rustix::process::WaitIdOptions::EXITED | rustix::process::WaitIdOptions::NOHANG,
            ),
            Err(rustix::io::Errno::CHILD)
        ),
        "only the original cleanup slot reaps the compiler"
    );
    (used, quota.work(), report.peak_storage)
}

fn observe<'work>(
    attempt: NativeAttempt<'work, compiler::Backing>,
    cleanup: &mut Cleanup,
    maximum: usize,
    quota: Quota,
    used: &mut usize,
    b: &mut Budget<'_>,
) -> Result<(NativeAttempt<'work, compiler::Backing>, Storage)> {
    let floor = b.storage();
    let peak = b.peak_storage();
    let work = b.work();
    let result = attempt.observe_publication(cleanup, maximum, b);
    *used = b.work() - work;
    assert!(*used <= quota.work(), "complete observation work quote");
    assert_eq!(b.storage(), floor);
    assert!(
        b.peak_storage() <= peak.max(floor.checked_add(quota.scratch()).unwrap()),
        "complete observation overlap quote"
    );
    result
}

fn assert_publication(
    attempt: &NativeAttempt<'_, compiler::Backing>,
    publication: &publication_fixture::Publication,
    b: &mut Budget<'_>,
) {
    attempt.validate_original(b).unwrap();
    let quota = attempt.publication_revalidation_quota().unwrap();
    let (work, floor, peak) = (b.work(), b.storage(), b.peak_storage());
    attempt.revalidate_publication(b).unwrap();
    assert!(b.work() - work <= quota.work());
    assert_eq!(b.storage(), floor);
    assert!(b.peak_storage() <= peak.max(floor.checked_add(quota.scratch()).unwrap()));
    assert_eq!(
        attempt.publication.as_ref().unwrap().subject(),
        &publication.subject
    );
    assert!(kernel_locked(publication.directory.path()));
    let mut foreign_work = Work::new(WORK);
    let mut foreign = Budget::new(&mut foreign_work, b.storage_limit());
    foreign.reserve_storage(b.storage()).unwrap();
    assert!(matches!(
        attempt.revalidate_publication(&mut foreign),
        Err(Error::Resource(Resource::Accounting))
    ));
    let missing = b.storage() - attempt.retained_storage() + 1;
    b.release_storage(missing).unwrap();
    assert!(matches!(
        attempt.revalidate_publication(b),
        Err(Error::Resource(Resource::Accounting))
    ));
    b.reserve_storage(missing).unwrap();
    attempt.revalidate_publication(b).unwrap();
}

fn report(stage: &str, b: &Budget<'_>) {
    eprintln!(
        "NATIVE_ROOT_PUBLICATION_STAGE stage={stage} limit={} work={} storage={} peak={}",
        b.storage_limit(),
        b.work(),
        b.storage(),
        b.peak_storage()
    );
}

fn terminal(pidfd: BorrowedFd<'_>) -> bool {
    let mut descriptors = [event::PollFd::new(&pidfd, event::PollFlags::IN)];
    event::poll(
        &mut descriptors,
        Some(&event::Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .unwrap();
    descriptors[0].revents().contains(event::PollFlags::IN)
}

#[allow(unsafe_code)]
fn kernel_locked(directory: &Path) -> bool {
    let file = disk::OpenOptions::new()
        .read(true)
        .write(true)
        .open(directory.join(".fe2o3-artifacts.lock"))
        .unwrap();
    // SAFETY: native flock is fully initialized for a nonblocking OFD probe on
    // a distinct live open description; no process wait or lock owner is stolen.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    let result = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &lock) };
    if result == 0 {
        return false;
    }
    assert!(matches!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EACCES | libc::EAGAIN)
    ));
    true
}
