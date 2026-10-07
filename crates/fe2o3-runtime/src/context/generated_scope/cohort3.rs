//! One fresh-primary whole ordered Batch<3>, using the existing lexical engine.

use super::*;
use crate::RuntimeGfx942GeneratedCohort3V1 as Cohort;

#[cfg(test)]
mod tests;

/// One whole-cohort observer identity; it is not three native receipts.
///
/// ```compile_fail
/// use fe2o3_runtime::*;
/// fn scalar_result<'s,P:RuntimeGfx942GeneratedCompletionCarrierV1>(
///     scope:&RuntimeGfx942GeneratedScopeV1<'s,'_,KfdRuntimeBackendV1,RuntimeGfx942GeneratedCohort3V1<P>>,
///     ticket:&RuntimeGfx942ScopedCohort3TicketV1<'s>) {
///     scope.completion_v1(ticket);
/// }
/// ```
pub struct RuntimeGfx942ScopedCohort3TicketV1<'scope> {
    ticket: RuntimeGfx942ScopedTicketV1<'scope>,
}

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    /// Owns exactly three original carriers through one real ordered native batch.
    /// This initial profile requires a fresh primary queue and admits one cohort
    /// in the scope. It offers no per-member cancellation or independent native
    /// completion, queue reuse, graph operations, or multi-device routing.
    ///
    /// ```compile_fail
    /// use fe2o3_runtime::*;
    /// fn escape<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
    ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, d: RuntimeDeviceIdV1,
    ///     s: RuntimeStreamIdV1, p: [P;3]) {
    ///     let _ = c.with_generated_gfx942_cohort3_scope_v1(std::time::Instant::now(),
    ///         |scope| scope.try_submit_cohort3_v1(d,s, |_| Ok::<_,()>(RuntimeGfx942GeneratedCohort3V1::new(p))).unwrap());
    /// }
    /// ```
    pub fn with_generated_gfx942_cohort3_scope_v1<'env, P, R>(
        &'env mut self,
        deadline: Instant,
        use_scope: impl for<'scope> FnOnce(
            &mut RuntimeGfx942GeneratedScopeV1<'scope, 'env, KfdRuntimeBackendV1, Cohort<P>>,
        ) -> R,
    ) -> Result<R, RuntimeGfx942ScopeErrorV1>
    where
        P: RuntimeGfx942GeneratedCompletionCarrierV1 + 'env,
    {
        let mut scope = self.new_cohort3_scope_v1(deadline)?;
        let value = use_scope(&mut scope);
        scope.drain_v1()?;
        Ok(value)
    }

    /// Async-primary whole-cohort scope with the same Context epoch and caller wake.
    /// Dropping live work fails stop; forgetting it cannot release original custody.
    /// Source/currentness checks remain bounded synchronous control-plane work.
    ///
    /// ```compile_fail
    /// use fe2o3_runtime::*;
    /// async fn escape<P:RuntimeGfx942GeneratedCompletionCarrierV1>(
    ///     c:&mut RuntimeContextV1<KfdRuntimeBackendV1>,d:RuntimeDeviceIdV1,
    ///     s:RuntimeStreamIdV1,p:[P;3]) {
    ///     let _ticket=c.with_generated_gfx942_cohort3_scope_async_v1(
    ///         std::time::Instant::now(), |_|std::future::ready(()), async |scope| {
    ///             scope.try_submit_cohort3_v1(d,s, |_|Ok::<_,()>(RuntimeGfx942GeneratedCohort3V1::new(p))).unwrap()
    ///         }).await;
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_runtime::*;
    /// fn no_singleton<'s,P:RuntimeGfx942GeneratedCompletionCarrierV1>(
    ///     scope:&mut RuntimeGfx942GeneratedScopeV1<'s,'_,KfdRuntimeBackendV1,RuntimeGfx942GeneratedCohort3V1<P>>,
    ///     d:RuntimeDeviceIdV1,s:RuntimeStreamIdV1,p:P) {
    ///     scope.try_submit_v1(d,s, |_|Ok::<_,()>(p));
    /// }
    /// ```
    pub async fn with_generated_gfx942_cohort3_scope_async_v1<'env, P, R, W, F>(
        &'env mut self,
        deadline: Instant,
        wait: W,
        use_scope: impl for<'scope> AsyncFnOnce(
            &mut RuntimeGfx942GeneratedScopeV1<'scope, 'env, KfdRuntimeBackendV1, Cohort<P>>,
        ) -> R,
    ) -> Result<R, RuntimeGfx942ScopeErrorV1>
    where
        P: RuntimeGfx942GeneratedCompletionCarrierV1 + 'env,
        W: FnMut(Instant) -> F,
        F: std::future::Future<Output = ()>,
    {
        let mut scope = self.new_cohort3_scope_v1(deadline)?;
        let value = use_scope(&mut scope).await;
        scope.drive_with_wake_v1(wait).await?;
        Ok(value)
    }

    fn new_cohort3_scope_v1<'scope, 'env, P>(
        &'env mut self,
        deadline: Instant,
    ) -> Result<
        RuntimeGfx942GeneratedScopeV1<'scope, 'env, KfdRuntimeBackendV1, Cohort<P>>,
        RuntimeGfx942ScopeErrorV1,
    >
    where
        P: RuntimeGfx942GeneratedCompletionCarrierV1 + 'env,
    {
        if Instant::now() >= deadline {
            return Err(RuntimeGfx942ScopeErrorV1::Deadline);
        }
        self.require_live()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        self.backend
            .preflight_generated_cohort3_lane_v1()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(map_backend_error(error)))?;
        let (slots, copies) = allocate_rosters(1)?;
        let identity = Rc::new(());
        let epoch = self
            .scope_epoch
            .begin()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        Ok(RuntimeGfx942GeneratedScopeV1 {
            epoch,
            context: self,
            slots,
            copies,
            graph: None,
            capacity: 1,
            identity,
            deadline,
            invariant: PhantomData,
            hooks: Hooks {
                domains: cohort_domains_v1::<P>,
                decode: RuntimeGfx942PreparedV1::complete_cohort3_readbacks_v1,
                progress_graph: |scope| {
                    if scope.graph.is_some() {
                        Err(RuntimeGfx942ScopeErrorV1::Unknown)
                    } else {
                        Ok(0)
                    }
                },
                progress_copies: |scope| {
                    if scope.copies.is_empty() {
                        Ok(0)
                    } else {
                        Err(RuntimeGfx942ScopeErrorV1::Unknown)
                    }
                },
                reserve: Self::reserve_gfx942_cohort3_v1::<P>,
                preflight: Self::preflight_gfx942_cohort3_v1::<P>,
                ready: Self::gfx942_adoption_ready_v1,
                adopt: Self::adopt_gfx942_cohort3_v1::<P>,
                progress: Self::progress_gfx942_cohort3_issue_v1::<P>,
                rejected: |_, _| Ok(false),
                retire_rejected: |_, _, _, _| {
                    Err(RuntimeValidationErrorV1::InvalidBackendDescription.into())
                },
                complete: Self::complete_gfx942_cohort3_issue_v1::<P>,
                unpublished: Self::gfx942_adoption_unpublished_v1,
                retire_unpublished: Self::retire_gfx942_unpublished_v1,
                copy_progress: Self::progress_stream_v1,
                graph_submit: Self::submit_graph_action_v1,
                graph_progress: |context, stream, access| {
                    context.drive_stream_with_graph_access_v1(
                        stream,
                        access,
                        <KfdRuntimeBackendV1 as RuntimeFlushBackendV1>::progress_stream_v1,
                    )
                },
            },
        })
    }
}

