//! Runtime binding of the existing completion graph; no compiler authority.

use super::*;
use crate::context::{ContextGraphReservationV1, PreparedContextGraphActionV1};
use crate::{
    RuntimeAccessV1, RuntimeArgumentsV1, RuntimeAsyncCopyBackendV1, RuntimeBindingV1,
    RuntimeLaunchGeometryV1, RuntimeMemoryRegionV1, RuntimeSubmissionV1, TypedRuntimeKernelV1,
};
use fe2o3_completion::{
    CancellationCodeV1, CompletionAuthorityV1, CompletionGraphIdentityV1, CompletionGraphV1,
    CompletionNodeIdV1, CompletionNodeKindV1, CompletionNodeStateV1, CompletionReportV1,
    ContextIdentityV1, FailureCodeV1, StreamIdentityV1,
};
use std::collections::VecDeque;
mod generated;
mod versions;
use generated::GraphReplyV1;
pub use generated::*;
pub use versions::{
    MAX_RUNTIME_GRAPH_VERSION_REFERENCES_V1, MAX_RUNTIME_GRAPH_VERSIONS_V1,
    RuntimeGraphDataVersionV1, RuntimeGraphInputVersionV1, RuntimeGraphVersionRecordV1,
    RuntimeGraphVersionSourceV1, RuntimeGraphVersionStateV1,
};

pub const MAX_RUNTIME_GRAPH_NODES_V1: usize = 256;
pub const MAX_RUNTIME_GRAPH_KERNARG_BYTES_V1: usize = 65_536;
pub const MAX_RUNTIME_GRAPH_EFFECTS_V1: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeGraphValidationErrorV1 {
    Capacity,
    StreamBindings,
    UnknownNode,
    NotOperation,
    DuplicateOperation,
    MissingOperation,
    DuplicateVersionInput,
    InvalidVersionInput,
    VersionNotAvailable,
    UnorderedMemoryConflict {
        first: CompletionNodeIdV1,
        second: CompletionNodeIdV1,
    },
}

impl fmt::Display for RuntimeGraphValidationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for RuntimeGraphValidationErrorV1 {}

#[derive(Debug)]
pub enum RuntimeGraphErrorV1<E> {
    Invalid(RuntimeGraphValidationErrorV1),
    Context(RuntimeErrorV1<E>),
    Busy,
}

impl<E: fmt::Display> fmt::Display for RuntimeGraphErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(f),
            Self::Context(error) => error.fmt(f),
            Self::Busy => f.write_str("graph admission requires an idle owner context"),
        }
    }
}
impl<E: Error + 'static> Error for RuntimeGraphErrorV1<E> {}

/// Fully retired dependency report, or a rejected/indeterminate graph error.
pub type RuntimeGraphResultV1<E> = Result<RuntimeGraphReportV1<E>, RuntimeGraphErrorV1<E>>;

/// A terminal dependency report is returned only after every native submission
/// has been released. Failed/cancelled nodes are not successful data versions.
#[derive(Debug)]
pub struct RuntimeGraphReportV1<E> {
    pub execution: RuntimeGraphExecutionIdentityV1,
    pub versions: Vec<RuntimeGraphVersionRecordV1>,
    pub version_inputs: Vec<RuntimeGraphInputVersionV1>,
    pub completion: CompletionReportV1,
    pub observations: Vec<(CompletionNodeIdV1, RuntimeCompletionStatusV1)>,
    pub errors: Vec<(CompletionNodeIdV1, RuntimeErrorV1<E>)>,
    pub rejected_observations: u64,
    pub rejected_releases: u64,
}

