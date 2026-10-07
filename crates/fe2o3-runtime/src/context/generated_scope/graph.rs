//! Borrowed generated owners on the same admitted graph core as the static engine.
use super::*;
use crate::async_engine::{AdmittedGraphV1, PreparedGraphAdmissionV1};
use crate::{RuntimeGraphErrorV1, RuntimeGraphReportV1, RuntimeGraphRequestV1};
use fe2o3_completion::{CompletionNodeIdV1, CompletionNodeStateV1};
use std::{future::Future, pin::Pin, task::Poll};

mod staging;
pub use staging::RuntimeGfx942ScopedGraphStagingErrorV1;

#[derive(Debug)]
pub enum RuntimeGfx942ScopedGraphAdmissionErrorV1<E> {
    Graph(RuntimeGraphErrorV1<KfdRuntimeBackendErrorV1>),
    Preparation(RuntimeGfx942PreparationErrorV1<E>),
    Scope(RuntimeGfx942ScopeErrorV1),
}
impl<E> From<RuntimeGraphErrorV1<KfdRuntimeBackendErrorV1>>
    for RuntimeGfx942ScopedGraphAdmissionErrorV1<E>
{
    fn from(error: RuntimeGraphErrorV1<KfdRuntimeBackendErrorV1>) -> Self {
        Self::Graph(error)
    }
}
impl<E: fmt::Debug> fmt::Display for RuntimeGfx942ScopedGraphAdmissionErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scoped graph admission: {self:?}")
    }
}
impl<E: fmt::Debug + 'static> Error for RuntimeGfx942ScopedGraphAdmissionErrorV1<E> {}

/// A descriptive reference to one scope-owned graph, not scheduling authority.
/// Dropping or forgetting it does not cancel or release any original owner.
///
/// ```compile_fail
/// use fe2o3_runtime::*;
/// fn escape<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, r: RuntimeGraphRequestV1<KfdRuntimeBackendV1>,
/// ) {
///     let _ticket = c.with_generated_gfx942_scope_v1::<P, _>(256,
///         std::time::Instant::now() + std::time::Duration::from_secs(1), |scope| {
///             scope.try_admit_graph_v1(r, |_, _| Err::<P, ()>(())).unwrap()
///         });
/// }
/// ```
pub struct RuntimeGfx942ScopedGraphTicketV1<'scope> {
    pub(super) scope: Rc<()>,
    pub(super) invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

/// Ready means the original graph reservation and every operation were retired,
/// not that every node succeeded. Inspect the retained graph report separately.
///
/// ```compile_fail
/// use fe2o3_runtime::RuntimeGfx942ScopedGraphFutureV1;
/// fn requires_send<T: Send>() {}
/// requires_send::<RuntimeGfx942ScopedGraphFutureV1<'static>>();
/// ```
#[must_use = "dropping observation does not cancel or retire the graph"]
pub struct RuntimeGfx942ScopedGraphFutureV1<'scope> {
    future: crate::RuntimeAsyncCommandFutureV1<()>,
    _scope: Rc<()>,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}
impl Future for RuntimeGfx942ScopedGraphFutureV1<'_> {
    type Output = Result<(), RuntimeGfx942ScopeErrorV1>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.future).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(_)) => Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::Unknown)),
        }
    }
}

struct Deferred<P> {
    prepared: RuntimeGfx942PreparedV1<P>,
    roster: GeneratedHostRosterV1,
    domain: crate::RuntimeGeneratedResultDomainV1,
    reply: crate::async_engine::RuntimeAsyncReplyV1<Outcome>,
    future: crate::RuntimeAsyncCommandFutureV1<Outcome>,
}
struct Ordinary {
    submission: RuntimeSubmissionV1<()>,
    retiring: Option<RuntimeCompletionStatusV1>,
}

fn filled<T>(
    len: usize,
    mut value: impl FnMut() -> T,
) -> Result<Vec<T>, RuntimeGfx942ScopeErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(len)
        .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?;
    result.resize_with(len, &mut value);
    Ok(result)
}

