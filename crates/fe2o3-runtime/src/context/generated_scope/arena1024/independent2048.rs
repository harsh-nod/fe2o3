//! Distinct public capacity, shared finite driver and original common owner.

use super::*;

/// A 2048-family locator, not a 1024 ticket or a native disposal receipt.
pub struct RuntimeGfx942IndependentArena2048TicketV1<'scope>(
    RuntimeGfx942Arena1024TicketV1<'scope>,
);

/// Original copied-result observer. It retains the same prepaid reply payload
/// and cannot discharge the common native allocation.
#[must_use]
pub struct RuntimeGfx942IndependentArena2048ResultFutureV1<'scope>(
    RuntimeGfx942Arena1024ResultFutureV1<'scope>,
);

impl Future for RuntimeGfx942IndependentArena2048ResultFutureV1<'_> {
    type Output = Result<(), RuntimeGfx942ScopeErrorV1>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

/// Exactly 2048 independent-WO originals under one retained Context epoch,
/// writer, debit and hold. The private common engine performs native destruction
/// before releasing source residuals. No rolling reuse, interrupt wake, measured
/// duration, hardware depth or out-of-order execution qualification is implied.
///
/// ```compile_fail
/// use fe2o3_runtime::*;
/// fn refuse<'s, 'e, P>(scope: RuntimeGfx942IndependentArena2048ScopeV1<'s, 'e, P>)
///     -> RuntimeGfx942Arena1024ScopeV1<'s, 'e, P> { scope }
/// ```
pub struct RuntimeGfx942IndependentArena2048ScopeV1<'scope, 'env, P>(
    RuntimeGfx942Arena1024ScopeV1<'scope, 'env, P>,
);

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    /// Uses the original source and metadata accounts without replenishment.
    /// Every table is prepaid before native entry; synchronous currentness work
    /// retains its existing bounds. Future cancellation/forget is governed by the
    /// same persistent Context and host-loan obligations as the 1024 profile.
    ///
    /// ```compile_fail
    /// use fe2o3_runtime::*;
    /// async fn escape<P: RuntimeGfx942RegistryCompletionCarrierV1>(
    ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    ///     a: fe2o3_resource_accounting::ResourceCreditAccountV1) {
    ///     let _ = c.with_generated_gfx942_independent_arena2048_scope_async_v1::<P,_,_,_>(a,
    ///         std::time::Instant::now(), |_|std::future::ready(()), async |scope| {
    ///             scope.ticket_v1(0)
    ///         }).await;
    /// }
    /// ```
    pub async fn with_generated_gfx942_independent_arena2048_scope_async_v1<'env, P, R, W, F>(
        &'env mut self,
        metadata: ResourceCreditAccountV1,
        deadline: Instant,
        wait: W,
        use_scope: impl for<'scope> AsyncFnOnce(
            &mut RuntimeGfx942IndependentArena2048ScopeV1<'scope, 'env, P>,
        ) -> R,
    ) -> Result<R, RuntimeGfx942ScopeErrorV1>
    where
        P: RuntimeGfx942RegistryCompletionCarrierV1 + 'env,
        W: FnMut(Instant) -> F,
        F: Future<Output = ()>,
    {
        let mut scope = RuntimeGfx942IndependentArena2048ScopeV1(self.open_arena_profile_v1(
            metadata,
            deadline,
            GeneratedProfileV1::IndependentFillArena2048,
        )?);
        let result = use_scope(&mut scope).await;
        scope.drive_with_wake_v1(wait).await?;
        Ok(result)
    }
}

impl<'scope, P: RuntimeGfx942RegistryCompletionCarrierV1>
    RuntimeGfx942IndependentArena2048ScopeV1<'scope, '_, P>
{
    pub fn try_submit_v1<E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        stream: RuntimeStreamIdV1,
        prepare: impl FnOnce(
            &fe2o3_kfd::CheckedGfx942XnackMinusDevice,
        )
            -> Result<crate::RuntimeGfx942GeneratedIndependentArena2048V1<P>, E>,
    ) -> Result<(), RuntimeGfx942ScopedSubmissionErrorV1<E>> {
        self.0.try_submit_profile_v1(
            device,
            stream,
            GeneratedProfileV1::IndependentFillArena2048,
            |checked| prepare(checked).map(|source| source.0),
        )
    }

    pub fn ticket_v1(
        &self,
        member: usize,
    ) -> Result<RuntimeGfx942IndependentArena2048TicketV1<'scope>, RuntimeGfx942ScopeErrorV1> {
        self.0
            .ticket_v1(member)
            .map(RuntimeGfx942IndependentArena2048TicketV1)
    }

    pub fn result_future_v1(
        &mut self,
        ticket: &RuntimeGfx942IndependentArena2048TicketV1<'scope>,
    ) -> Result<RuntimeGfx942IndependentArena2048ResultFutureV1<'scope>, RuntimeGfx942ScopeErrorV1>
    {
        self.0
            .result_future_v1(&ticket.0)
            .map(RuntimeGfx942IndependentArena2048ResultFutureV1)
    }

    pub fn result_matches_owner_v1<T: Send + Sync + 'static>(
        &self,
        ticket: &RuntimeGfx942IndependentArena2048TicketV1<'scope>,
        owner: &std::sync::Arc<T>,
    ) -> Result<bool, RuntimeGfx942ScopeErrorV1> {
        self.0.result_matches_owner_v1(&ticket.0, owner)
    }

    pub fn pending_v1(&self) -> usize {
        self.0.pending_v1()
    }
    pub fn progress_v1(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        self.0.progress_v1()
    }
    pub async fn drive_with_wake_v1<W, F>(
        &mut self,
        wait: W,
    ) -> Result<(), RuntimeGfx942ScopeErrorV1>
    where
        W: FnMut(Instant) -> F,
        F: Future<Output = ()>,
    {
        self.0.drive_with_wake_v1(wait).await
    }
    pub fn receipt_observations_v1(&self) -> Option<RuntimeGfx942ArenaObservationV1> {
        self.0.receipt_observations_v1()
    }
    pub fn member_receipt_observation_v1(
        &self,
        index: usize,
    ) -> Option<RuntimeGfx942ArenaMemberObservationV1> {
        self.0.member_receipt_observation_v1(index)
    }
}
