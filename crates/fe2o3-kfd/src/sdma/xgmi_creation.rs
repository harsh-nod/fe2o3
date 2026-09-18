//! Caller-rooted custody across the two-session XGMI creation envelope.

use super::*;
use crate::topology::Gfx942XgmiRouteV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

// Live authority must move inline without a fallible allocation.
#[allow(clippy::large_enum_variant)]
enum State {
    Vacant,
    Pending {
        route: Gfx942XgmiRouteV1,
        attempted: Option<TerminalGfx942SdmaQueueCreationV1>,
    },
    Confirmed {
        route: Gfx942XgmiRouteV1,
        owner: Gfx942SdmaQueueOwnerV1,
    },
}

/// Move-only custody for one directional XGMI queue creation.
///
/// An occupied root must remain owned alongside both participating sessions
/// until process teardown. It provides observations only, not cleanup authority.
/// Dropping an occupied root aborts, including while unwinding.
#[must_use = "occupied XGMI creation custody requires process teardown"]
pub struct Gfx942NativeXgmiSdmaQueueCreationRootV1 {
    state: State,
}

impl Gfx942NativeXgmiSdmaQueueCreationRootV1 {
    pub const fn new() -> Self {
        Self {
            state: State::Vacant,
        }
    }

    pub const fn is_vacant(&self) -> bool {
        matches!(self.state, State::Vacant)
    }

    pub const fn terminal_stage(&self) -> Option<&'static str> {
        match &self.state {
            State::Vacant => None,
            State::Pending {
                attempted: None, ..
            } => Some("memory-terminal-no-queue-custody"),
            State::Pending {
                attempted: Some(_), ..
            } => Some("create-attempt-retained"),
            State::Confirmed { .. } => Some("confirmed-queue-retained"),
        }
    }

    fn begin(&mut self, route: Gfx942XgmiRouteV1) {
        if !self.is_vacant() {
            std::process::abort();
        }
        self.state = State::Pending {
            route,
            attempted: None,
        };
    }

    fn attempted(&mut self) -> &mut Option<TerminalGfx942SdmaQueueCreationV1> {
        match &mut self.state {
            State::Pending { attempted, .. } => attempted,
            _ => std::process::abort(),
        }
    }

    fn confirm(&mut self, owner: Gfx942SdmaQueueOwnerV1) {
        let State::Pending {
            route,
            attempted: None,
        } = &self.state
        else {
            std::process::abort();
        };
        self.state = State::Confirmed {
            route: *route,
            owner,
        };
    }

    fn finish(&mut self) -> Gfx942NativeXgmiSdmaQueueV1 {
        let State::Confirmed { route, owner } = std::mem::replace(&mut self.state, State::Vacant)
        else {
            std::process::abort();
        };
        Gfx942NativeXgmiSdmaQueueV1 {
            route,
            owner: Some(owner),
        }
    }
}

impl Default for Gfx942NativeXgmiSdmaQueueCreationRootV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Gfx942NativeXgmiSdmaQueueCreationRootV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let route = match &self.state {
            State::Vacant => None,
            State::Pending { route, .. } | State::Confirmed { route, .. } => Some(route),
        };
        formatter
            .debug_struct("Gfx942NativeXgmiSdmaQueueCreationRootV1")
            .field("route", &route)
            .field("terminal_stage", &self.terminal_stage())
            .finish_non_exhaustive()
    }
}

impl Drop for Gfx942NativeXgmiSdmaQueueCreationRootV1 {
    fn drop(&mut self) {
        if !self.is_vacant() {
            std::process::abort();
        }
    }
}

trait Context {
    type Host;
    type Arm;
    fn prepare(
        &mut self,
        route: Gfx942XgmiRouteV1,
    ) -> Result<(KfdGfx942SdmaXgmiEngineId, Self::Host), Gfx942SdmaErrorV1>;
    fn arm(&mut self) -> Result<Self::Arm, Gfx942SdmaErrorV1>;
    fn validate(&mut self, route: Gfx942XgmiRouteV1) -> Result<(), Gfx942SdmaErrorV1>;
    fn key(&mut self) -> Result<QueueKeyV1, Gfx942SdmaErrorV1>;
    // On success, no injectable work may occur between taking the attempted
    // owner and returning it to the caller's infallible Confirmed transition.
    fn create_owner(
        &mut self,
        key: QueueKeyV1,
        engine: KfdGfx942SdmaXgmiEngineId,
        host: Self::Host,
        arm: &Self::Arm,
        attempted: &mut Option<TerminalGfx942SdmaQueueCreationV1>,
    ) -> Result<Gfx942SdmaQueueOwnerV1, Gfx942SdmaErrorV1>;
    fn disarm(&mut self, arm: Self::Arm);
    fn quarantine(&mut self, destination: bool, root: &Gfx942NativeXgmiSdmaQueueCreationRootV1);
    fn poison(&mut self, root: &Gfx942NativeXgmiSdmaQueueCreationRootV1);
}

