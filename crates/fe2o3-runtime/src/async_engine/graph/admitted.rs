//! Shared owner-local admission and graph state, independent of operation storage.

use super::*;

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod backing_tests {
    use super::*;

    #[test]
    fn prepared_staging_backing_observes_reserved_capacity_not_node_length() {
        let mut context =
            RuntimeContextV1::open(crate::KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1())
                .unwrap();
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        let identity = context.completion_stream_identity_v1(stream).unwrap();
        let node = CompletionNodeIdV1::new(1).unwrap();
        let mut request = Some(
            RuntimeGraphRequestV1::new(
                CompletionGraphV1::new(
                    identity.context(),
                    vec![identity],
                    vec![fe2o3_completion::CompletionNodeV1::future(
                        node,
                        fe2o3_completion::FutureIdentityV1::new(identity, [1; 32]),
                        None,
                    )],
                )
                .unwrap(),
                vec![(identity, stream)],
            )
            .unwrap(),
        );
        let mut plan =
            PreparedGraphAdmissionV1::prepare_scoped_v1(&mut context, &mut request, |_, _, _| {
                Ok::<(), RuntimeGraphErrorV1<crate::KfdRuntimeBackendErrorV1>>(())
            })
            .unwrap();
        plan.host_staging.reserve_exact(8);
        assert!(plan.host_staging.capacity() > plan.len());
        assert_eq!(
            plan.host_staging_backing_bytes_v1(),
            Some(plan.host_staging.capacity() * std::mem::size_of::<Option<HostStagingV1>>())
        );
        assert_ne!(
            plan.host_staging_backing_bytes_v1(),
            Some(plan.len() * std::mem::size_of::<Option<HostStagingV1>>())
        );
        drop(plan);
        assert!(context.cleanup().is_complete());
    }
}

/// Failed pre-publication admission returns the exact still-owned request.
pub(crate) struct GraphAdmissionFailureV1<B: RuntimeBackendV1, E> {
    pub(crate) request: RuntimeGraphRequestV1<B>,
    pub(crate) error: E,
}

type GraphAdmissionResultV1<B> = Result<
    (
        AdmittedGraphV1,
        Vec<Option<Box<PreparedContextGraphActionV1>>>,
    ),
    GraphAdmissionFailureV1<B, RuntimeGraphErrorV1<<B as RuntimeBackendV1>::Error>>,
>;

/// Frozen validation and ordinary preparations, before Context reservation.
/// Generated DATA remains frozen in the adapter's original carriers; dependency
/// edges do not rebind it or add it to the ordinary allocation version ledger.
pub(crate) struct PreparedGraphAdmissionV1<B: RuntimeBackendV1> {
    request: RuntimeGraphRequestV1<B>,
    versions: versions::VersionLedger,
    actions: Vec<Option<Box<PreparedContextGraphActionV1>>>,
    ids: Vec<CompletionNodeIdV1>,
    node_streams: Vec<RuntimeStreamIdV1>,
    issued: Vec<bool>,
    streams: Vec<RuntimeStreamIdV1>,
    host_staging: Vec<Option<HostStagingV1>>,
}

impl<B: RuntimeBackendV1> PreparedGraphAdmissionV1<B> {
    /// The callback must validate each otherwise-unbound Future against its
    /// retained original owner, including completion support. It may not publish
    /// work or acquire a stream hold. All checks run before graph reservation.
    /// The original request stays in its caller's owner on refusal or unwind;
    /// only successful preparation moves it into this owner.
    pub(crate) fn prepare<E: From<RuntimeGraphErrorV1<B::Error>>>(
        context: &mut RuntimeContextV1<B>,
        request: &mut Option<RuntimeGraphRequestV1<B>>,
        validate_generated: impl FnMut(
            &mut RuntimeContextV1<B>,
            CompletionNodeIdV1,
            RuntimeStreamIdV1,
        ) -> Result<(), E>,
    ) -> Result<Self, E> {
        Self::prepare_with_host_staging_v1(context, request, validate_generated, false)
    }

    pub(crate) fn prepare_scoped_v1<E: From<RuntimeGraphErrorV1<B::Error>>>(
        context: &mut RuntimeContextV1<B>,
        request: &mut Option<RuntimeGraphRequestV1<B>>,
        validate_generated: impl FnMut(
            &mut RuntimeContextV1<B>,
            CompletionNodeIdV1,
            RuntimeStreamIdV1,
        ) -> Result<(), E>,
    ) -> Result<Self, E> {
        Self::prepare_with_host_staging_v1(context, request, validate_generated, true)
    }