fn add_backing<T>(total: &mut usize, values: &Vec<T>) -> Result<(), RuntimeGfx942ScopeErrorV1> {
    let bytes = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(RuntimeGfx942ScopeErrorV1::Capacity)?;
    add_backing_bytes(total, bytes)
}

fn add_backing_bytes(total: &mut usize, bytes: usize) -> Result<(), RuntimeGfx942ScopeErrorV1> {
    *total = total
        .checked_add(bytes)
        .filter(|&bytes| bytes <= MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1)
        .ok_or(RuntimeGfx942ScopeErrorV1::Capacity)?;
    Ok(())
}

pub(super) struct Graph<P> {
    committed: bool,
    core: Option<AdmittedGraphV1>,
    deferred: Vec<Option<Deferred<P>>>,
    ordinary: Vec<Option<Box<PreparedContextGraphActionV1>>>,
    active: Vec<Option<Ordinary>>,
    generated: Vec<Option<usize>>,
    reconciled: Vec<bool>,
    ids: Vec<CompletionNodeIdV1>,
    report: Option<RuntimeGraphReportV1<KfdRuntimeBackendErrorV1>>,
    observations: Vec<(CompletionNodeIdV1, RuntimeCompletionStatusV1)>,
    errors: Vec<(CompletionNodeIdV1, NativeError)>,
    rejected_observations: u64,
    rejected_releases: u64,
    reply: crate::async_engine::RuntimeAsyncReplyV1<()>,
    future: Option<crate::RuntimeAsyncCommandFutureV1<()>>,
}
impl<P> Graph<P> {
    pub(super) fn unsettled(&self) -> bool {
        self.committed
    }
    pub(super) fn pending(&self) -> usize {
        self.core.as_ref().map_or(0, |core| {
            (0..core.len())
                .filter(|&i| !core.state(i).is_terminal())
                .count()
                .max(1)
        })
    }
    pub(super) fn notify_unknown(&mut self) {
        if self.unsettled() {
            self.reply
                .complete(Err(crate::RuntimeAsyncEngineCallErrorV1::EngineStopped));
        }
    }
}
impl<P> Drop for Graph<P> {
    fn drop(&mut self) {
        // Also guards an unwind while this owner is temporarily moved out of
        // the scope for a finite progress pass. No original field drops first.
        if self.unsettled() {
            std::process::abort();
        }
    }
}

