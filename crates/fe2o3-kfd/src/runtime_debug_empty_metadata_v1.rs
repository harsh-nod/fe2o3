//! Successor-only local metadata retirement. No imported acknowledgement.
use super::{MetadataErrorV1 as E, MetadataStorageV1, Phase, fe2o3_runtime_debug_state_v1};

pub(in super::super) trait DebugEmptyRetirementTransportV1 {
    fn check_currentness(&mut self) -> Result<(), E>;
    fn disable_runtime(&mut self) -> Result<(), E>;
    fn clear_trap(&mut self) -> Result<(), E>;
}
impl MetadataStorageV1 {
    pub(in super::super) fn withdraw_empty_queue(
        &mut self,
        transport: &mut impl DebugEmptyRetirementTransportV1,
    ) -> Result<(), E> {
        transport.check_currentness()?;
        self.transition(false, |_| {
            transport.check_currentness()?;
            fe2o3_runtime_debug_state_v1();
            transport.check_currentness()
        })
    }
    pub(in super::super) fn disable_empty_runtime(
        &mut self,
        transport: &mut impl DebugEmptyRetirementTransportV1,
    ) -> Result<(), E> {
        self.check_empty_phase(Phase::ActiveAbsent)?;
        transport.check_currentness()?;
        self.phase = Phase::Poisoned;
        transport.disable_runtime()?;
        transport.check_currentness()?;
        // Native return, not an authenticated debugger unload/ACK.
        self.phase = Phase::LocalRuntimeDisabled;
        Ok(())
    }
    pub(in super::super) fn clear_empty_trap(
        &mut self,
        transport: &mut impl DebugEmptyRetirementTransportV1,
    ) -> Result<(), E> {
        self.check_empty_phase(Phase::LocalRuntimeDisabled)?;
        transport.check_currentness()?;
        self.phase = Phase::Poisoned;
        transport.clear_trap()?;
        transport.check_currentness()?;
        self.phase = Phase::LocalTrapCleared;
        Ok(())
    }
    pub(in super::super) fn empty_local_trap_cleared(&self) -> bool {
        self.opener_pid == std::process::id() && self.phase == Phase::LocalTrapCleared
    }
    fn check_empty_phase(&self, phase: Phase) -> Result<(), E> {
        if self.opener_pid != std::process::id() {
            return Err(E::ProcessChanged);
        }
        if self.phase != phase
            || self.record[0].root.map != 0
            || self.record[0].root.state != super::abi::RT_CONSISTENT_V1
        {
            return Err(E::Transition);
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "runtime_debug_empty_metadata_v1_tests.rs"]
mod tests;