    fn prepare_with_host_staging_v1<E: From<RuntimeGraphErrorV1<B::Error>>>(
        context: &mut RuntimeContextV1<B>,
        request: &mut Option<RuntimeGraphRequestV1<B>>,
        mut validate_generated: impl FnMut(
            &mut RuntimeContextV1<B>,
            CompletionNodeIdV1,
            RuntimeStreamIdV1,
        ) -> Result<(), E>,
        allow_host_staging: bool,
    ) -> Result<Self, E> {
        let original = request.as_ref().expect("unadmitted graph request");
        if !allow_host_staging
            && original
                .actions
                .values()
                .any(|action| matches!(action, Action::HostStaging(_)))
        {
            return Err(RuntimeGraphErrorV1::Invalid(
                RuntimeGraphValidationErrorV1::HostStagingRequiresScope,
            )
            .into());
        }
        let mut prepare = || {
            for (&identity, &stream) in &original.streams {
                let actual = context
                    .completion_stream_identity_v1(stream)
                    .map_err(|e| RuntimeGraphErrorV1::Context(e.into()))?;
                if actual != identity {
                    return Err(RuntimeGraphErrorV1::Invalid(
                        RuntimeGraphValidationErrorV1::StreamBindings,
                    )
                    .into());
                }
            }
            let mut actions = Vec::new();
            let mut ids = Vec::new();
            let mut node_streams = Vec::new();
            let mut issued = Vec::new();
            let mut host_staging = Vec::new();
            for node in original.graph.nodes() {
                let stream = original.streams[&node.stream()];
                let action = match (node.kind(), original.actions.get(&node.id())) {
                    (CompletionNodeKindV1::Future(_), Some(Action::Launch(launch))) => Some(
                        launch
                            .prepare(context, stream)
                            .map_err(RuntimeGraphErrorV1::Context)?,
                    ),
                    (CompletionNodeKindV1::Future(_), Some(Action::Copy(source, destination))) => {
                        Some(
                            context
                                .prepare_graph_copy_v1(stream, *source, *destination)
                                .map_err(RuntimeGraphErrorV1::Context)?,
                        )
                    }
                    (CompletionNodeKindV1::Future(_), Some(Action::HostStaging(staging))) => {
                        context
                            .prepare_graph_host_staging_v1(stream, staging.destination)
                            .map_err(RuntimeGraphErrorV1::Context)?;
                        None
                    }
                    (CompletionNodeKindV1::Future(_), None) => {
                        validate_generated(context, node.id(), stream)?;
                        None
                    }
                    _ => None,
                };
                actions.push(action.map(Box::new));
                ids.push(node.id());
                node_streams.push(stream);
                issued.push(false);
                host_staging.push(match original.actions.get(&node.id()) {
                    Some(Action::HostStaging(staging)) => Some(*staging),
                    _ => None,
                });
            }
            original
                .validate_hazards()
                .map_err(RuntimeGraphErrorV1::Invalid)?;
            let versions =
                versions::VersionLedger::prepare(original).map_err(RuntimeGraphErrorV1::Invalid)?;
            let streams = original.streams.values().copied().collect();
            Ok((
                versions,
                actions,
                ids,
                node_streams,
                issued,
                streams,
                host_staging,
            ))
        };
        match prepare() {
            Ok((versions, actions, ids, node_streams, issued, streams, host_staging)) => Ok(Self {
                request: request.take().expect("successfully prepared request"),
                versions,
                actions,
                ids,
                node_streams,
                issued,
                streams,
                host_staging,
            }),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.ids.len()
    }

    /// Newly retained staging backing, separate from the pre-existing core.
    pub(crate) fn host_staging_backing_bytes_v1(&self) -> Option<usize> {
        self.host_staging
            .capacity()
            .checked_mul(std::mem::size_of::<Option<HostStagingV1>>())
    }

    /// The caller preallocates its active/observation rosters before this step.
    /// No native action is issued here. The exact retained request is consumed
    /// only after the original Context grants its exclusive reservation.
    // Refusal returns the original request without a new fallible heap allocation.
    #[allow(clippy::result_large_err)]
    pub(crate) fn commit(self, context: &mut RuntimeContextV1<B>) -> GraphAdmissionResultV1<B> {
        let token = match context.reserve_graph_v1(self.ids.len()) {
            Ok(token) => token,
            Err(error) => {
                return Err(GraphAdmissionFailureV1 {
                    request: self.request,
                    error: RuntimeGraphErrorV1::Context(error.into()),
                });
            }
        };
        let execution = RuntimeGraphExecutionIdentityV1 {
            context: self.request.graph.context(),
            graph: self.request.graph.identity(),
            generation: token.generation(),
        };
        Ok((
            AdmittedGraphV1 {
                versions: self.versions,
                authority: self.request.graph.into_completion_authority(),
                token,
                execution,
                ids: self.ids,
                node_streams: self.node_streams,
                issued: self.issued,
                streams: self.streams,
                host_staging: self.host_staging,
            },
            self.actions,
        ))
    }
}

/// One actual Context reservation and its exact in-memory scheduling authority.
/// Dropping this owner does not release the Context reservation or native work.
pub(crate) struct AdmittedGraphV1 {
    versions: versions::VersionLedger,
    authority: CompletionAuthorityV1,
    token: ContextGraphReservationV1,
    execution: RuntimeGraphExecutionIdentityV1,
    ids: Vec<CompletionNodeIdV1>,
    node_streams: Vec<RuntimeStreamIdV1>,
    issued: Vec<bool>,
    streams: Vec<RuntimeStreamIdV1>,
    host_staging: Vec<Option<HostStagingV1>>,
}

/// Descriptive terminal data, produced only after exact Context release.
pub(crate) struct RetiredGraphV1 {
    pub(crate) execution: RuntimeGraphExecutionIdentityV1,
    pub(crate) versions: Vec<RuntimeGraphVersionRecordV1>,
    pub(crate) version_inputs: Vec<RuntimeGraphInputVersionV1>,
    pub(crate) completion: CompletionReportV1,
}

pub(crate) struct GraphRetirementFailureV1<E> {
    pub(crate) graph: AdmittedGraphV1,
    pub(crate) error: RuntimeGraphErrorV1<E>,
}

impl AdmittedGraphV1 {
    pub(crate) fn len(&self) -> usize {
        self.ids.len()
    }

