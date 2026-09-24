//! Closed controller-state model for deterministic CPU transport tests.
//! Not a native transport: no spawn, attach, PID signaling or GPU operation.
//! The production launch-owned process/FD/currentness boundary is still pending.
use super::parser::{
    MAX_ARTIFACT_BYTES, MemoryObject, POST_HOST_SYMBOL, PRE_HOST_SYMBOL, Refusal, parse_host_stop,
    parse_libraries, parse_original_elf,
};

/// Fixture identity only. A future real controller must derive this from
/// its owned launch/pidfd/current executable, never accept it from a request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProcessStamp {
    pub(super) pid: u32,
    pub(super) start_ticks: u64,
    pub(super) executable_sha256: [u8; 32],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Ready,
    ColdStop,
    ColdList,
    Activating,
    PublishedStop,
    PublishedList,
    ContentMatched,
    Exited,
    Poisoned,
}
#[derive(Debug)]
pub(super) struct TranscriptModel {
    process: ProcessStamp,
    artifact_bytes: usize,
    artifact_sha256: [u8; 32],
    phase: Phase,
    selected: Option<MemoryObject>,
}

/// Inert summary of fixture transcript validation, not acceptance/custody.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct TranscriptMatch {
    pub(super) artifact_bytes: usize,
    pub(super) artifact_sha256: [u8; 32],
    pub(super) physical_register_capture: bool,
    pub(super) runtime_loaded_success_observed: bool,
    pub(super) process_control_authority: bool,
}

impl TranscriptModel {
    pub(super) fn new(
        process: ProcessStamp,
        artifact_bytes: usize,
        artifact_sha256: [u8; 32],
    ) -> Result<Self, Refusal> {
        if process.pid == 0
            || process.start_ticks == 0
            || process.executable_sha256 == [0; 32]
            || artifact_bytes == 0
            || artifact_bytes > MAX_ARTIFACT_BYTES
        {
            return Err(Refusal::Bound);
        }
        Ok(Self {
            process,
            artifact_bytes,
            artifact_sha256,
            phase: Phase::Ready,
            selected: None,
        })
    }
    fn advance<T>(
        &mut self,
        expected: Phase,
        next: Phase,
        current: ProcessStamp,
        apply: impl FnOnce(&mut Self) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        if self.phase != expected {
            self.phase = Phase::Poisoned;
            self.selected = None;
            return Err(Refusal::State);
        }
        self.phase = Phase::Poisoned;
        if self.process != current {
            self.selected = None;
            return Err(Refusal::Changed);
        }
        match apply(self) {
            Ok(value) => {
                self.phase = next;
                Ok(value)
            }
            Err(error) => {
                self.selected = None;
                Err(error)
            }
        }
    }
    pub(super) fn cold_stop(&mut self, line: &[u8], current: ProcessStamp) -> Result<(), Refusal> {
        self.advance(Phase::Ready, Phase::ColdStop, current, |_| {
            parse_host_stop(line, b"1", b"1", PRE_HOST_SYMBOL)
        })
    }
    pub(super) fn cold_list(
        &mut self,
        line: &[u8],
        token: u64,
        current: ProcessStamp,
    ) -> Result<(), Refusal> {
        self.advance(Phase::ColdStop, Phase::ColdList, current, |state| {
            if parse_libraries(line, token, state.process.pid, state.artifact_bytes)?.is_some() {
                return Err(Refusal::State);
            }
            Ok(())
        })
    }
    pub(super) fn begin_activation(&mut self, current: ProcessStamp) -> Result<(), Refusal> {
        self.advance(Phase::ColdList, Phase::Activating, current, |_| Ok(()))
    }
    pub(super) fn published_stop(
        &mut self,
        line: &[u8],
        current: ProcessStamp,
    ) -> Result<(), Refusal> {
        self.advance(Phase::Activating, Phase::PublishedStop, current, |_| {
            parse_host_stop(line, b"2", b"1", POST_HOST_SYMBOL)
        })
    }
    pub(super) fn published_list(
        &mut self,
        line: &[u8],
        token: u64,
        current: ProcessStamp,
    ) -> Result<(), Refusal> {
        self.advance(
            Phase::PublishedStop,
            Phase::PublishedList,
            current,
            |state| {
                state.selected = Some(
                    parse_libraries(line, token, state.process.pid, state.artifact_bytes)?
                        .ok_or(Refusal::Artifact)?,
                );
                Ok(())
            },
        )
    }
    pub(super) fn original_elf(
        &mut self,
        line: &[u8],
        token: u64,
        current: ProcessStamp,
    ) -> Result<(), Refusal> {
        self.advance(
            Phase::PublishedList,
            Phase::ContentMatched,
            current,
            |state| {
                parse_original_elf(
                    line,
                    token,
                    state.selected.ok_or(Refusal::State)?,
                    state.artifact_sha256,
                )
            },
        )
    }
    pub(super) fn exit_observation(
        &mut self,
        current: ProcessStamp,
        inferior_exit: Option<i32>,
        debugger_reaped: bool,
        complete_streams: bool,
    ) -> Result<(), Refusal> {
        self.advance(Phase::ContentMatched, Phase::Exited, current, |_| {
            if inferior_exit != Some(0) || !debugger_reaped || !complete_streams {
                return Err(Refusal::Exit);
            }
            Ok(())
        })
    }
    pub(super) fn cancel(&mut self) {
        self.phase = Phase::Poisoned;
        self.selected = None;
    }
    pub(super) fn finish(self) -> Result<TranscriptMatch, Refusal> {
        if self.phase != Phase::Exited {
            return Err(Refusal::Incomplete);
        }
        Ok(TranscriptMatch {
            artifact_bytes: self.artifact_bytes,
            artifact_sha256: self.artifact_sha256,
            physical_register_capture: false,
            runtime_loaded_success_observed: false,
            process_control_authority: false,
        })
    }
}