impl<'scope, B, P> RuntimeGfx942GeneratedScopeV1<'scope, '_, B, P>
where
    B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>,
    P: RuntimeGfx942GeneratedCompletionCarrierV1,
{
    pub(super) fn admit_graph_with_v1<E>(
        &mut self,
        request: RuntimeGraphRequestV1<B>,
        mut prepare: impl FnMut(
            &mut RuntimeContextV1<B>,
            CompletionNodeIdV1,
            RuntimeStreamIdV1,
        ) -> Result<
            RuntimeGfx942PreparedV1<P>,
            RuntimeGfx942PreparationErrorV1<E>,
        >,
    ) -> Result<RuntimeGfx942ScopedGraphTicketV1<'scope>, RuntimeGfx942ScopedGraphAdmissionErrorV1<E>>
    {
        type Admission<E> = RuntimeGfx942ScopedGraphAdmissionErrorV1<E>;
        self.check_submission().map_err(Admission::Scope)?;
        if !self.slots.is_empty() || !self.copies.is_empty() {
            return Err(Admission::Graph(RuntimeGraphErrorV1::Busy));
        }
        let _permit = self
            .epoch
            .enter()
            .map_err(|e| Admission::Scope(RuntimeGfx942ScopeErrorV1::Context(e.into())))?;
        let mut request = Some(request);
        let mut generated = Vec::new();
        let capacity = self.capacity;
        let graph_capacity = capacity.min(crate::MAX_RUNTIME_GRAPH_NODES_V1);
        let base = self.roster_capacity_bytes_v1().map_err(Admission::Scope)?;
        graph_capacity
            .checked_mul(size_of::<(CompletionNodeIdV1, Deferred<P>)>())
            .and_then(|n| base.checked_add(n))
            .filter(|&n| n <= MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1)
            .ok_or(Admission::Scope(RuntimeGfx942ScopeErrorV1::Capacity))?;
        generated
            .try_reserve_exact(graph_capacity)
            .map_err(|_| Admission::Scope(RuntimeGfx942ScopeErrorV1::Capacity))?;
        let mut staging = base;
        add_backing(&mut staging, &generated).map_err(Admission::Scope)?;
        let hooks = &self.hooks;
        let plan = PreparedGraphAdmissionV1::prepare_scoped_v1(
            self.context,
            &mut request,
            |context, node, stream| {
                if generated.len() >= capacity {
                    return Err(Admission::Scope(RuntimeGfx942ScopeErrorV1::Capacity));
                }
                let mut prepared =
                    prepare(context, node, stream).map_err(Admission::Preparation)?;
                let domain = prepared
                    .value()
                    .completion_domain_v1()
                    .map_err(|e| Admission::Scope(RuntimeGfx942ScopeErrorV1::Readback(e)))?;
                let roster = (hooks.reserve)(context, &mut prepared)
                    .map_err(|e| Admission::Scope(RuntimeGfx942ScopeErrorV1::Reservation(e)))?;
                (hooks.preflight)(context, &prepared, &roster, stream, None)
                    .map_err(|e| Admission::Scope(RuntimeGfx942ScopeErrorV1::Context(e)))?;
                let (reply, future) = crate::async_engine::RuntimeAsyncReplyV1::pair();
                generated.push((
                    node,
                    Deferred {
                        prepared,
                        roster,
                        domain,
                        reply,
                        future,
                    },
                ));
                Ok::<_, Admission<E>>(())
            },
        )?;
        if plan.len() > self.capacity {
            return Err(Admission::Scope(RuntimeGfx942ScopeErrorV1::Capacity));
        }
        let len = plan.len();
        add_backing_bytes(
            &mut staging,
            plan.host_staging_backing_bytes_v1()
                .ok_or(Admission::Scope(RuntimeGfx942ScopeErrorV1::Capacity))?,
        )
        .map_err(Admission::Scope)?;
        let per_node = size_of::<Option<Deferred<P>>>()
            .checked_add(size_of::<Option<Ordinary>>())
            .and_then(|n| n.checked_add(size_of::<Option<usize>>()))
            .and_then(|n| n.checked_add(size_of::<bool>()))
            .and_then(|n| n.checked_add(size_of::<CompletionNodeIdV1>()))
            .and_then(|n| {
                n.checked_add(size_of::<(CompletionNodeIdV1, RuntimeCompletionStatusV1)>())
            })
            .and_then(|n| n.checked_add(size_of::<(CompletionNodeIdV1, NativeError)>()))
            .ok_or(Admission::Scope(RuntimeGfx942ScopeErrorV1::Capacity))?;
        len.checked_mul(per_node)
            .and_then(|n| staging.checked_add(n))
            .filter(|&n| n <= MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1)
            .ok_or(Admission::Scope(RuntimeGfx942ScopeErrorV1::Capacity))?;
        let (reply, future) = crate::async_engine::RuntimeAsyncReplyV1::pair();
        let mut graph = Graph {
            committed: false,
            core: None,
            deferred: filled(len, || None).map_err(Admission::Scope)?,
            ordinary: Vec::new(),
            active: filled(len, || None).map_err(Admission::Scope)?,
            generated: filled(len, || None).map_err(Admission::Scope)?,
            reconciled: filled(len, || false).map_err(Admission::Scope)?,
            ids: Vec::new(),
            report: None,
            observations: Vec::new(),
            errors: Vec::new(),
            rejected_observations: 0,
            rejected_releases: 0,
            reply,
            future: Some(future),
        };
        graph
            .ids
            .try_reserve_exact(len)
            .map_err(|_| Admission::Scope(RuntimeGfx942ScopeErrorV1::Capacity))?;
        graph
            .observations
            .try_reserve_exact(len)
            .map_err(|_| Admission::Scope(RuntimeGfx942ScopeErrorV1::Capacity))?;
        graph
            .errors
            .try_reserve_exact(len)
            .map_err(|_| Admission::Scope(RuntimeGfx942ScopeErrorV1::Capacity))?;
        // Include the temporary original-preparation array and every new
        // adapter-owned array in the peak backing bound before graph commit.
        // New core staging backing was counted by actual capacity above. The
        // pre-existing admitted graph core and all pointees are separate.
        add_backing(&mut staging, &graph.deferred).map_err(Admission::Scope)?;
        add_backing(&mut staging, &graph.active).map_err(Admission::Scope)?;
        add_backing(&mut staging, &graph.generated).map_err(Admission::Scope)?;
        add_backing(&mut staging, &graph.reconciled).map_err(Admission::Scope)?;
        add_backing(&mut staging, &graph.ids).map_err(Admission::Scope)?;
        add_backing(&mut staging, &graph.observations).map_err(Admission::Scope)?;
        add_backing(&mut staging, &graph.errors).map_err(Admission::Scope)?;
        let (core, actions) = plan
            .commit(self.context)
            .map_err(|failure| Admission::Graph(failure.error))?;
        // Commit creates no GPU effects, but from here this owner must survive
        // every unwind until exact graph release. All retained arrays are fixed.
        graph.committed = true;
        graph.core = Some(core);
        let core = graph.core.as_ref().unwrap();
        graph.ids.extend((0..len).map(|i| core.id(i)));
        for (node, owner) in generated {
            let index = core.index(node).expect("prepared original graph node");
            graph.deferred[index] = Some(owner);
        }
        graph.ordinary = actions;
        self.graph = Some(graph);
        Ok(RuntimeGfx942ScopedGraphTicketV1 {
            scope: Rc::clone(&self.identity),
            invariant: PhantomData,
        })
    }

    fn check_graph_ticket(
        &self,
        ticket: &RuntimeGfx942ScopedGraphTicketV1<'scope>,
    ) -> Result<&Graph<P>, RuntimeGfx942ScopeErrorV1> {
        if !Rc::ptr_eq(&self.identity, &ticket.scope) {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        self.graph
            .as_ref()
            .ok_or(RuntimeGfx942ScopeErrorV1::InvalidTicket)
    }

    pub fn graph_completion_future_v1(
        &mut self,
        ticket: &RuntimeGfx942ScopedGraphTicketV1<'scope>,
    ) -> Result<RuntimeGfx942ScopedGraphFutureV1<'scope>, RuntimeGfx942ScopeErrorV1> {
        self.check_graph_ticket(ticket)?;
        let future = self
            .graph
            .as_mut()
            .unwrap()
            .future
            .take()
            .ok_or(RuntimeGfx942ScopeErrorV1::CompletionObserverTaken)?;
        Ok(RuntimeGfx942ScopedGraphFutureV1 {
            future,
            _scope: Rc::clone(&self.identity),
            invariant: PhantomData,
        })
    }

    pub fn graph_report_v1(
        &self,
        ticket: &RuntimeGfx942ScopedGraphTicketV1<'scope>,
    ) -> Result<Option<&RuntimeGraphReportV1<KfdRuntimeBackendErrorV1>>, RuntimeGfx942ScopeErrorV1>
    {
        Ok(self.check_graph_ticket(ticket)?.report.as_ref())
    }

    /// Retrieves the original decoder ticket only after complete graph retirement.
    /// A cancelled/unissued generated node has no successful result ticket.
    pub fn graph_generated_ticket_v1(
        &self,
        ticket: &RuntimeGfx942ScopedGraphTicketV1<'scope>,
        node: CompletionNodeIdV1,
    ) -> Result<Option<RuntimeGfx942ScopedTicketV1<'scope>>, RuntimeGfx942ScopeErrorV1> {
        let graph = self.check_graph_ticket(ticket)?;
        if graph.report.is_none() {
            return Ok(None);
        }
        let index = graph
            .ids
            .binary_search(&node)
            .map_err(|_| RuntimeGfx942ScopeErrorV1::InvalidTicket)?;
        Ok(
            graph.generated[index].map(|index| RuntimeGfx942ScopedTicketV1 {
                scope: Rc::clone(&self.identity),
                index,
                invariant: PhantomData,
            }),
        )
    }

    pub(super) fn progress_graph_v1(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        let Some(mut graph) = self.graph.take() else {
            return Ok(0);
        };
        let result = graph.advance(self);
        self.graph = Some(graph);
        result
    }
}

