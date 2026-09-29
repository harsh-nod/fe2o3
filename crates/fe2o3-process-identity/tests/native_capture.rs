use fe2o3_process_identity::{CapturedStdioV1, PinnedWorkingDirectoryV3};
use rustix::{fs::OFlags, io::FdFlags};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    mem::MaybeUninit,
    os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd},
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

const CHILD_CASE: &str = "FE2O3_NATIVE_CAPTURE_TEST_CASE";
const CHILD_TEST: &str = "stdio_capture_preserves_actual_slots_ofds_and_flags";

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-native-capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn cwd_native_source_keeps_original_object_after_path_replacement() {
    let root = Directory::new();
    let original = root.0.join("cwd");
    let moved = root.0.join("moved");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("input"), b"original").unwrap();
    let cwd = PinnedWorkingDirectoryV3::open(&original).unwrap();
    let identity = cwd.object_identity();
    fs::rename(&original, &moved).unwrap();
    fs::create_dir(&original).unwrap();
    fs::write(original.join("input"), b"replacement").unwrap();

    let source = cwd.native_source().unwrap();
    let staged = rustix::io::fcntl_dupfd_cloexec(source, 3).unwrap();
    cwd.validate_native_transfer(staged.as_fd()).unwrap();
    assert_eq!(cwd.object_identity(), identity);
    let mut input = File::from(
        rustix::fs::openat(
            &staged,
            "input",
            OFlags::RDONLY | OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    );
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"original");
    let replacement = PinnedWorkingDirectoryV3::open(&original).unwrap();
    assert_ne!(replacement.object_identity(), identity);
    assert_stale(cwd.validate_native_transfer(replacement.native_source().unwrap()));
}

#[test]
fn cwd_native_transfer_rejects_wrong_kind_path_only_and_lost_cloexec() {
    let root = Directory::new();
    let cwd = PinnedWorkingDirectoryV3::open(&root.0).unwrap();
    let staged = rustix::io::fcntl_dupfd_cloexec(cwd.native_source().unwrap(), 3).unwrap();
    rustix::io::fcntl_setfd(&staged, FdFlags::empty()).unwrap();
    assert_stale(cwd.validate_native_transfer(staged.as_fd()));
    rustix::io::fcntl_setfd(&staged, FdFlags::CLOEXEC).unwrap();
    cwd.validate_native_transfer(staged.as_fd()).unwrap();

    let path_only = rustix::fs::open(
        &root.0,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .unwrap();
    assert_stale(cwd.validate_native_transfer(path_only.as_fd()));
    let regular = File::create(root.0.join("file")).unwrap();
    assert_stale(cwd.validate_native_transfer(regular.as_fd()));
    let different_status = rustix::fs::open(
        &root.0,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .unwrap();
    assert_stale(cwd.validate_native_transfer(different_status.as_fd()));

    rustix::io::fcntl_setfd(cwd.native_source().unwrap(), FdFlags::empty()).unwrap();
    assert_eq!(
        cwd.native_source().unwrap_err().raw_os_error(),
        Some(libc::ESTALE)
    );
    assert_stale(cwd.validate_native_transfer(staged.as_fd()));
}

fn assert_stale(result: io::Result<()>) {
    assert_eq!(result.unwrap_err().raw_os_error(), Some(libc::ESTALE));
}

// Standard-slot mutation occurs only in the one selected test of a fresh process.
// Backups live above FD2 and restore the test harness streams even on unwind.
struct SavedStdio([(OwnedFd, i32); 3]);
impl SavedStdio {
    fn new() -> Self {
        Self(std::array::from_fn(|slot| {
            let fd = slot as RawFd;
            (duplicate(fd).unwrap(), raw_descriptor_flags(fd).unwrap())
        }))
    }
}
impl Drop for SavedStdio {
    fn drop(&mut self) {
        for (slot, (source, flags)) in self.0.iter().enumerate() {
            if install(source.as_raw_fd(), slot as RawFd, *flags).is_err() {
                std::process::abort();
            }
        }
    }
}

fn duplicate(fd: RawFd) -> io::Result<OwnedFd> {
    // SAFETY: fcntl has only scalar arguments; no raw-FD ownership is borrowed.
    let result = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fcntl returned a new owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(result) })
}

fn raw_descriptor_flags(fd: RawFd) -> io::Result<i32> {
    // SAFETY: this scalar observation also permits absent raw descriptors.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(flags)
    }
}

