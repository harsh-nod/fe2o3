//! Rootless observations use genuine files and a private owner policy. They do
//! not manufacture admitted owners or exercise root-only production composition.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    fs::{self, OpenOptions},
    io::{Seek, SeekFrom},
    os::unix::fs::PermissionsExt,
    panic::{AssertUnwindSafe, catch_unwind},
};

const N: usize = 16;
const EXTRA: usize = 29;
const LIMIT: usize = 1 << 30;

struct Fixture {
    dir: tempfile::TempDir,
    file: File,
    writer: File,
    policy: Policy,
}
impl Fixture {
    fn new(bytes: &[u8], mode: u32) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source");
        fs::write(&path, bytes).unwrap();
        // Retain a genuine writable alias to exercise mutations after chmod.
        let writer = OpenOptions::new().write(true).open(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        let file = File::open(&path).unwrap();
        let stat = rustix::fs::fstat(&file).unwrap();
        let policy = Policy {
            uid: stat.st_uid,
            gid: stat.st_gid,
            mode,
            length: bytes.len(),
        };
        Self {
            dir,
            file,
            writer,
            policy,
        }
    }

    fn record(&self, budget: &mut Budget<'_>) -> Result<[u8; N]> {
        read_record_with_policy(&self.file, self.policy, budget, || Ok(()))
    }

    fn seed(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(Zeroizing<[u8; SEED_BYTES]>, RootSourceStorage)> {
        read_seed_with_policy(&self.file, self.policy, budget, || Ok(()))
    }

    fn overwrite(&self, bytes: &[u8]) {
        assert_eq!(
            rustix::io::pwrite(&self.writer, bytes, 0).unwrap(),
            bytes.len()
        );
    }
}

#[test]
fn exact_queries_include_backing_bytes_and_check_overflow() {
    assert_eq!(FILE_STORAGE, size_of::<File>());
    assert_eq!(file_storage(N).unwrap(), FILE_STORAGE + N);
    assert_eq!(record_input_storage::<N>().unwrap(), FILE_STORAGE + N);
    assert_eq!(record_work::<SEED_BYTES>().unwrap(), SEED_WORK);
    assert_eq!(record_scratch::<SEED_BYTES>().unwrap(), SEED_SCRATCH);
    assert!(
        SEED_STORAGE >= size_of::<Zeroizing<[u8; SEED_BYTES]>>() + size_of::<RootSourceStorage>()
    );
    assert!(record_scratch::<N>().unwrap() >= 2 * N + VALIDATE_SCRATCH);
    assert_eq!(file_storage(0), Err(RootSourceErrorV2::InvalidLength));
    assert_eq!(
        file_storage(usize::MAX),
        Err(RootSourceErrorV2::InvalidLength)
    );
    assert_eq!(record_work::<0>(), Err(RootSourceErrorV2::InvalidLength));
    assert_eq!(record_scratch::<0>(), Err(RootSourceErrorV2::InvalidLength));
    assert_eq!(
        record_work::<{ usize::MAX / 32 + 1 }>(),
        Err(RootSourceErrorV2::Resource(Resource::Arithmetic))
    );
    assert_eq!(
        record_scratch::<{ usize::MAX / 3 + 1 }>(),
        Err(RootSourceErrorV2::Resource(Resource::Arithmetic))
    );
}

#[test]
fn record_success_has_exact_work_peak_and_unreserved_output() {
    let mut f = Fixture::new(&[0x41; N], RECORD_MODE);
    f.file.seek(SeekFrom::Start(7)).unwrap();
    let floor = EXTRA + record_input_storage::<N>().unwrap();
    let scratch = record_scratch::<N>().unwrap();
    let mut w = Work::new(record_work::<N>().unwrap());
    let mut b = Budget::new(&mut w, floor + scratch);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let bytes = f.record(&mut b).unwrap();
    assert_eq!(bytes, [0x41; N]);
    assert_eq!(f.file.stream_position().unwrap(), 7);
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), record_work::<N>().unwrap());
    assert_eq!(b.peak_storage(), floor + scratch);
    assert!(b.work_ledger_identity_v1() == ledger);
    b.reserve_storage(N).unwrap();
    assert_eq!(b.storage(), floor + N);
    b.release_storage(N).unwrap();
}

