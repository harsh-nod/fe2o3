use super::*;
use std::os::fd::BorrowedFd;

const CASE_ENV: &str = "FE2O3_WRAPPER_STDIO_CAPTURE_TEST";
const TEST: &str =
    "inert_rustc_invocation_capture::stdio::tests::owned_stdio_survives_reuse_drop_and_exec";

struct SavedStdio([(OwnedFd, FdFlags); 3]);

impl SavedStdio {
    fn new() -> Self {
        Self(std::array::from_fn(|slot| {
            // SAFETY: this isolated test's Rust runtime opened all standard slots;
            // no competing test, signal handler or foreign code changes them.
            let source = unsafe { BorrowedFd::borrow_raw(slot as RawFd) };
            (
                rustix::io::fcntl_dupfd_cloexec(source, 3).unwrap(),
                rustix::io::fcntl_getfd(source).unwrap(),
            )
        }))
    }
}

impl Drop for SavedStdio {
    fn drop(&mut self) {
        for (slot, (source, flags)) in self.0.iter().enumerate() {
            if install(source, slot as RawFd, *flags).is_err() {
                std::process::abort();
            }
        }
    }
}

fn install(source: &OwnedFd, slot: RawFd, flags: FdFlags) -> io::Result<()> {
    let flags = if flags.contains(FdFlags::CLOEXEC) {
        libc::O_CLOEXEC
    } else {
        0
    };
    // SAFETY: called only in the isolated test owning the standard slots.
    if unsafe { libc::dup3(source.as_raw_fd(), slot, flags) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn private_stream() -> OwnedFd {
    let source =
        rustix::fs::memfd_create(c"wrapper-stdio", rustix::fs::MemfdFlags::CLOEXEC).unwrap();
    let flags = rustix::fs::fcntl_getfl(&source).unwrap() | OFlags::APPEND | OFlags::NONBLOCK;
    rustix::fs::fcntl_setfl(&source, flags).unwrap();
    source
}

// All captures are of private OFDs installed by this one isolated test, never
// the test runner's externally aliased streams. 0 = absent, 1 = CLOEXEC, 2 = inherit.
fn capture(states: [u8; 3]) -> (CapturedStdioV1, [OwnedFd; 3]) {
    let saved = SavedStdio::new();
    let sources = std::array::from_fn(|_| private_stream());
    for (slot, state) in states.into_iter().enumerate() {
        match state {
            0 => {
                // SAFETY: standard slots are exclusively controlled by this test.
                assert_eq!(unsafe { libc::close(slot as RawFd) }, 0);
            }
            1 => install(&sources[slot], slot as RawFd, FdFlags::CLOEXEC).unwrap(),
            2 => install(&sources[slot], slot as RawFd, FdFlags::empty()).unwrap(),
            _ => unreachable!(),
        }
    }
    let capture = CapturedStdioV1::capture_current().unwrap();
    drop(saved);
    (capture, sources)
}

#[test]
fn owned_stdio_survives_reuse_drop_and_exec() {
    if std::env::var_os(CASE_ENV).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
            .env(CASE_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stdio fixture failed: {:?}\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    for case in 0..27 {
        let states = [case % 3, (case / 3) % 3, case / 9];
        command_preserves_capture(states);
    }
    missing_parent_slot_refuses_before_wrapper_setup();
    missing_parent_slot_refuses_before_installation();
    installed_absent_and_cloexec_preserve_exec_error();
    flag_drift_refuses_before_installation_and_at_spawn();
    partial_staging_failure_closes_duplicates();
    command_drop_releases_staged_descriptors();
    parent_custody_keeps_the_original_capture();
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn binding_wrapper_query_preserves_entry_stdio() {
    use crate::pinned_executable::PinnedExecutable;
    use std::path::Path;

    const SENTINEL: &str = "FE2O3_WRAPPER_QUERY_STDIO_TEST";
    const TEST: &str =
        "inert_rustc_invocation_capture::stdio::tests::binding_wrapper_query_preserves_entry_stdio";
    if std::env::var_os(SENTINEL).is_none() {
        let rustc = PinnedExecutable::open(Path::new("/bin/echo")).unwrap();
        let digest = rustc
            .sha256()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
            .env_clear()
            .env(SENTINEL, "1")
            .env(crate::EXPECTED_RUSTC_SHA256_ENV, digest);
        // SAFETY: the child hook only marks nonstdio descriptors CLOEXEC,
        // excluding unrelated inherited handles from this isolated fixture.
        unsafe {
            command.pre_exec(crate::application_exec::protect_all_nonstdio_descriptors);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "wrapper query fixture failed: {:?}\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    // Ordinary wrapper mechanics only: the sealed echo image supplies a query
    // response, with no compiler capabilities, managed attempt or authority.
    let image = PinnedExecutable::open(Path::new("/bin/echo"))
        .unwrap()
        .seal_executable_image()
        .unwrap();
    let source = image.try_clone_for_transfer().unwrap();
    let descriptor = rustix::io::fcntl_dupfd_cloexec(&source, crate::RUSTC_CHILD_FD).unwrap();
    assert_eq!(descriptor.as_raw_fd(), crate::RUSTC_CHILD_FD);
    let argv = vec![
        format!("/proc/self/fd/{}", crate::RUSTC_CHILD_FD).into(),
        "-vV".into(),
    ];
    let sources: [_; 3] = std::array::from_fn(|_| private_stream());
    let expected = sources.each_ref().map(|source| {
        let stat = rustix::fs::fstat(source).unwrap();
        (
            stat.st_dev,
            stat.st_ino,
            stat.st_mode,
            rustix::fs::fcntl_getfl(source).unwrap(),
        )
    });
    let saved = SavedStdio::new();
    let flags = [FdFlags::CLOEXEC, FdFlags::empty(), FdFlags::empty()];
    for (slot, source) in sources.iter().enumerate() {
        install(source, slot as RawFd, flags[slot]).unwrap();
    }
    // Exercise the actual wrapper entry and prepared Command, not the installer helper.
    let status = crate::binding_wrapper::run(argv);
    let observed: [_; 3] = std::array::from_fn(|slot| -> io::Result<_> {
        // SAFETY: scalar inspection also permits a missing slot on regression.
        let descriptor_flags = unsafe { libc::fcntl(slot as RawFd, libc::F_GETFD) };
        if descriptor_flags < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the preceding probe confirmed presence; only this isolated
        // test controls the standard slots, and no mutation occurs during use.
        let fd = unsafe { BorrowedFd::borrow_raw(slot as RawFd) };
        let stat = rustix::fs::fstat(fd)?;
        Ok((
            (
                stat.st_dev,
                stat.st_ino,
                stat.st_mode,
                rustix::fs::fcntl_getfl(fd)?,
            ),
            descriptor_flags,
            rustix::fs::seek(fd, rustix::fs::SeekFrom::Current(0))?,
        ))
    });
    // Restore harness streams before reporting failures or inspecting output.
    drop(saved);
    assert!(status.unwrap().success());
    for (slot, observation) in observed.into_iter().enumerate() {
        let (object_and_status, descriptor_flags, offset) = observation.unwrap();
        let expected_length = if slot == 1 { 4 } else { 0 };
        assert_eq!(object_and_status, expected[slot]);
        assert_eq!(descriptor_flags, flags[slot].bits() as i32);
        assert_eq!(offset, expected_length);
        assert_eq!(
            rustix::fs::seek(&sources[slot], rustix::fs::SeekFrom::Current(0)).unwrap(),
            expected_length,
        );
        assert_eq!(
            rustix::fs::fstat(&sources[slot]).unwrap().st_size,
            expected_length as i64
        );
        assert_eq!(
            rustix::fs::fcntl_getfl(&sources[slot]).unwrap(),
            expected[slot].3
        );
        assert_eq!(
            rustix::io::fcntl_getfd(&sources[slot]).unwrap(),
            FdFlags::CLOEXEC
        );
    }
    let mut bytes = [0_u8; 8];
    let count = rustix::io::pread(&sources[1], &mut bytes, 0).unwrap();
    assert_eq!(&bytes[..count], b"-vV\n");
    assert_eq!(
        rustix::fs::seek(&sources[1], rustix::fs::SeekFrom::Current(0)).unwrap(),
        4,
    );
}

fn command_preserves_capture(states: [u8; 3]) {
    let (capture, sources) = capture(states);
    let saved = SavedStdio::new();
    let replacement = private_stream();
    for slot in 0..=2 {
        // Model wrapper setup reusing formerly absent slots for protected files.
        install(&replacement, slot, FdFlags::empty()).unwrap();
    }
    let expected = sources.each_ref().map(|source| {
        let stat = rustix::fs::fstat(source).unwrap();
        (
            stat.st_dev,
            stat.st_ino,
            rustix::fs::fcntl_getfl(source).unwrap(),
        )
    });
    let captured_fds = [capture.stdin(), capture.stdout(), capture.stderr()]
        .map(|slot| slot.map(|slot| slot.source().as_raw_fd()));
    for (slot, state) in [capture.stdin(), capture.stdout(), capture.stderr()]
        .into_iter()
        .zip(states)
    {
        assert_eq!(slot.is_none(), state == 0);
        if let Some(slot) = slot {
            assert_eq!(
                slot.descriptor_flags().contains(FdFlags::CLOEXEC),
                state == 1
            );
        }
    }

    // coreutils test does not perform Rust's startup stdio sanitization. Check
    // post-exec stdout as well as all three slots in the final test-only hook.
    let mut command = Command::new("/usr/bin/test");
    if states[1] != 2 {
        command.arg("!");
    }
    command.args(["-e", "/proc/self/fd/1"]);
    configure_captured_stdio(&mut command, &capture).unwrap();
    drop(capture);
    for fd in captured_fds.into_iter().flatten() {
        // SAFETY: scalar probe verifies the original owner was dropped.
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
    }
    for source in &sources {
        rustix::fs::seek(source, rustix::fs::SeekFrom::Start(9)).unwrap();
    }
    // SAFETY: the test-only hook performs only scalar descriptor syscalls and
    // verifies the adapter's output without replacing its owned sources.
    unsafe {
        command.pre_exec(move || {
            for (slot, state) in states.into_iter().enumerate() {
                let fd = slot as RawFd;
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if state != 2 {
                    if flags != -1 || io::Error::last_os_error().raw_os_error() != Some(libc::EBADF)
                    {
                        return Err(io::Error::from_raw_os_error(libc::EINVAL));
                    }
                    continue;
                }
                if flags != 0 {
                    return Err(io::Error::from_raw_os_error(libc::EINVAL));
                }
                let source = BorrowedFd::borrow_raw(fd);
                let stat = rustix::fs::fstat(source)?;
                if (stat.st_dev, stat.st_ino, rustix::fs::fcntl_getfl(source)?) != expected[slot]
                    || libc::lseek(fd, 0, libc::SEEK_CUR) != 9
                {
                    return Err(io::Error::from_raw_os_error(libc::ESTALE));
                }
            }
            Ok(())
        });
    }
    // Command keeps the same owned sources alive through both launches.
    assert!(command.status().unwrap().success());
    assert!(command.status().unwrap().success());
    drop(saved);
}

fn missing_parent_slot_refuses_before_installation() {
    let (capture, _sources) = capture([2; 3]);
    let saved = SavedStdio::new();
    // SAFETY: this isolated test exclusively owns the standard slots.
    assert_eq!(unsafe { libc::close(0) }, 0);
    let mut command = Command::new("/bin/true");
    assert_eq!(
        configure_captured_stdio(&mut command, &capture)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EBADF)
    );
    drop(saved);
    assert!(command.status().unwrap().success());
}

fn missing_parent_slot_refuses_before_wrapper_setup() {
    for slot in 0..=2 {
        let saved = SavedStdio::new();
        // SAFETY: this isolated test exclusively owns the standard slots.
        assert_eq!(unsafe { libc::close(slot) }, 0);
        let error =
            crate::binding_wrapper::run(vec!["/proc/self/fd/-1".into(), "-vV".into()]).unwrap_err();
        assert!(
            matches!(error, crate::binding_wrapper::BindingWrapperError::Spawn(error)
            if error.raw_os_error() == Some(libc::EBADF))
        );
        // No wrapper setup or executable pinning may have filled the hole.
        assert_eq!(unsafe { libc::fcntl(slot, libc::F_GETFD) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
        drop(saved);
    }
}

fn installed_absent_and_cloexec_preserve_exec_error() {
    for states in [[0; 3], [1; 3]] {
        let (capture, _sources) = capture(states);
        // The negative procfd pathname cannot name a live executable. Failure
        // occurs after the stdio hook closes every child standard descriptor.
        let mut command = Command::new("/proc/self/fd/-1");
        configure_captured_stdio(&mut command, &capture).unwrap();
        assert_eq!(
            command.spawn().unwrap_err().raw_os_error(),
            Some(libc::ENOENT)
        );
    }
}

fn flag_drift_refuses_before_installation_and_at_spawn() {
    let (capture, sources) = capture([2; 3]);
    let original = rustix::fs::fcntl_getfl(&sources[0]).unwrap();
    let mut command = Command::new("/bin/true");
    rustix::fs::fcntl_setfl(&sources[0], original ^ OFlags::NONBLOCK).unwrap();
    assert_eq!(
        configure_captured_stdio(&mut command, &capture)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ESTALE)
    );
    rustix::fs::fcntl_setfl(&sources[0], original).unwrap();
    configure_captured_stdio(&mut command, &capture).unwrap();
    rustix::fs::fcntl_setfl(&sources[0], original ^ OFlags::NONBLOCK).unwrap();
    assert_eq!(
        command.spawn().unwrap_err().raw_os_error(),
        Some(libc::ESTALE)
    );
    rustix::fs::fcntl_setfl(&sources[0], original).unwrap();
    assert!(command.status().unwrap().success());
}

fn partial_staging_failure_closes_duplicates() {
    let (capture, sources) = capture([2; 3]);
    // Reserve a known first free staging FD, then allow exactly one duplicate.
    let probe = rustix::io::fcntl_dupfd_cloexec(&sources[0], SOURCE_FLOOR).unwrap();
    let first = probe.as_raw_fd();
    drop(probe);
    let mut previous = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: the test exclusively changes its own soft descriptor limit and
    // restores it before checking results or letting the test harness continue.
    let previous = unsafe {
        assert_eq!(
            libc::getrlimit(libc::RLIMIT_NOFILE, previous.as_mut_ptr()),
            0
        );
        previous.assume_init()
    };
    let limited = libc::rlimit {
        rlim_cur: (first + 1) as _,
        rlim_max: previous.rlim_max,
    };
    let mut command = Command::new("/bin/true");
    unsafe { assert_eq!(libc::setrlimit(libc::RLIMIT_NOFILE, &limited), 0) };
    let result = configure_captured_stdio(&mut command, &capture);
    unsafe { assert_eq!(libc::setrlimit(libc::RLIMIT_NOFILE, &previous), 0) };
    assert_eq!(result.unwrap_err().raw_os_error(), Some(libc::EMFILE));
    let probe = rustix::io::fcntl_dupfd_cloexec(&sources[0], SOURCE_FLOOR).unwrap();
    assert_eq!(probe.as_raw_fd(), first);
    assert!(command.status().unwrap().success());
}

fn parent_custody_keeps_the_original_capture() {
    use crate::inert_rustc_invocation_capture::{
        InertPreparedRustcInvocationCapture, InertRustcInvocationCaptureV2,
    };
    use crate::protected_compiler_handoff_v3::ParentRustcInvocationCustody;
    use fe2o3_build_authority::CompilerClosureV2;
    use fe2o3_compiler_closure_capability::RustcInvocationCapabilityV1;
    use std::{ffi::OsString, path::Path};

    let (stdio, sources) = capture([0, 1, 2]);
    let retained = stdio.stderr().unwrap().source().as_raw_fd();
    let mut command = Command::new("/toolchains/rustc");
    command.args([
        "--crate-name",
        "example",
        "--crate-type",
        "cdylib",
        "kernel.rs",
        "-Zcodegen-backend=/toolchains/backend.so",
    ]);
    configure_captured_stdio(&mut command, &stdio).unwrap();
    let environment = [
        ("PATH", "/usr/bin"),
        ("FE2O3_HSACO_DIR", "/proc/self/fd/197"),
        ("FE2O3_TARGET", "gfx942:xnack-"),
        ("FE2O3_VERIFY_KERNEL_IR", "1"),
    ]
    .map(|(k, v)| (OsString::from(k), OsString::from(v)));
    let capture = InertRustcInvocationCaptureV2::capture(
        &command,
        command.get_program(),
        Path::new("/workspace"),
        &environment,
        [4; 32],
        [6; 32],
    )
    .unwrap()
    .upgrade(CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap())
    .unwrap();
    let capability = RustcInvocationCapabilityV1::create(capture.descriptor().clone()).unwrap();
    let directory = fe2o3_process_identity::PinnedWorkingDirectoryV3::open(Path::new("/")).unwrap();
    directory.configure_child_fchdir(&mut command).unwrap();
    let custody = ParentRustcInvocationCustody::retain(
        Some(InertPreparedRustcInvocationCapture::V3(Box::new(capture))),
        Some(capability),
        Some(stdio),
        directory,
    )
    .unwrap()
    .unwrap();
    let stdio = custody.captured_stdio().unwrap();
    assert!(stdio.stdin().is_none());
    assert_eq!(stdio.stdout().unwrap().descriptor_flags(), FdFlags::CLOEXEC);
    assert_eq!(stdio.stderr().unwrap().descriptor_flags(), FdFlags::empty());
    assert!(!custody.grants_compiler_authority());
    custody.revalidate().unwrap();
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    let mut work = Work::new(30_000_000);
    let mut budget = Budget::new(&mut work, 4_000_000);
    let floor = custody.native_retained_storage().unwrap();
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    custody.revalidate_native(&mut budget).unwrap();
    let flags = rustix::fs::fcntl_getfl(&sources[2]).unwrap();
    rustix::fs::fcntl_setfl(&sources[2], flags ^ OFlags::NONBLOCK).unwrap();
    assert!(custody.revalidate().is_err());
    assert!(matches!(
        custody.revalidate_native(&mut budget),
        Err(
            fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2::Io {
                operation: "parent stdio",
                errno: libc::ESTALE,
            }
        )
    ));
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    drop(custody);
    // SAFETY: scalar probe checks that parent custody released its owned copy.
    assert_eq!(unsafe { libc::fcntl(retained, libc::F_GETFD) }, -1);
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
}

fn command_drop_releases_staged_descriptors() {
    let (capture, sources) = capture([2; 3]);
    let probe = rustix::io::fcntl_dupfd_cloexec(&sources[0], SOURCE_FLOOR).unwrap();
    let first = probe.as_raw_fd();
    drop(probe);
    let mut command = Command::new("/bin/true");
    configure_captured_stdio(&mut command, &capture).unwrap();
    // SAFETY: scalar probes inspect the known first staged descriptor.
    assert_eq!(
        unsafe { libc::fcntl(first, libc::F_GETFD) },
        libc::FD_CLOEXEC
    );
    drop(command);
    assert_eq!(unsafe { libc::fcntl(first, libc::F_GETFD) }, -1);
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
}
