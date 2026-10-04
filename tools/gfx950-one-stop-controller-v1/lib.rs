//! Generic one-stop MI2 protocol library. The separately compiled native binary
//! provides a fixed launch-owned adapter, hard-disabled by its absent source
//! profile. The library itself launches no process and grants no native custody.
//! A successful result is a historical protocol observation, never authority.
mod controller;
mod rocgdb_mi_parser_v3;
mod syntax;
mod target_report;
mod transitions;
pub use controller::observe;
pub use syntax::Refusal;

pub const MI_INTERPRETER: &str = "mi2";
pub const TARGET_PATH: &str = "/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/target-gfx950-one-stop-observer-r3/debug/fe2o3-private-one-stop-target";
pub const ENTRY: &str = "fe2o3_gfx950_one_stop_process_entry_v1";
pub const CHECKPOINT: &str = "fe2o3_gfx950_one_stop_prepublication_checkpoint_v1";
pub const MAX_LINE: usize = 192 * 1024;
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RECORDS: usize = 8192;
pub const MAX_COMMANDS: u64 = 64;
pub const DEADLINE_NS: u64 = 60_000_000_000;

/// Same custody operations as reviewed R6, with an explicit original-start clock.
/// The separate native binary implements this trait with real child/pidfd and
/// selected-inferior custody, but its source profile remains unbound. Any adapter
/// must own those actual handles before returning from observe_child/entry/current
/// and must enforce the same I/O bounds itself.
/// The caller must not claim a supplied/mock implementation establishes custody.
pub trait Peer {
    fn send(&mut self, token: u64, command: &str) -> Result<(), Refusal>;
    fn next(&mut self) -> Result<Option<Vec<u8>>, Refusal>;
    fn observe_child(&mut self, pid: u32) -> Result<(), Refusal>;
    fn entry(&mut self) -> Result<(), Refusal>;
    fn current(&mut self) -> Result<(), Refusal>;
    /// Nanoseconds from the original start, BEFORE the adapter's fresh spawn.
    fn elapsed_ns(&mut self) -> Result<u64, Refusal>;
    /// Rehash bound files and join actual EOF/reap/pidfd/reader observations.
    fn finish(&mut self) -> Result<Cleanup, Refusal>;
    /// Exactly one bounded failure teardown of known owners, never unrelated PIDs.
    /// This cannot certify success and must preserve unknown family uncertainty.
    fn cleanup(&mut self) -> Cleanup;
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct Cleanup {
    pub owned_inferior_pidfd_exit_observed: bool,
    pub debugger_direct_child_reaped: bool,
    pub streams_complete: bool,
    pub reader_threads_joined: bool,
    pub unadmitted_inferior_cleanup_not_proven: bool,
    pub inferior_reaped_by_controller: bool,
    pub cleanup_deadline_expired: bool,
}
impl Cleanup {
    fn complete(self) -> bool {
        self.owned_inferior_pidfd_exit_observed
            && self.debugger_direct_child_reaped
            && self.streams_complete
            && self.reader_threads_joined
            && !self.unadmitted_inferior_cleanup_not_proven
            && !self.inferior_reaped_by_controller
            && !self.cleanup_deadline_expired
    }
}
#[derive(Debug, serde::Serialize)]
pub struct ProtocolObservation {
    pub schema: &'static str,
    pub process_id: u32,
    pub producer_process_id: String,
    pub producer_rows: usize,
    pub observed_host_thread: String,
    pub observed_gpu_thread: String,
    pub observed_internal_checkpoint_id: String,
    pub same_producer_transition_sequence_observed: bool,
    pub independent_target_local_completion_observed: bool,
    pub normal_mi_exit_observed: bool,
    pub independent_native_tuple_replay: bool,
    pub physical_register_capture: bool,
    pub physical_memory_capture: bool,
    pub process_control_authority: bool,
    pub source_or_launch_authority: bool,
    pub whole_family_cleanup_proved: bool,
    pub general_host_exclusion_proved: bool,
    pub operational_qualification: bool,
    pub cleanup: Cleanup,
}
#[derive(Debug)]
pub struct FailedObservation {
    pub refusal: Refusal,
    pub cleanup: Cleanup,
}
#[cfg(test)]
mod tests;