#[test]
fn executable_checks_full_source_floor_and_exact_quota() {
    let f = Fixture::new(&[0x42; N], EXECUTABLE_MODE);
    for case in 0..4 {
        let floor = file_storage(N).unwrap() - usize::from(case == 1);
        let mut w = Work::new(VALIDATE_WORK - usize::from(case == 2));
        let mut b = Budget::new(&mut w, floor + VALIDATE_SCRATCH - usize::from(case == 3));
        b.reserve_storage(floor).unwrap();
        let result = validate_with_policy(&f.file, f.policy, &mut b);
        assert_eq!(b.storage(), floor);
        match case {
            0 => {
                result.unwrap();
                assert_eq!(b.work(), VALIDATE_WORK);
                assert_eq!(b.peak_storage(), floor + VALIDATE_SCRATCH);
            }
            1 => {
                assert_eq!(
                    result,
                    Err(RootSourceErrorV2::Resource(Resource::Accounting))
                );
                assert_eq!(b.work(), ENTRY_WORK);
            }
            2 => {
                assert!(matches!(
                    result,
                    Err(RootSourceErrorV2::Resource(Resource::Work(_)))
                ));
                assert_eq!(b.failed_work(), Some(VALIDATE_WORK));
                assert_eq!(b.peak_storage(), floor);
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(RootSourceErrorV2::Resource(Resource::Storage(_)))
                ));
                assert_eq!(b.work(), VALIDATE_WORK);
                assert_eq!(b.failed_storage(), Some(floor + VALIDATE_SCRATCH));
            }
        }
    }
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(FILE_STORAGE).unwrap();
    assert_eq!(
        validate_executable(&f.file, N, &mut b),
        Err(RootSourceErrorV2::Resource(Resource::Accounting))
    );
}

#[test]
fn record_and_seed_denials_precede_reading_or_callback() {
    for seed in [false, true] {
        let n = if seed { SEED_BYTES } else { N };
        let mode = if seed { SEED_MODE } else { RECORD_MODE };
        let f = Fixture::new(&vec![0x43; n], mode);
        let work = if seed {
            SEED_WORK
        } else {
            record_work::<N>().unwrap()
        };
        let scratch = if seed {
            SEED_SCRATCH
        } else {
            record_scratch::<N>().unwrap()
        };
        for case in 0..4 {
            let floor = file_storage(n).unwrap() - usize::from(case == 0);
            let work_limit = if case == 3 {
                ENTRY_WORK - 1
            } else {
                work - usize::from(case == 1)
            };
            let mut w = Work::new(work_limit);
            let mut b = Budget::new(&mut w, floor + scratch - usize::from(case == 2));
            b.reserve_storage(floor).unwrap();
            let called = Cell::new(false);
            let hook = || {
                called.set(true);
                Ok(())
            };
            let result = if seed {
                read_seed_with_policy(&f.file, f.policy, &mut b, hook).map(|_| ())
            } else {
                read_record_with_policy::<N>(&f.file, f.policy, &mut b, hook).map(|_| ())
            };
            assert!(!called.get());
            assert_eq!(b.storage(), floor);
            match case {
                0 => {
                    assert_eq!(
                        result,
                        Err(RootSourceErrorV2::Resource(Resource::Accounting))
                    );
                    assert_eq!(b.work(), ENTRY_WORK);
                }
                1 | 3 => {
                    assert!(matches!(
                        result,
                        Err(RootSourceErrorV2::Resource(Resource::Work(_)))
                    ));
                    assert_eq!(b.work(), if case == 3 { 0 } else { ENTRY_WORK });
                    assert_eq!(b.peak_storage(), floor);
                }
                _ => {
                    assert!(matches!(
                        result,
                        Err(RootSourceErrorV2::Resource(Resource::Storage(_)))
                    ));
                    assert_eq!(b.work(), work);
                    assert_eq!(b.failed_storage(), Some(floor + scratch));
                }
            }
        }
    }
}