/// One admitted process-local graph occurrence, not a data version or authority.
/// The structural identity excludes bound argument bytes. Generation may have
/// gaps and is scoped to the context, not a GPU-reset or distributed epoch.
///
/// ```compile_fail
/// use fe2o3_runtime::RuntimeGraphExecutionIdentityV1;
/// use fe2o3_runtime::completion::{ContextIdentityV1, CompletionGraphIdentityV1};
/// fn forge(context: ContextIdentityV1, graph: CompletionGraphIdentityV1) {
///     let forged = RuntimeGraphExecutionIdentityV1 { context, graph, generation: 1 };
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct RuntimeGraphExecutionIdentityV1 {
    context: ContextIdentityV1,
    graph: CompletionGraphIdentityV1,
    generation: u64,
}
impl RuntimeGraphExecutionIdentityV1 {
    pub const fn context(self) -> ContextIdentityV1 {
        self.context
    }
    pub const fn graph_identity(self) -> CompletionGraphIdentityV1 {
        self.graph
    }
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// Local cancellation requests closure of unissued nodes when the owner next
/// observes it. It does not withdraw native work or acknowledge quiescence.
#[derive(Clone)]
pub struct RuntimeGraphControlV1(Arc<AtomicBool>);
impl RuntimeGraphControlV1 {
    pub fn cancel_unissued(&self) {
        self.0.store(true, Ordering::Release);
    }
}

#[must_use = "dropping graph observation does not cancel execution"]
pub struct RuntimeAsyncGraphFutureV1<E> {
    future: RuntimeAsyncCommandFutureV1<Result<RuntimeGraphReportV1<E>, RuntimeGraphErrorV1<E>>>,
    control: RuntimeGraphControlV1,
}
impl<E> RuntimeAsyncGraphFutureV1<E> {
    pub fn control(&self) -> RuntimeGraphControlV1 {
        self.control.clone()
    }
}
impl<E> Future for RuntimeAsyncGraphFutureV1<E> {
    type Output = Result<
        Result<RuntimeGraphReportV1<E>, RuntimeGraphErrorV1<E>>,
        RuntimeAsyncEngineCallErrorV1,
    >;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.future).poll(cx)
    }
}

trait FrozenLaunch<B: RuntimeBackendV1>: Send {
    fn prepare(
        &self,
        context: &RuntimeContextV1<B>,
        stream: RuntimeStreamIdV1,
    ) -> Result<PreparedContextGraphActionV1, RuntimeErrorV1<B::Error>>;
    fn bindings(&self) -> &[RuntimeBindingV1];
}
struct Launch<A: RuntimeArgumentsV1> {
    kernel: Arc<TypedRuntimeKernelV1<A>>,
    bytes: Box<[u8]>,
    bindings: Box<[RuntimeBindingV1]>,
    geometry: RuntimeLaunchGeometryV1,
}
impl<B: RuntimeBackendV1, A: RuntimeArgumentsV1> FrozenLaunch<B> for Launch<A> {
    fn prepare(
        &self,
        context: &RuntimeContextV1<B>,
        stream: RuntimeStreamIdV1,
    ) -> Result<PreparedContextGraphActionV1, RuntimeErrorV1<B::Error>> {
        context.prepare_graph_launch_v1(
            stream,
            &self.kernel,
            &self.bytes,
            &self.bindings,
            self.geometry,
        )
    }
    fn bindings(&self) -> &[RuntimeBindingV1] {
        &self.bindings
    }
}
enum Action<B: RuntimeBackendV1> {
    Launch(Box<dyn FrozenLaunch<B>>),
    Copy(RuntimeMemoryRegionV1, RuntimeMemoryRegionV1),
}

/// A bounded, process-local graph request. Its identities and effects are
/// checked against a live context; they never grant executable authority.
///
/// Argument getters run once when binding. Only their returned bytes/bindings
/// are frozen, not an atomic snapshot of application state. Encoder allocation
/// is caller-controlled; the limits bound retained snapshots, not callbacks.
pub struct RuntimeGraphRequestV1<B: RuntimeBackendV1> {
    graph: CompletionGraphV1,
    streams: BTreeMap<StreamIdentityV1, RuntimeStreamIdV1>,
    actions: BTreeMap<CompletionNodeIdV1, Action<B>>,
    kernarg_bytes: usize,
    effects: usize,
    version_inputs: BTreeMap<versions::InputKey, RuntimeGraphVersionSourceV1>,
}
impl<B: RuntimeBackendV1> RuntimeGraphRequestV1<B> {
    pub fn new(
        graph: CompletionGraphV1,
        streams: Vec<(StreamIdentityV1, RuntimeStreamIdV1)>,
    ) -> Result<Self, RuntimeGraphValidationErrorV1> {
        if graph.nodes().len() > MAX_RUNTIME_GRAPH_NODES_V1
            || graph.streams().len() > MAX_RUNTIME_GRAPH_NODES_V1
        {
            return Err(RuntimeGraphValidationErrorV1::Capacity);
        }
        if streams.len() != graph.streams().len() {
            return Err(RuntimeGraphValidationErrorV1::StreamBindings);
        }
        let bindings: BTreeMap<_, _> = streams.iter().copied().collect();
        let unique: std::collections::BTreeSet<_> =
            streams.iter().map(|(_, runtime)| runtime).collect();
        if bindings.len() != streams.len()
            || unique.len() != streams.len()
            || bindings.keys().copied().ne(graph.streams().iter().copied())
        {
            return Err(RuntimeGraphValidationErrorV1::StreamBindings);
        }
        Ok(Self {
            graph,
            streams: bindings,
            actions: BTreeMap::new(),
            kernarg_bytes: 0,
            effects: 0,
            version_inputs: BTreeMap::new(),
        })
    }

