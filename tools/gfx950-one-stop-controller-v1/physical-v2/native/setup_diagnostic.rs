//! Fixed, bounded setup diagnostics only; never custody or admission authority.
use fe2o3_private_one_stop_protocol::Refusal;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SetupStage {
    Precheck,
    SpawnClock,
    Spawn,
    InitialChildWait,
    InitialChildStamp,
    PidConversion,
    PidfdOpen,
    ChildId,
    ChildWaitBeforeIdentity,
    PidfdBeforeIdentity,
    ChildStampBeforeIdentity,
    ExecutableLink,
    ExecutableMetadata,
    ExecutableRecheck,
    PidfdAfterIdentity,
    ChildStampAfterIdentity,
    ScopeMember,
    Cmdline,
    TakeStdin,
    TakeStdout,
    TakeStderr,
    StartStdoutReader,
    StartStderrReader,
    FinalClock,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ChildWaitObservation {
    Unobserved,
    Running,
    Exited(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SetupDiagnostic {
    stage: SetupStage,
    child_pid: Option<u32>,
    initial_stamp: Option<(u32, u32, u64)>,
    child_wait: ChildWaitObservation,
    debugger_custody_acquired: bool,
    readers_started: u8,
    cmdline_bytes: Option<usize>,
    cmdline_equal: Option<bool>,
    sent_commands: u64,
}
impl fmt::Display for SetupDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Only fixed enum names, bounded primitive values, and their Option tags.
        // No paths, environment, raw process strings or caller-controlled text.
        write!(
            f,
            "stage={:?},child_pid={:?},initial_stamp={:?},child_wait={:?},debugger_custody_acquired={},readers_started={},cmdline_bytes={:?},cmdline_equal={:?},sent_commands={}",
            self.stage,
            self.child_pid,
            self.initial_stamp,
            self.child_wait,
            self.debugger_custody_acquired,
            self.readers_started,
            self.cmdline_bytes,
            self.cmdline_equal,
            self.sent_commands,
        )
    }
}

pub(super) struct SetupTrace(SetupDiagnostic);
impl SetupTrace {
    pub(super) fn new() -> Self {
        Self(SetupDiagnostic {
            stage: SetupStage::Precheck,
            child_pid: None,
            initial_stamp: None,
            child_wait: ChildWaitObservation::Unobserved,
            debugger_custody_acquired: false,
            readers_started: 0,
            cmdline_bytes: None,
            cmdline_equal: None,
            sent_commands: 0,
        })
    }
    pub(super) fn step<T>(
        &mut self,
        stage: SetupStage,
        action: impl FnOnce() -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        self.0.stage = stage;
        action()
    }
    pub(super) fn require(
        &mut self,
        stage: SetupStage,
        predicate: impl FnOnce() -> Result<bool, Refusal>,
    ) -> Result<(), Refusal> {
        if self.step(stage, predicate)? {
            Ok(())
        } else {
            Err(Refusal::Changed)
        }
    }
    pub(super) fn child(&mut self, pid: u32) {
        self.0.child_pid = Some(pid);
    }
    pub(super) fn stamp(&mut self, pid: u32, parent: u32, start: u64) {
        self.0.initial_stamp = Some((pid, parent, start));
    }
    pub(super) fn waited(&mut self, status: Option<i32>) {
        self.0.child_wait = match status {
            None => ChildWaitObservation::Running,
            Some(status) => ChildWaitObservation::Exited(status),
        };
    }
    pub(super) fn custody_acquired(&mut self) {
        self.0.debugger_custody_acquired = true;
    }
    pub(super) fn stdout_reader_started(&mut self) {
        self.0.readers_started |= 1;
    }
    pub(super) fn stderr_reader_started(&mut self) {
        self.0.readers_started |= 2;
    }
    pub(super) fn cmdline(&mut self, bytes: usize, equal: bool) {
        self.0.cmdline_bytes = Some(bytes);
        self.0.cmdline_equal = Some(equal);
    }
    pub(super) fn freeze(&self, sent_commands: u64) -> SetupDiagnostic {
        let mut result = self.0;
        result.sent_commands = sent_commands;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAGES: [SetupStage; 24] = [
        SetupStage::Precheck,
        SetupStage::SpawnClock,
        SetupStage::Spawn,
        SetupStage::InitialChildWait,
        SetupStage::InitialChildStamp,
        SetupStage::PidConversion,
        SetupStage::PidfdOpen,
        SetupStage::ChildId,
        SetupStage::ChildWaitBeforeIdentity,
        SetupStage::PidfdBeforeIdentity,
        SetupStage::ChildStampBeforeIdentity,
        SetupStage::ExecutableLink,
        SetupStage::ExecutableMetadata,
        SetupStage::ExecutableRecheck,
        SetupStage::PidfdAfterIdentity,
        SetupStage::ChildStampAfterIdentity,
        SetupStage::ScopeMember,
        SetupStage::Cmdline,
        SetupStage::TakeStdin,
        SetupStage::TakeStdout,
        SetupStage::TakeStderr,
        SetupStage::StartStdoutReader,
        SetupStage::StartStderrReader,
        SetupStage::FinalClock,
    ];

    #[test]
    fn unknown_facts_stay_unknown_not_empty_or_successful() {
        let d = SetupTrace::new().freeze(0);
        assert_eq!(d.stage, SetupStage::Precheck);
        assert_eq!(d.child_pid, None);
        assert_eq!(d.initial_stamp, None);
        assert_eq!(d.child_wait, ChildWaitObservation::Unobserved);
        assert!(!d.debugger_custody_acquired);
        assert_eq!(d.readers_started, 0);
        assert_eq!(d.cmdline_bytes, None);
        assert_eq!(d.cmdline_equal, None);
        assert_eq!(d.sent_commands, 0);
    }

    #[test]
    fn every_step_preserves_the_original_refusal() {
        for stage in STAGES {
            for refusal in [
                Refusal::Changed,
                Refusal::Process,
                Refusal::Exit,
                Refusal::Artifact,
                Refusal::Bound,
                Refusal::Incomplete,
                Refusal::Deadline,
                Refusal::Shape,
                Refusal::Syntax,
                Refusal::Token,
                Refusal::Duplicate,
                Refusal::State,
                Refusal::Stop,
            ] {
                let mut trace = SetupTrace::new();
                assert_eq!(trace.step(stage, || Err::<(), _>(refusal)), Err(refusal));
                assert_eq!(trace.freeze(0).stage, stage);
            }
        }
    }

    #[test]
    fn requirement_false_is_changed_but_observation_errors_are_unchanged() {
        for stage in STAGES {
            let mut trace = SetupTrace::new();
            assert_eq!(trace.require(stage, || Ok(true)), Ok(()));
            assert_eq!(trace.require(stage, || Ok(false)), Err(Refusal::Changed));
            assert_eq!(
                trace.require(stage, || Err(Refusal::Process)),
                Err(Refusal::Process)
            );
        }
    }

    #[test]
    fn step_invokes_each_selected_observation_once() {
        let mut trace = SetupTrace::new();
        let mut count = 0;
        let value = trace.step(SetupStage::Cmdline, || {
            count += 1;
            Ok(17)
        });
        assert_eq!(value, Ok(17));
        assert_eq!(count, 1);
    }

    #[test]
    fn early_failure_keeps_order_and_skips_later_synthetic_observations() {
        for failed in 0..STAGES.len() {
            let mut trace = SetupTrace::new();
            let mut seen = Vec::new();
            let result = (|| {
                for (index, stage) in STAGES.iter().copied().enumerate() {
                    trace.require(stage, || {
                        seen.push(stage);
                        Ok(index != failed)
                    })?;
                }
                Ok::<_, Refusal>(())
            })();
            assert_eq!(result, Err(Refusal::Changed));
            assert_eq!(seen, STAGES[..=failed]);
            assert_eq!(trace.freeze(0).stage, STAGES[failed]);
        }
    }

    #[test]
    fn first_failure_snapshot_does_not_change_during_later_trace_activity() {
        let mut trace = SetupTrace::new();
        let _ = trace.require(SetupStage::ExecutableLink, || Ok(false));
        let failed = trace.freeze(0);
        trace.stdout_reader_started();
        trace.stderr_reader_started();
        trace.step(SetupStage::FinalClock, || Ok(())).unwrap();
        assert_eq!(failed.stage, SetupStage::ExecutableLink);
        assert_eq!(failed.readers_started, 0);
        assert_eq!(failed.sent_commands, 0);
    }

    #[test]
    fn running_and_exited_are_distinct_from_unobserved() {
        let mut trace = SetupTrace::new();
        trace.waited(None);
        assert_eq!(trace.freeze(0).child_wait, ChildWaitObservation::Running);
        trace.waited(Some(256));
        assert_eq!(
            trace.freeze(0).child_wait,
            ChildWaitObservation::Exited(256)
        );
        trace.child(7);
        trace.stamp(7, 3, 19);
        trace.custody_acquired();
        let d = trace.freeze(0);
        assert_eq!(d.child_pid, Some(7));
        assert_eq!(d.initial_stamp, Some((7, 3, 19)));
        assert!(d.debugger_custody_acquired);
    }

    #[test]
    fn readers_and_cmdline_are_only_recorded_after_observation() {
        let mut trace = SetupTrace::new();
        trace.stdout_reader_started();
        assert_eq!(trace.freeze(0).readers_started, 1);
        trace.stderr_reader_started();
        assert_eq!(trace.freeze(0).readers_started, 3);
        trace.cmdline(0, false);
        assert_eq!(trace.freeze(0).cmdline_bytes, Some(0));
        assert_eq!(trace.freeze(0).cmdline_equal, Some(false));
        assert_ne!(
            trace.freeze(0).cmdline_bytes,
            SetupTrace::new().freeze(0).cmdline_bytes
        );
    }

    #[test]
    fn entire_fixed_diagnostic_domain_fits_failure_envelope() {
        for stage in STAGES {
            let mut trace = SetupTrace::new();
            trace.0.stage = stage;
            trace.child(u32::MAX);
            trace.stamp(u32::MAX, u32::MAX, u64::MAX);
            trace.waited(Some(i32::MIN));
            trace.custody_acquired();
            trace.stdout_reader_started();
            trace.stderr_reader_started();
            trace.cmdline(usize::MAX, false);
            let raw = trace.freeze(u64::MAX).to_string();
            assert!(raw.len() < 1024);
            assert!(!raw.contains('\n') && !raw.contains('\r'));
            // Existing generic cleanup prefix plus fixed facts remains below 4 KiB.
            let report = format!(
                "one-stop debugger setup refused: {:?}; known cleanup={:?}; diagnostic={}; no whole-family claim\n",
                Refusal::Changed,
                Some(fe2o3_private_one_stop_protocol::Cleanup::default()),
                trace.freeze(u64::MAX),
            );
            assert!(report.len() <= 4096);
        }
    }
}
