//! Private routes for closed layer, bank-rearm and tail owners; no selector.
use super::*;
use crate::device::{ScopedCountsV1, ScopedCurrentnessV1};

#[path = "engineering_gfx950_peer_scoped_capacity_fence_v1.rs"]
pub(super) mod capacity_fence;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    incarnation: u64,
    devices: [u64; 2],
    epochs: [u64; 2],
}

fn validate_identity(actual: Identity, expected: Option<Identity>) -> Result<()> {
    if actual.incarnation == 0
        || actual.devices.contains(&0)
        || actual.devices[0] == actual.devices[1]
        || expected.is_some_and(|expected| expected != actual)
    {
        return Err("scoped Group/participant/queue identity changed".into());
    }
    Ok(())
}

fn identity(group: &Gfx950EngineeringPeerGroupV1) -> Result<Identity> {
    group.require_active()?;
    let [left, right] = group.contexts.as_slice() else {
        return Err("scoped currentness requires two Contexts".into());
    };
    for context in [left, right] {
        if context.ordered_batch_poisoned
            || context.raw_timestamps_enabled
            || context.host_observation.is_some()
            || context
                .performance
                .is_some_and(|p| p.operational_currentness || p.profile || p.cache_kernel_admission)
        {
            return Err("scoped currentness requires unchanged full unprofiled Contexts".into());
        }
    }
    let value = Identity {
        incarnation: group.incarnation,
        devices: [left.unique_id, right.unique_id],
        epochs: [left.queue_epoch, right.queue_epoch],
    };
    validate_identity(value, None)?;
    Ok(value)
}

// This is exactly the idle queue predicate, not an enduring currentness proof.
// Keep the legacy immediate-full-fence method and its contract unchanged.
fn idle_queues(group: &mut Gfx950EngineeringPeerGroupV1) -> Result<()> {
    for context in &mut group.contexts {
        context.check_idle_queue_predicates()?;
    }
    Ok(())
}

fn expired(until: Instant) -> Result<()> {
    if Instant::now() >= until {
        return Err("scoped Group deadline expired".into());
    }
    Ok(())
}

struct Attempt<'a> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
    scope: Option<&'a mut ScopedCurrentnessV1>,
    committed: bool,
}

impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.group.poisoned = true;
            for context in &mut self.group.contexts {
                context.ordered_batch_poisoned = true;
                let device = context.backend.engineering_peer_device();
                // No allocation, reset, observation, cleanup or fallible call.
                if let Some(scope) = self.scope.as_deref_mut() {
                    scope.poison(&mut [device]);
                } else {
                    ScopedCurrentnessV1::poison_devices(&mut [device]);
                }
            }
        }
    }
}

pub(in crate::engineering_gfx950) struct Window {
    scope: ScopedCurrentnessV1,
    identity: Identity,
    until: Instant,
    closed: bool,
}

impl Window {
    // Only closed layer, bank-rearm and TailOperation owners retain
    // this data between effects. Each owns its exact quarantine boundary through
    // successful result construction and final deadline admission.
    pub(super) fn enter(group: &mut Gfx950EngineeringPeerGroupV1, until: Instant) -> Result<Self> {
        let mut attempt = Attempt {
            group,
            scope: None,
            committed: false,
        };
        expired(until)?;
        let retained = identity(attempt.group)?;
        let [left, right] = attempt.group.contexts.as_mut_slice() else {
            return Err("scoped entry Context roster".into());
        };
        let scope = ScopedCurrentnessV1::enter(
            &mut [
                left.backend.engineering_peer_device(),
                right.backend.engineering_peer_device(),
            ],
            until,
        )
        .map_err(explain)?;
        idle_queues(attempt.group)?;
        validate_identity(identity(attempt.group)?, Some(retained))?;
        expired(until)?;
        attempt.committed = true;
        Ok(Self {
            scope,
            identity: retained,
            until,
            closed: false,
        })
    }

