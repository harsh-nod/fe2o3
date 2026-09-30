//! Ignored kernel mount-boundary controls, not service or compiler qualification.
//! No cgroup is created, populated, killed, removed or written by these tests.

use super::*;
use std::ffi::CString;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Reap(Option<Child>);
impl Drop for Reap {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
#[ignore = "requires isolated root/outside custodian, CAP_SYS_ADMIN, writable cgroup2, canonical non-root membership with no intervening mounts; never run inside the confined service"]
fn actual_membership_mount_boundary() {
    const CHILD: &str = "FE2O3_CGROUP_MOUNT_BOUNDARY_CHILD";
    if let Some(mode) = std::env::var_os(CHILD) {
        exercise(mode.to_str().unwrap());
        return;
    }
    for mode in [
        "intended",
        "wrong-parent",
        "wrong-subtree",
        "wrong-filesystem",
    ] {
        let mut child = Reap(Some(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "native_cgroup::tests::mounts::actual_membership_mount_boundary",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD, mode)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        ));
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = child.0.as_mut().unwrap().try_wait().unwrap() {
                assert!(status.success(), "mount boundary {mode}: {status}");
                let mut output = String::new();
                child
                    .0
                    .as_mut()
                    .unwrap()
                    .stdout
                    .take()
                    .unwrap()
                    .read_to_string(&mut output)
                    .unwrap();
                assert!(
                    output.contains("1 passed"),
                    "exact child did not execute: {output}"
                );
                assert!(
                    output.contains(&format!("CGROUP-MOUNT-CONTROL:{mode}:reached")),
                    "child did not reach the boundary assertion: {output}"
                );
                child.0.take();
                break;
            }
            assert!(Instant::now() < deadline, "mount boundary {mode} deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn mount(source: Option<&CStr>, target: &CStr, flags: libc::c_ulong) {
    // SAFETY: all strings remain alive through the syscall; only the isolated
    // subprocess's private mount namespace is modified, with no data argument.
    let result = unsafe {
        libc::mount(
            source.map_or(std::ptr::null(), CStr::as_ptr),
            target.as_ptr(),
            std::ptr::null(),
            flags,
            std::ptr::null(),
        )
    };
    assert_eq!(result, 0, "mount: {}", std::io::Error::last_os_error());
}

fn cgroup_path(path: &[u8]) -> CString {
    let mut bytes = b"/sys/fs/cgroup".to_vec();
    bytes.extend_from_slice(path);
    CString::new(bytes).unwrap()
}

fn root() -> OwnedFd {
    fs::open(
        c"/sys/fs/cgroup",
        READ_FLAGS | OFlags::DIRECTORY,
        Mode::empty(),
    )
    .unwrap()
}

fn exercise(mode: &str) {
    assert!(crate::syscall::has_exact_root_identity());
    let membership = open_proc(c"/proc/self/cgroup").unwrap();
    let mut bytes = [0; READ_LIMIT];
    let count = read_record(&membership, &mut bytes).unwrap();
    let path = membership_path(&bytes[..count]).unwrap();
    let (prefix, _) = membership_components(path).unwrap();
    assert_ne!(
        prefix, b"/",
        "fixture requires at least two membership components"
    );
    let target = cgroup_path(path);
    let prefix_target = cgroup_path(prefix);
    let original = root();
    let pid = rustix::process::getpid();
    let (_, expected) = open_membership_directory(&original, path, pid).unwrap();
    let mut relative = [0; READ_LIMIT];
    let relative = relative_path(path, &mut relative).unwrap();
    open_relative(&original, relative, READ_FLAGS | OFlags::DIRECTORY)
        .expect("fixture baseline must have no preexisting mount boundary");
    assert!(
        !fs::fstatvfs(&original)
            .unwrap()
            .f_flag
            .contains(fs::StatVfsMountFlags::RDONLY)
    );
    let mut wrong_subtree_empty = false;
    if mode == "wrong-subtree" {
        let procs = open_relative(&original, c"cgroup.procs", READ_FLAGS).unwrap();
        validate_control(&procs, &original, c"cgroup.procs").unwrap();
        let mut root_procs = [0; READ_LIMIT];
        let count = rustix::io::pread(&procs, &mut root_procs[..], 0).unwrap();
        assert!(
            count < READ_LIMIT,
            "wrong-subtree control must not be an overlong-read refusal"
        );
        wrong_subtree_empty = count == 0;
        if !wrong_subtree_empty {
            assert!(
                record_lines(&root_procs[..count])
                    .unwrap()
                    .split(|b| *b == b'\n')
                    .all(|row| decimal(row).unwrap() != pid.as_raw_pid() as u32)
            );
        }
    }
    drop(original);

    // SAFETY: this exact-test subprocess creates no children. Unsharing before
    // making propagation private ensures no following mount reaches the host.
    assert_eq!(unsafe { libc::unshare(libc::CLONE_NEWNS) }, 0);
    mount(None, c"/", libc::MS_REC | libc::MS_PRIVATE);
    mount(Some(c"/sys/fs/cgroup"), c"/sys/fs/cgroup", libc::MS_BIND);
    match mode {
        "intended" => mount(Some(&target), &target, libc::MS_BIND),
        "wrong-parent" => mount(Some(&prefix_target), &prefix_target, libc::MS_BIND),
        "wrong-subtree" => mount(Some(c"/sys/fs/cgroup"), &target, libc::MS_BIND),
        "wrong-filesystem" => mount(Some(c"/proc"), &target, libc::MS_BIND),
        _ => panic!("unknown exact-test child mode"),
    }
    mount(
        None,
        c"/sys/fs/cgroup",
        libc::MS_BIND
            | libc::MS_REMOUNT
            | libc::MS_RDONLY
            | libc::MS_NOSUID
            | libc::MS_NODEV
            | libc::MS_NOEXEC,
    );
    let root = root();
    assert!(
        fs::fstatvfs(&root)
            .unwrap()
            .f_flag
            .contains(fs::StatVfsMountFlags::RDONLY)
    );
    assert!(matches!(
        open_relative(&root, relative, READ_FLAGS | OFlags::DIRECTORY),
        Err(Error::Io {
            source: Errno::XDEV,
            ..
        })
    ));
    let result = open_membership_directory(&root, path, pid);
    match mode {
        "intended" => {
            let (actual, stat) = result.unwrap();
            assert_eq!(Identity::of(&stat), Identity::of(&expected));
            assert!(
                !fs::fstatvfs(&actual)
                    .unwrap()
                    .f_flag
                    .contains(fs::StatVfsMountFlags::RDONLY)
            );
            let domain = NativeCgroupDomainV1::prepare().unwrap();
            assert_eq!(domain.phase, Phase::Prepared);
            assert_eq!(domain.parent_identity, Identity::of(&expected));
            drop(domain); // Preparation created no domain or cleanup obligation.
            // A control-file submount is still forbidden after admission.
            let procs = CString::new(format!("{}/cgroup.procs", target.to_str().unwrap())).unwrap();
            mount(Some(&procs), &procs, libc::MS_BIND);
            assert!(matches!(
                open_relative(&actual, c"cgroup.procs", READ_FLAGS),
                Err(Error::Io {
                    source: Errno::XDEV,
                    ..
                })
            ));
        }
        "wrong-parent" => assert!(matches!(
            result,
            Err(Error::Io {
                operation: "open bounded cgroup component",
                source: Errno::XDEV
            })
        )),
        "wrong-subtree" => {
            // The root mount has the right filesystem/owner but not this PID's
            // direct membership. Empty cgroup.procs also refuses at that read.
            match result {
                Err(Error::State(message)) => assert_eq!(
                    message,
                    if wrong_subtree_empty {
                        "control record is empty or exceeds its bound"
                    } else {
                        "resolved cgroup does not contain this process"
                    }
                ),
                Err(error) => panic!("wrong-subtree did not reach membership refusal: {error}"),
                Ok(_) => panic!("wrong-subtree was admitted"),
            }
        }
        "wrong-filesystem" => assert!(matches!(
            result,
            Err(Error::State("unexpected control filesystem"))
        )),
        _ => unreachable!(),
    }
    // The namespace dies with this subprocess even on panic; the outer guard
    // kills and waits on timeout/unwind. No host mounts need path-based cleanup.
    println!("CGROUP-MOUNT-CONTROL:{mode}:reached");
}
