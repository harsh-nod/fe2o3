use super::*;
use std::{fs, os::fd::AsRawFd, process::Command};

#[test]
fn filesystem_policy_handles_all_abi3_mutations_without_read_or_exec_grants() {
    assert_eq!(HANDLED, ((1 << 15) - 1) & !((1 << 0) | (1 << 2) | (1 << 3)));
    assert_eq!(ALLOWED & !HANDLED, 0);
    assert_eq!(
        ALLOWED & (MAKE_CHAR | MAKE_SOCK | MAKE_FIFO | MAKE_BLOCK | MAKE_SYM),
        0
    );
    assert_eq!(ALLOWED & (WRITE_FILE | TRUNCATE), WRITE_FILE | TRUNCATE);
    assert_eq!(REQUIRED_ABI, 3);
    assert_eq!(size_of::<Ruleset>(), 8);
    assert_eq!(size_of::<PathBeneath>(), 12);
    assert_eq!(WORK, 6 * (1024 + 64) + 256);
}

#[test]
#[ignore = "irreversible thread confinement; run only this exact test in an isolated process on Landlock ABI3+"]
fn actual_output_write_confinement_and_exec_inheritance() {
    let name =
        "syscall::compiler_filesystem::tests::actual_output_write_confinement_and_exec_inheritance";
    let args: Vec<_> = std::env::args().collect();
    assert!(args.iter().any(|arg| arg == "--exact") && args.iter().any(|arg| arg == name));
    let tree = tempfile::tempdir().unwrap();
    let output = tree.path().join("output");
    fs::create_dir(&output).unwrap();
    let source = tree.path().join("source");
    fs::write(&source, b"unchanged").unwrap();
    let directory = fs::File::open(&output).unwrap();
    let inherited = fs::OpenOptions::new().write(true).open(&source).unwrap();
    // The parent retains cleanup custody and remains unconfined, including when
    // the worker panics. Only the joined worker and its descendants are changed.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                // SAFETY: this explicitly isolated worker restricts only itself and
                // its descendants. The original directory remains owned through the call.
                unsafe {
                    assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
                    assert!(!install(-1));
                    assert!(
                        install(directory.as_raw_fd()),
                        "Landlock ABI3+ is required; no skipped pass"
                    );
                }
                let file = output.join("artifact");
                fs::write(&file, b"artifact").unwrap();
                fs::OpenOptions::new()
                    .write(true)
                    .open(&file)
                    .unwrap()
                    .set_len(3)
                    .unwrap();
                assert_eq!(fs::read(&file).unwrap(), b"art");
                assert_eq!(fs::read(&source).unwrap(), b"unchanged");
                assert_eq!(
                    fs::write(&source, b"bad").unwrap_err().raw_os_error(),
                    Some(libc::EACCES)
                );
                assert_eq!(
                    fs::write(tree.path().join("new"), b"bad")
                        .unwrap_err()
                        .raw_os_error(),
                    Some(libc::EACCES)
                );
                // O_RDONLY|O_TRUNC is meaningful on Linux although std rejects the pair.
                assert_eq!(
                    rustix::fs::open(
                        &source,
                        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::TRUNC,
                        rustix::fs::Mode::empty()
                    )
                    .unwrap_err(),
                    rustix::io::Errno::ACCESS
                );
                assert!(fs::rename(&file, tree.path().join("escaped")).is_err());
                assert!(fs::hard_link(&source, output.join("linked-source")).is_err());
                assert!(std::os::unix::fs::symlink(&source, output.join("source-link")).is_err());
                let moved = output.join("moved");
                fs::rename(&file, &moved).unwrap();
                fs::remove_file(&moved).unwrap();
                let descendant = output.join("descendant");
                let denied = tree.path().join("denied-child");
                let status = Command::new("/bin/sh")
                    .args([
                        "-c",
                        "printf child > \"$1\" && ! (printf denied > \"$2\")",
                        "landlock-test",
                    ])
                    .arg(&descendant)
                    .arg(&denied)
                    .status()
                    .unwrap();
                assert!(status.success());
                assert_eq!(fs::read(descendant).unwrap(), b"child");
                assert!(!denied.exists());
                // This explicit limitation prevents treating Landlock as FD revocation.
                inherited.set_len(3).unwrap();
                assert_eq!(fs::read(&source).unwrap(), b"unc");
            })
            .join()
            .unwrap()
    });
    drop(inherited);
    drop(directory);
    tree.close().unwrap();
}