    fn checkpoint(&mut self, group: &mut Gfx950EngineeringPeerGroupV1, idle: bool) -> Result<()> {
        let mut attempt = Attempt {
            group,
            scope: Some(&mut self.scope),
            committed: false,
        };
        if self.closed {
            return Err("scoped Group window already closed".into());
        }
        expired(self.until)?;
        validate_identity(identity(attempt.group)?, Some(self.identity))?;
        let [left, right] = attempt.group.contexts.as_mut_slice() else {
            return Err("scoped checkpoint Context roster".into());
        };
        attempt
            .scope
            .as_deref_mut()
            .ok_or("scoped observation missing")?
            .checkpoint(&mut [
                left.backend.engineering_peer_device(),
                right.backend.engineering_peer_device(),
            ])
            .map_err(explain)?;
        if idle {
            idle_queues(attempt.group)?;
        }
        validate_identity(identity(attempt.group)?, Some(self.identity))?;
        expired(self.until)?;
        attempt.committed = true;
        Ok(())
    }

    pub(super) fn finish(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
    ) -> Result<ScopedCountsV1> {
        let mut attempt = Attempt {
            group,
            scope: Some(&mut self.scope),
            committed: false,
        };
        if self.closed {
            return Err("scoped Group window already closed".into());
        }
        expired(self.until)?;
        validate_identity(identity(attempt.group)?, Some(self.identity))?;
        let [left, right] = attempt.group.contexts.as_mut_slice() else {
            return Err("scoped exit Context roster".into());
        };
        let counts = attempt
            .scope
            .as_deref_mut()
            .ok_or("scoped observation missing")?
            .finish(&mut [
                left.backend.engineering_peer_device(),
                right.backend.engineering_peer_device(),
            ])
            .map_err(explain)?;
        idle_queues(attempt.group)?;
        validate_identity(identity(attempt.group)?, Some(self.identity))?;
        expired(self.until)?;
        self.closed = true;
        attempt.committed = true;
        Ok(counts)
    }
}

/// Short private borrow; never stored in Group or supplied by a public caller.
pub(super) enum Currentness<'call> {
    Full,
    Scoped(&'call mut Window),
}

impl Currentness<'_> {
    // Only the closed zero-add census adapter may select this rank-local fence.
    pub(super) fn capacity_fence(
        &mut self,
        group: &mut Gfx950EngineeringPeerGroupV1,
    ) -> Result<u32> {
        if !matches!(self, Self::Scoped(_)) {
            return Err("capacity census requires a live scoped route".into());
        }
        capacity_fence::run(group, self)
    }

    pub(super) fn idle_group(&mut self, group: &mut Gfx950EngineeringPeerGroupV1) -> Result<()> {
        match self {
            Self::Full => check_contexts(&mut group.contexts, group.shared_full_currentness),
            Self::Scoped(window) => window.checkpoint(group, true),
        }
    }

    pub(super) fn publication(&mut self, group: &mut Gfx950EngineeringPeerGroupV1) -> Result<()> {
        match self {
            Self::Full => round::fresh_publication_fence(group),
            Self::Scoped(window) => {
                // Keep every queue exception at publication, never an idle check
                // on an in-flight peer. Both halves keep unwind quarantine.
                window.checkpoint(group, false)?;
                let mut attempt = Attempt {
                    group,
                    scope: Some(&mut window.scope),
                    committed: false,
                };
                for context in &mut attempt.group.contexts {
                    if Backend::observe_i64_acquire(
                        &mut context.internal[CONTROL].mapping,
                        PAGE_BYTES,
                        256,
                    )
                    .map_err(explain)?
                        != 0
                    {
                        return Err("peer round participant queue exception".into());
                    }
                }
                expired(window.until)?;
                attempt.committed = true;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_group_identity_rejects_each_substitution_without_native_contexts() {
        let expected = Identity {
            incarnation: 9,
            devices: [11, 22],
            epochs: [3, 4],
        };
        validate_identity(expected, Some(expected)).unwrap();
        let mut mutations = Vec::new();
        let mut value = expected;
        value.incarnation = 0;
        mutations.push(value);
        let mut value = expected;
        value.incarnation += 1;
        mutations.push(value);
        let mut value = expected;
        value.devices[0] = 0;
        mutations.push(value);
        let mut value = expected;
        value.devices.swap(0, 1);
        mutations.push(value);
        let mut value = expected;
        value.devices[1] = value.devices[0];
        mutations.push(value);
        let mut value = expected;
        value.epochs[0] += 1;
        mutations.push(value);
        let mut value = expected;
        value.epochs[1] += 1;
        mutations.push(value);
        for value in mutations {
            assert!(validate_identity(value, Some(expected)).is_err());
        }
    }
}

struct RankAttempt<'a> {
    context: &'a mut Context,
    window: &'a mut Window,
    committed: bool,
}
impl Drop for RankAttempt<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.context.ordered_batch_poisoned = true;
            self.window
                .scope
                .poison(&mut [self.context.backend.engineering_peer_device()]);
        }
    }
}

