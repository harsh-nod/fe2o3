//! Test-only callback timings. No phase observation is admission authority.
use super::SourceStage;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) const CHILD_PROGRESS: &str = "FE2O3_TEST_CHECKED_OUTPUT_PROGRESS_V1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Outcome {
    Complete,
    Refused,
    Panicked,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PhaseTiming {
    stage: SourceStage,
    outcome: Outcome,
    elapsed_millis: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ActivePhase {
    stage: SourceStage,
    started_millis: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Snapshot {
    schema: String,
    elapsed_millis: u64,
    outcome: Option<Outcome>,
    active: Option<ActivePhase>,
    phases: Vec<PhaseTiming>,
}

pub(super) struct CallbackProgress {
    started: Instant,
    active_started: Option<Instant>,
    path: Option<PathBuf>,
    snapshot: Snapshot,
}

impl Default for CallbackProgress {
    fn default() -> Self {
        Self::new(None)
    }
}

pub(super) fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn external_path(path: &Path, workspace: &Path) -> Result<PathBuf, String> {
    let name = path.file_name().ok_or("progress path has no filename")?;
    let parent = path.parent().ok_or("progress path has no parent")?;
    let parent = parent.canonicalize().map_err(|e| e.to_string())?;
    let workspace = workspace.canonicalize().map_err(|e| e.to_string())?;
    if parent.starts_with(workspace) {
        return Err("callback progress must be outside the checkout".to_owned());
    }
    Ok(parent.join(name))
}

impl CallbackProgress {
    fn new(path: Option<PathBuf>) -> Self {
        Self {
            started: Instant::now(),
            active_started: None,
            path,
            snapshot: Snapshot {
                schema: "fe2o3-test-callback-progress-v1".to_owned(),
                elapsed_millis: 0,
                outcome: None,
                active: None,
                phases: Vec::new(),
            },
        }
    }

    pub(super) fn from_environment() -> Self {
        let Some(path) = std::env::var_os(CHILD_PROGRESS) else {
            return Self::default();
        };
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        match external_path(Path::new(&path), &workspace) {
            Ok(path) => Self::new(Some(path)),
            Err(error) => {
                let _ = writeln!(std::io::stderr(), "callback progress disabled: {error}");
                Self::default()
            }
        }
    }

    pub(super) fn begin(&mut self, stage: SourceStage) {
        debug_assert!(self.snapshot.active.is_none());
        self.snapshot.elapsed_millis = elapsed_millis(self.started);
        self.snapshot.active = Some(ActivePhase {
            stage,
            started_millis: self.snapshot.elapsed_millis,
        });
        self.snapshot.outcome = None;
        self.persist();
        // The measured call excludes its progress-file publication overhead.
        self.active_started = Some(Instant::now());
    }

    pub(super) fn end(&mut self, outcome: Outcome) {
        let Some(active) = self.snapshot.active.take() else {
            return;
        };
        let elapsed = self.active_started.take().map(elapsed_millis).unwrap_or(0);
        self.snapshot.phases.push(PhaseTiming {
            stage: active.stage,
            outcome,
            elapsed_millis: elapsed,
        });
        self.snapshot.elapsed_millis = elapsed_millis(self.started);
        self.persist();
    }

    pub(super) fn run<T, E>(
        &mut self,
        stage: SourceStage,
        action: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        self.begin(stage);
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)) {
            Ok(result) => {
                self.end(if result.is_ok() {
                    Outcome::Complete
                } else {
                    Outcome::Refused
                });
                result
            }
            Err(panic) => {
                self.end(Outcome::Panicked);
                std::panic::resume_unwind(panic)
            }
        }
    }

    pub(super) fn finish(&mut self, outcome: Outcome) {
        self.end(outcome);
        self.snapshot.outcome = Some(outcome);
        self.snapshot.elapsed_millis = elapsed_millis(self.started);
        self.persist();
    }

    fn persist(&self) {
        let Some(path) = &self.path else { return };
        if let Err(error) = write_snapshot(path, &self.snapshot) {
            // Instrumentation failure must not replace the real compiler result.
            let _ = writeln!(std::io::stderr(), "callback progress unavailable: {error}");
        }
    }
}

fn write_snapshot(path: &Path, snapshot: &Snapshot) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(snapshot).map_err(|e| e.to_string())?;
    let temporary = path.with_extension("json.tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub(super) fn clear_inherited_jobserver(command: &mut std::process::Command) {
    for key in ["CARGO_MAKEFLAGS", "MAKEFLAGS", "MFLAGS"] {
        command.env_remove(key);
    }
}

