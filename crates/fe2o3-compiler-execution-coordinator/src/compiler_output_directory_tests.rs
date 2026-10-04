//! Real directory/Stage mechanics only; no approved compiler or namespace owner.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{os::fd::AsRawFd, os::unix::fs::MetadataExt, panic::AssertUnwindSafe};

fn identity(file: &File) -> (u64, u64) {
    let m = file.metadata().unwrap();
    (m.dev(), m.ino())
}
fn refs(file: &File) -> usize {
    let expected = identity(file);
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == expected)
        .count()
}

#[derive(Clone, Copy, Debug)]
enum Boundary {
    Exact,
    WorkShort,
    ScratchShort,
    OwnerFloorShort,
}
const BOUNDARIES: [Boundary; 4] = [
    Boundary::Exact,
    Boundary::WorkShort,
    Boundary::ScratchShort,
    Boundary::OwnerFloorShort,
];

fn check_boundary(
    result: Result<()>,
    case: Boundary,
    setup_work: usize,
    total_work: usize,
    attempted_peak: usize,
    b: &mut Budget<'_>,
) {
    let (used, denied_work, denied_storage) = match case {
        Boundary::Exact => {
            result.unwrap();
            assert_eq!(b.peak_storage(), attempted_peak);
            (total_work, None, None)
        }
        Boundary::WorkShort => {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            // Each scope charges its eight entry units before the remaining work.
            (
                total_work - CompilerOutputDirectory::WORK + 8,
                Some(total_work),
                None,
            )
        }
        Boundary::ScratchShort => {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            (total_work, None, Some(attempted_peak))
        }
        Boundary::OwnerFloorShort => {
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            (setup_work + 8, None, None)
        }
    };
    assert_eq!(b.work(), used, "{case:?}");
    assert_eq!(b.failed_work(), denied_work, "{case:?}");
    assert_eq!(b.failed_storage(), denied_storage, "{case:?}");
    if denied_work.is_some() {
        assert!(b.charge_work(total_work).is_err());
        assert_eq!(b.failed_work(), denied_work);
    }
    if denied_storage.is_some() {
        assert!(b.reserve_storage(b.storage_limit()).is_err());
        assert_eq!(b.failed_storage(), denied_storage);
    }
    assert_eq!(b.work(), used);
}

