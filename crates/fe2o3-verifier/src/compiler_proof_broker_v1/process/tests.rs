use super::*;
use std::{
    io::Write,
    os::unix::fs::PermissionsExt,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct TestChild(Child);
impl Drop for TestChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn sealed_child(image: &str, args: &[&str]) -> (File, TestChild, [u8; 32]) {
    let mut sealed = File::from(
        rustix::fs::memfd_create(
            c"compiler-proof-test-image",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    );
    io::copy(&mut File::open(image).unwrap(), &mut sealed).unwrap();
    sealed
        .set_permissions(std::fs::Permissions::from_mode(0o500))
        .unwrap();
    rustix::fs::fcntl_add_seals(&sealed, EXACT_IMMUTABLE_MEMFD_SEALS_V1).unwrap();
    let path = format!("/proc/self/fd/{}", sealed.as_raw_fd());
    let digest = measure_executable_sha256_v3(Path::new(&path)).unwrap();
    let child = Command::new(path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    (sealed, TestChild(child), digest)
}

fn pidfd(child: &TestChild) -> OwnedFd {
    rustix::process::pidfd_open(
        rustix::process::Pid::from_raw(child.0.id() as i32).unwrap(),
        rustix::process::PidfdFlags::empty(),
    )
    .unwrap()
}

fn credentials(child: &TestChild) -> (u32, u32, u32) {
    (
        child.0.id(),
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
}

#[test]
fn original_peer_requires_exact_image_process_and_generation() {
    let (_image, child, digest) = sealed_child("/bin/sleep", &["30"]);
    let peer = Peer::admit(pidfd(&child), credentials(&child), digest).unwrap();
    peer.revalidate().unwrap();
    peer.require_parent(std::process::id()).unwrap();
    peer.require_start_time(peer.process.start_time_ticks())
        .unwrap();
    assert!(
        peer.require_start_time(peer.process.start_time_ticks() + 1)
            .is_err()
    );
    assert!(peer.require_parent(std::process::id() + 1).is_err());
    assert!(Peer::admit(pidfd(&child), credentials(&child), [0; 32]).is_err());
    let (_foreign_image, foreign, _) = sealed_child("/bin/sleep", &["30"]);
    assert!(Peer::admit(pidfd(&foreign), credentials(&child), digest).is_err());
    drop(child);
    assert!(peer.revalidate().is_err());
}

#[test]
fn same_pid_exec_invalidates_retained_image() {
    let (_image, mut child, digest) =
        sealed_child("/bin/sh", &["-c", "read line; exec /bin/sleep 30"]);
    let peer = Peer::admit(pidfd(&child), credentials(&child), digest).unwrap();
    child
        .0
        .stdin
        .take()
        .unwrap()
        .write_all(b"continue\n")
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while peer.revalidate().is_ok() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(peer.revalidate().is_err());
    assert!(child.0.try_wait().unwrap().is_none());
}

#[test]
fn unsealed_filesystem_image_is_not_a_protected_peer() {
    let child = TestChild(Command::new("/bin/sleep").arg("30").spawn().unwrap());
    let digest = measure_executable_sha256_v3(Path::new("/bin/sleep")).unwrap();
    assert!(Peer::admit(pidfd(&child), credentials(&child), digest).is_err());
}