fn install(source: RawFd, target: RawFd, flags: i32) -> io::Result<()> {
    assert!(source >= 3 && (0..=2).contains(&target));
    let flags = if flags & libc::FD_CLOEXEC != 0 {
        libc::O_CLOEXEC
    } else {
        0
    };
    // SAFETY: these tests exclusively own the child process's standard slots.
    if unsafe { libc::dup3(source, target, flags) } != target {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[test]
fn stdio_capture_preserves_actual_slots_ofds_and_flags() {
    if let Ok(case) = std::env::var(CHILD_CASE) {
        match case.as_str() {
            "partial-failure" => partial_failure(),
            "pipe" => capture_pipe(),
            "cloexec" => capture_cloexec(),
            _ => capture_mask(case.parse().unwrap()),
        }
        return;
    }
    for case in (0..8).map(|n| n.to_string()).chain([
        "partial-failure".to_owned(),
        "pipe".to_owned(),
        "cloexec".to_owned(),
    ]) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
            .env(CHILD_CASE, &case)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "case {case}: {:?}\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn capture_cloexec() {
    let root = Directory::new();
    let source = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(root.0.join("stdio"))
        .unwrap();
    let flags = rustix::fs::fcntl_getfl(&source).unwrap() | OFlags::APPEND | OFlags::NONBLOCK;
    rustix::fs::fcntl_setfl(&source, flags).unwrap();
    let status_flags = rustix::fs::fcntl_getfl(&source).unwrap();
    rustix::fs::seek(&source, rustix::fs::SeekFrom::Start(2)).unwrap();
    let states = [None, Some(FdFlags::CLOEXEC), Some(FdFlags::empty())];
    // Rotate all three states through each slot; all open slots share one OFD.
    for rotation in 0..3 {
        let _saved = SavedStdio::new();
        for slot in 0..3 {
            match states[(slot + rotation) % 3] {
                Some(flags) => {
                    install(source.as_raw_fd(), slot as RawFd, flags.bits() as i32).unwrap();
                }
                None => {
                    // SAFETY: this isolated test exclusively controls standard slots.
                    assert_eq!(unsafe { libc::close(slot as RawFd) }, 0);
                }
            }
        }
        // SAFETY: only this test controls the slots, their flags and this private OFD.
        let captured = unsafe { CapturedStdioV1::capture_current() }.unwrap();
        let streams = [captured.stdin(), captured.stdout(), captured.stderr()];
        for (slot, stream) in streams.into_iter().enumerate() {
            let expected = states[(slot + rotation) % 3];
            assert_eq!(stream.map(|s| s.descriptor_flags()), expected);
            // Future adapter contract: Stage Some clears CLOEXEC, None closes.
            let stage_source = stream
                .filter(|s| !s.descriptor_flags().contains(FdFlags::CLOEXEC))
                .map(|s| s.source());
            assert_eq!(stage_source.is_some(), expected == Some(FdFlags::empty()));
            match stream {
                None => assert_eq!(
                    raw_descriptor_flags(slot as RawFd)
                        .unwrap_err()
                        .raw_os_error(),
                    Some(libc::EBADF)
                ),
                Some(stream) => {
                    assert_eq!(
                        raw_descriptor_flags(slot as RawFd).unwrap(),
                        expected.unwrap().bits() as i32
                    );
                    assert_eq!(stream.status_flags(), status_flags);
                    assert_eq!(
                        rustix::fs::fcntl_getfl(stream.source()).unwrap(),
                        status_flags
                    );
                    assert_eq!(
                        rustix::io::fcntl_getfd(stream.source()).unwrap(),
                        FdFlags::CLOEXEC
                    );
                    // An unfiltered Stage Some would turn original CLOEXEC into
                    // inheritance. Destination FD flags do not affect the OFD or
                    // the captured original flags, nor the retained copy's flags.
                    install(stream.source().as_raw_fd(), slot as RawFd, 0).unwrap();
                    assert_eq!(raw_descriptor_flags(slot as RawFd).unwrap(), 0);
                    assert_eq!(stream.descriptor_flags(), expected.unwrap());
                    captured.revalidate().unwrap();
                }
            }
        }
        assert_eq!(
            rustix::fs::seek(&source, rustix::fs::SeekFrom::Current(0)).unwrap(),
            2
        );
    }
}

fn capture_pipe() {
    let mut fds = [-1; 2];
    // SAFETY: pipe2 initializes two distinct descriptors on success.
    assert_eq!(
        unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) },
        0
    );
    // SAFETY: these successful pipe2 results have no other owners.
    let (reader, writer) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    let saved = SavedStdio::new();
    install(reader.as_raw_fd(), 0, 0).unwrap();
    install(writer.as_raw_fd(), 1, 0).unwrap();
    install(writer.as_raw_fd(), 2, libc::FD_CLOEXEC).unwrap();
    // SAFETY: the selected test exclusively controls all standard slots and the
    // private pipe. No other process or handler holds an alias to this pipe.
    let captured = unsafe { CapturedStdioV1::capture_current() }.unwrap();
    drop(saved);
    drop((reader, writer));
    captured.revalidate().unwrap();
    let stdin = captured.stdin().unwrap();
    let stdout = captured.stdout().unwrap();
    let stderr = captured.stderr().unwrap();
    assert_eq!(stdin.status_flags() & OFlags::ACCMODE, OFlags::RDONLY);
    assert_eq!(stdout.status_flags() & OFlags::ACCMODE, OFlags::WRONLY);
    assert!(stdout.status_flags().contains(OFlags::NONBLOCK));
    assert_eq!(stderr.descriptor_flags(), FdFlags::CLOEXEC);
    let mut bytes = [0; 2];
    assert_eq!(
        rustix::io::read(stdin.source(), &mut bytes),
        Err(rustix::io::Errno::AGAIN)
    );
    assert_eq!(rustix::io::write(stdout.source(), b"a").unwrap(), 1);
    assert_eq!(rustix::io::write(stderr.source(), b"b").unwrap(), 1);
    assert_eq!(rustix::io::read(stdin.source(), &mut bytes).unwrap(), 2);
    assert_eq!(&bytes, b"ab");
}

