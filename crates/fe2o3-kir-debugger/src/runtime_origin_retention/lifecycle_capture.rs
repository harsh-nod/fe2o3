//! Same-run factory; these raw adapters and the final assembly stay leaf-private.
use super::*;
use crate::{
    DebugTerminalFaultV1, DebugTranscriptCompletenessV1, DebugWaveWidthV1, DebuggerLimitsV1,
};
use fe2o3_kir_sim::*;
use std::cell::Cell;

#[path = "lifecycle_cursor.rs"]
mod cursor;
pub(super) use cursor::{QueryError, State};

/// Only this leaf can construct the paired owner. No Clone or raw-join API.
pub(super) struct LifecycleSession {
    observed: ObservedSession,
    lifecycle: Ledger,
    execution_completed: bool,
    debug_completed: bool,
}
const _: () = assert!(size_of::<LifecycleSession>() <= 8192);

struct EventAdapter<'a> {
    boundary: &'a Cell<usize>,
    ledger: &'a mut Ledger,
}
impl EventAdapter<'_> {
    fn accept(&mut self, event: &SimulationEventV1) -> SimulationEventSinkControlV1 {
        use SimulationEventKindV1::*;
        use SimulationEventSinkControlV1::{Continue, DropAndStop};
        let boundary = self.boundary.get();
        if self.ledger.gap.is_some() || !self.ledger.charge(boundary, 1) {
            return DropAndStop;
        }
        let (allocation, action, shape) = match event.kind {
            AllocationPreexisting {
                allocation,
                address_space,
                bytes,
            } => (
                allocation,
                Action::Preexisting,
                Some((address_space, bytes)),
            ),
            AllocationCreated {
                allocation,
                address_space,
                bytes,
            } => (allocation, Action::Create, Some((address_space, bytes))),
            AllocationReleased { allocation } => (allocation, Action::Release, None),
            _ => return Continue,
        };
        // The scan is prepaid even when it fails or finds its answer early.
        if !self.ledger.charge(boundary, self.ledger.rows.len()) {
            return DropAndStop;
        }
        let prior = self
            .ledger
            .rows
            .iter()
            .rev()
            .find(|row| row.allocation == allocation)
            .copied();
        let ordered = self
            .ledger
            .rows
            .last()
            .is_none_or(|row| row.boundary <= boundary);
        let valid = allocation != 0
            && ordered
            && match action {
                Action::Preexisting | Action::Create => {
                    prior.is_none() && allocation > self.ledger.last_allocation
                }
                Action::Release => prior.is_some_and(|row| row.action != Action::Release),
            };
        if !valid {
            self.ledger.fail(boundary, Gap::InvalidProducer);
            return DropAndStop;
        }
        let (address_space, bytes, scope) = match (shape, prior) {
            (Some((space, bytes)), _) => (
                space,
                bytes,
                if action == Action::Preexisting {
                    Scope::Dispatch
                } else {
                    Scope::created(space, event.invocation)
                },
            ),
            (None, Some(prior)) => {
                // Workgroup release may use a different representative invocation.
                let scope_matches = match prior.scope {
                    Scope::Dispatch => true,
                    Scope::Invocation(owner) => owner == event.invocation,
                    Scope::Workgroup { .. } => {
                        prior.scope == Scope::created(AddressSpace::Workgroup, event.invocation)
                    }
                };
                if !scope_matches {
                    self.ledger.fail(boundary, Gap::InvalidProducer);
                    return DropAndStop;
                }
                (prior.address_space, prior.bytes, prior.scope)
            }
            _ => {
                self.ledger.fail(boundary, Gap::InvalidProducer);
                return DropAndStop;
            }
        };
        if self.ledger.rows.len() == self.ledger.limit {
            self.ledger.fail(boundary, self.ledger.capacity_gap);
            return DropAndStop;
        }
        self.ledger.rows.push(Transition {
            boundary,
            allocation,
            action,
            scope,
            address_space,
            bytes,
            site: event.site,
        });
        if action != Action::Release {
            self.ledger.last_allocation = allocation;
        }
        Continue
    }
}
impl SimulationEventSinkV1 for EventAdapter<'_> {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        let _ = self.accept(event);
        Ok(())
    }
    fn record_controlled(
        &mut self,
        event: &SimulationEventV1,
    ) -> Result<SimulationEventSinkControlV1, SimulationEventSinkErrorV1> {
        Ok(self.accept(event))
    }
}