macro_rules! graph_admission {
    ($backend:ty) => {
        impl<'scope, P: RuntimeGfx942GeneratedCompletionCarrierV1>
            RuntimeGfx942GeneratedScopeV1<'scope, '_, $backend, P>
        {
            /// Admits one bounded graph into an otherwise-empty scope. Every
            /// unbound Future is prepared once from the exact stream's checked
            /// device; existing ordinary graph copy/launch bindings are reused.
            /// Generated DATA is already frozen: edges order execution but do
            /// not turn predecessor outputs into new successor inputs. The
            /// existing single-device graph and ordinary hazard/version limits
            /// remain unchanged. No independent submissions may follow.
            pub fn try_admit_graph_v1<E>(
                &mut self,
                request: RuntimeGraphRequestV1<$backend>,
                mut prepare: impl FnMut(
                    CompletionNodeIdV1,
                    &fe2o3_kfd::CheckedGfx942XnackMinusDevice,
                ) -> Result<P, E>,
            ) -> Result<
                RuntimeGfx942ScopedGraphTicketV1<'scope>,
                RuntimeGfx942ScopedGraphAdmissionErrorV1<E>,
            > {
                self.admit_graph_with_v1(request, |context, node, stream| {
                    let device = context
                        .streams
                        .get(&stream)
                        .expect("validated graph stream")
                        .device;
                    context
                        .with_gfx942_preparation_device_v1(device, |checked| prepare(node, checked))
                })
            }
        }
    };
}
graph_admission!(KfdRuntimeBackendV1);
graph_admission!(KfdMultiDeviceRuntimeBackendV1);