fn validation_boundaries(binding: bool) {
    let directory = tempfile::tempdir().unwrap();
    let file = File::open(directory.path()).unwrap();
    let total_work = 2 * CompilerOutputDirectory::WORK;
    let limit = FILE_STORAGE + CompilerOutputDirectory::STORAGE + CompilerOutputDirectory::SCRATCH;
    for case in BOUNDARIES {
        let mut work = Work::new(total_work - usize::from(matches!(case, Boundary::WorkShort)));
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(FILE_STORAGE).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let address = &b as *const Budget<'_>;
        let owner = CompilerOutputDirectory::capture(
            file.as_fd(),
            identity(&file),
            "/proc/self/fd/197",
            &mut b,
        )
        .unwrap();
        // Missing result funding is tested without releasing any live reservation.
        let retained = if matches!(case, Boundary::OwnerFloorShort) {
            CompilerOutputDirectory::STORAGE - FILE_STORAGE - 1
        } else {
            CompilerOutputDirectory::STORAGE
        };
        b.reserve_storage(retained).unwrap();
        b.reserve_storage(usize::from(matches!(case, Boundary::ScratchShort)))
            .unwrap();
        let floor = b.storage();
        let fd = owner.file.as_raw_fd();
        assert_eq!(refs(&file), 2);
        let result = if binding {
            owner.binding(&mut b).map(|binding| {
                assert_eq!(binding.destination(), 197);
            })
        } else {
            owner.revalidate("/proc/self/fd/197", &mut b)
        };
        check_boundary(
            result,
            case,
            CompilerOutputDirectory::WORK,
            total_work,
            limit + usize::from(matches!(case, Boundary::ScratchShort)),
            &mut b,
        );
        assert_eq!(b.storage(), floor);
        assert_eq!(b.storage_limit(), limit);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(&b as *const Budget<'_>, address);
        assert_eq!(owner.file.as_raw_fd(), fd);
        assert_eq!(identity(&owner.file), identity(&file));
        assert_eq!(refs(&file), 2);
        let history = (
            b.work(),
            b.peak_storage(),
            b.failed_work(),
            b.failed_storage(),
        );
        drop(owner);
        assert_eq!(refs(&file), 1);
        assert_eq!(b.storage(), floor);
        assert_eq!(
            (
                b.work(),
                b.peak_storage(),
                b.failed_work(),
                b.failed_storage()
            ),
            history
        );
    }
}

#[test]
fn output_revalidation_exact_and_one_short_preserve_original_account() {
    validation_boundaries(false);
}

#[test]
fn output_binding_exact_and_one_short_preserve_original_account() {
    validation_boundaries(true);
}

#[test]
fn exact_output_owner_prepays_and_one_short_refuses_before_duplication() {
    let directory = tempfile::tempdir().unwrap();
    let file = File::open(directory.path()).unwrap();
    let peak = CompilerOutputDirectory::SCRATCH + CompilerOutputDirectory::STORAGE;
    for mode in 0..4 {
        let mut work = Work::new(CompilerOutputDirectory::WORK - usize::from(mode == 1));
        let floor = FILE_STORAGE - usize::from(mode == 3);
        let mut b = Budget::new(&mut work, floor + peak - usize::from(mode == 2));
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = CompilerOutputDirectory::capture(
            file.as_fd(),
            identity(&file),
            CompilerOutputDirectory::CHILD_PATH,
            &mut b,
        );
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        match mode {
            0 => {
                let owner = result.unwrap();
                assert_eq!(b.work(), CompilerOutputDirectory::WORK);
                assert_eq!(b.peak_storage(), floor + peak);
                assert_eq!(refs(&file), 2);
                drop(owner);
            }
            1 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
            2 => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            _ => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
        }
        assert_eq!(refs(&file), 1);
    }
}

#[test]
fn output_owner_rejects_foreign_shape_identity_route_and_descriptor_flags() {
    let directory = tempfile::tempdir().unwrap();
    let file = File::open(directory.path()).unwrap();
    let regular = tempfile::tempfile().unwrap();
    let other = tempfile::tempdir().unwrap();
    let other_file = File::open(other.path()).unwrap();
    let path_only = fs::open(
        directory.path(),
        fs::OFlags::PATH | fs::OFlags::CLOEXEC,
        fs::Mode::empty(),
    )
    .unwrap();
    let missing_cloexec = io::fcntl_dupfd_cloexec(&file, 0).unwrap();
    io::fcntl_setfd(&missing_cloexec, io::FdFlags::empty()).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 1_000_000);
    b.reserve_storage(8 * FILE_STORAGE).unwrap();
    for (fd, expected, route) in [
        (
            regular.as_fd(),
            identity(&regular),
            CompilerOutputDirectory::CHILD_PATH,
        ),
        (
            other_file.as_fd(),
            identity(&file),
            CompilerOutputDirectory::CHILD_PATH,
        ),
        (
            path_only.as_fd(),
            identity(&file),
            CompilerOutputDirectory::CHILD_PATH,
        ),
        (
            missing_cloexec.as_fd(),
            identity(&file),
            CompilerOutputDirectory::CHILD_PATH,
        ),
        (file.as_fd(), identity(&file), "/tmp/output"),
        (file.as_fd(), identity(&file), "/proc/self/fd/198"),
    ] {
        assert!(matches!(
            CompilerOutputDirectory::capture(fd, expected, route, &mut b),
            Err(Error::Invalid(_))
        ));
    }
}

