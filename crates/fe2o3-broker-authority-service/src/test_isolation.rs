#![cfg(test)]

//! EOF assertions must not share a descriptor table with concurrent clone fixtures.
use std::{
    process::Command,
    time::{Duration, Instant},
};

pub(crate) fn enter(qualified: &'static str) -> bool {
    const MARKER: &str = "FE2O3_PRIVATE_BROKER_EOF_TEST";
    let (_, test) = qualified
        .split_once("::")
        .expect("crate-qualified fixture name");
    if std::env::var(MARKER).as_deref() == Ok(test) {
        return true;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(MARKER, test)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert!(
                    status.success(),
                    "isolated broker EOF fixture {test}: {status}"
                );
                return false;
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("isolated broker EOF fixture {test} timed out or wait failed: {result:?}");
            }
        }
    }
}
