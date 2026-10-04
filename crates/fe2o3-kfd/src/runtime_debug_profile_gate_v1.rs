//! Exclusive debug-profile reservation and exact local terminal completion.
//!
//! This reserves a profile only. It does not enable a runtime, install a trap,
//! expose metadata or retain native resources. Only the separate concrete local
//! empty-queue terminal witness can complete an exposed reservation.

#[cfg(feature = "engineering-gfx950")]
use super::KFD_RUNTIME_GATE;
use super::LinuxDoorbellErrorV1;
#[cfg(any(test, feature = "engineering-gfx950"))]
use super::{ProcessGlobalKfdRuntimeGateV1, ProcessKfdRuntimeStateV1, lock_runtime_gate_v1};
#[cfg(any(test, feature = "engineering-gfx950"))]
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};
#[cfg(any(test, feature = "engineering-gfx950"))]
use std::rc::Rc;
#[cfg(any(test, feature = "engineering-gfx950"))]
use std::sync::Mutex;

// Deliberately never reset on successful teardown: an inherited global mutex
// and gate must not be adopted by a fork child, even when the parent was idle.
static GLOBAL_GATE_PROCESS_V1: AtomicU32 = AtomicU32::new(0);

fn bind_gate_process_v1(
    binding: &AtomicU32,
    observed_pid: u32,
) -> Result<(), LinuxDoorbellErrorV1> {
    if observed_pid == 0 {
        return Err(LinuxDoorbellErrorV1::ProcessChanged);
    }
    match binding.compare_exchange(0, observed_pid, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => Ok(()),
        Err(bound_pid) if bound_pid == observed_pid => Ok(()),
        Err(_) => Err(LinuxDoorbellErrorV1::ProcessChanged),
    }
}

/// Checked before taking the actual global mutex, including teardown paths.
/// No native resource is released when this rejects a fork/process change.
pub(super) fn check_global_runtime_gate_process_v1() -> Result<(), LinuxDoorbellErrorV1> {
    bind_gate_process_v1(&GLOBAL_GATE_PROCESS_V1, std::process::id())
}