#[test]
fn output_owner_requires_original_account_and_unwind_never_refunds_it() {
    fn send_static<T: Send + 'static>() {}
    send_static::<CompilerOutputDirectory>();
    let directory = tempfile::tempdir().unwrap();
    let file = File::open(directory.path()).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut b = Box::new(Budget::new(&mut work, 1_000_000));
    b.reserve_storage(FILE_STORAGE).unwrap();
    let owner = CompilerOutputDirectory::capture(
        file.as_fd(),
        identity(&file),
        CompilerOutputDirectory::CHILD_PATH,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(CompilerOutputDirectory::STORAGE).unwrap();
    owner
        .revalidate(CompilerOutputDirectory::CHILD_PATH, &mut b)
        .unwrap();
    let mut foreign_work = Work::new(usize::MAX);
    let mut foreign = Budget::new(&mut foreign_work, 1_000_000);
    foreign
        .reserve_storage(CompilerOutputDirectory::STORAGE)
        .unwrap();
    assert!(matches!(
        owner.revalidate(CompilerOutputDirectory::CHILD_PATH, &mut foreign),
        Err(Error::Resource(Resource::Accounting))
    ));
    let mut moved = *b;
    assert!(matches!(
        owner.revalidate(CompilerOutputDirectory::CHILD_PATH, &mut moved),
        Err(Error::Resource(Resource::Accounting))
    ));
    let floor = moved.storage();
    let spent = moved.work();
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(move || {
            let _owner = owner;
            panic!("output owner unwind");
        }))
        .is_err()
    );
    assert_eq!(refs(&file), 1);
    assert_eq!(moved.storage(), floor);
    assert_eq!(moved.work(), spent);
}

// Executable, output, three control copies and the one-entry binding slice.
// All fixture images are empty; no executable bytes or production admission.
const STAGE_SOURCES: usize = 5 * FILE_STORAGE + size_of::<Binding<'static>>();

fn stage_limit() -> usize {
    let retained = Stage::storage_for_sources(STAGE_SOURCES).unwrap();
    let capture =
        STAGE_SOURCES + CompilerOutputDirectory::STORAGE + CompilerOutputDirectory::SCRATCH;
    let stage = STAGE_SOURCES
        + CompilerOutputDirectory::STORAGE
        + retained
        + Stage::STAGING_SCRATCH.max(2 * CompilerOutputDirectory::SCRATCH);
    capture.max(stage)
}

#[allow(unsafe_code)]
fn stage_directory(file: &File, destination: i32, b: &mut Budget<'_>) -> Stage {
    let image = tempfile::tempfile().unwrap();
    let channel = tempfile::tempfile().unwrap();
    let binding = Binding::new(file.as_fd(), destination).unwrap();
    // SAFETY: caller holds STAGE_SOURCES for these inert, exclusively owned
    // inputs. This helper only stages; no caller spawns or claims admission.
    let (stage, charge) = unsafe {
        Stage::stage(
            &image,
            &[binding],
            channel.as_fd(),
            channel.as_fd(),
            channel.as_fd(),
            STAGE_SOURCES,
            b,
        )
    }
    .unwrap();
    assert_eq!(stage.retained_storage(), charge.additional_storage());
    stage // Full result funding remains the caller's obligation.
}

