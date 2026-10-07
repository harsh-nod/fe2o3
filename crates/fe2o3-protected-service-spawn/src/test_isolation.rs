#![cfg(test)]

//! Pipe EOF assertions must not share a process with concurrent clone fixtures.
use std::{
    process::Command,
    time::{Duration, Instant},
};

/// Returns true only in the isolated single-test subprocess.
pub(crate) fn enter(test: &'static str) -> bool {
    const MARKER: &str = "FE2O3_PRIVATE_PIPE_CUSTODY_TEST";
    if std::env::var(MARKER).as_deref() == Ok(test) {
        return true;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(MARKER, test)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert!(
                    status.success(),
                    "isolated pipe custody fixture {test} failed: {status}"
                );
                return false;
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("isolated pipe custody fixture {test} timed out or wait failed: {result:?}");
            }
        }
    }
}
