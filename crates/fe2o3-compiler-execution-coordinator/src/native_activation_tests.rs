//! Pure bounded inputs and private syscall-result seams only. No test mutates
//! process environ, blocks signals, publishes readiness, or adopts descriptors.
//! Real capture/mask tests belong in isolated child processes owned by integration.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

const EXTRA: usize = 29;
const LIMIT: usize = 1 << 30;
const NAMES: &[u8] = b"runtime-root:supervisor-root:anchor-root:supervisor:launcher:issuer:anchor-helper:anchor-daemon:supervisor-deployment:issuer-policy:anchor-deployment:anchor-provisioning:issuer-key-seed:anchor-key-seed";

fn plan() -> Activation {
    parse_activation(1234, b"1234", b"14", NAMES, b"/run/systemd/notify").unwrap()
}

fn env() -> Vec<Vec<u8>> {
    vec![
        b"LISTEN_PID=1234\0".to_vec(),
        b"LISTEN_FDS=14\0".to_vec(),
        [b"LISTEN_FDNAMES=".as_slice(), NAMES, b"\0"].concat(),
        b"NOTIFY_SOCKET=/run/systemd/notify\0".to_vec(),
    ]
}

#[allow(unsafe_code)]
fn snapshot(entries: &[Vec<u8>]) -> Result<EnvironmentSnapshot> {
    // Establish precisely the finite readability precondition, including fixtures
    // intentionally lacking a terminator within the entry byte bound.
    assert!(
        entries
            .iter()
            .all(|e| e.contains(&0) || e.len() >= ENV_ENTRY_MAX_BYTES)
    );
    let mut pointers: Vec<*const libc::c_char> =
        entries.iter().map(|e| e.as_ptr().cast()).collect();
    pointers.push(std::ptr::null());
    // SAFETY: pointer table has a trailing null; every bounded string read is valid,
    // and the borrowed vectors remain stable throughout this call.
    unsafe { snapshot_environment(pointers.as_ptr()) }
}

#[allow(unsafe_code)]
fn signal_state_fixture() -> TerminationSignals {
    // Pure state fixture only; never passed to real wait_interval/restore. It
    // does not claim an installed signal mask or any admitted deployment owner.
    TerminationSignals {
        // SAFETY: libc sigset storage permits all-zero bytes; no libc call uses it.
        set: unsafe { std::mem::zeroed() },
        previous: unsafe { std::mem::zeroed() },
        main_pid: 1234,
        active: true,
        thread_affine: PhantomData,
    }
}

#[test]
fn constants_cover_full_returned_owners_and_fixed_scratch() {
    assert_eq!(ACTIVATION_STORAGE, size_of::<(Activation, Storage)>());
    assert_eq!(SIGNALS_STORAGE, size_of::<(TerminationSignals, Storage)>());
    assert!(
        CAPTURE_SCRATCH >= size_of::<EnvironmentSnapshot>() + ARGV0_MAX_BYTES + ACTIVATION_STORAGE
    );
    assert!(INSTALL_SCRATCH >= SIGNALS_STORAGE);
    assert_eq!(WAIT_INTERVAL_SECONDS, 1);
    for work in [
        CAPTURE_WORK,
        PUBLISH_WORK,
        INSTALL_WORK,
        WAIT_WORK,
        RESTORE_WORK,
    ] {
        assert!(work >= ENTRY_WORK);
    }
}

