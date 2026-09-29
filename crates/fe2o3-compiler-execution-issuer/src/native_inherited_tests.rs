use std::{
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const CASE_ENV: &str = "FE2O3_TEST_NATIVE_INHERITED_CASE";
const SLOT_ENV: &str = "FE2O3_TEST_NATIVE_INHERITED_SLOT";

fn descriptor_flags(fd: i32) -> i32 {
    // SAFETY: F_GETFD inspects a scalar descriptor without allocating or adopting it.
    unsafe { libc::fcntl(fd, libc::F_GETFD) }
}

fn inherited_child_case(mode: &str, slot: i32) {
    use crate::CompilerExecutionIssuerEntrypointErrorV1 as DescriptorError;

    assert_eq!(
        crate::NATIVE_INHERITED_DESCRIPTORS,
        [3, 4, 5, 6, 7, 8, 9, 10, 11]
    );
    assert_eq!(crate::PRIVATE_DESCRIPTOR_FLOOR, 12);
    for fd in crate::NATIVE_INHERITED_DESCRIPTORS {
        assert_eq!(descriptor_flags(fd), 0);
    }
    let private_flags: [i32; 20] = std::array::from_fn(|i| descriptor_flags(12 + i as i32));
    let prefix = 17;
    let floor = 19;
    let limit = prefix + IO_WORK;
    let mut work = Work::new(limit);
    let mut b = Budget::new(&mut work, floor + FRAME);
    b.charge_work(prefix).unwrap();
    b.reserve_storage(floor).unwrap();
    assert!(b.charge_work(limit).is_err());
    assert!(b.reserve_storage(FRAME + 1).is_err());
    let denial = (b.failed_work(), b.failed_storage());
    let ledger = b.work_ledger_identity_v1();

    // All fixture allocations precede mutation of a mandatory slot. Only this
    // freshly exec'd, single-test child changes the raw inherited descriptors.
    match mode {
        "missing" => {
            // SAFETY: the fixture installed this raw slot and has no Rust owner for it.
            assert_eq!(unsafe { libc::close(slot) }, 0);
        }
        "cloexec" => {
            // SAFETY: F_SETFD changes only this fixture-owned scalar descriptor.
            assert_eq!(
                unsafe { libc::fcntl(slot, libc::F_SETFD, libc::FD_CLOEXEC) },
                0
            );
        }
        "valid" => {}
        _ => panic!("unknown inherited descriptor case"),
    }

    if mode == "valid" {
        b.with_prepaid_scope(floor, 8, IO_WORK, FRAME, |_| -> Result<()> {
            let _process = Process::harden().map_err(Error::Process)?;
            crate::require_native_inherited()?;
            Ok(())
        })
        .unwrap();
    } else {
        let error = entrypoint(&mut b).unwrap_err();
        match (mode, error) {
            ("missing", Error::Descriptor(DescriptorError::Descriptor(e))) => {
                assert_eq!(e.raw_os_error(), Some(libc::EBADF));
            }
            ("cloexec", Error::Descriptor(DescriptorError::UnexpectedCloseOnExec(fd))) => {
                assert_eq!(fd, slot);
            }
            (_, error) => panic!("table refusal must precede policy input I/O: {error:?}"),
        }
    }
    assert_eq!(b.work(), prefix + IO_WORK);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!((b.failed_work(), b.failed_storage()), denial);
    for fd in crate::NATIVE_INHERITED_DESCRIPTORS {
        let expected = if fd == slot && mode == "missing" {
            -1
        } else if fd == slot && mode == "cloexec" {
            libc::FD_CLOEXEC
        } else {
            0
        };
        assert_eq!(descriptor_flags(fd), expected);
    }
    for (i, flags) in private_flags.into_iter().enumerate() {
        assert_eq!(descriptor_flags(12 + i as i32), flags);
    }
    if mode == "valid" {
        // Each accepted raw slot remains available for the unchanged consuming
        // take. This tests descriptor intake, not admission of these /dev/null files.
        for fd in crate::NATIVE_INHERITED_DESCRIPTORS {
            let owned = crate::take_inherited(fd).unwrap();
            assert!(owned.as_raw_fd() >= crate::PRIVATE_DESCRIPTOR_FLOOR);
            assert_eq!(descriptor_flags(owned.as_raw_fd()), libc::FD_CLOEXEC);
            assert_eq!(descriptor_flags(fd), -1);
            drop(owned);
        }
        for (i, flags) in private_flags.into_iter().enumerate() {
            assert_eq!(descriptor_flags(12 + i as i32), flags);
        }
    }
    println!("NATIVE_INHERITED_TABLE_CHECKED {mode} {slot}");
}

fn inherited_matrix(test: &str, mode: &str, slots: &[i32]) {
    if let Ok(child_mode) = std::env::var(CASE_ENV) {
        assert_eq!(child_mode, mode);
        let slot = std::env::var(SLOT_ENV).unwrap().parse().unwrap();
        assert!(slots.contains(&slot));
        inherited_child_case(mode, slot);
        return;
    }
    let _fork_guard = crate::TEST_FORK_FD_LOCK.lock().unwrap();
    let module = module_path!().split_once("::").unwrap().1;
    for slot in slots {
        let source = File::open("/dev/null").unwrap();
        let source = rustix::io::fcntl_dupfd_cloexec(&source, 32).unwrap();
        // Keep Command's stdio and exec-error pipes above the installation table.
        let mut occupied = Vec::new();
        loop {
            let fd = rustix::io::fcntl_dupfd_cloexec(&source, 3).unwrap();
            let above_table = fd.as_raw_fd() >= crate::PRIVATE_DESCRIPTOR_FLOOR;
            occupied.push(fd);
            if above_table {
                break;
            }
        }
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                &format!("{module}::{test}"),
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CASE_ENV, mode)
            .env(SLOT_ENV, slot.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: the child uses only dup2 with a retained source above the fixed
        // table. Every mandatory slot stays occupied across exec and test startup.
        unsafe {
            command.pre_exec(move || {
                for fd in crate::NATIVE_INHERITED_DESCRIPTORS {
                    if libc::dup2(source.as_raw_fd(), fd) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let timed_out = loop {
            if child.try_wait().unwrap().is_some() {
                break false;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                break true;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let output = child.wait_with_output().unwrap();
        assert!(
            !timed_out,
            "inherited table subprocess timed out: {mode} {slot}"
        );
        assert!(
            output.status.success(),
            "{mode} {slot}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains(&format!("NATIVE_INHERITED_TABLE_CHECKED {mode} {slot}"))
        );
    }
}

#[test]
fn native_inherited_table_rejects_each_missing_slot_before_input_io() {
    inherited_matrix(
        "native_inherited_table_rejects_each_missing_slot_before_input_io",
        "missing",
        &crate::NATIVE_INHERITED_DESCRIPTORS,
    );
}

#[test]
fn native_inherited_table_rejects_each_cloexec_slot_before_input_io() {
    inherited_matrix(
        "native_inherited_table_rejects_each_cloexec_slot_before_input_io",
        "cloexec",
        &crate::NATIVE_INHERITED_DESCRIPTORS,
    );
}

#[test]
fn native_inherited_table_preserves_valid_slots_for_consuming_intake() {
    inherited_matrix(
        "native_inherited_table_preserves_valid_slots_for_consuming_intake",
        "valid",
        &[3],
    );
}

#[test]
fn native_inherited_table_work_is_prepaid_before_entrypoint_io() {
    let prefix = 17;
    let floor = 19;
    assert_eq!(crate::NATIVE_INHERITED_CHECK_WORK, 9 * (1024 + 64));
    assert_eq!(IO_WORK, 8 + 64 * 1024 + crate::NATIVE_INHERITED_CHECK_WORK);
    let mut work = Work::new(prefix + IO_WORK - 1);
    let mut b = Budget::new(&mut work, floor + FRAME);
    b.charge_work(prefix).unwrap();
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    assert!(matches!(
        entrypoint(&mut b),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(b.work(), prefix + 8);
    assert_eq!(b.failed_work(), Some(prefix + IO_WORK));
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
}
