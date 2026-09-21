//! Test-only callback timings. No phase observation is admission authority.
use super::SourceStage;
use crate::production_pipeline::checked_output_progress_v1 as subphases;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    policy4: Option<Policy4Progress>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ActivePhase {
    stage: SourceStage,
    started_millis: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    policy4: Option<Policy4Progress>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Policy4Progress {
    route: subphases::Route,
    active: Option<ActiveSubphase>,
    phases: Vec<SubphaseTiming>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unavailable: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ActiveSubphase {
    phase: subphases::Phase,
    started_millis: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SubphaseTiming {
    phase: subphases::Phase,
    outcome: subphases::PhaseOutcome,
    elapsed_millis: u64,
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
    state: Arc<Mutex<ProgressState>>,
}

struct ProgressState {
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
            state: Arc::new(Mutex::new(ProgressState {
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
            })),
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
        self.state().begin(stage);
    }

    pub(super) fn end(&mut self, outcome: Outcome) {
        self.state().end(outcome);
    }

    pub(super) fn finish(&mut self, outcome: Outcome) {
        self.state().finish(outcome);
    }

    pub(super) fn panic_failure(&self) -> super::SourceFailure {
        let state = self.state();
        let stage = state
            .snapshot
            .phases
            .iter()
            .find(|phase| phase.outcome == Outcome::Panicked)
            .map_or(SourceStage::Rustc, |phase| phase.stage);
        super::SourceFailure::new(
            stage,
            "rustc or callback panicked; see captured diagnostics",
        )
    }

    fn state(&self) -> MutexGuard<'_, ProgressState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(super) fn run<T, E>(
        &mut self,
        stage: SourceStage,
        action: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        self.begin(stage);
        let state = self.state.clone();
        let observed = || {
            if stage == SourceStage::Policy4 {
                subphases::with_sink(
                    Rc::new(move |event| match state.try_lock() {
                        Ok(mut state) => state.policy4_event(event),
                        Err(TryLockError::Poisoned(error)) => {
                            error.into_inner().policy4_event(event)
                        }
                        Err(TryLockError::WouldBlock) => {}
                    }),
                    action,
                )
            } else {
                action()
            }
        };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(observed)) {
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
}

impl ProgressState {
    fn begin(&mut self, stage: SourceStage) {
        debug_assert!(self.snapshot.active.is_none());
        self.snapshot.elapsed_millis = elapsed_millis(self.started);
        self.snapshot.active = Some(ActivePhase {
            stage,
            started_millis: self.snapshot.elapsed_millis,
            policy4: None,
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
            policy4: active.policy4,
        });
        self.snapshot.elapsed_millis = elapsed_millis(self.started);
        self.persist();
    }

    pub(super) fn finish(&mut self, outcome: Outcome) {
        self.end(outcome);
        self.snapshot.outcome = Some(outcome);
        self.snapshot.elapsed_millis = elapsed_millis(self.started);
        self.persist();
    }

    fn policy4_event(&mut self, event: subphases::Event) {
        let Some(active) = self.snapshot.active.as_mut() else {
            return;
        };
        if active.stage != SourceStage::Policy4 {
            return;
        }
        let (route, phase) = match event {
            subphases::Event::Started { route, phase }
            | subphases::Event::Finished { route, phase, .. } => (route, phase),
        };
        let detail = active.policy4.get_or_insert_with(|| Policy4Progress {
            route,
            active: None,
            phases: Vec::new(),
            unavailable: None,
        });
        if detail.unavailable.is_some() {
            return;
        }
        let ordered =
            detail.route == route && route.phases().get(detail.phases.len()) == Some(&phase);
        match event {
            subphases::Event::Started { .. } if ordered && detail.active.is_none() => {
                detail.active = Some(ActiveSubphase {
                    phase,
                    started_millis: elapsed_millis(self.started),
                });
            }
            subphases::Event::Finished {
                outcome,
                elapsed_millis,
                ..
            } if ordered
                && detail
                    .active
                    .as_ref()
                    .is_some_and(|active| active.phase == phase) =>
            {
                detail.active = None;
                detail.phases.push(SubphaseTiming {
                    phase,
                    outcome,
                    elapsed_millis,
                });
            }
            _ => {
                // Closed per-parent sequence: never grow beyond its five rows.
                detail.unavailable = Some("unexpected Policy4 diagnostic sequence".to_owned());
            }
        }
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
    assert_eq!(progress.state().snapshot.phases.len(), 2);
    assert_eq!(
        progress.state().snapshot.phases[0].stage,
        SourceStage::SourceCollection
    );
    assert_eq!(
        progress.state().snapshot.phases[0].outcome,
        Outcome::Complete
    );
    assert_eq!(
        progress.state().snapshot.phases[1].stage,
        SourceStage::RankedChecks
    );
    assert_eq!(
        progress.state().snapshot.phases[1].outcome,
        Outcome::Refused
    );
    assert!(progress.state().snapshot.active.is_none());
    assert_eq!(progress.state().snapshot.outcome, Some(Outcome::Refused));
    let state = progress.state();
    assert!(
        state
            .snapshot
            .phases
            .iter()
            .all(|p| { p.elapsed_millis <= state.snapshot.elapsed_millis })
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
    assert_eq!(progress.state().snapshot.phases.len(), 1);
    assert_eq!(
        progress.state().snapshot.phases[0].stage,
        SourceStage::Policy4
    );
    assert_eq!(
        progress.state().snapshot.phases[0].outcome,
        Outcome::Panicked
    );
    assert!(progress.state().snapshot.active.is_none());
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
    assert_eq!(
        progress.state().snapshot.phases[0].outcome,
        Outcome::Refused
    );
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

#[test]
fn callback_panic_attribution_preserves_refusal_and_panic() {
    for stage in [SourceStage::Policy4, SourceStage::NativeHandoff] {
        let mut progress = CallbackProgress::default();
        assert_eq!(progress.panic_failure().stage, SourceStage::Rustc);
        let refusal = progress.run(SourceStage::NativeSourceProof, || {
            Err::<(), _>("earlier refusal")
        });
        assert_eq!(refusal, Err("earlier refusal"));
        assert_eq!(progress.panic_failure().stage, SourceStage::Rustc);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            progress.run::<(), ()>(stage, || panic!("original phase panic"))
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<&str>(), Some(&"original phase panic"));
        let failure = progress.panic_failure();
        assert_eq!(failure.stage, stage);
        assert_eq!(
            failure.detail,
            "rustc or callback panicked; see captured diagnostics"
        );
        progress.finish(Outcome::Panicked);
        let state = progress.state();
        assert_eq!(state.snapshot.outcome, Some(Outcome::Panicked));
        assert!(state.snapshot.active.is_none());
        assert_eq!(state.snapshot.phases.len(), 2);
        assert_eq!(state.snapshot.phases[0].outcome, Outcome::Refused);
        assert_eq!(state.snapshot.phases[1].outcome, Outcome::Panicked);
    }
}

#[path = "production_rustc_driver_checked_output_subphase_progress_v1_tests.rs"]
mod subphase_tests;