#[test]
fn staged_output_validation_exact_and_one_short_preserve_original_account() {
    let directory = tempfile::tempdir().unwrap();
    let file = File::open(directory.path()).unwrap();
    let setup_work = CompilerOutputDirectory::WORK + Stage::STAGING_WORK;
    let total_work = setup_work + 2 * CompilerOutputDirectory::WORK;
    let limit = stage_limit();
    for case in BOUNDARIES {
        let mut work = Work::new(total_work - usize::from(matches!(case, Boundary::WorkShort)));
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(STAGE_SOURCES).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let address = &b as *const Budget<'_>;
        let owner = CompilerOutputDirectory::capture(
            file.as_fd(),
            identity(&file),
            "/proc/self/fd/197",
            &mut b,
        )
        .unwrap();
        b.reserve_storage(CompilerOutputDirectory::STORAGE).unwrap();
        let stage = stage_directory(&file, 197, &mut b);
        // Underfund only the returned stage in the negative input-floor case.
        let retained = if matches!(case, Boundary::OwnerFloorShort) {
            stage.retained_storage() - STAGE_SOURCES - 1
        } else {
            stage.retained_storage()
        };
        b.reserve_storage(retained).unwrap();
        if !matches!(case, Boundary::OwnerFloorShort) {
            // Consume only setup's excess headroom on the unchanged original cap.
            let padding = b.storage_limit() - b.storage() - 2 * CompilerOutputDirectory::SCRATCH
                + usize::from(matches!(case, Boundary::ScratchShort));
            b.reserve_storage(padding).unwrap();
        }
        let floor = b.storage();
        let owner_fd = owner.file.as_raw_fd();
        let staged_fd = stage.binding(197).unwrap().as_raw_fd();
        assert_eq!(refs(&file), 3);
        let result = owner.validate_staged(&stage, &mut b);
        let attempted_peak =
            b.storage_limit() + usize::from(matches!(case, Boundary::ScratchShort));
        check_boundary(result, case, setup_work, total_work, attempted_peak, &mut b);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.storage_limit(), limit);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(&b as *const Budget<'_>, address);
        assert_eq!(owner.file.as_raw_fd(), owner_fd);
        assert_eq!(stage.binding(197).unwrap().as_raw_fd(), staged_fd);
        assert_eq!(identity(stage.binding(197).unwrap()), identity(&file));
        assert_eq!(refs(&file), 3);
        let history = (
            b.work(),
            b.peak_storage(),
            b.failed_work(),
            b.failed_storage(),
        );
        drop(stage);
        assert_eq!(refs(&file), 2);
        drop(owner);
        assert_eq!(refs(&file), 1);
        assert_eq!(b.storage(), floor);
        assert_eq!(
            (
                b.work(),
                b.peak_storage(),
                b.failed_work(),
                b.failed_storage()
            ),
            history
        );
    }
}

#[test]
fn output_binding_checks_the_actual_staged_197_slot() {
    let directory = tempfile::tempdir().unwrap();
    let file = File::open(directory.path()).unwrap();
    let foreign_directory = tempfile::tempdir().unwrap();
    let foreign = File::open(foreign_directory.path()).unwrap();
    let total_work = Stage::STAGING_WORK + 3 * CompilerOutputDirectory::WORK;
    for (destination, actual, expected_error) in [
        (197, &file, None),
        (196, &file, Some("stage has no original output at 197")),
        (
            197,
            &foreign,
            Some("compiler output directory differs from authenticated object"),
        ),
    ] {
        let mut work = Work::new(total_work);
        let mut b = Budget::new(&mut work, stage_limit());
        b.reserve_storage(STAGE_SOURCES).unwrap();
        let owner = CompilerOutputDirectory::capture(
            file.as_fd(),
            identity(&file),
            "/proc/self/fd/197",
            &mut b,
        )
        .unwrap();
        b.reserve_storage(CompilerOutputDirectory::STORAGE).unwrap();
        let before = refs(actual);
        let stage = stage_directory(actual, destination, &mut b);
        b.reserve_storage(stage.retained_storage()).unwrap();
        let floor = b.storage();
        let staged_fd = stage.binding(destination).unwrap().as_raw_fd();
        assert_eq!(
            identity(stage.binding(destination).unwrap()),
            identity(actual)
        );
        assert_eq!(refs(actual), before + 1);
        match (owner.validate_staged(&stage, &mut b), expected_error) {
            (Ok(()), None) => {}
            (Err(Error::Invalid(actual)), Some(expected)) => assert_eq!(actual, expected),
            (result, expected) => panic!("{destination}: expected {expected:?}, got {result:?}"),
        }
        assert_eq!(stage.binding(destination).unwrap().as_raw_fd(), staged_fd);
        assert_eq!(refs(actual), before + 1);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), total_work);
        assert_eq!((b.failed_work(), b.failed_storage()), (None, None));
        drop(stage);
        assert_eq!(refs(actual), before);
        drop(owner);
        assert_eq!(refs(&file), 1);
        assert_eq!(refs(&foreign), 1);
        assert_eq!(b.storage(), floor);
    }
}