impl<P: RuntimeGfx942GeneratedCompletionCarrierV1> Graph<P> {
    // SAFETY: these calls publish graph state only after the matching original
    // Context release and decoder, or dispose a definitely never-issued owner.
    #[allow(unsafe_code)]
    fn advance<B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>>(
        &mut self,
        scope: &mut RuntimeGfx942GeneratedScopeV1<'_, '_, B, P>,
    ) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        if !self.committed {
            return Ok(0);
        }
        let mut changed = 0;
        let core = self.core.as_mut().expect("retained graph authority");
        for index in 0..core.len() {
            if !core.issued(index) && core.state(index).is_terminal() && !self.reconciled[index] {
                // Failure propagation cannot dispose an issued owner. These
                // exact preparations never acquired a hold or reached submit.
                drop(self.deferred[index].take());
                drop(self.ordinary[index].take());
                self.reconciled[index] = true;
                changed += 1;
            }
            if let Some(slot) = self.generated[index]
                && !self.reconciled[index]
                && let Some(outcome) = &scope.slots[slot].lifecycle.outcome
            {
                if outcome.is_ok() {
                    // SAFETY: lifecycle outcome follows original native
                    // release plus consumption of its original decoder.
                    if !unsafe { core.succeed_operation(index) } {
                        scope.context.quarantine_after_async_command_panic_v1();
                        return Err(RuntimeGfx942ScopeErrorV1::Unknown);
                    }
                    self.observations
                        .push((core.id(index), RuntimeCompletionStatusV1::Succeeded));
                } else {
                    // SAFETY: a failed decoder is reached only after the
                    // same native release; it cannot create successful DATA.
                    unsafe {
                        core.fail(index, 4);
                    }
                    self.observations.push((
                        core.id(index),
                        RuntimeCompletionStatusV1::QuiescentWithoutResult,
                    ));
                }
                self.reconciled[index] = true;
                changed += 1;
            }
            let Some(mut active) = self.active[index].take() else {
                continue;
            };
            if let Some(status) = active.retiring {
                match scope
                    .context
                    .release_graph_submission_v1(core.token(), &active.submission)
                {
                    Ok(()) => {
                        if status == RuntimeCompletionStatusV1::Succeeded {
                            // SAFETY: the exact ordinary Context submission was
                            // just disposed after its successful terminal result.
                            if !unsafe { core.succeed_operation(index) } {
                                scope.context.quarantine_after_async_command_panic_v1();
                                return Err(RuntimeGfx942ScopeErrorV1::Unknown);
                            }
                        } else {
                            // SAFETY: failed/quiescent status and exact release
                            // are both observed on the original submission.
                            unsafe {
                                core.fail(index, 2);
                            }
                        }
                        self.reconciled[index] = true;
                        changed += 1;
                    }
                    Err(error) => {
                        self.active[index] = Some(active);
                        if scope.context.is_terminal() {
                            return Err(RuntimeGfx942ScopeErrorV1::Context(error));
                        }
                        self.rejected_releases = self.rejected_releases.saturating_add(1);
                    }
                }
                continue;
            }
            match scope
                .context
                .poll_with_graph_access_v1(&mut active.submission, Some(core.token()))
            {
                Ok(_) => {}
                Err(RuntimeErrorV1::BackendRejected(_)) => {
                    self.rejected_observations = self.rejected_observations.saturating_add(1);
                    self.active[index] = Some(active);
                    continue;
                }
                Err(error) => {
                    if scope.context.is_terminal()
                        || !matches!(error, RuntimeErrorV1::BackendQuiescent(_))
                    {
                        self.active[index] = Some(active);
                        scope.context.quarantine_after_async_command_panic_v1();
                        return Err(RuntimeGfx942ScopeErrorV1::Context(error));
                    }
                    self.errors.push((core.id(index), error));
                }
            }
            let status = scope
                .context
                .query_submission(&active.submission)
                .expect("original graph submission");
            if status != RuntimeCompletionStatusV1::Pending {
                self.observations.push((core.id(index), status));
                active.retiring = Some(status);
                changed += 1;
            }
            self.active[index] = Some(active);
        }

