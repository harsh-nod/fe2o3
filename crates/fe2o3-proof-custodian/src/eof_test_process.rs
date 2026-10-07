//! Keep original-pipe EOF assertions away from unrelated concurrent test forks.
use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
const CASE: &str = "FE2O3_CUSTODIAN_EOF_CASE";
const REPORT: &str = "FE2O3_CUSTODIAN_EOF_REPORT";
struct OwnedChild(Option<Child>);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
pub(crate) fn isolated(module: &str, function: &str, run: impl FnOnce()) {
    let (_, module) = module.split_once("::").expect("qualified test module");
    let name = format!("{module}::{function}");
    if let Some(selected) = std::env::var_os(CASE) {
        assert_eq!(selected, std::ffi::OsStr::new(&name));
        run();
        std::fs::write(std::env::var_os(REPORT).expect("completion path"), name).unwrap();
        return;
    }
    let report = tempfile::NamedTempFile::new().unwrap();
    let mut child = OwnedChild(Some(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture", "--test-threads=1"])
            .env(CASE, &name)
            .env(REPORT, report.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    ));
    let deadline = Instant::now() + Duration::from_secs(20);
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
    assert_eq!(std::fs::read(report.path()).unwrap(), name.as_bytes());
}