/// Cannot be created by a Context or public caller. A closed layer, bank or
/// tail owner retains the whole Group and quarantines ranks on error/unwind.
pub(in crate::engineering_gfx950) enum RankCurrentness<'call> {
    Full,
    Scoped {
        window: &'call mut Window,
        rank: usize,
    },
}
impl RankCurrentness<'_> {
    pub(in crate::engineering_gfx950) fn check(
        &mut self,
        context: &mut Context,
        lifecycle: bool,
    ) -> Result<()> {
        match self {
            Self::Full => context.check_currentness(lifecycle),
            Self::Scoped { window, rank } => {
                let mut attempt = RankAttempt {
                    context,
                    window,
                    committed: false,
                };
                if lifecycle || attempt.window.closed || *rank >= 2 {
                    return Err("scoped rank lifecycle/state admission".into());
                }
                expired(attempt.window.until)?;
                let expected = (
                    attempt.window.identity.devices[*rank],
                    attempt.window.identity.epochs[*rank],
                );
                if (attempt.context.unique_id, attempt.context.queue_epoch) != expected
                    || attempt.context.ordered_batch_poisoned
                    || attempt.context.raw_timestamps_enabled
                    || attempt.context.host_observation.is_some()
                    || attempt.context.performance.is_some_and(|p| {
                        p.operational_currentness || p.profile || p.cache_kernel_admission
                    })
                {
                    return Err("scoped rank Context substitution or policy".into());
                }
                attempt
                    .window
                    .scope
                    .checkpoint_rank(*rank, attempt.context.backend.engineering_peer_device())
                    .map_err(explain)?;
                if (attempt.context.unique_id, attempt.context.queue_epoch) != expected {
                    return Err("scoped rank trailing Context substitution".into());
                }
                expired(attempt.window.until)?;
                attempt.committed = true;
                Ok(())
            }
        }
    }
    pub(in crate::engineering_gfx950) fn idle(&mut self, context: &mut Context) -> Result<()> {
        if matches!(self, Self::Full) {
            return context.check_idle();
        }
        self.check(context, false)?;
        match self {
            Self::Full => unreachable!("full route returned above"),
            Self::Scoped { window, .. } => {
                let mut attempt = RankAttempt {
                    context,
                    window,
                    committed: false,
                };
                attempt.context.check_idle_queue_predicates()?;
                expired(attempt.window.until)?;
                attempt.committed = true;
                Ok(())
            }
        }
    }
}
impl Currentness<'_> {
    pub(super) fn rank<'short>(
        &'short mut self,
        group: &Gfx950EngineeringPeerGroupV1,
        rank: usize,
    ) -> Result<RankCurrentness<'short>> {
        match self {
            Self::Full => Ok(RankCurrentness::Full),
            Self::Scoped(window) => {
                if rank >= 2 || window.closed {
                    return Err("scoped rank route admission".into());
                }
                validate_identity(identity(group)?, Some(window.identity))?;
                expired(window.until)?;
                Ok(RankCurrentness::Scoped { window, rank })
            }
        }
    }
}