        if let Some(index) = core.pop_ready_notification() {
            changed += 1;
            // Host staging is driven explicitly with the original charged
            // output. Popping readiness is not a successful host write.
            if core.state(index) == CompletionNodeStateV1::Ready
                && core.host_staging(index).is_none()
            {
                if !core.begin(index) {
                    scope.context.quarantine_after_async_command_panic_v1();
                    return Err(RuntimeGfx942ScopeErrorV1::Unknown);
                }
                if let Some(owner) = self.deferred[index].take() {
                    match activate(scope, owner, core.stream(index), core.token()) {
                        Ok(slot) => self.generated[index] = Some(slot),
                        Err(error) => {
                            if scope.context.is_terminal() {
                                return Err(RuntimeGfx942ScopeErrorV1::Context(error));
                            }
                            self.errors.push((core.id(index), error));
                            // SAFETY: activation errors occur before the stream
                            // hold/slot is committed and before native adoption.
                            unsafe {
                                core.fail(index, 3);
                            }
                            self.reconciled[index] = true;
                        }
                    }
                } else if let Some(action) = self.ordinary[index].take() {
                    match (scope.hooks.graph_submit)(scope.context, core.token(), *action) {
                        Ok(submission) => {
                            self.active[index] = Some(Ordinary {
                                submission,
                                retiring: None,
                            })
                        }
                        Err(error) => {
                            if scope.context.is_terminal() {
                                return Err(RuntimeGfx942ScopeErrorV1::Context(error));
                            }
                            self.errors.push((core.id(index), error));
                            // SAFETY: Context retains ambiguous attempts and
                            // seals itself; a live-context refusal has no owner.
                            unsafe {
                                core.fail(index, 1);
                            }
                            self.reconciled[index] = true;
                        }
                    }
                } else {
                    // SAFETY: every unbound Future was populated by preparation;
                    // only validated host-only joins have neither action kind.
                    unsafe {
                        core.succeed_join(index);
                    }
                    self.reconciled[index] = true;
                }
            }
        }

