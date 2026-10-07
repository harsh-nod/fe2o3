//! Isolate exact EOF tests from unrelated libtest forks inheriting pipe writers.
use std::time::{Duration, Instant};

pub(crate) fn run(name: &str, run: impl FnOnce()) {
    const CASE: &str = "FE2O3_HOST_HANDOFF_EOF_CASE";
    const REPORT: &str = "FE2O3_HOST_HANDOFF_EOF_REPORT";
    if let Some(selected) = std::env::var_os(CASE) {
        assert_eq!(selected, std::ffi::OsStr::new(name));
        let report = std::env::var_os(REPORT).expect("isolated EOF completion path");
        run();
        std::fs::write(report, name).unwrap();
        return;
    }
    struct Child(Option<std::process::Child>);
    impl Drop for Child {
        fn drop(&mut self) {
            if let Some(mut child) = self.0.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    // Create the pipe after re-exec. The actual EOF assertion stays single-attempt.
    let report = tempfile::NamedTempFile::new().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut child = Child(Some(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env(CASE, name)
            .env(REPORT, report.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap(),
    ));
    let status = loop {
        assert!(
            Instant::now() < deadline,
            "isolated EOF test timed out: {name}"
        );
        if let Some(status) = child.0.as_mut().unwrap().try_wait().unwrap() {
            drop(child.0.take());
            break status;
        }
        std::thread::yield_now();
    };
    assert!(
        status.success(),
        "isolated EOF test failed: {name}: {status}"
    );
    // A misspelled exact filter must not pass by running zero tests.
    assert_eq!(std::fs::read(report.path()).unwrap(), name.as_bytes());
}