    fn check_node(&self, node: CompletionNodeIdV1) -> Result<(), RuntimeGraphValidationErrorV1> {
        let index = self
            .graph
            .nodes()
            .binary_search_by_key(&node, |n| n.id())
            .map_err(|_| RuntimeGraphValidationErrorV1::UnknownNode)?;
        if !matches!(
            self.graph.nodes()[index].kind(),
            CompletionNodeKindV1::Future(_)
        ) {
            return Err(RuntimeGraphValidationErrorV1::NotOperation);
        }
        if self.actions.contains_key(&node) {
            return Err(RuntimeGraphValidationErrorV1::DuplicateOperation);
        }
        Ok(())
    }

    pub fn bind_launch<A: RuntimeArgumentsV1>(
        &mut self,
        node: CompletionNodeIdV1,
        kernel: Arc<TypedRuntimeKernelV1<A>>,
        arguments: &A,
        geometry: RuntimeLaunchGeometryV1,
    ) -> Result<(), RuntimeGraphValidationErrorV1> {
        self.check_node(node)?;
        let bytes = arguments.encode_explicit_kernarg_v1();
        if bytes.len() > MAX_RUNTIME_GRAPH_KERNARG_BYTES_V1 - self.kernarg_bytes {
            return Err(RuntimeGraphValidationErrorV1::Capacity);
        }
        let bindings = arguments.bindings_v1();
        if bindings.len() > MAX_RUNTIME_GRAPH_EFFECTS_V1 - self.effects {
            return Err(RuntimeGraphValidationErrorV1::Capacity);
        }
        self.kernarg_bytes += bytes.len();
        self.effects += bindings.len();
        self.actions.insert(
            node,
            Action::Launch(Box::new(Launch {
                kernel,
                bytes: bytes.into_boxed_slice(),
                bindings: bindings.into_boxed_slice(),
                geometry,
            })),
        );
        Ok(())
    }

