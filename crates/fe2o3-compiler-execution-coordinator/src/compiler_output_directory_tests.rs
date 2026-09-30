//! Real directory/Stage mechanics only; no approved compiler or namespace owner.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{os::unix::fs::MetadataExt, panic::AssertUnwindSafe};

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

#[test]
#[allow(unsafe_code)]
fn output_binding_checks_the_actual_staged_197_slot() {
    let directory = tempfile::tempdir().unwrap();
    let file = File::open(directory.path()).unwrap();
    let foreign_directory = tempfile::tempdir().unwrap();
    let foreign = File::open(foreign_directory.path()).unwrap();
    // Only staging descriptors. No clone, exec, approval or executable bytes.
    let image = tempfile::tempfile().unwrap();
    let channel = tempfile::tempfile().unwrap();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 10_000_000);
    let source = 8 * FILE_STORAGE;
    b.reserve_storage(source).unwrap();
    let owner = CompilerOutputDirectory::capture(
        file.as_fd(),
        identity(&file),
        CompilerOutputDirectory::CHILD_PATH,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(CompilerOutputDirectory::STORAGE).unwrap();
    assert_eq!(owner.binding(&mut b).unwrap().destination(), OUTPUT_FD);
    for mode in 0..3 {
        let binding = Binding::new(
            if mode == 2 {
                foreign.as_fd()
            } else {
                file.as_fd()
            },
            if mode == 1 { 196 } else { OUTPUT_FD },
        )
        .unwrap();
        // SAFETY: this inert test owns every actual descriptor and prepays its
        // source backing; it never spawns or treats Stage as process authority.
        let (stage, charge) = unsafe {
            Stage::stage(
                &image,
                &[binding],
                channel.as_fd(),
                channel.as_fd(),
                channel.as_fd(),
                source,
                &mut b,
            )
        }
        .unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(owner.validate_staged(&stage, &mut b).is_ok(), mode == 0);
        let charge = stage.retained_storage();
        drop(stage);
        b.release_storage(charge).unwrap();
    }
}