#[test]
fn descriptor_roles_match_existing_fourteen_role_entrypoint() {
    let expected: Vec<&[u8]> = NAMES.split(|&b| b == b':').collect();
    assert_eq!(DESCRIPTOR_ROLES.as_slice(), expected.as_slice());
    let existing = include_str!("entrypoint.rs");
    assert!(existing.contains(std::str::from_utf8(NAMES).unwrap()));
    assert_eq!(
        crate::native_inherited::DESCRIPTORS,
        [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    );
    validate_roles(NAMES).unwrap();
    for index in 0..DESCRIPTOR_ROLES.len() {
        let mut wrong = expected.clone();
        wrong[index] = b"substituted";
        let joined = wrong.join(&b':');
        assert!(validate_roles(&joined).is_err());
    }
    for value in [b"runtime-root".as_slice(), &NAMES[..NAMES.len() - 1], b""] {
        assert!(validate_roles(value).is_err());
    }
    assert!(validate_roles(&[NAMES, b":extra"].concat()).is_err());
    assert!(validate_roles(&[NAMES, b":"].concat()).is_err());
    assert!(validate_roles(&vec![b'x'; 2 * ENV_ENTRY_MAX_BYTES]).is_err());
}

#[test]
fn pid_is_exact_positive_decimal_without_aliases() {
    for pid in [1, 10, 1234, i32::MAX] {
        let text = pid.to_string();
        assert_eq!(parse_pid(text.as_bytes()).unwrap(), pid);
        assert!(parse_activation(pid, text.as_bytes(), b"14", NAMES, b"@notify").is_ok());
    }
    for value in [
        b"".as_slice(),
        b"0",
        b"00",
        b"01234",
        b"+1234",
        b"-1234",
        b" 1234",
        b"1234 ",
        b"1234\n",
        b"1234\0",
        b"1_234",
        b"2147483648",
        b"99999999999",
        b"\xff",
    ] {
        assert!(parse_pid(value).is_err());
    }
    assert!(parse_activation(1235, b"1234", b"14", NAMES, b"@notify").is_err());
    assert!(parse_activation(0, b"0", b"14", NAMES, b"@notify").is_err());
    assert!(parse_activation(-1, b"1", b"14", NAMES, b"@notify").is_err());
    for count in [
        b"".as_slice(),
        b"13",
        b"15",
        b"014",
        b"+14",
        b"14 ",
        b"14\0",
    ] {
        assert!(parse_activation(1234, b"1234", count, NAMES, b"@notify").is_err());
    }
}

#[test]
fn captured_main_identity_rejects_children_other_threads_and_invalid_pids() {
    check_main_identity(1234, 1234, 1234).unwrap();
    for (captured, current, thread) in [
        (1234, 1235, 1235),
        (1234, 1234, 1235),
        (1234, 1235, 1234),
        (0, 0, 0),
        (-1, -1, -1),
    ] {
        assert!(check_main_identity(captured, current, thread).is_err());
    }
}

#[test]
fn readiness_targets_are_bounded_absolute_or_abstract_bytes() {
    for (value, abstract_namespace, length) in [
        (b"/run/systemd/notify".as_slice(), false, 19),
        (b"@notify".as_slice(), true, 6),
        (b"/".as_slice(), false, 1),
        (b"@\xff".as_slice(), true, 1),
    ] {
        let p = parse_activation(1234, b"1234", b"14", NAMES, value).unwrap();
        assert_eq!(p.abstract_namespace, abstract_namespace);
        assert_eq!(p.target_length, length);
        let bytes = if abstract_namespace {
            &value[1..]
        } else {
            value
        };
        assert_eq!(&p.target[..length], bytes);
        assert!(!p.publish_attempted);
    }
    for abstract_namespace in [false, true] {
        let mut exact = vec![b'x'; NOTIFY_PATH_MAX_BYTES + usize::from(abstract_namespace)];
        exact[0] = if abstract_namespace { b'@' } else { b'/' };
        assert!(parse_activation(1234, b"1234", b"14", NAMES, &exact).is_ok());
        exact.push(b'x');
        assert!(parse_activation(1234, b"1234", b"14", NAMES, &exact).is_err());
    }
    for value in [b"".as_slice(), b"@", b"relative", b"/x\0y", b"@x\0y"] {
        assert!(parse_activation(1234, b"1234", b"14", NAMES, value).is_err());
    }
}

#[test]
fn argv0_requires_one_nonempty_bounded_nul_terminated_argument() {
    for value in [
        b"coordinator\0".as_slice(),
        b"/bin/coordinator\0",
        b"\xff\0",
    ] {
        validate_cmdline(value).unwrap();
    }
    let mut exact = vec![b'x'; ARGV0_MAX_BYTES];
    exact[ARGV0_MAX_BYTES - 1] = 0;
    validate_cmdline(&exact).unwrap();
    exact.insert(0, b'x');
    assert!(validate_cmdline(&exact).is_err());
    for value in [
        b"".as_slice(),
        b"\0",
        b"name",
        b"name\0extra\0",
        b"name\0\0",
        b"name\0extra",
    ] {
        assert!(validate_cmdline(value).is_err());
    }
    assert!(validate_cmdline(&vec![0; 16 * ARGV0_MAX_BYTES]).is_err());
}

#[test]
fn command_line_read_is_one_pread_and_one_eof_probe() {
    let calls = Cell::new(0);
    read_cmdline(|buffer, offset| {
        calls.set(calls.get() + 1);
        match calls.get() {
            1 => {
                assert_eq!(offset, 0);
                assert_eq!(buffer.len(), ARGV0_MAX_BYTES + 1);
                buffer[..5].copy_from_slice(b"main\0");
                Ok(5)
            }
            2 => {
                assert_eq!((offset, buffer.len()), (5, 1));
                Ok(0)
            }
            _ => panic!("unbounded command line retry"),
        }
    })
    .unwrap();
    assert_eq!(calls.get(), 2);
    for trailing in [1, 2, usize::MAX] {
        let calls = Cell::new(0);
        assert!(
            read_cmdline(|buffer, _| {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    buffer[..2].copy_from_slice(b"x\0");
                    Ok(2)
                } else {
                    Ok(trailing)
                }
            })
            .is_err()
        );
        assert_eq!(calls.get(), 2);
    }
}