    pub fn bind_copy(
        &mut self,
        node: CompletionNodeIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<(), RuntimeGraphValidationErrorV1> {
        self.check_node(node)?;
        if self.effects > MAX_RUNTIME_GRAPH_EFFECTS_V1 - 2 {
            return Err(RuntimeGraphValidationErrorV1::Capacity);
        }
        self.effects += 2;
        self.actions.insert(node, Action::Copy(source, destination));
        Ok(())
    }

    fn validate_hazards(&self) -> Result<(), RuntimeGraphValidationErrorV1> {
        let nodes = self.graph.nodes();
        let index = |id| {
            nodes
                .binary_search_by_key(&id, |n| n.id())
                .expect("validated graph node")
        };
        let mut ancestors = vec![[0u64; MAX_RUNTIME_GRAPH_NODES_V1 / 64]; nodes.len()];
        for id in self.graph.topological_order() {
            let i = index(*id);
            let record = match nodes[i].kind() {
                CompletionNodeKindV1::EventWait { recorded_by, .. } => Some(recorded_by),
                _ => None,
            };
            for predecessor in [nodes[i].stream_predecessor(), record.copied()]
                .into_iter()
                .flatten()
            {
                let p = index(predecessor);
                for word in 0..ancestors[i].len() {
                    ancestors[i][word] |= ancestors[p][word];
                }
                ancestors[i][p / 64] |= 1 << (p % 64);
            }
        }
        let mut effects = Vec::with_capacity(self.effects);
        for (&node, action) in &self.actions {
            let mut add = |region: RuntimeMemoryRegionV1| {
                effects.push((
                    region.allocation,
                    region.byte_offset,
                    region.byte_len,
                    region.access,
                    index(node),
                ))
            };
            match action {
                Action::Launch(launch) => {
                    for binding in launch.bindings() {
                        add(binding.region);
                    }
                }
                Action::Copy(source, destination) => {
                    add(*source);
                    add(*destination);
                }
            }
        }
        effects.sort_unstable_by_key(|effect| (effect.0, effect.1));
        for (i, &(allocation, start, len, access, first)) in effects.iter().enumerate() {
            // Context range validation precedes this check, so addition cannot overflow.
            let end = start.checked_add(len).expect("validated region");
            for &(other, offset, _, other_access, second) in &effects[i + 1..] {
                if other != allocation || offset >= end {
                    break;
                }
                if first == second
                    || (access == RuntimeAccessV1::Read && other_access == RuntimeAccessV1::Read)
                {
                    continue;
                }
                if ancestors[first][second / 64] & (1 << (second % 64)) == 0
                    && ancestors[second][first / 64] & (1 << (first % 64)) == 0
                {
                    return Err(RuntimeGraphValidationErrorV1::UnorderedMemoryConflict {
                        first: nodes[first].id(),
                        second: nodes[second].id(),
                    });
                }
            }
        }
        Ok(())
    }
}

pub(super) trait EngineGraphV1<B: RuntimeBackendV1>: Send {
    fn admit(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
    ) -> bool;
    fn advance(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
        budget: usize,
        flush_budget: usize,
    ) -> bool;
    fn reject(&mut self, error: RuntimeGraphErrorV1<B::Error>);
    fn stop(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
    );
    fn take_generated_request(&mut self) -> Option<RuntimeGeneratedGraphRequestV1<B>> {
        None
    }
}

enum PreparedGraphActionV1 {
    Ordinary(Box<PreparedContextGraphActionV1>),
    Generated(RuntimeAsyncReservedTicketV1),
}

enum ActiveGraphActionV1 {
    Ordinary(
        usize,
        RuntimeSubmissionV1<()>,
        Option<RuntimeCompletionStatusV1>,
    ),
    Generated(usize, RuntimeAsyncGeneratedCompletionV1),
}

struct Graph<B: RuntimeBackendV1> {
    versions: Option<versions::VersionLedger>,
    request: Option<RuntimeGraphRequestV1<B>>,
    generated: BTreeMap<CompletionNodeIdV1, RuntimeAsyncReservedTicketV1>,
    authority: Option<CompletionAuthorityV1>,
    token: Option<ContextGraphReservationV1>,
    execution: Option<RuntimeGraphExecutionIdentityV1>,
    actions: Vec<Option<PreparedGraphActionV1>>,
    ids: Vec<CompletionNodeIdV1>,
    node_streams: Vec<RuntimeStreamIdV1>,
    issued: Vec<bool>,
    active: VecDeque<ActiveGraphActionV1>,
    streams: Vec<RuntimeStreamIdV1>,
    flush_cursor: usize,
    next_poll: bool,
    cancel_applied: bool,
    terminal_cleanup_pending: bool,
    control: RuntimeGraphControlV1,
    slot: Option<Arc<AtomicBool>>,
    reply: GraphReplyV1<B>,
    completions: Vec<(CompletionNodeIdV1, RuntimeGeneratedCompletionReceiptV1)>,
    generated_errors: Vec<(
        CompletionNodeIdV1,
        RuntimeGeneratedGraphNodeErrorV1<B::Error>,
    )>,
    observations: Vec<(CompletionNodeIdV1, RuntimeCompletionStatusV1)>,
    errors: Vec<(CompletionNodeIdV1, RuntimeErrorV1<B::Error>)>,
    rejected_observations: u64,
    rejected_releases: u64,
}

impl<B: RuntimeBackendV1> Drop for Graph<B> {
    fn drop(&mut self) {
        if self.request.is_some() && matches!(self.reply, GraphReplyV1::Generated(_)) {
            self.reject_admission(RuntimeGeneratedGraphAdmissionErrorV1::Engine(
                RuntimeAsyncEngineCallErrorV1::EngineStopped,
            ));
        }
        self.release_slot();
    }
}

impl<B: RuntimeBackendV1> Graph<B> {
    fn new(
        request: RuntimeGraphRequestV1<B>,
        generated: BTreeMap<CompletionNodeIdV1, RuntimeAsyncReservedTicketV1>,
        control: RuntimeGraphControlV1,
        slot: Arc<AtomicBool>,
        reply: GraphReplyV1<B>,
    ) -> Self {
        Self {
            versions: None,
            request: Some(request),
            generated,
            authority: None,
            token: None,
            execution: None,
            actions: Vec::new(),
            ids: Vec::new(),
            node_streams: Vec::new(),
            issued: Vec::new(),
            active: VecDeque::new(),
            streams: Vec::new(),
            flush_cursor: 0,
            next_poll: false,
            cancel_applied: false,
            terminal_cleanup_pending: false,
            control,
            slot: Some(slot),
            reply,
            completions: Vec::new(),
            generated_errors: Vec::new(),
            observations: Vec::new(),
            errors: Vec::new(),
            rejected_observations: 0,
            rejected_releases: 0,
        }
    }
    fn release_slot(&mut self) {
        if let Some(slot) = self.slot.take() {
            slot.store(false, Ordering::Release);
        }
    }
    fn prepare(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
    ) -> Result<(), RuntimeGeneratedGraphAdmissionErrorV1<B::Error>> {
        let request = self.request.as_ref().expect("unadmitted graph");
        for (&identity, &stream) in &request.streams {
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
        for node in request.graph.nodes() {
            let action = match (node.kind(), request.actions.get(&node.id())) {
                (CompletionNodeKindV1::Future(_), Some(Action::Launch(launch))) => Some(
                    launch
                        .prepare(context, request.streams[&node.stream()])
                        .map_err(RuntimeGraphErrorV1::Context)?,
                ),
                (CompletionNodeKindV1::Future(_), Some(Action::Copy(source, destination))) => Some(
                    context
                        .prepare_graph_copy_v1(
                            request.streams[&node.stream()],
                            *source,
                            *destination,
                        )
                        .map_err(RuntimeGraphErrorV1::Context)?,
                ),
                (CompletionNodeKindV1::Future(_), None)
                    if self.generated.contains_key(&node.id()) =>
                {
                    let ticket = &self.generated[&node.id()];
                    if !ticket.has_completion_v1() {
                        return Err(RuntimeGeneratedGraphAdmissionErrorV1::Activation(
                            generated_operation::adoption::ActivationErrorV1::Context(
                                crate::RuntimeValidationErrorV1::Unsupported.into(),
                            ),
                        ));
                    }
                    operations
                        .validate_reserved(context, ticket, request.streams[&node.stream()], None)
                        .map_err(RuntimeGeneratedGraphAdmissionErrorV1::Activation)?;
                    None
                }
                (CompletionNodeKindV1::Future(_), None) => {
                    return Err(RuntimeGraphErrorV1::Invalid(
                        RuntimeGraphValidationErrorV1::MissingOperation,
                    )
                    .into());
                }
                _ => None,
            };
            self.actions
                .push(action.map(|action| PreparedGraphActionV1::Ordinary(Box::new(action))));
            self.ids.push(node.id());
            self.node_streams.push(request.streams[&node.stream()]);
            self.issued.push(false);
        }
        request
            .validate_hazards()
            .map_err(RuntimeGraphErrorV1::Invalid)?;
        self.versions =
            Some(versions::VersionLedger::prepare(request).map_err(RuntimeGraphErrorV1::Invalid)?);
        self.streams.extend(request.streams.values().copied());
        let graph_context = request.graph.context();
        let graph_identity = request.graph.identity();
        self.active.reserve(self.ids.len());
        self.observations.reserve(self.ids.len());
        self.errors.reserve(self.ids.len());
        self.completions.reserve(self.generated.len());
        self.generated_errors.reserve(self.generated.len());
        let token = context
            .reserve_graph_v1(self.ids.len())
            .map_err(|e| RuntimeGraphErrorV1::Context(e.into()))?;
        let request = self.request.take().expect("committed request");
        self.authority = Some(request.graph.into_completion_authority());
        for (node, ticket) in core::mem::take(&mut self.generated) {
            let index = self
                .ids
                .binary_search(&node)
                .expect("validated generated node");
            self.actions[index] = Some(PreparedGraphActionV1::Generated(ticket));
        }
        self.token = Some(token);
        self.execution = Some(RuntimeGraphExecutionIdentityV1 {
            context: graph_context,
            graph: graph_identity,
            generation: token.generation(),
        });
        Ok(())
    }

    fn fail_node(&mut self, index: usize, code: u32) {
        self.terminal_cleanup_pending = true;
        self.versions.as_mut().unwrap().fail(index);
        // SAFETY: only this owner issues nodes. The caller established either
        // definite nonpublication or exact quiescent failure for this occurrence.
        unsafe {
            self.authority
                .as_mut()
                .unwrap()
                .mark_failed(self.ids[index], FailureCodeV1::new(code).unwrap())
                .unwrap();
        }
    }

    fn apply_cancel(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
    ) -> bool {
        let cancel = self.control.0.load(Ordering::Acquire);
        if (!cancel || self.cancel_applied) && !self.terminal_cleanup_pending {
            return true;
        }
        let reason = CancellationCodeV1::new(1).unwrap();
        for (index, &node) in self.ids.iter().enumerate() {
            if self.issued[index] {
                continue;
            }
            let state = self.authority.as_ref().unwrap().state(node).unwrap();
            if !cancel && !state.is_terminal() {
                continue;
            }
            // Drop of a ticket cannot reclaim its parked owner. Retire that
            // exact registry entry before recording conclusive cancellation.
            if let Some(PreparedGraphActionV1::Generated(ticket)) = &self.actions[index]
                && !operations.discard_reserved(&ticket.key)
            {
                context.quarantine_after_async_command_panic_v1();
                self.reject(RuntimeGraphErrorV1::Context(
                    crate::RuntimeValidationErrorV1::InvalidBackendDescription.into(),
                ));
                return false;
            }
            let authority = self.authority.as_mut().unwrap();
            match state {
                CompletionNodeStateV1::Blocked => {
                    authority.cancel_blocked(node, reason).unwrap();
                }
                CompletionNodeStateV1::Ready => {
                    authority.request_cancel(node, reason).unwrap();
                    // SAFETY: no token exists and this node can no longer issue.
                    unsafe {
                        authority.mark_cancelled(node).unwrap();
                    }
                }
                _ => {}
            }
            self.actions[index] = None;
        }
        self.cancel_applied |= cancel;
        self.terminal_cleanup_pending = false;
        true
    }

    fn finish(&mut self, context: &mut RuntimeContextV1<B>) -> bool {
        if !self.authority.as_ref().unwrap().is_terminal() || !self.active.is_empty() {
            return false;
        }
        let execution = self.execution.expect("admitted graph occurrence");
        let Some((versions, version_inputs)) = self.versions.take().unwrap().report(execution)
        else {
            context.quarantine_after_async_command_panic_v1();
            self.reject(RuntimeGraphErrorV1::Invalid(
                RuntimeGraphValidationErrorV1::VersionNotAvailable,
            ));
            return true;
        };
        // Terminal graph state permanently closes issue, and every token was
        // retired successfully. No observer can reopen this consumed authority.
        let token = self.token.take().unwrap();
        let result = context
            .close_graph_issue_v1(token)
            .and_then(|()| context.release_graph_v1(token));
        if let Err(error) = result {
            context.quarantine_after_async_command_panic_v1();
            self.reject(RuntimeGraphErrorV1::Context(error.into()));
            return true;
        }
        let completion = self
            .authority
            .take()
            .unwrap()
            .try_into_report()
            .unwrap_or_else(|_| panic!("terminal graph"));
        self.release_slot();
        let report = RuntimeGraphReportV1 {
            execution: self.execution.take().expect("admitted graph occurrence"),
            versions,
            version_inputs,
            completion,
            observations: core::mem::take(&mut self.observations),
            errors: core::mem::take(&mut self.errors),
            rejected_observations: self.rejected_observations,
            rejected_releases: self.rejected_releases,
        };
        match &mut self.reply {
            GraphReplyV1::Ordinary(reply) => reply.complete(Ok(Ok(report))),
            GraphReplyV1::Generated(reply) => {
                reply.complete(Ok(Ok(RuntimeGeneratedGraphReportV1 {
                    graph: report,
                    completions: core::mem::take(&mut self.completions),
                    errors: core::mem::take(&mut self.generated_errors),
                })))
            }
        }
        true
    }

    fn reject(&mut self, error: RuntimeGraphErrorV1<B::Error>) {
        if self.request.is_some() {
            self.reject_admission(RuntimeGeneratedGraphAdmissionErrorV1::Graph(error));
            return;
        }
        self.release_slot();
        match &mut self.reply {
            GraphReplyV1::Ordinary(reply) => reply.complete(Ok(Err(error))),
            GraphReplyV1::Generated(reply) => {
                reply.complete(Ok(Err(RuntimeGeneratedGraphFailureV1::Execution(error))))
            }
        }
    }
}

impl<B: RuntimeAsyncCopyBackendV1 + RuntimeFlushBackendV1 + 'static> Graph<B> {
    fn issue_one(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
    ) -> bool {
        if self.control.0.load(Ordering::Acquire) && !self.apply_cancel(context, operations) {
            return true;
        }
        let Some(node) = self.authority.as_mut().unwrap().pop_ready_notification() else {
            return false;
        };
        if self.authority.as_ref().unwrap().state(node).unwrap() != CompletionNodeStateV1::Ready {
            return true;
        }
        let index = self.ids.binary_search(&node).unwrap();
        assert!(!self.issued[index], "graph occurrence cannot issue twice");
        if !self.versions.as_mut().unwrap().begin(index) {
            context.quarantine_after_async_command_panic_v1();
            self.reject(RuntimeGraphErrorV1::Invalid(
                RuntimeGraphValidationErrorV1::VersionNotAvailable,
            ));
            return true;
        }
        self.issued[index] = true;
        match self.actions[index].take() {
            Some(PreparedGraphActionV1::Generated(ticket)) => {
                let mut ticket = Some(ticket);
                match operations.activate_reserved(
                    context,
                    &mut ticket,
                    self.node_streams[index],
                    self.token,
                ) {
                    Ok(completion) => self
                        .active
                        .push_back(ActiveGraphActionV1::Generated(index, completion)),
                    Err(error) => {
                        if context.is_terminal() {
                            return true;
                        }
                        if !operations
                            .discard_reserved(&ticket.as_ref().expect("rejected ticket").key)
                        {
                            context.quarantine_after_async_command_panic_v1();
                            return true;
                        }
                        self.generated_errors
                            .push((node, RuntimeGeneratedGraphNodeErrorV1::Activation(error)));
                        self.fail_node(index, 3);
                    }
                }
            }
            Some(PreparedGraphActionV1::Ordinary(action)) => {
                match context.submit_graph_action_v1(self.token.unwrap(), *action) {
                    Ok(submission) => self
                        .active
                        .push_back(ActiveGraphActionV1::Ordinary(index, submission, None)),
                    Err(error) => {
                        if context.is_terminal() {
                            self.reject(RuntimeGraphErrorV1::Context(error));
                        } else {
                            self.errors.push((node, error));
                            self.fail_node(index, 1);
                        }
                    }
                }
            }
            None => {
                // SAFETY: this is a validated host-only event join; Ready means
                // all exact predecessors succeeded, and the context is reserved.
                unsafe {
                    self.authority
                        .as_mut()
                        .unwrap()
                        .mark_succeeded(node)
                        .unwrap();
                }
            }
        }
        true
    }

