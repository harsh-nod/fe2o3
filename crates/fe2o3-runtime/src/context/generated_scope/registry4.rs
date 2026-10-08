//! Four copied results share one common native destruction obligation.

use super::*;
use crate::kfd_backend::RegistryStorageV1;
use crate::{
    RuntimeGfx942GeneratedResidentRegistryV1 as Registry, RuntimeGfx942RegistryCompletionCarrierV1,
};
use fe2o3_resource_accounting::ResourceCreditAccountV1;
use std::{future::Future, pin::Pin, task::Poll};

mod scope;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Adopting,
    Active,
    Rearming,
    Closing,
    Closed,
    Unknown,
}

struct Root<P, const N: usize> {
    prepared: Option<RuntimeGfx942PreparedV1<Registry<P, N>>>,
    storage: Option<RegistryStorageV1>,
    roster: GeneratedHostRosterV1,
    hold: ContextUnpublishedHoldV1,
    state: State,
    outcomes: [Option<Outcome>; N],
    domains: [crate::RuntimeGeneratedResultDomainV1; N],
    replies: [crate::async_engine::RuntimeAsyncReplyV1<Outcome>; N],
    futures: [Option<crate::RuntimeAsyncCommandFutureV1<Outcome>>; N],
    second: Option<CycleResults<N>>,
    cycle: u8,
}

struct CycleResults<const N: usize> {
    outcomes: [Option<Outcome>; N],
    domains: [crate::RuntimeGeneratedResultDomainV1; N],
    replies: [crate::async_engine::RuntimeAsyncReplyV1<Outcome>; N],
    futures: [Option<crate::RuntimeAsyncCommandFutureV1<Outcome>>; N],
}

/// Exact same-scope member locator, not a scalar completion or DATA release.
pub struct RuntimeGfx942Registry4TicketV1<'scope, const N: usize = 4> {
    identity: Rc<()>,
    member: usize,
    cycle: u8,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

/// Observes a copied, decoded result while common DATA may still be live.
/// Dropping the observer never cancels or retires the original registry.
///
/// ```compile_fail
/// fn send<T: Send>() {}
/// send::<fe2o3_runtime::RuntimeGfx942Registry4ResultFutureV1<'static>>();
/// ```
#[must_use]
pub struct RuntimeGfx942Registry4ResultFutureV1<'scope, const N: usize = 4> {
    future: crate::RuntimeAsyncCommandFutureV1<Outcome>,
    _identity: Rc<()>,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl<const N: usize> Future for RuntimeGfx942Registry4ResultFutureV1<'_, N> {
    type Output = Result<(), RuntimeGfx942ScopeErrorV1>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.future).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(outcome)) => {
                Poll::Ready(outcome.map_err(RuntimeGfx942ScopeErrorV1::Readback))
            }
            Poll::Ready(Err(_)) => Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::Unknown)),
        }
    }
}