fn create_with(
    context: &mut impl Context,
    route: Gfx942XgmiRouteV1,
    root: &mut Gfx942NativeXgmiSdmaQueueCreationRootV1,
) -> Result<Gfx942NativeXgmiSdmaQueueV1, Gfx942NativeXgmiSdmaQueueCreationFailureV1> {
    if !root.is_vacant() {
        return Err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::terminal(
            Gfx942SdmaErrorV1::Contract("XGMI creation root is occupied"),
            root,
        ));
    }
    let (engine, host) = context
        .prepare(route)
        .map_err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::retryable)?;
    let mut arm = Some(
        context
            .arm()
            .map_err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::retryable)?,
    );
    root.begin(route);
    // The root and arm stay outside the caught suffix. On failure, both
    // sessions are quarantined before the still-armed guard is dropped.
    let result = catch_unwind(AssertUnwindSafe(|| {
        context.validate(route)?;
        let key = context.key()?;
        let owner = context.create_owner(
            key,
            engine,
            host,
            arm.as_ref().unwrap_or_else(|| std::process::abort()),
            root.attempted(),
        )?;
        root.confirm(owner);
        context.validate(route)?;
        context.disarm(arm.take().unwrap_or_else(|| std::process::abort()));
        Ok::<(), Gfx942SdmaErrorV1>(())
    }));
    if matches!(result, Ok(Ok(()))) {
        return Ok(root.finish());
    }
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
                // Panic payload destructors are arbitrary user code.
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
            Err(Gfx942NativeXgmiSdmaQueueCreationFailureV1::terminal(
                error, root,
            ))
        }
        Ok(Ok(())) => std::process::abort(),
    }
}

struct Sessions<'a> {
    source: &'a mut SharedGttMemorySessionV1,
    destination: &'a mut SharedGttMemorySessionV1,
}

impl Context for Sessions<'_> {
    type Host = PreparedGfx942SdmaQueueHostResourcesV1;
    type Arm = ProcessGlobalKfdRuntimeCreationArmV1;

    fn prepare(
        &mut self,
        route: Gfx942XgmiRouteV1,
    ) -> Result<(KfdGfx942SdmaXgmiEngineId, Self::Host), Gfx942SdmaErrorV1> {
        if self.source.gpu_id() != route.source_gpu_id() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "XGMI queue executing GPU does not match directional route",
            ));
        }
        let engine =
            admit_kfd_gfx942_sdma_xgmi_engine_mask(route.link().recommended_sdma_engine_id_mask())
                .map_err(|_| Gfx942SdmaErrorV1::Contract("XGMI SDMA route engine mask"))?;
        if engine.value() != route.recommended_engine_id() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "XGMI SDMA route engine identity",
            ));
        }
        let host =
            prepare_sdma_queue_host_resources().map_err(recover_sdma_owner_preflight_error)?;
        Ok((engine, host))
    }

    fn arm(&mut self) -> Result<Self::Arm, Gfx942SdmaErrorV1> {
        arm_process_global_kfd_runtime_gate_for_creation_v1().map_err(|_| {
            Gfx942SdmaErrorV1::Contract("process-global KFD creation gate unavailable")
        })
    }

    fn validate(&mut self, route: Gfx942XgmiRouteV1) -> Result<(), Gfx942SdmaErrorV1> {
        self.source
            .validate_gfx942_xgmi_route_with_peer(self.destination, route)
            .map_err(Into::into)
    }

    fn key(&mut self) -> Result<QueueKeyV1, Gfx942SdmaErrorV1> {
        self.source.next_xgmi_sdma_queue_key().map_err(Into::into)
    }

    fn create_owner(
        &mut self,
        key: QueueKeyV1,
        engine: KfdGfx942SdmaXgmiEngineId,
        host: Self::Host,
        arm: &Self::Arm,
        attempted: &mut Option<TerminalGfx942SdmaQueueCreationV1>,
    ) -> Result<Gfx942SdmaQueueOwnerV1, Gfx942SdmaErrorV1> {
        Gfx942SdmaQueueOwnerV1::create_on_xgmi_engine_in_armed_scope(
            self.source,
            key,
            engine,
            host,
            arm,
            attempted,
        )
    }

    fn disarm(&mut self, arm: Self::Arm) {
        arm.disarm();
    }

    fn quarantine(&mut self, destination: bool, _root: &Gfx942NativeXgmiSdmaQueueCreationRootV1) {
        let session = if destination {
            &mut self.destination
        } else {
            &mut self.source
        };
        let _ = session.quarantine_queue_composition("XGMI SDMA terminal creation");
    }

    fn poison(&mut self, _root: &Gfx942NativeXgmiSdmaQueueCreationRootV1) {
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

pub(super) fn create(
    source: &mut SharedGttMemorySessionV1,
    destination: &mut SharedGttMemorySessionV1,
    route: Gfx942XgmiRouteV1,
    root: &mut Gfx942NativeXgmiSdmaQueueCreationRootV1,
) -> Result<Gfx942NativeXgmiSdmaQueueV1, Gfx942NativeXgmiSdmaQueueCreationFailureV1> {
    create_with(
        &mut Sessions {
            source,
            destination,
        },
        route,
        root,
    )
}

#[cfg(test)]
mod tests;