struct DebugAdapter<'a> {
    boundary: &'a Cell<usize>,
    inner: &'a mut OriginCollector,
}
impl SimulationDebugSinkV1 for DebugAdapter<'_> {
    fn record(&mut self, record: SimulationDebugRecordV1) -> Control {
        let control = self.inner.record(record);
        self.boundary.set(self.inner.record_count());
        control
    }
    fn wants_operation_origin_v1(&self) -> bool {
        self.inner.wants_operation_origin_v1()
    }
    fn record_with_operation_origin_v1(
        &mut self,
        record: SimulationDebugRecordV1,
        origin: Context,
    ) -> Control {
        let control = self.inner.record_with_operation_origin_v1(record, origin);
        self.boundary.set(self.inner.record_count());
        control
    }
    fn terminal_out_of_bounds_v2(&mut self, detail: SimulationOutOfBoundsV2) {
        self.inner.terminal_out_of_bounds_v2(detail);
    }
    fn terminal_barrier_divergence_v2(&mut self, detail: DivergentWorkgroupBarrierV2) {
        self.inner.terminal_barrier_divergence_v2(detail);
    }
}

pub(super) struct Configuration {
    pub(super) target: SimulationTargetV1,
    pub(super) simulation: SimulationLimitsV1,
    pub(super) debugger: DebuggerLimitsV1,
    pub(super) capture: SimulationDebugCaptureLimitsV1,
    pub(super) origins: Option<Limits>,
    pub(super) lifecycle: LifecycleLimits,
    pub(super) width: DebugWaveWidthV1,
}

pub(super) fn capture(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    schedule: SimulationScheduleRequestV1<'_>,
    config: Configuration,
) -> (
    Result<SimulationExecutionV1, SimulationErrorV1>,
    LifecycleSession,
) {
    let boundary = Cell::new(0);
    let mut ledger = Ledger::new(config.lifecycle, request.events == EventPolicyV1::Enabled);
    let mut origin = OriginCollector::new(config.debugger, config.origins);
    let result = module.simulate_debugged_scheduled_with_sinks(
        request,
        config.target,
        config.simulation,
        schedule,
        config.capture,
        &mut EventAdapter {
            boundary: &boundary,
            ledger: &mut ledger,
        },
        &mut DebugAdapter {
            boundary: &boundary,
            inner: &mut origin,
        },
    );
    let fault = match &result {
        Err(SimulationErrorV1::Execution(error)) => Some(DebugTerminalFaultV1 {
            ordinal: origin.record_count() as u64,
            invocation: error.invocation,
            site: error.site.clone(),
            kind: error.kind.clone(),
        }),
        _ => None,
    };
    let observed = origin.finish(
        crate::DebugKirIdentityV1 {
            digest: *module.identity().digest(),
            canonical_len: module.identity().canonical_length(),
        },
        config.width,
        fault,
    );
    let debug_completed =
        observed.transcript.completeness() == DebugTranscriptCompletenessV1::Complete;
    let owner = LifecycleSession {
        observed: observed.into_session(),
        lifecycle: ledger,
        execution_completed: result.is_ok(),
        debug_completed,
    };
    (result, owner)
}

#[cfg(test)]
pub(super) fn synthetic_event(
    ledger: &mut Ledger,
    boundary: usize,
    event: &SimulationEventV1,
) -> SimulationEventSinkControlV1 {
    EventAdapter {
        boundary: &Cell::new(boundary),
        ledger,
    }
    .accept(event)
}