#[cfg(any(test, feature = "engineering-gfx950"))]
impl ProcessGlobalKfdRuntimeGateV1 {
    fn reserve_debug_profile_v1(&mut self, opener_pid: u32) -> Result<u64, LinuxDoorbellErrorV1> {
        if opener_pid == 0 {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        if self.is_blocked() {
            return Err(LinuxDoorbellErrorV1::Runtime(
                "process-global gate poisoned",
            ));
        }
        match self.runtime {
            ProcessKfdRuntimeStateV1::Disabled => {}
            ProcessKfdRuntimeStateV1::Enabled {
                opener_pid: owner_pid,
                ..
            }
            | ProcessKfdRuntimeStateV1::DebugReserved {
                opener_pid: owner_pid,
                ..
            } if owner_pid != opener_pid => {
                return Err(LinuxDoorbellErrorV1::ProcessChanged);
            }
            ProcessKfdRuntimeStateV1::Enabled { .. } => {
                return Err(LinuxDoorbellErrorV1::Runtime(
                    "plain/debug runtime profile conflict",
                ));
            }
            ProcessKfdRuntimeStateV1::DebugReserved { .. } => {
                return Err(LinuxDoorbellErrorV1::Runtime(
                    "debug runtime reservation already held",
                ));
            }
            ProcessKfdRuntimeStateV1::Poisoned => {
                return Err(LinuxDoorbellErrorV1::Runtime(
                    "process runtime context poisoned",
                ));
            }
        }
        // Reserve a fresh nonzero identity before modifying occupancy. This is
        // an in-process token identity, not a device, runtime or telemetry ID.
        let reservation_id = self.next_debug_reservation;
        let next = reservation_id
            .checked_add(1)
            .filter(|_| reservation_id != 0)
            .ok_or(LinuxDoorbellErrorV1::Runtime(
                "debug runtime reservation capacity",
            ))?;
        self.runtime = ProcessKfdRuntimeStateV1::DebugReserved {
            opener_pid,
            reservation_id,
            exposed: false,
        };
        self.next_debug_reservation = next;
        Ok(reservation_id)
    }

    fn begin_debug_external_transition_v1(
        &mut self,
        opener_pid: u32,
        reservation_id: u64,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if self.is_blocked() {
            return Err(LinuxDoorbellErrorV1::Runtime(
                "process-global gate poisoned",
            ));
        }
        match self.runtime {
            ProcessKfdRuntimeStateV1::DebugReserved {
                opener_pid: owner_pid,
                reservation_id: owner_id,
                exposed: false,
            } if owner_pid == opener_pid && owner_id == reservation_id && reservation_id != 0 => {
                // Set before any future external effect, never afterward.
                self.runtime = ProcessKfdRuntimeStateV1::DebugReserved {
                    opener_pid,
                    reservation_id,
                    exposed: true,
                };
                Ok(())
            }
            _ => {
                self.poison();
                Err(LinuxDoorbellErrorV1::Runtime(
                    "debug runtime reservation identity or phase",
                ))
            }
        }
    }

    fn drop_debug_reservation_v1(&mut self, opener_pid: u32, reservation_id: u64) {
        match self.runtime {
            ProcessKfdRuntimeStateV1::DebugReserved {
                opener_pid: owner_pid,
                reservation_id: owner_id,
                exposed: false,
            } if owner_pid == opener_pid
                && owner_id == reservation_id
                && reservation_id != 0
                && !self.permanently_poisoned =>
            {
                // No native pointer or transition was exposed. A concurrent
                // teardown arm remains counted and continues to block admission.
                self.runtime = ProcessKfdRuntimeStateV1::Disabled;
            }
            _ => self.poison(),
        }
    }

    #[cfg(all(feature = "engineering-gfx950", target_endian = "little"))]
    fn finish_local_debug_teardown_v1(
        &mut self,
        opener_pid: u32,
        reservation_id: u64,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if !self.is_blocked()
            && matches!(self.runtime,
            ProcessKfdRuntimeStateV1::DebugReserved {
                opener_pid: owner, reservation_id: identity, exposed: true,
            } if owner == opener_pid && identity == reservation_id && identity != 0)
        {
            self.runtime = ProcessKfdRuntimeStateV1::Disabled;
            Ok(())
        } else {
            self.poison();
            Err(LinuxDoorbellErrorV1::Runtime(
                "local debug terminal reservation identity",
            ))
        }
    }

    // Synthetic tests remain separate from the private real terminal witness.
    #[cfg(test)]
    fn acknowledge_synthetic_debug_teardown_v1(
        &mut self,
        opener_pid: u32,
        reservation_id: u64,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if !self.is_blocked()
            && matches!(self.runtime,
            ProcessKfdRuntimeStateV1::DebugReserved {
                opener_pid: owner_pid, reservation_id: owner_id, exposed: true,
            } if owner_pid == opener_pid && owner_id == reservation_id && reservation_id != 0)
        {
            self.runtime = ProcessKfdRuntimeStateV1::Disabled;
            Ok(())
        } else {
            self.poison();
            Err(LinuxDoorbellErrorV1::Runtime(
                "synthetic debug teardown identity or phase",
            ))
        }
    }
}

#[cfg(any(test, feature = "engineering-gfx950"))]
struct DebugRuntimeReservationV1<'gate> {
    gate: &'gate Mutex<ProcessGlobalKfdRuntimeGateV1>,
    process_binding: &'gate AtomicU32,
    opener_pid: u32,
    reservation_id: u64,
    finished: bool,
    not_send_sync: PhantomData<Rc<()>>,
}

#[cfg(any(test, feature = "engineering-gfx950"))]
impl<'gate> DebugRuntimeReservationV1<'gate> {
    fn reserve(
        gate: &'gate Mutex<ProcessGlobalKfdRuntimeGateV1>,
        process_binding: &'gate AtomicU32,
        observed_pid: u32,
    ) -> Result<Self, LinuxDoorbellErrorV1> {
        // Must precede the mutex: it may be inherited locked after a fork.
        bind_gate_process_v1(process_binding, observed_pid)?;
        let reservation_id = lock_runtime_gate_v1(gate).reserve_debug_profile_v1(observed_pid)?;
        Ok(Self {
            gate,
            process_binding,
            opener_pid: observed_pid,
            reservation_id,
            finished: false,
            not_send_sync: PhantomData,
        })
    }