fn capture_mask(mask: u8) {
    let root = Directory::new();
    fs::write(root.0.join("input"), b"abcdef").unwrap();
    let input = File::open(root.0.join("input")).unwrap();
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.0.join("output"))
        .unwrap();
    output.write_all(b"seed").unwrap();
    let flags = rustix::fs::fcntl_getfl(&output).unwrap() | OFlags::APPEND | OFlags::NONBLOCK;
    rustix::fs::fcntl_setfl(&output, flags).unwrap();
    rustix::fs::seek(&input, rustix::fs::SeekFrom::Start(1)).unwrap();
    let saved = SavedStdio::new();
    let sources = [input.as_raw_fd(), output.as_raw_fd(), output.as_raw_fd()];
    let descriptor_flags = [0, 0, libc::FD_CLOEXEC];
    for slot in 0..3 {
        if mask & (1 << slot) != 0 {
            install(sources[slot], slot as RawFd, descriptor_flags[slot]).unwrap();
        } else {
            // SAFETY: only this isolated test mutates its standard slots.
            assert_eq!(unsafe { libc::close(slot as RawFd) }, 0);
        }
    }
    // SAFETY: no other test runs in this process; its harness is waiting for this
    // test. All backing files are private and no handler mutates their flags.
    let captured = unsafe { CapturedStdioV1::capture_current() }.unwrap();
    captured.revalidate().unwrap();
    let streams = [captured.stdin(), captured.stdout(), captured.stderr()];
    for (slot, stream) in streams.iter().enumerate() {
        assert_eq!(stream.is_some(), mask & (1 << slot) != 0);
        match stream {
            Some(stream) => {
                assert!(stream.source().as_raw_fd() >= 3);
                assert_eq!(
                    stream.descriptor_flags().bits() as i32,
                    descriptor_flags[slot]
                );
                assert_eq!(
                    raw_descriptor_flags(slot as RawFd).unwrap(),
                    descriptor_flags[slot]
                );
                assert_eq!(
                    rustix::io::fcntl_getfd(stream.source()).unwrap(),
                    FdFlags::CLOEXEC
                );
                let original = if slot == 0 { &input } else { &output };
                assert_eq!(
                    stream.status_flags(),
                    rustix::fs::fcntl_getfl(original).unwrap()
                );
            }
            None => assert_eq!(
                raw_descriptor_flags(slot as RawFd)
                    .unwrap_err()
                    .raw_os_error(),
                Some(libc::EBADF)
            ),
        }
    }
    assert_eq!(
        rustix::fs::seek(&input, rustix::fs::SeekFrom::Current(0)).unwrap(),
        1
    );
    assert_eq!(
        rustix::fs::seek(&output, rustix::fs::SeekFrom::Current(0)).unwrap(),
        4
    );
    if let Some(stdin) = captured.stdin() {
        let mut byte = [0];
        assert_eq!(rustix::io::read(stdin.source(), &mut byte).unwrap(), 1);
        assert_eq!(byte, *b"b");
        assert_eq!(
            rustix::fs::seek(&input, rustix::fs::SeekFrom::Current(0)).unwrap(),
            2
        );
    }
    if let Some(stdout) = captured.stdout() {
        assert_eq!(rustix::io::write(stdout.source(), b"!").unwrap(), 1);
        assert_eq!(
            rustix::fs::seek(&output, rustix::fs::SeekFrom::Current(0)).unwrap(),
            5
        );
        if let Some(stderr) = captured.stderr() {
            assert_eq!(
                rustix::fs::seek(stderr.source(), rustix::fs::SeekFrom::Current(0)).unwrap(),
                5
            );
        }
    }
    if let Some(stream) = streams.into_iter().flatten().next() {
        let flags = stream.status_flags();
        rustix::fs::fcntl_setfl(stream.source(), flags ^ OFlags::NONBLOCK).unwrap();
        assert_stale(captured.revalidate());
        rustix::fs::fcntl_setfl(stream.source(), flags).unwrap();
        captured.revalidate().unwrap();
        rustix::io::fcntl_setfd(stream.source(), FdFlags::empty()).unwrap();
        assert_stale(captured.revalidate());
        rustix::io::fcntl_setfd(stream.source(), FdFlags::CLOEXEC).unwrap();
    }
    let raw_copies = streams.map(|stream| stream.map(|stream| stream.source().as_raw_fd()));
    // Restoring (and thus reusing) original slots must not replace captured OFDs.
    drop(saved);
    captured.revalidate().unwrap();
    if let Some(stdout) = captured.stdout() {
        assert_eq!(rustix::io::write(stdout.source(), b"?").unwrap(), 1);
        assert_eq!(
            rustix::fs::seek(&output, rustix::fs::SeekFrom::Current(0)).unwrap(),
            6
        );
    }
    drop(captured);
    for fd in raw_copies.into_iter().flatten() {
        assert_eq!(
            raw_descriptor_flags(fd).unwrap_err().raw_os_error(),
            Some(libc::EBADF)
        );
    }
}

