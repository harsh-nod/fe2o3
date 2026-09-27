//! Run closure witnesses without concurrent test forks inheriting their writers.
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const CASE_ENV: &str = "FE2O3_SUPERVISOR_EOF_CASE";
const REPORT_ENV: &str = "FE2O3_SUPERVISOR_EOF_REPORT";

struct EofChild(Option<Child>);
impl Drop for EofChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub(crate) fn isolated_eof_case(name: &str, run: impl FnOnce()) {
    if let Some(selected) = std::env::var_os(CASE_ENV) {
        assert_eq!(selected, std::ffi::OsStr::new(name));
        let report = std::env::var_os(REPORT_ENV).expect("isolated EOF completion path");
        run();
        std::fs::write(report, name).unwrap();
        return;
    }

    // CLOEXEC writers may survive a concurrent test's fork until its exec. Create
    // all probes only after re-exec in a single-test process that never forks.
    let report = tempfile::NamedTempFile::new().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut child = EofChild(Some(
        Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(name)
            .args(["--nocapture", "--test-threads=1"])
            .env(CASE_ENV, name)
            .env(REPORT_ENV, report.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    ));
    let status = loop {
        assert!(
            Instant::now() < deadline,
            "isolated EOF case timed out: {name}"
        );
        if let Some(status) = child.0.as_mut().unwrap().try_wait().unwrap() {
            drop(child.0.take());
            break status;
        }
        // Poll only helper completion, never retry an EOF assertion.
        std::thread::yield_now();
    };
    assert!(
        status.success(),
        "isolated EOF case failed: {name}: {status}"
    );
    // An incorrect exact filter must not silently succeed with zero tests.
    assert_eq!(std::fs::read(report.path()).unwrap(), name.as_bytes());
}