        for &stream in core.streams() {
            if let Err(error) =
                (scope.hooks.graph_progress)(scope.context, stream, Some(core.token()))
                && scope.context.is_terminal()
            {
                return Err(RuntimeGfx942ScopeErrorV1::Context(error));
            }
        }
        if core.is_terminal()
            && self.active.iter().all(Option::is_none)
            && self.deferred.iter().all(Option::is_none)
            && self.ordinary.iter().all(Option::is_none)
            && self
                .generated
                .iter()
                .flatten()
                .all(|&slot| !scope.slots[slot].lifecycle.unsettled())
        {
            let core = self.core.take().expect("retained graph");
            let retired = match core.finish(scope.context) {
                Ok(retired) => retired,
                Err(failure) => {
                    self.core = Some(failure.graph);
                    return Err(RuntimeGfx942ScopeErrorV1::Graph(failure.error));
                }
            };
            self.report = Some(RuntimeGraphReportV1 {
                execution: retired.execution,
                versions: retired.versions,
                version_inputs: retired.version_inputs,
                completion: retired.completion,
                observations: std::mem::take(&mut self.observations),
                errors: std::mem::take(&mut self.errors),
                rejected_observations: self.rejected_observations,
                rejected_releases: self.rejected_releases,
            });
            self.committed = false;
            self.reply.complete(Ok(()));
            changed += 1;
        }
        Ok(changed)
    }
}

fn activate<B, P>(
    scope: &mut RuntimeGfx942GeneratedScopeV1<'_, '_, B, P>,
    owner: Deferred<P>,
    stream: RuntimeStreamIdV1,
    token: ContextGraphReservationV1,
) -> Result<usize, NativeError>
where
    B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>,
    P: RuntimeGfx942GeneratedCompletionCarrierV1,
{
    (scope.hooks.preflight)(
        scope.context,
        &owner.prepared,
        &owner.roster,
        stream,
        Some(token),
    )?;
    let hold = scope
        .context
        .hold_unpublished_stream_with_access_v1(stream, Some(token))?;
    let index = scope.slots.len();
    // Graph admission bounds the complete roster and preallocates each reply.
    // No fallible operation lies between this original hold and slot rooting.
    scope.slots.push(Slot {
        lifecycle: Lifecycle::new(owner.prepared),
        roster: owner.roster,
        hold,
        domain: owner.domain,
        reply: owner.reply,
        future: Some(owner.future),
    });
    Ok(index)
}

#[cfg(test)]
mod backing_tests {
    use super::*;
    #[test]
    fn scoped_graph_backing_uses_actual_capacities_and_checked_combined_ceiling() {
        let entries = vec![0u64; 3];
        let bytes = entries.capacity() * size_of::<u64>();
        let mut total = MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1 - bytes;
        add_backing(&mut total, &entries).unwrap();
        assert_eq!(total, MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1);
        assert!(add_backing(&mut total, &entries).is_err());
        assert_eq!(total, MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1);
        let mut overflow = usize::MAX;
        assert!(add_backing(&mut overflow, &entries).is_err());
        assert_eq!(overflow, usize::MAX);
    }

    #[test]
    fn staging_backing_counts_capacity_and_refuses_without_changing_total() {
        let mut entries = Vec::<Option<crate::async_engine::HostStagingV1>>::with_capacity(8);
        entries.push(None);
        assert!(entries.capacity() > entries.len());
        let bytes = entries.capacity() * size_of::<Option<crate::async_engine::HostStagingV1>>();
        let mut total = MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1 - bytes;
        add_backing_bytes(&mut total, bytes).unwrap();
        assert_eq!(total, MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1);
        assert!(add_backing_bytes(&mut total, 1).is_err());
        assert_eq!(total, MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1);
        assert!(add_backing_bytes(&mut total, usize::MAX).is_err());
        assert_eq!(total, MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1);
    }
}