    fn poll_one(&mut self, context: &mut RuntimeContextV1<B>) -> bool {
        let Some(active) = self.active.pop_front() else {
            return false;
        };
        let (index, mut submission, retiring) = match active {
            ActiveGraphActionV1::Ordinary(index, submission, retiring) => {
                (index, submission, retiring)
            }
            ActiveGraphActionV1::Generated(index, completion) => {
                self.observe_generated(context, index, completion);
                return true;
            }
        };
        let token = self.token.unwrap();
        if let Some(status) = retiring {
            match context.release_graph_submission_v1(token, &submission) {
                Ok(()) => {
                    if status == RuntimeCompletionStatusV1::Succeeded {
                        self.succeed_operation(context, index);
                    } else {
                        self.fail_node(index, 2);
                    }
                }
                Err(error) if context.is_terminal() => {
                    self.reject(RuntimeGraphErrorV1::Context(error))
                }
                Err(_) => {
                    self.rejected_releases = self.rejected_releases.saturating_add(1);
                    self.active.push_back(ActiveGraphActionV1::Ordinary(
                        index,
                        submission,
                        Some(status),
                    ));
                }
            }
            return true;
        }
        match context.poll_with_graph_access_v1(&mut submission, Some(token)) {
            Ok(_) => {}
            Err(RuntimeErrorV1::BackendRejected(_)) => {
                self.rejected_observations = self.rejected_observations.saturating_add(1);
                self.active
                    .push_back(ActiveGraphActionV1::Ordinary(index, submission, None));
                return true;
            }
            Err(error) if context.is_terminal() => {
                self.reject(RuntimeGraphErrorV1::Context(error));
                return true;
            }
            Err(error) => {
                if !matches!(error, RuntimeErrorV1::BackendQuiescent(_)) {
                    context.quarantine_after_async_command_panic_v1();
                    self.reject(RuntimeGraphErrorV1::Context(error));
                    return true;
                }
                self.errors.push((self.ids[index], error));
            }
        }
        let status = context
            .query_submission(&submission)
            .expect("exact owned submission");
        if status == RuntimeCompletionStatusV1::Pending {
            self.active
                .push_back(ActiveGraphActionV1::Ordinary(index, submission, None));
        } else {
            self.observations.push((self.ids[index], status));
            self.active.push_back(ActiveGraphActionV1::Ordinary(
                index,
                submission,
                Some(status),
            ));
        }
        true
    }