#[test]
fn successful_reads_preserve_earlier_denials_peak_prefix_and_identity() {
    let f = Fixture::new(&[0x44; N], RECORD_MODE);
    let work = record_work::<N>().unwrap();
    let floor = EXTRA + record_input_storage::<N>().unwrap();
    let peak = floor + 2 * record_scratch::<N>().unwrap();
    let mut w = Work::new(EXTRA + work);
    let mut b = Budget::new(&mut w, peak);
    b.reserve_storage(floor).unwrap();
    b.charge_work(EXTRA).unwrap();
    assert!(b.charge_work(work + 1).is_err());
    assert!(b.reserve_storage(peak - floor + 1).is_err());
    b.reserve_storage(peak - floor).unwrap();
    b.release_storage(peak - floor).unwrap();
    let history = (b.failed_work(), b.failed_storage(), b.peak_storage());
    let ledger = b.work_ledger_identity_v1();
    f.record(&mut b).unwrap();
    assert_eq!(b.work(), EXTRA + work);
    assert_eq!(b.storage(), floor);
    assert_eq!(
        (b.failed_work(), b.failed_storage(), b.peak_storage()),
        history
    );
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn overflow_on_original_account_keeps_history_without_io() {
    let f = Fixture::new(&[0x45; N], RECORD_MODE);
    for storage_overflow in [false, true] {
        let floor = if storage_overflow {
            usize::MAX
        } else {
            file_storage(N).unwrap()
        };
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        b.reserve_storage(floor).unwrap();
        if !storage_overflow {
            b.charge_work(usize::MAX - ENTRY_WORK).unwrap();
        }
        let result =
            read_record_with_policy::<N>(&f.file, f.policy, &mut b, || panic!("unpaid read"));
        assert_eq!(b.storage(), floor);
        if storage_overflow {
            assert!(matches!(
                result,
                Err(RootSourceErrorV2::Resource(Resource::Storage(_)))
            ));
            assert_eq!(b.failed_storage(), Some(usize::MAX));
            assert_eq!(b.work(), record_work::<N>().unwrap());
        } else {
            assert!(matches!(
                result,
                Err(RootSourceErrorV2::Resource(Resource::Work(_)))
            ));
            assert_eq!(b.failed_work(), Some(usize::MAX));
            assert_eq!(b.work(), usize::MAX);
        }
    }
}

#[test]
fn genuine_metadata_and_access_policy_refusals() {
    let f = Fixture::new(&[0x46; N], RECORD_MODE);
    snapshot(&f.file, f.policy).unwrap();
    for policy in [
        Policy {
            uid: f.policy.uid ^ 1,
            ..f.policy
        },
        Policy {
            gid: f.policy.gid ^ 1,
            ..f.policy
        },
        Policy {
            mode: EXECUTABLE_MODE,
            ..f.policy
        },
        Policy {
            length: N - 1,
            ..f.policy
        },
        Policy {
            length: N + 1,
            ..f.policy
        },
    ] {
        assert_eq!(snapshot(&f.file, policy), Err(RootSourceErrorV2::Metadata));
    }
    assert_eq!(
        snapshot(&f.writer, f.policy),
        Err(RootSourceErrorV2::Descriptor)
    );
    rustix::io::fcntl_setfd(&f.file, FdFlags::empty()).unwrap();
    assert_eq!(
        snapshot(&f.file, f.policy),
        Err(RootSourceErrorV2::Descriptor)
    );
    rustix::io::fcntl_setfd(&f.file, FdFlags::CLOEXEC).unwrap();
    let flags = rustix::fs::fcntl_getfl(&f.file).unwrap();
    rustix::fs::fcntl_setfl(&f.file, flags | OFlags::APPEND).unwrap();
    assert_eq!(
        snapshot(&f.file, f.policy),
        Err(RootSourceErrorV2::Descriptor)
    );
    rustix::fs::fcntl_setfl(&f.file, flags).unwrap();
    let directory = File::open(f.dir.path()).unwrap();
    assert_eq!(
        snapshot(&directory, f.policy),
        Err(RootSourceErrorV2::Metadata)
    );
    let path_only = File::from(
        rustix::fs::open(
            f.dir.path().join("source"),
            OFlags::PATH | OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    );
    assert_eq!(
        snapshot(&path_only, f.policy),
        Err(RootSourceErrorV2::Descriptor)
    );
    fs::hard_link(f.dir.path().join("source"), f.dir.path().join("alias")).unwrap();
    assert_eq!(
        snapshot(&f.file, f.policy),
        Err(RootSourceErrorV2::Metadata)
    );
    fs::remove_file(f.dir.path().join("alias")).unwrap();
    fs::set_permissions(
        f.dir.path().join("source"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_eq!(
        snapshot(&f.file, f.policy),
        Err(RootSourceErrorV2::Metadata)
    );
    fs::set_permissions(
        f.dir.path().join("source"),
        fs::Permissions::from_mode(RECORD_MODE),
    )
    .unwrap();
    fs::remove_file(f.dir.path().join("source")).unwrap();
    assert_eq!(
        snapshot(&f.file, f.policy),
        Err(RootSourceErrorV2::Metadata)
    );
}

#[test]
fn production_wrappers_never_infer_owner_from_the_process_or_file() {
    for (mode, n) in [
        (EXECUTABLE_MODE, N),
        (RECORD_MODE, N),
        (SEED_MODE, SEED_BYTES),
    ] {
        let f = Fixture::new(&vec![0x47; n], mode);
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        b.reserve_storage(file_storage(n).unwrap()).unwrap();
        let result = match mode {
            EXECUTABLE_MODE => validate_executable(&f.file, n, &mut b),
            RECORD_MODE => read_record::<N>(&f.file, &mut b).map(|_| ()),
            _ => read_seed(&f.file, &mut b).map(|_| ()),
        };
        if f.policy.uid == 0 && f.policy.gid == 0 {
            result.unwrap();
        } else {
            assert_eq!(result, Err(RootSourceErrorV2::Metadata));
        }
    }
}

#[test]
fn fixed_flag_predicate_rejects_every_forbidden_flag_and_access_mode() {
    let f = Fixture::new(&[0x48; N], RECORD_MODE);
    let before = snapshot(&f.file, f.policy).unwrap();
    for flag in [
        OFlags::APPEND,
        OFlags::ASYNC,
        OFlags::DIRECT,
        OFlags::PATH,
        OFlags::WRONLY,
        OFlags::RDWR,
    ] {
        let mut changed = before;
        changed.status |= flag;
        assert_eq!(
            changed.validate(f.policy),
            Err(RootSourceErrorV2::Descriptor)
        );
    }
    for mode in [0o400, 0o644, 0o2444, 0o4444] {
        let mut changed = before;
        changed.mode = (changed.mode & !0o7777) | mode;
        assert_eq!(changed.validate(f.policy), Err(RootSourceErrorV2::Metadata));
    }
}

#[test]
fn snapshot_equality_covers_every_stable_field_and_ignores_atime() {
    let f = Fixture::new(&[0x49; N], RECORD_MODE);
    let before = snapshot(&f.file, f.policy).unwrap();
    macro_rules! changed {
        ($field:ident) => {{
            let mut after = before;
            after.$field ^= 1;
            assert_eq!(unchanged(before, after), Err(RootSourceErrorV2::Changed));
        }};
    }
    changed!(device);
    changed!(inode);
    changed!(mode);
    changed!(uid);
    changed!(gid);
    changed!(links);
    changed!(byte_len);
    changed!(rdev);
    changed!(block_size);
    changed!(blocks);
    changed!(modified_seconds);
    changed!(modified_nanoseconds);
    changed!(changed_seconds);
    changed!(changed_nanoseconds);
    let mut flags = before;
    flags.status |= OFlags::NONBLOCK;
    assert_eq!(unchanged(before, flags), Err(RootSourceErrorV2::Changed));
    flags = before;
    flags.descriptor = FdFlags::empty();
    assert_eq!(unchanged(before, flags), Err(RootSourceErrorV2::Changed));
    let mut stat = rustix::fs::fstat(&f.file).unwrap();
    stat.st_atime ^= 1;
    stat.st_atime_nsec ^= 1;
    assert_eq!(
        Snapshot::from_stat(stat, before.descriptor, before.status).unwrap(),
        before
    );
    let mut stat = rustix::fs::fstat(&f.file).unwrap();
    stat.st_size = -1;
    assert_eq!(
        Snapshot::from_stat(stat, before.descriptor, before.status),
        Err(RootSourceErrorV2::Metadata)
    );
}

#[test]
fn complete_byte_comparison_rejects_changes_even_with_equal_metadata() {
    let first = [0x4a; SEED_BYTES];
    equal_bytes(&first, &first).unwrap();
    for index in 0..SEED_BYTES {
        let mut second = first;
        second[index] ^= 1;
        assert_eq!(
            equal_bytes(&first, &second),
            Err(RootSourceErrorV2::Changed)
        );
    }
    assert_eq!(
        equal_bytes(&first, &first[..SEED_BYTES - 1]),
        Err(RootSourceErrorV2::Changed)
    );
}

#[test]
fn attribute_queries_fail_closed_without_retrying_or_allocating() {
    for result in [Ok(0), Ok(1), Err(Errno::RANGE)] {
        assert_eq!(
            attribute_absent(result),
            Err(RootSourceErrorV2::ForbiddenAttribute)
        );
    }
    for errno in [Errno::NODATA, Errno::OPNOTSUPP] {
        attribute_absent(Err(errno)).unwrap();
    }
    for errno in [Errno::INTR, Errno::ACCES, Errno::IO] {
        assert!(
            matches!(attribute_absent(Err(errno)), Err(RootSourceErrorV2::Io { errno: e, .. }) if e == errno.raw_os_error())
        );
    }
}

#[test]
fn genuine_extended_acl_is_rejected_when_supported() {
    let f = Fixture::new(&[0x4a; SEED_BYTES], SEED_MODE);
    // Linux ACL xattr v2: an extra named user with no access leaves mode 0400.
    let mut acl = Vec::from(2u32.to_le_bytes());
    for (tag, perm, id) in [
        (1u16, 4u16, u32::MAX),
        (2, 0, f.policy.uid ^ 1),
        (4, 0, u32::MAX),
        (16, 0, u32::MAX),
        (32, 0, u32::MAX),
    ] {
        acl.extend_from_slice(&tag.to_le_bytes());
        acl.extend_from_slice(&perm.to_le_bytes());
        acl.extend_from_slice(&id.to_le_bytes());
    }
    match rustix::fs::fsetxattr(
        &f.file,
        "system.posix_acl_access",
        &acl,
        rustix::fs::XattrFlags::empty(),
    ) {
        Ok(()) => assert_eq!(
            snapshot(&f.file, f.policy),
            Err(RootSourceErrorV2::ForbiddenAttribute)
        ),
        // Some test filesystems or sandboxes do not support setting ACLs.
        Err(Errno::OPNOTSUPP | Errno::PERM | Errno::ACCES) => {}
        Err(error) => panic!("unexpected ACL setup refusal: {error}"),
    }
}

#[test]
fn single_attempt_transfer_rejects_short_eof_and_interrupts() {
    for result in [Ok(N), Ok(N - 1), Ok(0), Err(Errno::INTR), Err(Errno::IO)] {
        let calls = Cell::new(0);
        let actual = exact_read(N, || {
            calls.set(calls.get() + 1);
            result
        });
        assert_eq!(calls.get(), 1);
        match result {
            Ok(n) if n == N => actual.unwrap(),
            Ok(_) => assert_eq!(actual, Err(RootSourceErrorV2::ShortRead)),
            Err(errno) => assert!(
                matches!(actual, Err(RootSourceErrorV2::Io { errno: e, .. }) if e == errno.raw_os_error())
            ),
        }
    }
    for n in [N - 1, N, N + 1] {
        let f = Fixture::new(&vec![0x4b; n], RECORD_MODE);
        let mut bytes = [0; N];
        let result = read_copy(&f.file, &mut bytes);
        match n {
            n if n < N => assert_eq!(result, Err(RootSourceErrorV2::ShortRead)),
            N => result.unwrap(),
            _ => assert_eq!(result, Err(RootSourceErrorV2::TrailingBytes)),
        }
    }
}

#[test]
fn changes_between_real_reads_refuse_and_restore_storage() {
    for seed in [false, true] {
        for change in 0..6 {
            let n = if seed { SEED_BYTES } else { N };
            let mode = if seed { SEED_MODE } else { RECORD_MODE };
            let f = Fixture::new(&vec![0x4c; n], mode);
            let floor = EXTRA + file_storage(n).unwrap();
            let mut w = Work::new(LIMIT);
            let mut b = Budget::new(&mut w, LIMIT);
            b.reserve_storage(floor).unwrap();
            let hook = || {
                match change {
                    0 => f.overwrite(&vec![0x4d; n]),
                    1 => f.writer.set_len((n - 1) as u64).unwrap(),
                    2 => f.writer.set_len((n + 1) as u64).unwrap(),
                    3 => fs::set_permissions(
                        f.dir.path().join("source"),
                        fs::Permissions::from_mode(0o600),
                    )
                    .unwrap(),
                    4 => {
                        let flags = rustix::fs::fcntl_getfl(&f.file).unwrap();
                        rustix::fs::fcntl_setfl(&f.file, flags | OFlags::NONBLOCK).unwrap();
                    }
                    _ => {
                        // Deterministic mtime-only change; the file bytes remain equal.
                        f.writer
                            .set_times(fs::FileTimes::new().set_modified(
                                std::time::UNIX_EPOCH + std::time::Duration::from_secs(123),
                            ))
                            .unwrap();
                    }
                }
                Ok(())
            };
            let result = if seed {
                read_seed_with_policy(&f.file, f.policy, &mut b, hook).map(|_| ())
            } else {
                read_record_with_policy::<N>(&f.file, f.policy, &mut b, hook).map(|_| ())
            };
            let expected = match change {
                1 => RootSourceErrorV2::ShortRead,
                2 => RootSourceErrorV2::TrailingBytes,
                3 => RootSourceErrorV2::Metadata,
                _ => RootSourceErrorV2::Changed,
            };
            assert_eq!(result, Err(expected));
            assert_eq!(b.storage(), floor);
            assert_eq!(
                b.work(),
                if seed {
                    SEED_WORK
                } else {
                    record_work::<N>().unwrap()
                }
            );
        }
    }
}

#[test]
fn seed_is_guarded_with_explicit_caller_charge_and_original_constructor_error() {
    let f = Fixture::new(&[0x5a; SEED_BYTES], SEED_MODE);
    let floor = EXTRA + file_storage(SEED_BYTES).unwrap();
    let mut w = Work::new(SEED_WORK);
    let mut b = Budget::new(&mut w, floor + SEED_SCRATCH);
    b.reserve_storage(floor).unwrap();
    let (mut seed, charge): (Zeroizing<[u8; SEED_BYTES]>, RootSourceStorage) =
        f.seed(&mut b).unwrap();
    assert!(seed.iter().all(|byte| *byte == 0x5a));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.peak_storage(), floor + SEED_SCRATCH);
    assert_eq!(b.work(), SEED_WORK);
    assert_eq!(charge.additional_storage(), SEED_STORAGE);
    b.reserve_storage(charge.additional_storage()).unwrap();
    // This represents a caller's error type, not a fabricated native capability.
    struct ConstructorError(u8);
    let constructor =
        |_: &mut [u8; SEED_BYTES],
         _: &mut Budget<'_>|
         -> std::result::Result<(), ConstructorError> { Err(ConstructorError(7)) };
    let error = constructor(&mut seed, &mut b).err().unwrap();
    assert_eq!(error.0, 7);
    // Exercise an explicit zeroization while the guard is still live. Never
    // inspect dropped stack memory to try to observe a destructor's writes.
    zeroize::Zeroize::zeroize(&mut seed);
    assert!(seed.iter().all(|byte| *byte == 0));
    drop(seed);
    b.release_storage(charge.additional_storage()).unwrap();
    assert_eq!(b.storage(), floor);
}

#[test]
fn seed_failure_and_unwind_retire_guards_and_restore_original_account() {
    let f = Fixture::new(&[0x5b; SEED_BYTES], SEED_MODE);
    let floor = EXTRA + file_storage(SEED_BYTES).unwrap();
    for unwind in [false, true] {
        let mut w = Work::new(SEED_WORK);
        let mut b = Budget::new(&mut w, floor + SEED_SCRATCH);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let observed = catch_unwind(AssertUnwindSafe(|| {
            read_seed_with_policy(&f.file, f.policy, &mut b, || {
                if unwind {
                    panic!("after first guarded seed read");
                }
                Err(RootSourceErrorV2::Changed)
            })
            .map(|_| ())
        }));
        if unwind {
            assert!(observed.is_err());
        } else {
            assert_eq!(observed.unwrap(), Err(RootSourceErrorV2::Changed));
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), SEED_WORK);
        assert_eq!(b.peak_storage(), floor + SEED_SCRATCH);
        assert!(b.work_ledger_identity_v1() == ledger);
        // The caller still owns the exact source; no clone or fd transfer occurred.
        snapshot(&f.file, f.policy).unwrap();
    }
}

#[test]
fn caller_unwind_after_reserving_seed_restores_outer_storage() {
    let f = Fixture::new(&[0x5c; SEED_BYTES], SEED_MODE);
    let floor = EXTRA + file_storage(SEED_BYTES).unwrap();
    let mut w = Work::new(SEED_WORK);
    let mut b = Budget::new(&mut w, floor + EXTRA + SEED_SCRATCH);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<()> = b.with_prepaid_scope(floor, 0, 0, EXTRA, |b| {
            let (seed, charge) = f.seed(b)?;
            b.reserve_storage(charge.additional_storage())?;
            assert!(seed.iter().all(|byte| *byte == 0x5c));
            panic!("caller after guarded seed return");
        });
    }));
    assert!(outcome.is_err());
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), SEED_WORK);
    assert_eq!(b.peak_storage(), floor + EXTRA + SEED_SCRATCH);
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn seed_errors_have_no_owned_diagnostics_or_secret_bytes() {
    assert!(!std::mem::needs_drop::<RootSourceErrorV2>());
    assert!(std::mem::needs_drop::<Zeroizing<[u8; SEED_BYTES]>>());
    let f = Fixture::new(&[b'Z'; SEED_BYTES], SEED_MODE);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(file_storage(SEED_BYTES).unwrap())
        .unwrap();
    let error = read_seed_with_policy(&f.file, f.policy, &mut b, || {
        f.overwrite(&[b'Y'; SEED_BYTES]);
        Ok(())
    })
    .err()
    .unwrap();
    let diagnostic = format!("{error}: {error:?}");
    assert_eq!(error, RootSourceErrorV2::Changed);
    assert!(!diagnostic.contains("ZZZZ"));
    assert!(!diagnostic.contains("YYYY"));
    assert!(!diagnostic.contains(&f.dir.path().display().to_string()));
}