    pub(crate) fn index(&self, node: CompletionNodeIdV1) -> Option<usize> {
        self.ids.binary_search(&node).ok()
    }

    pub(crate) fn id(&self, index: usize) -> CompletionNodeIdV1 {
        self.ids[index]
    }

    pub(crate) fn stream(&self, index: usize) -> RuntimeStreamIdV1 {
        self.node_streams[index]
    }

    pub(crate) fn streams(&self) -> &[RuntimeStreamIdV1] {
        &self.streams
    }

    pub(crate) fn token(&self) -> ContextGraphReservationV1 {
        self.token
    }

    pub(crate) fn host_staging(&self, index: usize) -> Option<HostStagingV1> {
        self.host_staging[index]
    }

    pub(crate) fn issued(&self, index: usize) -> bool {
        self.issued[index]
    }

    pub(crate) fn state(&self, index: usize) -> CompletionNodeStateV1 {
        self.authority
            .state(self.ids[index])
            .expect("admitted node")
    }

    /// May return a stale cancellation notification. The adapter must check
    /// `state` and preserve one-notification-per-step scheduling semantics.
    pub(crate) fn pop_ready_notification(&mut self) -> Option<usize> {
        let node = self.authority.pop_ready_notification()?;
        Some(self.index(node).expect("admitted notification"))
    }

    /// Commits the exact planned input/output lineage before backend entry.
    pub(crate) fn begin(&mut self, index: usize) -> bool {
        assert!(!self.issued[index], "graph occurrence cannot issue twice");
        if !self.versions.begin(index) {
            return false;
        }
        self.issued[index] = true;
        true
    }

    /// # Safety
    /// The exact operation must have successfully retired through the original
    /// Context/native owner and decoder, not merely a notification or status ID.
    pub(crate) unsafe fn succeed_operation(&mut self, index: usize) -> bool {
        if !self.versions.commit(index) {
            return false;
        }
        // SAFETY: delegated exact-retirement obligation is established above.
        unsafe { self.authority.mark_succeeded(self.ids[index]).unwrap() };
        true
    }