#[test]
fn callback_progress_records_success_refusal_and_preserves_results() {
    let mut progress = CallbackProgress::default();
    assert_eq!(
        progress.run(SourceStage::SourceCollection, || Ok::<_, u32>(17)),
        Ok(17)
    );
    assert_eq!(
        progress.run(SourceStage::RankedChecks, || Err::<(), _>(23)),
        Err(23)
    );
    progress.finish(Outcome::Refused);
    assert_eq!(progress.snapshot.phases.len(), 2);
    assert_eq!(
        progress.snapshot.phases[0].stage,
        SourceStage::SourceCollection
    );
    assert_eq!(progress.snapshot.phases[0].outcome, Outcome::Complete);
    assert_eq!(progress.snapshot.phases[1].stage, SourceStage::RankedChecks);
    assert_eq!(progress.snapshot.phases[1].outcome, Outcome::Refused);
    assert!(progress.snapshot.active.is_none());
    assert_eq!(progress.snapshot.outcome, Some(Outcome::Refused));
    assert!(
        progress
            .snapshot
            .phases
            .iter()
            .all(|p| p.elapsed_millis <= progress.snapshot.elapsed_millis)
    );
}

#[test]
fn callback_progress_keeps_original_panic_and_exact_active_phase() {
    let mut progress = CallbackProgress::default();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        progress.run::<(), ()>(SourceStage::Policy4, || panic!("original phase panic"))
    }))
    .unwrap_err();
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"original phase panic"));
    progress.finish(Outcome::Panicked);
    assert_eq!(progress.snapshot.phases.len(), 1);
    assert_eq!(progress.snapshot.phases[0].stage, SourceStage::Policy4);
    assert_eq!(progress.snapshot.phases[0].outcome, Outcome::Panicked);
    assert!(progress.snapshot.active.is_none());
}

#[test]
fn callback_progress_publishes_active_phase_before_running_the_call() {
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-callback-progress");
    let path = scratch.path().join("progress.json");
    let mut progress = CallbackProgress::new(Some(path.clone()));
    assert_eq!(
        progress.run(SourceStage::NativeHandoff, || {
            let live: Snapshot = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(live.active.unwrap().stage, SourceStage::NativeHandoff);
            assert!(live.outcome.is_none());
            Err::<(), _>(31)
        }),
        Err(31)
    );
    progress.finish(Outcome::Refused);
    let final_state: Snapshot = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(final_state.active.is_none());
    assert_eq!(final_state.phases[0].outcome, Outcome::Refused);
    assert!(!path.with_extension("json.tmp").exists());
}

#[test]
fn callback_progress_refuses_checkout_paths_without_writing_them() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(external_path(&workspace.join("callback-progress.json"), &workspace).is_err());
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-progress-path");
    assert!(external_path(&scratch.path().join("progress.json"), &workspace).is_ok());
}

#[test]
fn callback_progress_io_failure_does_not_change_the_compiler_result() {
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-progress-io");
    let mut progress = CallbackProgress::new(Some(scratch.path().join("absent/progress.json")));
    assert_eq!(
        progress.run(SourceStage::Policy4, || Err::<(), _>(41)),
        Err(41)
    );
    assert_eq!(progress.snapshot.phases[0].outcome, Outcome::Refused);
}

#[test]
fn standalone_callback_drops_only_jobserver_variables() {
    let mut command = std::process::Command::new("unused-no-process-is-launched");
    command.env_clear().args(["--target", "amdgcn-amd-amdhsa"]);
    for key in ["CARGO_MAKEFLAGS", "MAKEFLAGS", "MFLAGS"] {
        command.env(key, "--jobserver-auth=8,9");
    }
    command
        .env("CARGO_FEATURE_KERNEL", "1")
        .env("FE2O3_TARGET", "gfx950");
    clear_inherited_jobserver(&mut command);
    let environment = command
        .get_envs()
        .collect::<std::collections::BTreeMap<_, _>>();
    for key in ["CARGO_MAKEFLAGS", "MAKEFLAGS", "MFLAGS"] {
        assert_eq!(
            environment
                .get(std::ffi::OsStr::new(key))
                .copied()
                .flatten(),
            None
        );
    }
    assert_eq!(
        environment.get(std::ffi::OsStr::new("CARGO_FEATURE_KERNEL")),
        Some(&Some(std::ffi::OsStr::new("1")))
    );
    assert_eq!(
        environment.get(std::ffi::OsStr::new("FE2O3_TARGET")),
        Some(&Some(std::ffi::OsStr::new("gfx950")))
    );
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            std::ffi::OsStr::new("--target"),
            std::ffi::OsStr::new("amdgcn-amd-amdhsa")
        ]
    );
}
