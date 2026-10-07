//! Copied results do not discharge the original common arena owner.

use super::*;
use crate::generated_source::GeneratedProfileV1;
use crate::generated_source::arena1024::SLOTS;
use crate::kfd_backend::ArenaPreallocationV1;
use crate::{RuntimeGfx942GeneratedArena1024V1 as Arena, RuntimeGfx942RegistryCompletionCarrierV1};
use fe2o3_resource_accounting::{HostMetadataTableV1, ResourceCreditAccountV1};
use std::{future::Future, pin::Pin, task::Poll};

mod progress;
mod results;
#[cfg(test)]
mod tests;
use results::{Cell, ReplyPayload};

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Adopting,
    Active,
    Closing,
    Closed,
    Unknown,
}

struct Root<P> {
    prepared: Option<RuntimeGfx942PreparedV1<Arena<P>>>,
    storage: Option<ArenaPreallocationV1>,
    roster: GeneratedHostRosterV1,
    hold: ContextUnpublishedHoldV1,
    state: State,
    cells: HostMetadataTableV1<Option<Cell>>,
    copied: HostMetadataTableV1<bool>,
    // All original replies/futures above drop before their shared payload charge.
    payload: Rc<ReplyPayload>,
}

/// Original member locator. This cannot stand for scalar/graph completion or
/// destruction of the common native allocation.
pub struct RuntimeGfx942Arena1024TicketV1<'scope> {
    identity: Rc<()>,
    member: usize,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

/// Copied-result observer, not a native retirement receipt. Dropping it does not
/// cancel its member, destroy common DATA, or release original source loans.
///
/// ```compile_fail
/// fn send<T: Send>() {}
/// send::<fe2o3_runtime::RuntimeGfx942Arena1024ResultFutureV1<'static>>();
/// ```
#[must_use]
pub struct RuntimeGfx942Arena1024ResultFutureV1<'scope> {
    future: crate::RuntimeAsyncCommandFutureV1<Outcome>,
    _identity: Rc<()>,
    _payload: Rc<ReplyPayload>,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl Future for RuntimeGfx942Arena1024ResultFutureV1<'_> {
    type Output = Result<(), RuntimeGfx942ScopeErrorV1>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.future).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(result)) => {
                Poll::Ready(result.map_err(RuntimeGfx942ScopeErrorV1::Readback))
            }
            Poll::Ready(Err(_)) => Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::Unknown)),
        }
    }
}