    /// # Safety
    /// This separately admitted HostStaging node's exact synchronous Context
    /// write and original journal settlement must have succeeded under `token`.
    /// Generated completion alone or supplied bytes cannot establish this premise.
    pub(crate) unsafe fn succeed_host_staging(&mut self, index: usize) -> bool {
        if self.host_staging[index].is_none() || !self.versions.commit(index) {
            return false;
        }
        // SAFETY: the exact ordinary host-write settlement is the caller's
        // obligation; this commits no generated kernel effect or provenance.
        unsafe { self.authority.mark_succeeded(self.ids[index]).unwrap() };
        true
    }

    /// # Safety
    /// This node must be a validated host-only event join whose exact
    /// predecessors succeeded. This does not accept a native operation result.
    pub(crate) unsafe fn succeed_join(&mut self, index: usize) {
        // SAFETY: the caller distinguishes host joins from operation nodes.
        unsafe { self.authority.mark_succeeded(self.ids[index]).unwrap() };
    }

    /// # Safety
    /// Establish definite nonpublication or exact quiescent failure, retaining
    /// every potentially reachable owner on all other outcomes.
    pub(crate) unsafe fn fail(&mut self, index: usize, code: u32) {
        self.versions.fail(index);
        // SAFETY: exact failed-occurrence observation is the caller's obligation.
        unsafe {
            self.authority
                .mark_failed(self.ids[index], FailureCodeV1::new(code).unwrap())
                .unwrap()
        };
    }

    /// # Safety
    /// First dispose the exact never-issued generated native owner; a ticket or
    /// observer drop alone is not disposal. Ordinary Context preparations are
    /// inert and may remain retained, but must never issue after this transition.
    /// No possibly published owner may be released.
    pub(crate) unsafe fn cancel_unissued(&mut self, index: usize) {
        assert!(!self.issued[index], "issued work cannot be cancelled here");
        let node = self.ids[index];
        let reason = CancellationCodeV1::new(1).unwrap();
        match self.state(index) {
            CompletionNodeStateV1::Blocked => {
                self.authority.cancel_blocked(node, reason).unwrap();
            }
            CompletionNodeStateV1::Ready => {
                self.authority.request_cancel(node, reason).unwrap();
                // SAFETY: no issued token exists and original preparation retired.
                unsafe { self.authority.mark_cancelled(node).unwrap() };
            }
            _ => {}
        }
    }

    pub(crate) fn is_terminal(&self) -> bool {
        self.authority.is_terminal()
    }

    /// The adapter must retain this owner while operations remain active. This
    /// final step also checks the actual Context's complete native owner roster;
    /// a terminal dependency state alone never releases the reservation.
    // Failed retirement returns the complete owner without allocating to retain it.
    #[allow(clippy::result_large_err)]
    pub(crate) fn finish<B: RuntimeBackendV1>(
        self,
        context: &mut RuntimeContextV1<B>,
    ) -> Result<RetiredGraphV1, GraphRetirementFailureV1<B::Error>> {
        if !self.authority.is_terminal() {
            return Err(GraphRetirementFailureV1 {
                graph: self,
                error: RuntimeGraphErrorV1::Busy,
            });
        }
        if !self.versions.report_ready() {
            context.quarantine_after_async_command_panic_v1();
            return Err(GraphRetirementFailureV1 {
                graph: self,
                error: RuntimeGraphErrorV1::Invalid(
                    RuntimeGraphValidationErrorV1::VersionNotAvailable,
                ),
            });
        }
        if let Err(error) = context
            .close_graph_issue_v1(self.token)
            .and_then(|()| context.release_graph_v1(self.token))
        {
            context.quarantine_after_async_command_panic_v1();
            return Err(GraphRetirementFailureV1 {
                graph: self,
                error: RuntimeGraphErrorV1::Context(error.into()),
            });
        }
        let (versions, version_inputs) = self
            .versions
            .report(self.execution)
            .expect("unchanged retired version ledger");
        let completion = self
            .authority
            .try_into_report()
            // fe2o3-hygiene: allow-panic (#182): the same authority was checked terminal and never mutated; refusal cannot be recovered after exact reservation retirement.
            .unwrap_or_else(|_| panic!("terminal graph"));
        Ok(RetiredGraphV1 {
            execution: self.execution,
            versions,
            version_inputs,
            completion,
        })
    }
}