    fn observe_generated(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        index: usize,
        mut completion: RuntimeAsyncGeneratedCompletionV1,
    ) {
        match completion.try_take_ready_v1() {
            None => self
                .active
                .push_back(ActiveGraphActionV1::Generated(index, completion)),
            Some(Ok(Ok(receipt))) => {
                self.completions.push((self.ids[index], receipt));
                self.succeed_operation(context, index);
            }
            Some(result) => {
                let error = match result {
                    Err(error) => RuntimeGeneratedGraphNodeErrorV1::Engine(error),
                    Ok(Err(error)) => RuntimeGeneratedGraphNodeErrorV1::Readback(error),
                    Ok(Ok(_)) => unreachable!(),
                };
                self.generated_errors.push((self.ids[index], error));
                self.fail_node(index, 4);
            }
        }
    }

    fn succeed_operation(&mut self, context: &mut RuntimeContextV1<B>, index: usize) {
        if !self.versions.as_mut().unwrap().commit(index) {
            context.quarantine_after_async_command_panic_v1();
            self.reject(RuntimeGraphErrorV1::Invalid(
                RuntimeGraphValidationErrorV1::VersionNotAvailable,
            ));
            return;
        }
        // SAFETY: the exact ordinary submission was successfully retired, or
        // the original generated decoder/gate receipt followed full settlement.
        unsafe {
            self.authority
                .as_mut()
                .unwrap()
                .mark_succeeded(self.ids[index])
                .unwrap();
        }
    }
}

impl<B: RuntimeAsyncCopyBackendV1 + RuntimeFlushBackendV1 + 'static> EngineGraphV1<B> for Graph<B> {
    fn admit(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
    ) -> bool {
        match self.prepare(context, operations) {
            Ok(()) => true,
            Err(error) => {
                self.reject_admission(error);
                false
            }
        }
    }
    fn advance(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
        budget: usize,
        flush_budget: usize,
    ) -> bool {
        if !self.apply_cancel(context, operations) {
            return true;
        }
        for _ in 0..budget {
            self.next_poll = !self.next_poll;
            let mut progressed = false;
            for poll in [self.next_poll, !self.next_poll] {
                progressed = if poll {
                    self.poll_one(context)
                } else {
                    self.issue_one(context, operations)
                };
                if progressed {
                    break;
                }
            }
            if context.is_terminal() {
                return true;
            }
            if !progressed {
                break;
            }
        }
        if !self.apply_cancel(context, operations) || self.finish(context) {
            return true;
        }
        for _ in 0..flush_budget.min(self.streams.len()) {
            let stream = self.streams[self.flush_cursor];
            self.flush_cursor = (self.flush_cursor + 1) % self.streams.len();
            if let Err(error) = context.flush_with_graph_access_v1(stream, self.token)
                && context.is_terminal()
            {
                self.reject(RuntimeGraphErrorV1::Context(error));
                return true;
            }
        }
        false
    }
    fn reject(&mut self, error: RuntimeGraphErrorV1<B::Error>) {
        Graph::reject(self, error);
    }
    fn stop(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
    ) {
        if context.is_terminal() {
            return;
        }
        self.control.cancel_unissued();
        if !self.apply_cancel(context, operations) {
            return;
        }
        // The last operation phase may have published a settled result before
        // Stop was dequeued. Consume only those original cells, without native
        // polling, decoding, ordinary retirement or issuing any successor.
        for _ in 0..self.active.len() {
            match self.active.pop_front().expect("bounded active roster") {
                ActiveGraphActionV1::Generated(index, completion) => {
                    self.observe_generated(context, index, completion);
                }
                ordinary => self.active.push_back(ordinary),
            }
            if context.is_terminal() {
                return;
            }
        }
        if !self.apply_cancel(context, operations) {
            return;
        }
        if !self.finish(context) {
            // Stop is not native quiescence. Preserve the context reservation
            // even after the driver/reply is dropped by owner shutdown.
            context.quarantine_after_async_command_panic_v1();
        }
    }
    fn take_generated_request(&mut self) -> Option<RuntimeGeneratedGraphRequestV1<B>> {
        Graph::take_generated_request(self)
    }
}