    fn check_process(&self, observed_pid: u32) -> Result<(), LinuxDoorbellErrorV1> {
        if self.opener_pid != observed_pid {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        bind_gate_process_v1(self.process_binding, observed_pid)
    }

    fn begin_external_transition(&mut self) -> Result<(), LinuxDoorbellErrorV1> {
        self.check_process(std::process::id())?;
        lock_runtime_gate_v1(self.gate)
            .begin_debug_external_transition_v1(self.opener_pid, self.reservation_id)
    }

    #[cfg(test)]
    fn acknowledge_synthetic_teardown(mut self) -> Result<(), LinuxDoorbellErrorV1> {
        self.check_process(std::process::id())?;
        lock_runtime_gate_v1(self.gate)
            .acknowledge_synthetic_debug_teardown_v1(self.opener_pid, self.reservation_id)?;
        self.finished = true;
        Ok(())
    }
}

#[cfg(any(test, feature = "engineering-gfx950"))]
impl Drop for DebugRuntimeReservationV1<'_> {
    fn drop(&mut self) {
        if self.finished || self.check_process(std::process::id()).is_err() {
            // The sticky process binding already refuses future gate access in
            // the fork child. Do not lock or roll back its inherited claim.
            // This token owns no resources: the consuming cold owner must
            // separately retain mappings/metadata/trap storage on PID drift.
            return;
        }
        lock_runtime_gate_v1(self.gate)
            .drop_debug_reservation_v1(self.opener_pid, self.reservation_id);
    }
}

/// Crate-private, move-only, process-local profile exclusion. This token is not
/// device admission or permission to issue any native request. Registration of
/// its first caller belongs to the future consuming cold debug owner.
#[cfg(feature = "engineering-gfx950")]
pub(crate) struct ProcessGlobalKfdDebugReservationV1(DebugRuntimeReservationV1<'static>);

#[cfg(feature = "engineering-gfx950")]
impl ProcessGlobalKfdDebugReservationV1 {
    pub(crate) fn reserve() -> Result<Self, LinuxDoorbellErrorV1> {
        DebugRuntimeReservationV1::reserve(
            &KFD_RUNTIME_GATE,
            &GLOBAL_GATE_PROCESS_V1,
            std::process::id(),
        )
        .map(Self)
    }

    /// Accepts only the actual same-owner terminal object minted by the fixed
    /// empty-queue retirement transaction. No IDs, flags or reports are inputs.
    #[cfg(target_endian = "little")]
    pub(crate) fn finish_local_empty_teardown(
        witness: &mut crate::engineering_gfx950::DebugLocalTeardownWitnessV1,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        let reservation = witness.reservation_mut();
        // Must precede inherited mutex access, including the terminal path.
        reservation.0.check_process(std::process::id())?;
        lock_runtime_gate_v1(reservation.0.gate).finish_local_debug_teardown_v1(
            reservation.0.opener_pid,
            reservation.0.reservation_id,
        )?;
        reservation.0.finished = true;
        Ok(())
    }

    /// Distinct actual completed-dispatch witness; never an EmptyQueue witness.
    #[cfg(target_endian = "little")]
    pub(crate) fn finish_local_one_stop_teardown(
        witness: &mut crate::engineering_gfx950::DebugOneStopTeardownWitnessV1,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        let reservation = witness.reservation_mut();
        reservation.0.check_process(std::process::id())?;
        lock_runtime_gate_v1(reservation.0.gate).finish_local_debug_teardown_v1(
            reservation.0.opener_pid,
            reservation.0.reservation_id,
        )?;
        reservation.0.finished = true;
        Ok(())
    }

    /// Call before the first SET_TRAP_HANDLER or other external publication.
    /// This marks possible exposure, not successful runtime enable.
    pub(crate) fn begin_external_transition(&mut self) -> Result<(), LinuxDoorbellErrorV1> {
        self.0.begin_external_transition()
    }
}

#[cfg(test)]
#[path = "runtime_debug_profile_gate_v1_tests.rs"]
mod tests;

#[cfg(all(test, feature = "engineering-gfx950", target_endian = "little"))]
#[path = "runtime_debug_empty_gate_v1_tests.rs"]
mod empty_tests;
