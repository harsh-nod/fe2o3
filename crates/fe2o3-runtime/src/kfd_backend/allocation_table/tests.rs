#![cfg(test)]

use super::*;
use std::io::Read;
use std::os::unix::process::ExitStatusExt;

struct OwnedChild(std::process::Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn bounded_stderr(stderr: &mut std::process::ChildStderr, bytes: &mut Vec<u8>) {
    let mut chunk = [0; 1024];
    loop {
        match stderr.read(&mut chunk) {
            Ok(0) => return,
            Ok(count) => {
                assert!(bytes.len() + count <= 16 * 1024, "child stderr limit");
                bytes.extend_from_slice(&chunk[..count]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return,
            Err(error) => panic!("child stderr: {error}"),
        }
    }
}

#[test]
fn ordinary_duplicate_insert_stops_without_unwinding_either_owner() {
    const CHILD: &str = "FE2O3_TEST_ORDINARY_ALLOCATION_COLLISION";
    const TEST: &str = "kfd_backend::allocation_table::tests::ordinary_duplicate_insert_stops_without_unwinding_either_owner";
    const READY: &str = "ordinary collision original and incoming records retained";
    const UNWIND: &str = "ordinary collision unwound";
    const PANIC: &str = "ordinary collision panic hook invoked";

    if std::env::var_os(CHILD).is_some() {
        // Piped crash handlers ignore RLIMIT_CORE; keep deliberate aborts local.
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let mut backend = KfdRuntimeBackendV1::mock();
        let original = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 4, 4)
            .unwrap();
        let incoming = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 4, 4)
            .unwrap();
        let incoming = backend.allocations.remove(&incoming).unwrap();
        assert!(backend.allocations.get(&original).is_some());
        struct UnwindProbe;
        impl Drop for UnwindProbe {
            fn drop(&mut self) {
                eprintln!("{UNWIND}");
            }
        }
        let _probe = UnwindProbe;
        std::panic::set_hook(Box::new(|_| eprintln!("{PANIC}")));
        eprintln!("{READY}");
        backend.allocations.insert(original, incoming);
        panic!("ordinary collision returned");
    }

    let mut child = OwnedChild(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .env("RUST_BACKTRACE", "0")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut stderr = child.0.stderr.take().unwrap();
    let flags = rustix::fs::fcntl_getfl(&stderr).unwrap();
    rustix::fs::fcntl_setfl(&stderr, flags | rustix::fs::OFlags::NONBLOCK).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut bytes = Vec::new();
    let status = loop {
        bounded_stderr(&mut stderr, &mut bytes);
        if let Some(status) = child.0.try_wait().unwrap() {
            bounded_stderr(&mut stderr, &mut bytes);
            break status;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "collision child timeout"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let stderr = String::from_utf8_lossy(&bytes);
    assert_eq!(status.signal(), Some(6), "{stderr}");
    assert!(stderr.contains(READY), "{stderr}");
    assert!(!stderr.contains(UNWIND), "{stderr}");
    assert!(!stderr.contains(PANIC), "{stderr}");
}