/// Same-thread four-recipe scope over one original Context hold.
/// Each copied result has its own observer; the source, native DATA debit and
/// common backing stay retained until every recipe is recycled and real common
/// destruction completes. The distinct repeat profile permits two fixed cycles
/// of those same originals, not rolling source replacement. Neither profile
/// admits graphs, mixed-order hardware completion, interrupt wakes or thousands
/// of native operations.
pub struct RuntimeGfx942Registry4ScopeV1<'scope, 'env, P, const N: usize = 4> {
    epoch: scope_epoch::Owner,
    context: &'env mut RuntimeContextV1<KfdRuntimeBackendV1>,
    root: Option<Root<P, N>>,
    storage: Option<RegistryStorageV1>,
    repeat2: bool,
    identity: Rc<()>,
    deadline: Instant,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl<P, const N: usize> Drop for RuntimeGfx942Registry4ScopeV1<'_, '_, P, N> {
    fn drop(&mut self) {
        if self
            .root
            .as_ref()
            .is_some_and(|root| root.state != State::Closed)
        {
            std::process::abort();
        }
        self.epoch.close();
    }
}

impl<'scope, P: RuntimeGfx942RegistryCompletionCarrierV1, const N: usize>
    RuntimeGfx942Registry4ScopeV1<'scope, '_, P, N>
{
    fn tickets(&self, cycle: u8) -> [RuntimeGfx942Registry4TicketV1<'scope, N>; N] {
        core::array::from_fn(|member| RuntimeGfx942Registry4TicketV1 {
            identity: Rc::clone(&self.identity),
            member,
            cycle,
            invariant: PhantomData,
        })
    }

    fn try_submit_inner<E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        stream: RuntimeStreamIdV1,
        prepare: impl FnOnce(&fe2o3_kfd::CheckedGfx942XnackMinusDevice) -> Result<Registry<P, N>, E>,
    ) -> Result<(), RuntimeGfx942ScopedSubmissionErrorV1<E>> {
        if self.root.is_some() || self.storage.is_none() {
            return Err(RuntimeGfx942ScopedSubmissionErrorV1::Scope(
                RuntimeGfx942ScopeErrorV1::Capacity,
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(RuntimeGfx942ScopedSubmissionErrorV1::Scope(
                RuntimeGfx942ScopeErrorV1::Deadline,
            ));
        }
        let _permit = self.epoch.enter().map_err(|e| {
            RuntimeGfx942ScopedSubmissionErrorV1::Scope(RuntimeGfx942ScopeErrorV1::Context(
                e.into(),
            ))
        })?;
        let mut prepared = self
            .context
            .with_gfx942_preparation_device_v1(device, prepare)
            .map_err(RuntimeGfx942ScopedSubmissionErrorV1::Preparation)?;
        self.admit(&mut prepared, stream)
            .map_err(RuntimeGfx942ScopedSubmissionErrorV1::Scope)?;
        // Admission has no native entry: move the same owner directly into the
        // already-rooted record before returning or allowing progress.
        self.root
            .as_mut()
            .unwrap_or_else(|| std::process::abort())
            .prepared = Some(prepared);
        Ok(())
    }

    fn admit(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<Registry<P, N>>,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        if prepared.value().repeat2 != self.repeat2 {
            return Err(RuntimeGfx942ScopeErrorV1::Readback(
                RuntimeGfx942ReadbackErrorV1::InvalidStorage,
            ));
        }
        // Preserve eager first-domain observations before checking their errors.
        let values = prepared
            .value()
            .members
            .each_ref()
            .map(|member| member.registry_completion_domain_v1());
        let domains = collect_domains(values)?;
        let second = if self.repeat2 {
            if N != 4 {
                return Err(RuntimeGfx942ScopeErrorV1::Capacity);
            }
            let mut values = core::array::from_fn(|_| None);
            for (slot, member) in values.iter_mut().zip(&prepared.value().members) {
                *slot = Some(
                    member
                        .registry_second_completion_domain_v1()
                        .map_err(RuntimeGfx942ScopeErrorV1::Readback)?,
                );
            }
            Some(CycleResults::new(
                values.map(|value| value.unwrap_or_else(|| std::process::abort())),
            ))
        } else {
            None
        };
        if let Some(second) = &second {
            for i in 0..N {
                if domains
                    .iter()
                    .any(|first| first.same_original_v1(&second.domains[i]))
                    || second.domains[..i]
                        .iter()
                        .any(|prior| prior.same_original_v1(&second.domains[i]))
                {
                    return Err(RuntimeGfx942ScopeErrorV1::Readback(
                        RuntimeGfx942ReadbackErrorV1::InvalidStorage,
                    ));
                }
            }
        }
        for i in 0..N {
            for j in 0..i {
                if domains[i].same_original_v1(&domains[j]) {
                    return Err(RuntimeGfx942ScopeErrorV1::Readback(
                        RuntimeGfx942ReadbackErrorV1::InvalidStorage,
                    ));
                }
            }
        }
        let roster = self
            .context
            .reserve_gfx942_registry4_v1(prepared)
            .map_err(RuntimeGfx942ScopeErrorV1::Reservation)?;
        self.context
            .preflight_gfx942_registry4_v1(prepared, &roster, stream)
            .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
        let pairs: [_; N] =
            core::array::from_fn(|_| crate::async_engine::RuntimeAsyncReplyV1::pair());
        let mut futures = core::array::from_fn(|_| None);
        let mut index = 0;
        let replies = pairs.map(|(reply, future)| {
            futures[index] = Some(future);
            index += 1;
            reply
        });
        let hold = self
            .context
            .hold_unpublished_stream_with_access_v1(stream, None)
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        self.root = Some(Root {
            prepared: None,
            storage: self.storage.take(),
            roster,
            hold,
            state: State::Adopting,
            outcomes: core::array::from_fn(|_| None),
            domains,
            replies,
            futures,
            second,
            cycle: 0,
        });
        Ok(())
    }

    pub fn result_future_v1(
        &mut self,
        ticket: &RuntimeGfx942Registry4TicketV1<'scope, N>,
    ) -> Result<RuntimeGfx942Registry4ResultFutureV1<'scope, N>, RuntimeGfx942ScopeErrorV1> {
        self.validate_ticket(ticket)?;
        let root = self.root.as_mut().unwrap_or_else(|| std::process::abort());
        let futures = if ticket.cycle == 0 {
            &mut root.futures
        } else {
            &mut root
                .second
                .as_mut()
                .unwrap_or_else(|| std::process::abort())
                .futures
        };
        let future = futures[ticket.member]
            .take()
            .ok_or(RuntimeGfx942ScopeErrorV1::CompletionObserverTaken)?;
        Ok(RuntimeGfx942Registry4ResultFutureV1 {
            future,
            _identity: Rc::clone(&self.identity),
            invariant: PhantomData,
        })
    }

    fn validate_ticket(
        &self,
        ticket: &RuntimeGfx942Registry4TicketV1<'scope, N>,
    ) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        if !Rc::ptr_eq(&self.identity, &ticket.identity)
            || ticket.member >= N
            || self.root.is_none()
            || ticket.cycle > 1
            || (ticket.cycle == 1 && self.root.as_ref().is_none_or(|root| root.second.is_none()))
        {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        Ok(())
    }

    /// Matches the original host result gate after this recipe's copy/decoder,
    /// without claiming common native retirement or a graph-visible version.
    pub fn result_matches_owner_v1<T: Send + Sync + 'static>(
        &self,
        ticket: &RuntimeGfx942Registry4TicketV1<'scope, N>,
        owner: &std::sync::Arc<T>,
    ) -> Result<bool, RuntimeGfx942ScopeErrorV1> {
        self.validate_ticket(ticket)?;
        let root = self.root.as_ref().unwrap_or_else(|| std::process::abort());
        let (outcomes, domains) = if ticket.cycle == 0 {
            (&root.outcomes, &root.domains)
        } else {
            let second = root
                .second
                .as_ref()
                .unwrap_or_else(|| std::process::abort());
            (&second.outcomes, &second.domains)
        };
        Ok(matches!(outcomes[ticket.member], Some(Ok(())))
            && domains[ticket.member].matches_owner(owner))
    }
}