struct FileLimit(libc::rlimit);
impl FileLimit {
    fn lower() -> Self {
        let mut original = MaybeUninit::uninit();
        // SAFETY: successful getrlimit initializes the complete output record.
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, original.as_mut_ptr()) },
            0
        );
        // SAFETY: getrlimit succeeded.
        let original = unsafe { original.assume_init() };
        assert!(original.rlim_cur >= 64);
        let lowered = libc::rlimit {
            rlim_cur: 64,
            rlim_max: original.rlim_max,
        };
        // SAFETY: this isolated child retains the original limit for restoration.
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &lowered) }, 0);
        Self(original)
    }
}
impl Drop for FileLimit {
    fn drop(&mut self) {
        // SAFETY: restore only this child's original soft limit, with unchanged hard limit.
        if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &self.0) } != 0 {
            std::process::abort();
        }
    }
}

fn partial_failure() {
    let source = File::open("/dev/null").unwrap();
    let _saved = SavedStdio::new();
    for target in 0..=2 {
        install(source.as_raw_fd(), target, 0).unwrap();
    }
    let mut occupied = Vec::with_capacity(64);
    let _limit = FileLimit::lower();
    loop {
        match duplicate(source.as_raw_fd()) {
            Ok(fd) => occupied.push(fd),
            Err(error) => {
                assert_eq!(error.raw_os_error(), Some(libc::EMFILE));
                break;
            }
        }
    }
    assert!(occupied.len() >= 2);
    drop(occupied.pop());
    drop(occupied.pop());
    // SAFETY: this exact isolated test exclusively owns all three standard slots
    // and the private backing; the first two captures fit, the third must refuse.
    let error = match unsafe { CapturedStdioV1::capture_current() } {
        Ok(_) => panic!("three duplicates unexpectedly fit in two free slots"),
        Err(error) => error,
    };
    assert_eq!(error.raw_os_error(), Some(libc::EMFILE));
    let first = duplicate(source.as_raw_fd()).unwrap();
    let second = duplicate(source.as_raw_fd()).unwrap();
    assert_eq!(
        duplicate(source.as_raw_fd()).unwrap_err().raw_os_error(),
        Some(libc::EMFILE)
    );
    for slot in 0..=2 {
        assert_eq!(raw_descriptor_flags(slot).unwrap(), 0);
    }
    drop((first, second));
}