impl<B: RuntimeAsyncCopyBackendV1 + RuntimeFlushBackendV1 + 'static>
    RuntimeAsyncProgressHandleV1<B>
{
    /// Enqueues one exclusive graph. Only one queued/active graph per engine is
    /// retained. Admission rejects preexisting operations, submissions or events.
    /// A terminal engine outcome is not a graph completion or cleanup receipt.
    pub fn submit_graph(
        &self,
        request: RuntimeGraphRequestV1<B>,
    ) -> Result<RuntimeAsyncGraphFutureV1<B::Error>, RuntimeAsyncEngineCallErrorV1> {
        if self.observer.is_worker_thread() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let (reply, future) = owned::Reply::budgeted_pair(&self.observer.reply_budget)?;
        if self
            .observer
            .graph_slot
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(RuntimeAsyncEngineCallErrorV1::GraphCapacity);
        }
        let control = RuntimeGraphControlV1(Arc::new(AtomicBool::new(false)));
        let graph = Graph::new(
            request,
            BTreeMap::new(),
            control.clone(),
            Arc::clone(&self.observer.graph_slot),
            GraphReplyV1::Ordinary(reply),
        );
        match self
            .observer
            .try_send_command(RuntimeAsyncEngineCommandV1::Graph(Box::new(graph)))
        {
            Ok(()) => Ok(RuntimeAsyncGraphFutureV1 { future, control }),
            Err(TrySendError::Full(_)) => Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull),
            Err(TrySendError::Disconnected(_)) => Err(RuntimeAsyncEngineCallErrorV1::EngineStopped),
        }
    }
}