impl<const N: usize> CycleResults<N> {
    fn new(domains: [crate::RuntimeGeneratedResultDomainV1; N]) -> Self {
        let pairs: [_; N] =
            core::array::from_fn(|_| crate::async_engine::RuntimeAsyncReplyV1::pair());
        let mut futures = core::array::from_fn(|_| None);
        let mut index = 0;
        let replies = pairs.map(|(reply, future)| {
            futures[index] = Some(future);
            index += 1;
            reply
        });
        Self {
            outcomes: core::array::from_fn(|_| None),
            domains,
            replies,
            futures,
        }
    }
}

impl<'scope, P: crate::RuntimeGfx942RegistryRepeat2CarrierV1>
    RuntimeGfx942Registry4ScopeV1<'scope, '_, P>
{
    /// Admit the original four sources once; each row identifies a distinct
    /// prepaid result cycle. Neither row can authorize common DATA retirement.
    pub fn try_submit_repeat2_v1<E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        stream: RuntimeStreamIdV1,
        prepare: impl FnOnce(
            &fe2o3_kfd::CheckedGfx942XnackMinusDevice,
        ) -> Result<crate::RuntimeGfx942GeneratedRegistry4Repeat2V1<P>, E>,
    ) -> Result<
        [[RuntimeGfx942Registry4TicketV1<'scope>; 4]; 2],
        RuntimeGfx942ScopedSubmissionErrorV1<E>,
    > {
        if !self.repeat2 {
            return Err(RuntimeGfx942ScopedSubmissionErrorV1::Scope(
                RuntimeGfx942ScopeErrorV1::Capacity,
            ));
        }
        self.try_submit_inner(device, stream, |device| {
            prepare(device).map(|original| original.0)
        })?;
        Ok([self.tickets(0), self.tickets(1)])
    }
}

impl<'scope, P: RuntimeGfx942RegistryCompletionCarrierV1>
    RuntimeGfx942Registry4ScopeV1<'scope, '_, P>
{
    pub fn try_submit_v1<E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        stream: RuntimeStreamIdV1,
        prepare: impl FnOnce(&fe2o3_kfd::CheckedGfx942XnackMinusDevice) -> Result<Registry<P, 4>, E>,
    ) -> Result<[RuntimeGfx942Registry4TicketV1<'scope>; 4], RuntimeGfx942ScopedSubmissionErrorV1<E>>
    {
        if self.repeat2 {
            return Err(RuntimeGfx942ScopedSubmissionErrorV1::Scope(
                RuntimeGfx942ScopeErrorV1::Capacity,
            ));
        }
        self.try_submit_inner(device, stream, prepare)?;
        Ok(self.tickets(0))
    }
}
impl<'scope, P: RuntimeGfx942RegistryCompletionCarrierV1>
    RuntimeGfx942Registry4ScopeV1<'scope, '_, P, 16>
{
    /// Admit sixteen originals once, retaining one common native root.
    pub fn try_submit_v1<E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        stream: RuntimeStreamIdV1,
        prepare: impl FnOnce(
            &fe2o3_kfd::CheckedGfx942XnackMinusDevice,
        ) -> Result<crate::RuntimeGfx942GeneratedRegistry16V1<P>, E>,
    ) -> Result<
        [RuntimeGfx942Registry4TicketV1<'scope, 16>; 16],
        RuntimeGfx942ScopedSubmissionErrorV1<E>,
    > {
        self.try_submit_inner(device, stream, |owner| prepare(owner).map(|value| value.0))?;
        Ok(self.tickets(0))
    }
}

fn collect_domains<const N: usize>(
    values: [Result<crate::RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1>; N],
) -> Result<[crate::RuntimeGeneratedResultDomainV1; N], RuntimeGfx942ScopeErrorV1> {
    let mut result = core::array::from_fn(|_| None);
    for (slot, value) in result.iter_mut().zip(values) {
        *slot = Some(value.map_err(RuntimeGfx942ScopeErrorV1::Readback)?);
    }
    Ok(result.map(|value| value.unwrap_or_else(|| std::process::abort())))
}

/// Sixteen-original scope; distinct count-branded tickets cannot authorize N4.
pub type RuntimeGfx942Registry16ScopeV1<'scope, 'env, P> =
    RuntimeGfx942Registry4ScopeV1<'scope, 'env, P, 16>;
/// Exact N16 member locator, not a common DATA retirement receipt.
pub type RuntimeGfx942Registry16TicketV1<'scope> = RuntimeGfx942Registry4TicketV1<'scope, 16>;
/// Copied-result observer from an N16 original scope.
pub type RuntimeGfx942Registry16ResultFutureV1<'scope> =
    RuntimeGfx942Registry4ResultFutureV1<'scope, 16>;