fn cohort_domains_v1<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
    value: &Cohort<P>,
) -> Result<CompletionDomainsV1, RuntimeGfx942ReadbackErrorV1> {
    let [a, b, c] = value.members.each_ref();
    let domains = [
        a.completion_domain_v1()?,
        b.completion_domain_v1()?,
        c.completion_domain_v1()?,
    ];
    if domains[0].same_original_v1(&domains[1])
        || domains[0].same_original_v1(&domains[2])
        || domains[1].same_original_v1(&domains[2])
    {
        return Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage);
    }
    Ok(CompletionDomainsV1::Cohort3(domains))
}

impl<'scope, P: RuntimeGfx942GeneratedCompletionCarrierV1>
    RuntimeGfx942GeneratedScopeV1<'scope, '_, KfdRuntimeBackendV1, Cohort<P>>
{
    pub fn try_submit_cohort3_v1<E>(
        &mut self,
        device: RuntimeDeviceIdV1,
        stream: RuntimeStreamIdV1,
        prepare: impl FnOnce(&fe2o3_kfd::CheckedGfx942XnackMinusDevice) -> Result<Cohort<P>, E>,
    ) -> Result<RuntimeGfx942ScopedCohort3TicketV1<'scope>, RuntimeGfx942ScopedSubmissionErrorV1<E>>
    {
        self.check_submission()
            .map_err(RuntimeGfx942ScopedSubmissionErrorV1::Scope)?;
        let prepared = {
            let _permit = self.epoch.enter().map_err(|error| {
                RuntimeGfx942ScopedSubmissionErrorV1::Scope(RuntimeGfx942ScopeErrorV1::Context(
                    error.into(),
                ))
            })?;
            self.context
                .with_gfx942_preparation_device_v1(device, prepare)
                .map_err(RuntimeGfx942ScopedSubmissionErrorV1::Preparation)?
        };
        self.admit(prepared, stream)
            .map(|ticket| RuntimeGfx942ScopedCohort3TicketV1 { ticket })
            .map_err(RuntimeGfx942ScopedSubmissionErrorV1::Scope)
    }

    pub fn cohort3_completion_future_v1(
        &mut self,
        ticket: &RuntimeGfx942ScopedCohort3TicketV1<'scope>,
    ) -> Result<RuntimeGfx942ScopedCompletionFutureV1<'scope>, RuntimeGfx942ScopeErrorV1> {
        self.completion_future_v1(&ticket.ticket)
    }

    /// Checks the original indexed host gate only after all native/readback/decoder work.
    pub fn cohort3_completion_matches_owner_v1<T: Send + Sync + 'static>(
        &self,
        ticket: &RuntimeGfx942ScopedCohort3TicketV1<'scope>,
        member: usize,
        owner: &std::sync::Arc<T>,
    ) -> Result<bool, RuntimeGfx942ScopeErrorV1> {
        if member >= 3 {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        let complete = matches!(self.completion_v1(&ticket.ticket)?, Some(Ok(())));
        let CompletionDomainsV1::Cohort3(domains) = &self.slots[ticket.ticket.index].domain else {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        };
        Ok(complete && domains[member].matches_owner(owner))
    }
}
