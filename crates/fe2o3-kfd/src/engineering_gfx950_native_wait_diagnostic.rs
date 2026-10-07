//! One bounded stderr record per native execution in diagnostic builds only.

use std::io::Write;
use std::time::Instant;

use super::wait_observation as observation;
pub(super) use observation::SignalState;

pub(super) struct Diagnostic {
    started: Instant,
    pub(super) observation: observation::Observation,
    publication_ns: Option<u64>,
}

impl Diagnostic {
    pub(super) fn new() -> Self {
        Self {
            started: Instant::now(),
            observation: observation::Observation::default(),
            publication_ns: None,
        }
    }

    pub(super) fn stamp(&mut self) -> u64 {
        match u64::try_from(self.started.elapsed().as_nanos()) {
            Ok(value) => value,
            Err(_) => {
                self.observation.valid = false;
                u64::MAX
            }
        }
    }

    pub(super) fn published(&mut self) {
        self.publication_ns = Some(self.stamp());
    }

    pub(super) fn emit(&mut self, queue_epoch: u64, next: u64, dispatches: usize, success: bool) {
        let end_ns = self.stamp();
        let value = &self.observation;
        let window = |window: Option<observation::ReadWindow>| {
            window.map(|value| [value.before_ns, value.after_ns])
        };
        let record = serde_json::json!({
            "schema": "Fe2o3NativeWaitObservationV1",
            "clock": "host_monotonic_ns_since_native_execution_entry",
            "queue_epoch": queue_epoch,
            "next_write": next,
            "dispatches": dispatches,
            "execution_succeeded": success,
            "observation_valid": value.valid,
            "publication_return_ns": self.publication_ns,
            "execution_return_ns": end_ns,
            "signal_reads": value.reads,
            "pending_reads": value.pending_reads,
            "completed_reads": value.completed_reads,
            "unexpected_reads": value.unexpected_reads,
            "last_pending_read_ns": window(value.last_pending),
            "first_completed_read_ns": window(value.first_completed),
            "completion_bracket_ns": value.completion_bracket_ns(),
            "max_inter_read_ns": value.max_inter_read_ns,
            "signal_read_ns": value.read_ns,
            "pause_count": value.pauses,
            "pause_elapsed_ns": value.pause_ns,
            "max_pause_ns": value.max_pause_ns,
            "post_read_ns": value.post_read_ns,
            "currentness_nested_ns": value.currentness_ns,
            "retirement_signals_ns": value.retirement_ns,
            "poll_ready_ns": value.ready_ns,
        });
        // Diagnostics neither use the protocol stream nor change the result.
        let _ = writeln!(std::io::stderr().lock(), "{record}");
    }
}