/// Single-use 1024-original arena over one Context writer/debit/hold. The scope
/// retains the explicit ordered or independent-disjoint-WO entry profile.
/// Each member keeps its original source, result account and decoder. All remain
/// rooted until actual common queue destruction and Context settlement.
/// This is not an interrupt wake or hardware throughput, duration, overlap,
/// out-of-order execution or high-depth qualification.
pub struct RuntimeGfx942Arena1024ScopeV1<'scope, 'env, P> {
    epoch: scope_epoch::Owner,
    context: &'env mut RuntimeContextV1<KfdRuntimeBackendV1>,
    root: Option<Root<P>>,
    metadata: ResourceCreditAccountV1,
    identity: Rc<()>,
    deadline: Instant,
    profile: GeneratedProfileV1,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl<P> Drop for RuntimeGfx942Arena1024ScopeV1<'_, '_, P> {
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

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    /// Borrows all original Arena carriers across caller-driven asynchronous
    /// progress. The existing finite Driver is reused. Native/source checks are
    /// bounded synchronous operations, not a claim of nonblocking control I/O.
    ///
    /// Metadata tables and reply payloads are charged before native entry. Arc
    /// headers, allocator bookkeeping and the fixed scope header are excluded;
    /// this is not total RSS accounting. The unchanged original source/proof
    /// budgets may refuse this larger roster; this API never replenishes them.
    ///
    /// ```compile_fail
    /// use fe2o3_runtime::*;
    /// async fn escape<P: RuntimeGfx942RegistryCompletionCarrierV1>(
    ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    ///     a: fe2o3_resource_accounting::ResourceCreditAccountV1) {
    ///     let _ = c.with_generated_gfx942_arena1024_scope_async_v1::<P,_,_,_>(a,
    ///         std::time::Instant::now(), |_|std::future::ready(()), async |scope| {
    ///             scope.ticket_v1(0)
    ///         }).await;
    /// }
    /// ```
    pub async fn with_generated_gfx942_arena1024_scope_async_v1<'env, P, R, W, F>(
        &'env mut self,
        metadata: ResourceCreditAccountV1,
        deadline: Instant,
        wait: W,
        use_scope: impl for<'scope> AsyncFnOnce(
            &mut RuntimeGfx942Arena1024ScopeV1<'scope, 'env, P>,
        ) -> R,
    ) -> Result<R, RuntimeGfx942ScopeErrorV1>
    where
        P: RuntimeGfx942RegistryCompletionCarrierV1 + 'env,
        W: FnMut(Instant) -> F,
        F: Future<Output = ()>,
    {
        self.with_arena_profile_async_v1(
            metadata,
            deadline,
            wait,
            GeneratedProfileV1::NativeFillArena1024,
            use_scope,
        )
        .await
    }

    /// Borrows a separately admitted disjoint-WO arena. Only the distinct
    /// independent source factory can populate this scope. Per-member outputs
    /// remain protected by their actual completion; no completion order is
    /// inferred from packet ordering or this API.
    pub async fn with_generated_gfx942_independent_arena1024_scope_async_v1<'env, P, R, W, F>(
        &'env mut self,
        metadata: ResourceCreditAccountV1,
        deadline: Instant,
        wait: W,
        use_scope: impl for<'scope> AsyncFnOnce(
            &mut RuntimeGfx942Arena1024ScopeV1<'scope, 'env, P>,
        ) -> R,
    ) -> Result<R, RuntimeGfx942ScopeErrorV1>
    where
        P: RuntimeGfx942RegistryCompletionCarrierV1 + 'env,
        W: FnMut(Instant) -> F,
        F: Future<Output = ()>,
    {
        self.with_arena_profile_async_v1(
            metadata,
            deadline,
            wait,
            GeneratedProfileV1::IndependentFillArena1024,
            use_scope,
        )
        .await
    }

    async fn with_arena_profile_async_v1<'env, P, R, W, F>(
        &'env mut self,
        metadata: ResourceCreditAccountV1,
        deadline: Instant,
        wait: W,
        profile: GeneratedProfileV1,
        use_scope: impl for<'scope> AsyncFnOnce(
            &mut RuntimeGfx942Arena1024ScopeV1<'scope, 'env, P>,
        ) -> R,
    ) -> Result<R, RuntimeGfx942ScopeErrorV1>
    where
        P: RuntimeGfx942RegistryCompletionCarrierV1 + 'env,
        W: FnMut(Instant) -> F,
        F: Future<Output = ()>,
    {
        if Instant::now() >= deadline {
            return Err(RuntimeGfx942ScopeErrorV1::Deadline);
        }
        self.require_live()
            .map_err(|e| RuntimeGfx942ScopeErrorV1::Context(e.into()))?;
        self.backend
            .preflight_generated_cohort3_lane_v1()
            .map_err(|e| RuntimeGfx942ScopeErrorV1::Context(map_backend_error(e)))?;
        let identity = Rc::new(());
        let epoch = self
            .scope_epoch
            .begin()
            .map_err(|e| RuntimeGfx942ScopeErrorV1::Context(e.into()))?;
        let mut scope = RuntimeGfx942Arena1024ScopeV1 {
            epoch,
            context: self,
            root: None,
            metadata,
            identity,
            deadline,
            profile,
            invariant: PhantomData,
        };
        let result = use_scope(&mut scope).await;
        scope.drive_with_wake_v1(wait).await?;
        Ok(result)
    }
}

