//! Queue-rooted custody across the two-session XGMI retirement envelope.

#![forbid(unsafe_code)]

use super::owner_release::{OwnerProgressV1, SdmaOwnerReleaseMemoryV1};
use super::*;
use crate::topology::Gfx942XgmiRouteV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

// Moving native authority into custody must not allocate.
#[allow(clippy::large_enum_variant)]
enum State {
    Vacant,
    Pending {
        route: Gfx942XgmiRouteV1,
        owner: Gfx942SdmaQueueOwnerV1,
        progress: OwnerProgressV1,
    },
}

pub(super) struct RetirementRoot {
    state: State,
}

impl RetirementRoot {
    pub(super) const fn new() -> Self {
        Self {
            state: State::Vacant,
        }
    }

    pub(super) const fn is_vacant(&self) -> bool {
        matches!(self.state, State::Vacant)
    }
}

impl Drop for RetirementRoot {
    fn drop(&mut self) {
        if !self.is_vacant() {
            std::process::abort();
        }
    }
}

trait Context {
    type Memory: SdmaOwnerReleaseMemoryV1;
    fn prepare_doorbell_error(&mut self) -> Result<String, Gfx942SdmaErrorV1>;
    fn memory(&mut self) -> &mut Self::Memory;
    fn validate(
        &mut self,
        route: Gfx942XgmiRouteV1,
        root: &RetirementRoot,
    ) -> Result<(), Gfx942SdmaErrorV1>;
    fn quarantine(&mut self, destination: bool, root: &RetirementRoot);
    fn poison(&mut self, root: &RetirementRoot);
}

fn preflight_shape(
    owner: &Gfx942SdmaQueueOwnerV1,
    route: Gfx942XgmiRouteV1,
) -> Result<(), Gfx942SdmaErrorV1> {
    owner.require_live()?;
    if owner.engine_index != Some(route.recommended_engine_id())
        || owner.records.len() != GFX942_SDMA_RING_SLOT_COUNT_V1
        || owner.xgmi_records.len() != GFX942_SDMA_RING_SLOT_COUNT_V1
        || owner.persistent_window_slots.len() != GFX942_SDMA_RING_SLOT_COUNT_V1
        || owner.persistent_window_records.len() != GFX942_SDMA_RING_SLOT_COUNT_V1
        || owner.ring.is_none()
        || owner.control.is_none()
        || owner.completions.is_none()
    {
        return Err(Gfx942SdmaErrorV1::Contract("XGMI retirement owner shape"));
    }
    owner
        .doorbell
        .as_ref()
        .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA doorbell"))?
        .validate_release_v1()
        .map_err(|_| Gfx942SdmaErrorV1::Contract("XGMI retirement doorbell owner"))
}

fn retire_with(
    queue: &mut Gfx942NativeXgmiSdmaQueueV1,
    context: &mut impl Context,
) -> Result<(), Gfx942SdmaErrorV1> {
    if !queue.retirement.is_vacant() {
        if queue.owner.is_some() {
            std::process::abort();
        }
        return Err(Gfx942SdmaErrorV1::Contract(
            "XGMI retirement root is occupied",
        ));
    }
    let owner = queue
        .owner
        .as_ref()
        .ok_or(Gfx942SdmaErrorV1::Contract("missing XGMI SDMA queue owner"))?;
    let preflight = preflight_shape(owner, queue.route);
    if preflight.is_ok()
        && (owner.records.iter().any(Option::is_some)
            || owner.xgmi_records.iter().any(Option::is_some)
            || owner.persistent_window_slots.iter().any(Option::is_some)
            || owner.persistent_window_records.iter().any(Option::is_some)
            || owner.uncertain_xgmi_ticket.is_some())
    {
        return Err(Gfx942SdmaErrorV1::Pending);
    }
    let doorbell_failure = if preflight.is_ok() {
        Some(context.prepare_doorbell_error()?)
    } else {
        None
    };
    queue.retirement.state = State::Pending {
        route: queue.route,
        owner: queue.owner.take().unwrap_or_else(|| std::process::abort()),
        progress: OwnerProgressV1::default(),
    };
    let root = &mut queue.retirement;
    // Full route validation can quarantine a session. Root authority before
    // even the opening validation, and retain it through closing validation.
    let result = catch_unwind(AssertUnwindSafe(|| {
        preflight?;
        let State::Pending { route, .. } = &root.state else {
            std::process::abort();
        };
        let route = *route;
        context.validate(route, root)?;
        let State::Pending {
            owner, progress, ..
        } = &mut root.state
        else {
            std::process::abort();
        };
        progress.destroy_in_place(owner, context.memory(), |_| {
            Gfx942SdmaErrorV1::Doorbell(doorbell_failure.unwrap_or_else(|| std::process::abort()))
        })?;
        progress.release_resources_in_place(owner, context.memory())?;
        context.validate(route, root)?;
        Ok::<(), Gfx942SdmaErrorV1>(())
    }));
    if matches!(result, Ok(Ok(()))) {
        root.state = State::Vacant;
        return Ok(());
    }
    let State::Pending { owner, .. } = &mut root.state else {
        std::process::abort();
    };
    owner.poisoned = true;
    let mut secondary = None;
    for stage in 0..3 {
        let outcome = catch_unwind(AssertUnwindSafe(|| match stage {
            0 => context.quarantine(false, root),
            1 => context.quarantine(true, root),
            _ => context.poison(root),
        }));
        if let Err(payload) = outcome {
            if secondary.is_none() {
                secondary = Some(payload);
            } else {
                // Arbitrary secondary panic destructors must not replace the
                // original failure or prevent the remaining terminalizers.
                std::mem::forget(payload);
            }
        }
    }
    match result {
        Err(original) => {
            std::mem::forget(secondary);
            resume_unwind(original)
        }
        Ok(Err(error)) => {
            if let Some(payload) = secondary {
                resume_unwind(payload);
            }
            Err(error)
        }
        Ok(Ok(())) => std::process::abort(),
    }
}

struct Sessions<'a> {
    source: &'a mut SharedGttMemorySessionV1,
    destination: &'a mut SharedGttMemorySessionV1,
}

impl Context for Sessions<'_> {
    type Memory = SharedGttMemorySessionV1;

    fn prepare_doorbell_error(&mut self) -> Result<String, Gfx942SdmaErrorV1> {
        preallocate_doorbell_failure_message()
    }

    fn memory(&mut self) -> &mut Self::Memory {
        self.source
    }

    fn validate(
        &mut self,
        route: Gfx942XgmiRouteV1,
        _root: &RetirementRoot,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        self.source
            .validate_gfx942_xgmi_route_with_peer(self.destination, route)
            .map_err(Into::into)
    }

    fn quarantine(&mut self, destination: bool, _root: &RetirementRoot) {
        let session = if destination {
            &mut self.destination
        } else {
            &mut self.source
        };
        let _ = session.quarantine_queue_composition("XGMI SDMA terminal retirement");
    }

    fn poison(&mut self, _root: &RetirementRoot) {
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

pub(super) fn retire(
    queue: &mut Gfx942NativeXgmiSdmaQueueV1,
    source: &mut SharedGttMemorySessionV1,
    destination: &mut SharedGttMemorySessionV1,
) -> Result<(), Gfx942SdmaErrorV1> {
    retire_with(
        queue,
        &mut Sessions {
            source,
            destination,
        },
    )
}

#[cfg(test)]
mod tests;