#[test]
fn command_line_short_invalid_and_interrupted_reads_never_retry() {
    for length in [0, 1, ARGV0_MAX_BYTES + 1, usize::MAX] {
        let calls = Cell::new(0);
        assert!(
            read_cmdline(|_, _| {
                calls.set(calls.get() + 1);
                Ok(length)
            })
            .is_err()
        );
        assert_eq!(calls.get(), 1);
    }
    for fail_on in [1, 2] {
        let calls = Cell::new(0);
        let error = read_cmdline(|buffer, _| {
            calls.set(calls.get() + 1);
            if calls.get() == fail_on {
                Err(Errno::INTR)
            } else {
                buffer[..2].copy_from_slice(b"x\0");
                Ok(2)
            }
        })
        .err()
        .unwrap();
        assert!(matches!(
            error,
            Failure::Io {
                source: Errno::INTR,
                ..
            }
        ));
        assert_eq!(calls.get(), fail_on);
    }
    let calls = Cell::new(0);
    assert!(
        read_cmdline(|buffer, _| {
            calls.set(calls.get() + 1);
            buffer[..4].copy_from_slice(b"name");
            Ok(4)
        })
        .is_err()
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn snapshot_and_parser_accept_reordering_and_ignore_unrelated_variables() {
    let mut entries = env();
    entries.reverse();
    entries.push(b"OTHER=value=with=equals\0".to_vec());
    entries.push(b"OTHER=duplicate-unrelated\0".to_vec());
    let snapshot = snapshot(&entries).unwrap();
    assert_eq!(snapshot.count, entries.len());
    assert_eq!(snapshot.used, entries.iter().map(Vec::len).sum::<usize>());
    for (index, entry) in entries.iter().enumerate() {
        assert_eq!(snapshot.entry(index), &entry[..entry.len() - 1]);
    }
    assert_eq!(parse_environment(1234, &snapshot).unwrap().main_pid, 1234);
}

#[test]
fn snapshot_rejects_entry_and_total_byte_overflow_before_copying_more() {
    let mut entry = vec![b'x'; ENV_ENTRY_MAX_BYTES];
    entry[ENV_ENTRY_MAX_BYTES - 1] = 0;
    assert_eq!(
        snapshot(&[entry.clone()]).unwrap().used,
        ENV_ENTRY_MAX_BYTES
    );
    let mut oversized = entry.clone();
    oversized.insert(0, b'x');
    assert!(snapshot(&[oversized]).is_err());
    assert!(snapshot(&[vec![b'x'; ENV_ENTRY_MAX_BYTES]]).is_err());
    assert!(snapshot(&[vec![b'x'; 32 * ENV_ENTRY_MAX_BYTES]]).is_err());
    assert_eq!(ENV_TOTAL_MAX_BYTES % ENV_ENTRY_MAX_BYTES, 0);
    let mut entries = vec![entry; ENV_TOTAL_MAX_BYTES / ENV_ENTRY_MAX_BYTES];
    assert_eq!(snapshot(&entries).unwrap().used, ENV_TOTAL_MAX_BYTES);
    entries.push(b"A=\0".to_vec());
    assert!(snapshot(&entries).is_err());
}

#[test]
#[allow(unsafe_code)]
fn snapshot_caps_pointer_count_and_handles_empty_environment() {
    let mut entries = vec![b"A=\0".to_vec(); ENV_MAX_ENTRIES];
    assert_eq!(snapshot(&entries).unwrap().count, ENV_MAX_ENTRIES);
    entries.push(b"A=\0".to_vec());
    assert!(snapshot(&entries).is_err());
    let empty = snapshot(&[]).unwrap();
    assert_eq!((empty.count, empty.used), (0, 0));
    assert!(parse_environment(1234, &empty).is_err());
    // SAFETY: a null environment pointer is expressly handled without dereferencing it.
    let null = unsafe { snapshot_environment(std::ptr::null()) }.unwrap();
    assert_eq!((null.count, null.used), (0, 0));
}

#[test]
fn every_activation_duplicate_and_missing_variable_is_refused() {
    for slot in 0..4 {
        for at_front in [false, true] {
            let mut entries = env();
            let duplicate = entries[slot].clone();
            if at_front {
                entries.insert(0, duplicate);
            } else {
                entries.push(duplicate);
            }
            assert!(matches!(
                parse_environment(1234, &snapshot(&entries).unwrap()),
                Err(Failure::Invalid {
                    reason: "duplicate activation variable",
                    ..
                })
            ));
        }
        let mut entries = env();
        entries.remove(slot);
        assert!(parse_environment(1234, &snapshot(&entries).unwrap()).is_err());
    }
    for malformed in [b"\0".as_slice(), b"no-equals\0", b"=empty-key\0"] {
        let mut entries = env();
        entries.push(malformed.to_vec());
        assert!(parse_environment(1234, &snapshot(&entries).unwrap()).is_err());
    }
}

#[test]
fn invalid_environment_errors_do_not_retain_input_bytes() {
    let mut entries = env();
    entries[0] = b"LISTEN_PID=private-marker-not-a-PID\0".to_vec();
    let error = parse_environment(1234, &snapshot(&entries).unwrap())
        .err()
        .unwrap();
    assert!(!format!("{error:?} {error}").contains("private-marker"));
}

#[test]
fn capture_exact_quota_returns_unreserved_owner_on_original_account() {
    let mut work = Work::new(CAPTURE_WORK);
    let mut b = Budget::new(&mut work, EXTRA + CAPTURE_SCRATCH);
    b.reserve_storage(EXTRA).unwrap();
    let identity = b.work_ledger_identity_v1();
    let stage = Cell::new(0);
    let (owner, storage) = capture_with(
        &mut b,
        || {
            stage.set(1);
            Ok(plan())
        },
        || {
            assert_eq!(stage.get(), 1);
            stage.set(2);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(stage.get(), 2);
    assert_eq!(storage.additional_storage(), ACTIVATION_STORAGE);
    assert_eq!(b.storage(), EXTRA);
    assert_eq!(b.peak_storage(), EXTRA + CAPTURE_SCRATCH);
    assert_eq!(b.work(), CAPTURE_WORK);
    assert!(b.work_ledger_identity_v1() == identity);
    b.reserve_storage(storage.additional_storage()).unwrap();
    drop(owner);
    b.release_storage(storage.additional_storage()).unwrap();
    assert_eq!(b.storage(), EXTRA);
}

#[test]
fn capture_work_or_scratch_one_short_never_probes_or_clears() {
    for short_work in [false, true] {
        let mut work = Work::new(CAPTURE_WORK - usize::from(short_work));
        let mut b = Budget::new(
            &mut work,
            EXTRA + CAPTURE_SCRATCH - usize::from(!short_work),
        );
        b.reserve_storage(EXTRA).unwrap();
        let identity = b.work_ledger_identity_v1();
        let result = capture_with(
            &mut b,
            || panic!("probe after denial"),
            || panic!("clear after denial"),
        );
        if short_work {
            assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
            assert_eq!(b.failed_work(), Some(CAPTURE_WORK));
            assert_eq!(b.work(), ENTRY_WORK);
        } else {
            assert!(matches!(
                result,
                Err(Failure::Resource(Resource::Storage(_)))
            ));
            assert_eq!(b.failed_storage(), Some(EXTRA + CAPTURE_SCRATCH));
            assert_eq!(b.work(), CAPTURE_WORK);
        }
        assert_eq!(b.storage(), EXTRA);
        assert!(b.work_ledger_identity_v1() == identity);
    }
}

#[test]
fn capture_preserves_prior_denials_and_exact_error_without_clearing_on_invalid_plan() {
    let mut work = Work::new(CAPTURE_WORK);
    let mut b = Budget::new(&mut work, EXTRA + CAPTURE_SCRATCH);
    b.reserve_storage(EXTRA).unwrap();
    assert!(b.charge_work(CAPTURE_WORK + 1).is_err());
    assert!(b.reserve_storage(CAPTURE_SCRATCH + 1).is_err());
    let error = capture_with(
        &mut b,
        || Err(io("probe fixture", Errno::ACCESS)),
        || panic!("clear after failed validation"),
    )
    .err()
    .unwrap();
    assert!(matches!(
        error,
        Failure::Io {
            operation: "probe fixture",
            source: Errno::ACCESS
        }
    ));
    assert_eq!(b.storage(), EXTRA);
    assert_eq!(b.work(), CAPTURE_WORK);
    assert_eq!(b.failed_work(), Some(CAPTURE_WORK + 1));
    assert_eq!(b.failed_storage(), Some(EXTRA + CAPTURE_SCRATCH + 1));
}

#[test]
fn capture_clear_failure_and_unwind_restore_only_storage() {
    for unwind in [false, true] {
        let mut work = Work::new(CAPTURE_WORK);
        let mut b = Budget::new(&mut work, EXTRA + CAPTURE_SCRATCH);
        b.reserve_storage(EXTRA).unwrap();
        let identity = b.work_ledger_identity_v1();
        let calls = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            capture_with(
                &mut b,
                || Ok(plan()),
                || {
                    calls.set(calls.get() + 1);
                    if unwind {
                        panic!("clear fixture unwind");
                    }
                    Err(io("clear fixture", Errno::IO))
                },
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(Failure::Io {
                    operation: "clear fixture",
                    source: Errno::IO
                })
            ));
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(b.storage(), EXTRA);
        assert_eq!(b.work(), CAPTURE_WORK);
        assert_eq!(b.peak_storage(), EXTRA + CAPTURE_SCRATCH);
        assert!(b.work_ledger_identity_v1() == identity);
    }
}

#[test]
fn readiness_requires_full_owner_floor_before_side_effects() {
    let mut owner = plan();
    let mut work = Work::new(PUBLISH_WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(ACTIVATION_STORAGE - 1).unwrap();
    assert!(matches!(
        publish_with(&mut owner, &mut b, |_| panic!("unpaid owner")),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert!(!owner.publish_attempted);
    assert_eq!(b.work(), ENTRY_WORK);
    assert_eq!(b.storage(), ACTIVATION_STORAGE - 1);
}

#[test]
fn readiness_exact_quota_and_one_short_preserve_attempt_state() {
    for (work_limit, scratch_limit, success) in [
        (PUBLISH_WORK, PUBLISH_SCRATCH, true),
        (PUBLISH_WORK - 1, PUBLISH_SCRATCH, false),
        (PUBLISH_WORK, PUBLISH_SCRATCH - 1, false),
    ] {
        let mut owner = plan();
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, ACTIVATION_STORAGE + scratch_limit);
        b.reserve_storage(ACTIVATION_STORAGE).unwrap();
        let called = Cell::new(false);
        let result = publish_with(&mut owner, &mut b, |_| {
            called.set(true);
            exact_ready_count(7)
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(called.get(), success);
        assert_eq!(owner.publish_attempted, success);
        assert_eq!(b.storage(), ACTIVATION_STORAGE);
        if success {
            assert_eq!(b.peak_storage(), ACTIVATION_STORAGE + PUBLISH_SCRATCH);
        }
    }
}

#[test]
fn readiness_attempt_is_consumed_on_success_failure_or_unwind() {
    for outcome in 0..3 {
        let mut owner = plan();
        let mut work = Work::new(2 * PUBLISH_WORK);
        let mut b = Budget::new(&mut work, ACTIVATION_STORAGE + PUBLISH_SCRATCH);
        b.reserve_storage(ACTIVATION_STORAGE).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            publish_with(&mut owner, &mut b, |_| match outcome {
                0 => Ok(()),
                1 => Err(io("send fixture", Errno::AGAIN)),
                _ => panic!("send fixture unwind"),
            })
        }));
        assert_eq!(result.is_err(), outcome == 2);
        assert!(owner.publish_attempted);
        assert!(publish_with(&mut owner, &mut b, |_| panic!("readiness retry")).is_err());
        assert_eq!(b.storage(), ACTIVATION_STORAGE);
        assert_eq!(b.work(), 2 * PUBLISH_WORK);
    }
    for size in [0, 1, 6, 8, usize::MAX] {
        assert!(exact_ready_count(size).is_err());
    }
    exact_ready_count(7).unwrap();
}

#[test]
fn signal_install_exact_quota_returns_unreserved_charge_and_one_short_never_installs() {
    for (work_limit, scratch_limit, success) in [
        (INSTALL_WORK, INSTALL_SCRATCH, true),
        (INSTALL_WORK - 1, INSTALL_SCRATCH, false),
        (INSTALL_WORK, INSTALL_SCRATCH - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, EXTRA + scratch_limit);
        b.reserve_storage(EXTRA).unwrap();
        let identity = b.work_ledger_identity_v1();
        let called = Cell::new(false);
        let result = install_with(&mut b, || {
            called.set(true);
            Ok(signal_state_fixture())
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(called.get(), success);
        if success {
            let (owner, storage) = result.unwrap();
            assert!(owner.active);
            assert_eq!(storage.additional_storage(), SIGNALS_STORAGE);
            assert_eq!(b.peak_storage(), EXTRA + INSTALL_SCRATCH);
            assert_eq!(b.work(), INSTALL_WORK);
        }
        assert_eq!(b.storage(), EXTRA);
        assert!(b.work_ledger_identity_v1() == identity);
    }
}

#[test]
fn signal_wait_results_are_finite_ticks_or_exact_errors() {
    assert_eq!(wait_result(libc::SIGTERM, 0).unwrap(), Some(libc::SIGTERM));
    assert_eq!(wait_result(libc::SIGINT, 0).unwrap(), Some(libc::SIGINT));
    assert_eq!(wait_result(-1, libc::EINTR).unwrap(), None);
    assert_eq!(wait_result(-1, libc::EAGAIN).unwrap(), None);
    assert!(matches!(
        wait_result(-1, libc::EINVAL),
        Err(Failure::Io {
            source: Errno::INVAL,
            ..
        })
    ));
    assert!(wait_result(0, 0).is_err());
    assert!(wait_result(libc::SIGUSR1, 0).is_err());
    mask_status(0, "mask fixture").unwrap();
    assert!(matches!(
        mask_status(libc::EPERM, "mask fixture"),
        Err(Failure::Io {
            operation: "mask fixture",
            source: Errno::PERM
        })
    ));
}

#[test]
fn wait_and_restore_require_complete_owner_floor() {
    let mut signals = signal_state_fixture();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(SIGNALS_STORAGE - 1).unwrap();
    assert!(matches!(
        wait_with(&signals, &mut b, |_| panic!("unpaid wait")),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        restore_with(&mut signals, &mut b, |_| panic!("unpaid restore")),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(b.work(), 2 * ENTRY_WORK);
    assert_eq!(b.storage(), SIGNALS_STORAGE - 1);
    assert!(signals.active);
}

#[test]
fn wait_exact_and_one_short_never_retries_or_mutates_state() {
    for (work_limit, scratch_limit, success) in [
        (WAIT_WORK, WAIT_SCRATCH, true),
        (WAIT_WORK - 1, WAIT_SCRATCH, false),
        (WAIT_WORK, WAIT_SCRATCH - 1, false),
    ] {
        let signals = signal_state_fixture();
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, SIGNALS_STORAGE + scratch_limit);
        b.reserve_storage(SIGNALS_STORAGE).unwrap();
        let calls = Cell::new(0);
        let result = wait_with(&signals, &mut b, |_| {
            calls.set(calls.get() + 1);
            wait_result(-1, libc::EINTR)
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(calls.get(), usize::from(success));
        assert!(signals.active);
        assert_eq!(b.storage(), SIGNALS_STORAGE);
        if success {
            assert_eq!(result.unwrap(), None);
            assert_eq!(b.work(), WAIT_WORK);
        }
    }
}

#[test]
fn restore_exact_and_one_short_preserve_live_state_until_success() {
    for (work_limit, scratch_limit, success) in [
        (RESTORE_WORK, RESTORE_SCRATCH, true),
        (RESTORE_WORK - 1, RESTORE_SCRATCH, false),
        (RESTORE_WORK, RESTORE_SCRATCH - 1, false),
    ] {
        let mut signals = signal_state_fixture();
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, SIGNALS_STORAGE + scratch_limit);
        b.reserve_storage(SIGNALS_STORAGE).unwrap();
        let calls = Cell::new(0);
        let result = restore_with(&mut signals, &mut b, |_| {
            calls.set(calls.get() + 1);
            Ok(())
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(calls.get(), usize::from(success));
        assert_eq!(signals.active, !success);
        assert_eq!(b.storage(), SIGNALS_STORAGE);
        if success {
            assert_eq!(b.work(), RESTORE_WORK);
        }
    }
}

#[test]
fn failed_restore_keeps_owner_then_success_disables_wait_and_restore() {
    let mut signals = signal_state_fixture();
    let mut work = Work::new(3 * RESTORE_WORK + WAIT_WORK);
    let mut b = Budget::new(
        &mut work,
        SIGNALS_STORAGE + RESTORE_SCRATCH.max(WAIT_SCRATCH),
    );
    b.reserve_storage(SIGNALS_STORAGE).unwrap();
    assert!(matches!(
        restore_with(&mut signals, &mut b, |_| Err(io(
            "restore fixture",
            Errno::PERM
        ))),
        Err(Failure::Io {
            operation: "restore fixture",
            source: Errno::PERM
        })
    ));
    assert!(signals.active);
    restore_with(&mut signals, &mut b, |_| Ok(())).unwrap();
    assert!(!signals.active);
    assert!(restore_with(&mut signals, &mut b, |_| panic!("restore twice")).is_err());
    assert!(wait_with(&signals, &mut b, |_| panic!("wait after restoration")).is_err());
    assert_eq!(b.storage(), SIGNALS_STORAGE);
    assert_eq!(b.work(), 3 * RESTORE_WORK + WAIT_WORK);
}

#[test]
fn signal_scope_unwind_preserves_original_ledger_and_live_mask_state() {
    for operation in 0..3 {
        let mut signals = signal_state_fixture();
        let quota = [INSTALL_WORK, WAIT_WORK, RESTORE_WORK][operation];
        let scratch = [INSTALL_SCRATCH, WAIT_SCRATCH, RESTORE_SCRATCH][operation];
        let mut work = Work::new(quota);
        let mut b = Budget::new(&mut work, SIGNALS_STORAGE + scratch);
        b.reserve_storage(SIGNALS_STORAGE).unwrap();
        let identity = b.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| match operation {
            0 => {
                let _ = install_with(&mut b, || panic!("install fixture"));
            }
            1 => {
                let _ = wait_with(&signals, &mut b, |_| panic!("wait fixture"));
            }
            _ => {
                let _ = restore_with(&mut signals, &mut b, |_| panic!("restore fixture"));
            }
        }));
        assert!(result.is_err());
        assert_eq!(b.storage(), SIGNALS_STORAGE);
        assert_eq!(b.work(), quota);
        assert_eq!(b.peak_storage(), SIGNALS_STORAGE + scratch);
        assert!(b.work_ledger_identity_v1() == identity);
        assert!(signals.active);
    }
}

#[test]
fn source_owns_only_bounded_mechanics_not_legacy_or_native_authority() {
    let source = include_str!("native_activation.rs");
    for forbidden in [
        "std::env::",
        "CStr::from_ptr",
        "getenv(",
        "args_os(",
        "var_os(",
        "read_to_end(",
        "read_to_string(",
        "from_raw_fd(",
        "Budget::new(",
        "RootManagedCompilerExecutionServiceV1",
        "CompilerExecutionCapability",
        "Vec<",
        "String",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden source dependency: {forbidden}"
        );
    }
    assert!(source.contains("PhantomData<Rc<()>>"));
    assert!(source.contains("pub(crate) unsafe fn capture"));
    assert!(source.contains("pub(crate) unsafe fn install"));
    assert_eq!(source.matches("libc::clearenv()").count(), 1);
    assert_eq!(source.matches("libc::sigtimedwait(").count(), 1);
    assert_eq!(source.matches("libc::pthread_sigmask(").count(), 2);
    assert!(!source.contains("impl Drop"));
}