impl<'scope, P: RuntimeGfx942RegistryCompletionCarrierV1>
    RuntimeGfx942Arena1024ScopeV1<'scope, '_, P>
{
    /// Accepts one original roster. Refusal before rooting has no native effects.
    pub fn try_submit_v1<E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        stream: RuntimeStreamIdV1,
        prepare: impl FnOnce(&fe2o3_kfd::CheckedGfx942XnackMinusDevice) -> Result<Arena<P>, E>,
    ) -> Result<(), RuntimeGfx942ScopedSubmissionErrorV1<E>> {
        self.try_submit_profile_v1(
            device,
            stream,
            GeneratedProfileV1::NativeFillArena1024,
            prepare,
        )
    }

    /// Accepts only the separate independent owner into an independent scope.
    pub fn try_submit_independent_v1<E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        stream: RuntimeStreamIdV1,
        prepare: impl FnOnce(
            &fe2o3_kfd::CheckedGfx942XnackMinusDevice,
        )
            -> Result<crate::RuntimeGfx942GeneratedIndependentArena1024V1<P>, E>,
    ) -> Result<(), RuntimeGfx942ScopedSubmissionErrorV1<E>> {
        self.try_submit_profile_v1(
            device,
            stream,
            GeneratedProfileV1::IndependentFillArena1024,
            |checked| prepare(checked).map(|source| source.0),
        )
    }

    fn try_submit_profile_v1<E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        stream: RuntimeStreamIdV1,
        profile: GeneratedProfileV1,
        prepare: impl FnOnce(&fe2o3_kfd::CheckedGfx942XnackMinusDevice) -> Result<Arena<P>, E>,
    ) -> Result<(), RuntimeGfx942ScopedSubmissionErrorV1<E>> {
        use RuntimeGfx942ScopedSubmissionErrorV1 as Error;
        if self.profile != profile {
            return Err(Error::Scope(RuntimeGfx942ScopeErrorV1::Readback(
                RuntimeGfx942ReadbackErrorV1::InvalidStorage,
            )));
        }
        if self.root.is_some() {
            return Err(Error::Scope(RuntimeGfx942ScopeErrorV1::Capacity));
        }
        if Instant::now() >= self.deadline {
            return Err(Error::Scope(RuntimeGfx942ScopeErrorV1::Deadline));
        }
        let _permit = self
            .epoch
            .enter()
            .map_err(|e| Error::Scope(RuntimeGfx942ScopeErrorV1::Context(e.into())))?;
        let mut prepared = self
            .context
            .with_gfx942_preparation_device_v1(device, prepare)
            .map_err(Error::Preparation)?;
        self.admit(&mut prepared, stream).map_err(Error::Scope)?;
        self.root
            .as_mut()
            .unwrap_or_else(|| std::process::abort())
            .prepared = Some(prepared);
        Ok(())
    }

    fn admit(
        &mut self,
        prepared: &mut RuntimeGfx942PreparedV1<Arena<P>>,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        if prepared.value().original_roster().source_identity.profile() != self.profile
            || !self
                .metadata
                .shares_ledger_with(prepared.value().metadata())
        {
            return Err(RuntimeGfx942ScopeErrorV1::Readback(
                RuntimeGfx942ReadbackErrorV1::InvalidStorage,
            ));
        }
        let results = results::prepare(&self.metadata, |index| {
            prepared.value().members[index]
                .as_ref()
                .unwrap_or_else(|| std::process::abort())
                .registry_completion_domain_v1()
        })?;
        let copied = HostMetadataTableV1::try_new(SLOTS, Some(&self.metadata), || false)
            .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?;
        let roster = self
            .context
            .reserve_gfx942_arena_v1(prepared)
            .map_err(RuntimeGfx942ScopeErrorV1::Reservation)?;
        self.context
            .preflight_gfx942_arena_v1(prepared, &roster, stream)
            .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
        let storage = ArenaPreallocationV1::new(&self.metadata, roster.readback_bytes)
            .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?;
        if Instant::now() >= self.deadline {
            return Err(RuntimeGfx942ScopeErrorV1::Deadline);
        }
        let hold = self
            .context
            .hold_unpublished_stream_with_access_v1(stream, None)
            .map_err(|e| RuntimeGfx942ScopeErrorV1::Context(e.into()))?;
        // No fallible allocation or callback follows acquisition of the hold.
        self.root = Some(Root {
            prepared: None,
            storage: Some(storage),
            roster,
            hold,
            state: State::Adopting,
            cells: results.cells,
            copied,
            payload: results.payload,
        });
        Ok(())
    }

    pub fn ticket_v1(
        &self,
        member: usize,
    ) -> Result<RuntimeGfx942Arena1024TicketV1<'scope>, RuntimeGfx942ScopeErrorV1> {
        if member >= SLOTS || self.root.is_none() {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        Ok(RuntimeGfx942Arena1024TicketV1 {
            identity: Rc::clone(&self.identity),
            member,
            invariant: PhantomData,
        })
    }

    fn validate_ticket(
        &self,
        ticket: &RuntimeGfx942Arena1024TicketV1<'scope>,
    ) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        if ticket.member >= SLOTS
            || self.root.is_none()
            || !Rc::ptr_eq(&self.identity, &ticket.identity)
        {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        Ok(())
    }

    pub fn result_future_v1(
        &mut self,
        ticket: &RuntimeGfx942Arena1024TicketV1<'scope>,
    ) -> Result<RuntimeGfx942Arena1024ResultFutureV1<'scope>, RuntimeGfx942ScopeErrorV1> {
        self.validate_ticket(ticket)?;
        let root = self.root.as_mut().unwrap_or_else(|| std::process::abort());
        let cell = root.cells[ticket.member]
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        let future = cell
            .future
            .take()
            .ok_or(RuntimeGfx942ScopeErrorV1::CompletionObserverTaken)?;
        Ok(RuntimeGfx942Arena1024ResultFutureV1 {
            future,
            _identity: Rc::clone(&self.identity),
            _payload: Rc::clone(&root.payload),
            invariant: PhantomData,
        })
    }

    /// Checks the retained original gate only after this member's actual copy
    /// and decoder. No scalar receipt or common DATA retirement is produced.
    pub fn result_matches_owner_v1<T: Send + Sync + 'static>(
        &self,
        ticket: &RuntimeGfx942Arena1024TicketV1<'scope>,
        owner: &std::sync::Arc<T>,
    ) -> Result<bool, RuntimeGfx942ScopeErrorV1> {
        self.validate_ticket(ticket)?;
        let root = self.root.as_ref().unwrap_or_else(|| std::process::abort());
        let cell = root.cells[ticket.member]
            .as_ref()
            .unwrap_or_else(|| std::process::abort());
        Ok(matches!(cell.outcome, Some(Ok(()))) && cell.domain.matches_owner(owner))
    }
}
