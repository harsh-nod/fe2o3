use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::wire::CompletionGraphIdentityV1;

/// Size of each opaque backend identity used by completion graph V1.
pub const COMPLETION_IDENTITY_BYTES_V1: usize = 32;
/// Maximum number of streams represented by one completion graph.
pub const MAX_COMPLETION_GRAPH_STREAMS_V1: usize = 4_096;
/// Maximum number of nodes represented by one completion graph.
pub const MAX_COMPLETION_GRAPH_NODES_V1: usize = 65_536;
/// Maximum number of dependency edges represented by one completion graph.
pub const MAX_COMPLETION_GRAPH_EDGES_V1: usize = 131_072;

macro_rules! opaque_identity {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name([u8; COMPLETION_IDENTITY_BYTES_V1]);

        impl $name {
            /// Wraps an exact, generation-scoped identity supplied by a runtime adapter.
            ///
            /// This validates no backend handle and grants no runtime authority.
            pub const fn from_bytes(bytes: [u8; COMPLETION_IDENTITY_BYTES_V1]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(self) -> [u8; COMPLETION_IDENTITY_BYTES_V1] {
                self.0
            }
        }
    };
}

opaque_identity!(
    DeviceIdentityV1,
    "Exact generation-scoped identity of one physical or logical device."
);

/// Exact identity of one context on one device.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContextIdentityV1 {
    device: DeviceIdentityV1,
    local: [u8; COMPLETION_IDENTITY_BYTES_V1],
}

impl ContextIdentityV1 {
    pub const fn new(device: DeviceIdentityV1, local: [u8; COMPLETION_IDENTITY_BYTES_V1]) -> Self {
        Self { device, local }
    }

    pub const fn device(self) -> DeviceIdentityV1 {
        self.device
    }

    pub const fn local_bytes(self) -> [u8; COMPLETION_IDENTITY_BYTES_V1] {
        self.local
    }
}

/// Exact identity of one stream in one context.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamIdentityV1 {
    context: ContextIdentityV1,
    local: [u8; COMPLETION_IDENTITY_BYTES_V1],
}

impl StreamIdentityV1 {
    pub const fn new(
        context: ContextIdentityV1,
        local: [u8; COMPLETION_IDENTITY_BYTES_V1],
    ) -> Self {
        Self { context, local }
    }

    pub const fn context(self) -> ContextIdentityV1 {
        self.context
    }

    pub const fn device(self) -> DeviceIdentityV1 {
        self.context.device()
    }

    pub const fn local_bytes(self) -> [u8; COMPLETION_IDENTITY_BYTES_V1] {
        self.local
    }
}

/// Exact identity of one event in one context.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EventIdentityV1 {
    context: ContextIdentityV1,
    local: [u8; COMPLETION_IDENTITY_BYTES_V1],
}

impl EventIdentityV1 {
    pub const fn new(
        context: ContextIdentityV1,
        local: [u8; COMPLETION_IDENTITY_BYTES_V1],
    ) -> Self {
        Self { context, local }
    }

    pub const fn context(self) -> ContextIdentityV1 {
        self.context
    }

    pub const fn device(self) -> DeviceIdentityV1 {
        self.context.device()
    }

    pub const fn local_bytes(self) -> [u8; COMPLETION_IDENTITY_BYTES_V1] {
        self.local
    }
}

/// Exact identity of one future submitted to one stream.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FutureIdentityV1 {
    stream: StreamIdentityV1,
    local: [u8; COMPLETION_IDENTITY_BYTES_V1],
}

impl FutureIdentityV1 {
    pub const fn new(stream: StreamIdentityV1, local: [u8; COMPLETION_IDENTITY_BYTES_V1]) -> Self {
        Self { stream, local }
    }

    pub const fn stream(self) -> StreamIdentityV1 {
        self.stream
    }

    pub const fn context(self) -> ContextIdentityV1 {
        self.stream.context()
    }

    pub const fn device(self) -> DeviceIdentityV1 {
        self.stream.device()
    }

    pub const fn local_bytes(self) -> [u8; COMPLETION_IDENTITY_BYTES_V1] {
        self.local
    }
}

/// Nonzero, graph-local node identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct CompletionNodeIdV1(u32);

impl CompletionNodeIdV1 {
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One operation represented by a completion graph node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompletionNodeKindV1 {
    /// An asynchronously completed operation represented as a future.
    Future(FutureIdentityV1),
    /// Recording an event after all earlier work on this stream.
    EventRecord {
        stream: StreamIdentityV1,
        event: EventIdentityV1,
    },
    /// Waiting on the exact node that recorded an event.
    EventWait {
        stream: StreamIdentityV1,
        event: EventIdentityV1,
        recorded_by: CompletionNodeIdV1,
    },
}

impl CompletionNodeKindV1 {
    pub const fn stream(&self) -> StreamIdentityV1 {
        match self {
            Self::Future(future) => future.stream(),
            Self::EventRecord { stream, .. } | Self::EventWait { stream, .. } => *stream,
        }
    }
}

/// One operation and its immediate predecessor in the same stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionNodeV1 {
    id: CompletionNodeIdV1,
    stream_predecessor: Option<CompletionNodeIdV1>,
    kind: CompletionNodeKindV1,
}

impl CompletionNodeV1 {
    pub const fn future(
        id: CompletionNodeIdV1,
        future: FutureIdentityV1,
        stream_predecessor: Option<CompletionNodeIdV1>,
    ) -> Self {
        Self {
            id,
            stream_predecessor,
            kind: CompletionNodeKindV1::Future(future),
        }
    }

    pub const fn record_event(
        id: CompletionNodeIdV1,
        stream: StreamIdentityV1,
        event: EventIdentityV1,
        stream_predecessor: Option<CompletionNodeIdV1>,
    ) -> Self {
        Self {
            id,
            stream_predecessor,
            kind: CompletionNodeKindV1::EventRecord { stream, event },
        }
    }

    pub const fn wait_event(
        id: CompletionNodeIdV1,
        stream: StreamIdentityV1,
        event: EventIdentityV1,
        recorded_by: CompletionNodeIdV1,
        stream_predecessor: Option<CompletionNodeIdV1>,
    ) -> Self {
        Self {
            id,
            stream_predecessor,
            kind: CompletionNodeKindV1::EventWait {
                stream,
                event,
                recorded_by,
            },
        }
    }

    pub const fn id(&self) -> CompletionNodeIdV1 {
        self.id
    }

    pub const fn stream(&self) -> StreamIdentityV1 {
        self.kind.stream()
    }

    pub const fn stream_predecessor(&self) -> Option<CompletionNodeIdV1> {
        self.stream_predecessor
    }

    pub const fn kind(&self) -> &CompletionNodeKindV1 {
        &self.kind
    }
}

/// A validated, runtime-independent stream/event/future dependency graph.
///
/// Nodes on each declared stream must form exactly one linear chain. Cross-stream
/// dependencies can only be introduced by an event wait naming the exact event
/// record node. Validation grants no hardware execution or completion authority.
///
/// The graph is intentionally linear because converting it into a
/// [`CompletionAuthorityV1`] consumes it.
///
/// ```compile_fail
/// use fe2o3_completion::CompletionGraphV1;
/// fn duplicate(graph: CompletionGraphV1) {
///     let _second = graph.clone();
/// }
/// ```
#[derive(Debug)]
#[must_use = "a validated graph must be retained or converted into completion authority"]
pub struct CompletionGraphV1 {
    pub(crate) context: ContextIdentityV1,
    pub(crate) streams: Vec<StreamIdentityV1>,
    pub(crate) nodes: Vec<CompletionNodeV1>,
    pub(crate) topological_order: Vec<CompletionNodeIdV1>,
}

impl CompletionGraphV1 {
    pub fn new(
        context: ContextIdentityV1,
        mut streams: Vec<StreamIdentityV1>,
        mut nodes: Vec<CompletionNodeV1>,
    ) -> Result<Self, CompletionGraphErrorV1> {
        if streams.is_empty() {
            return Err(CompletionGraphErrorV1::EmptyStreams);
        }
        if streams.len() > MAX_COMPLETION_GRAPH_STREAMS_V1 {
            return Err(CompletionGraphErrorV1::TooManyStreams {
                actual: streams.len(),
                maximum: MAX_COMPLETION_GRAPH_STREAMS_V1,
            });
        }
        if nodes.is_empty() {
            return Err(CompletionGraphErrorV1::EmptyNodes);
        }
        if nodes.len() > MAX_COMPLETION_GRAPH_NODES_V1 {
            return Err(CompletionGraphErrorV1::TooManyNodes {
                actual: nodes.len(),
                maximum: MAX_COMPLETION_GRAPH_NODES_V1,
            });
        }

        streams.sort_unstable();
        if let Some(stream) = adjacent_duplicate(&streams) {
            return Err(CompletionGraphErrorV1::DuplicateStream(*stream));
        }
        for stream in &streams {
            if stream.context() != context {
                return Err(CompletionGraphErrorV1::ForeignStreamContext(*stream));
            }
        }

        nodes.sort_unstable_by_key(CompletionNodeV1::id);
        if let Some(nodes) = nodes.windows(2).find(|pair| pair[0].id() == pair[1].id()) {
            return Err(CompletionGraphErrorV1::DuplicateNode(nodes[0].id()));
        }

        let node_indices: BTreeMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id(), index))
            .collect();
        let declared_streams: BTreeSet<_> = streams.iter().copied().collect();
        let mut stream_node_counts = BTreeMap::<_, usize>::new();
        let mut stream_head_counts = BTreeMap::<_, usize>::new();
        let mut stream_successors = BTreeMap::<CompletionNodeIdV1, CompletionNodeIdV1>::new();
        let mut event_records = BTreeMap::<EventIdentityV1, CompletionNodeIdV1>::new();
        let mut futures = BTreeMap::<FutureIdentityV1, CompletionNodeIdV1>::new();

        for node in &nodes {
            let stream = node.stream();
            if stream.context() != context {
                return Err(CompletionGraphErrorV1::ForeignStreamContext(stream));
            }
            if !declared_streams.contains(&stream) {
                return Err(CompletionGraphErrorV1::UndeclaredStream(stream));
            }
            *stream_node_counts.entry(stream).or_default() += 1;

            match node.kind() {
                CompletionNodeKindV1::Future(future) => {
                    if let Some(first) = futures.insert(*future, node.id()) {
                        return Err(CompletionGraphErrorV1::DuplicateFuture {
                            first,
                            duplicate: node.id(),
                        });
                    }
                }
                CompletionNodeKindV1::EventRecord { event, .. }
                | CompletionNodeKindV1::EventWait { event, .. } => {
                    if event.context() != context {
                        return Err(CompletionGraphErrorV1::ForeignEventContext(*event));
                    }
                    if matches!(node.kind(), CompletionNodeKindV1::EventRecord { .. })
                        && let Some(first) = event_records.insert(*event, node.id())
                    {
                        return Err(CompletionGraphErrorV1::DuplicateEventRecord {
                            event: *event,
                            first,
                            duplicate: node.id(),
                        });
                    }
                }
            }

            match node.stream_predecessor() {
                None => *stream_head_counts.entry(stream).or_default() += 1,
                Some(predecessor) => {
                    let Some(&predecessor_index) = node_indices.get(&predecessor) else {
                        return Err(CompletionGraphErrorV1::MissingNode(predecessor));
                    };
                    if nodes[predecessor_index].stream() != stream {
                        return Err(CompletionGraphErrorV1::CrossStreamPredecessor {
                            node: node.id(),
                            predecessor,
                        });
                    }
                    if let Some(first) = stream_successors.insert(predecessor, node.id()) {
                        return Err(CompletionGraphErrorV1::StreamFork {
                            predecessor,
                            first,
                            second: node.id(),
                        });
                    }
                }
            }
        }

        for stream in &streams {
            if !stream_node_counts.contains_key(stream) {
                return Err(CompletionGraphErrorV1::UnusedStream(*stream));
            }
            let heads = stream_head_counts.get(stream).copied().unwrap_or(0);
            if heads != 1 {
                return Err(CompletionGraphErrorV1::InvalidStreamHeadCount {
                    stream: *stream,
                    actual: heads,
                });
            }
        }

        for node in &nodes {
            if let CompletionNodeKindV1::EventWait {
                event, recorded_by, ..
            } = node.kind()
            {
                let Some(&record_index) = node_indices.get(recorded_by) else {
                    return Err(CompletionGraphErrorV1::MissingNode(*recorded_by));
                };
                match nodes[record_index].kind() {
                    CompletionNodeKindV1::EventRecord {
                        event: recorded_event,
                        ..
                    } if recorded_event == event => {}
                    _ => {
                        return Err(CompletionGraphErrorV1::EventRecordMismatch {
                            wait: node.id(),
                            recorded_by: *recorded_by,
                        });
                    }
                }
            }
        }

        let dependencies = dependencies(&nodes);
        let edge_count: usize = dependencies.iter().map(Vec::len).sum();
        if edge_count > MAX_COMPLETION_GRAPH_EDGES_V1 {
            return Err(CompletionGraphErrorV1::TooManyEdges {
                actual: edge_count,
                maximum: MAX_COMPLETION_GRAPH_EDGES_V1,
            });
        }
        let topological_order = deterministic_topological_order(&nodes, &dependencies)?;

        Ok(Self {
            context,
            streams,
            nodes,
            topological_order,
        })
    }

    pub const fn context(&self) -> ContextIdentityV1 {
        self.context
    }

    pub const fn device(&self) -> DeviceIdentityV1 {
        self.context.device()
    }

    pub fn streams(&self) -> &[StreamIdentityV1] {
        &self.streams
    }

    pub fn nodes(&self) -> &[CompletionNodeV1] {
        &self.nodes
    }

    pub fn topological_order(&self) -> &[CompletionNodeIdV1] {
        &self.topological_order
    }

    pub const fn authenticates_backend_identity(&self) -> bool {
        false
    }

    pub const fn grants_hardware_execution_authority(&self) -> bool {
        false
    }

    pub fn into_completion_authority(self) -> CompletionAuthorityV1 {
        CompletionAuthorityV1::new(self)
    }

    fn node_index(&self, id: CompletionNodeIdV1) -> Option<usize> {
        self.nodes
            .binary_search_by_key(&id, CompletionNodeV1::id)
            .ok()
    }
}

/// Application-defined nonzero cancellation reason.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct CancellationCodeV1(u32);

impl CancellationCodeV1 {
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Application-defined nonzero asynchronous failure code.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct FailureCodeV1(u32);

impl FailureCodeV1 {
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// State of one node under the graph's linear completion authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionNodeStateV1 {
    Blocked,
    Ready,
    CancelRequested(CancellationCodeV1),
    Succeeded,
    Failed {
        origin: CompletionNodeIdV1,
        error: FailureCodeV1,
    },
    Cancelled {
        origin: CompletionNodeIdV1,
        reason: CancellationCodeV1,
    },
    DependencyFailed {
        origin: CompletionNodeIdV1,
        error: FailureCodeV1,
    },
    DependencyCancelled {
        origin: CompletionNodeIdV1,
        reason: CancellationCodeV1,
    },
}

impl CompletionNodeStateV1 {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded
                | Self::Failed { .. }
                | Self::Cancelled { .. }
                | Self::DependencyFailed { .. }
                | Self::DependencyCancelled { .. }
        )
    }
}

/// Exclusive state-transition capability for one exact validated graph.
///
/// This type is not `Clone` or `Copy`. It only controls this in-memory model;
/// it does not authenticate observations and grants no backend execution,
/// event, stream, or resource-reclamation authority.
///
/// ```compile_fail
/// use fe2o3_completion::CompletionAuthorityV1;
/// fn duplicate(authority: CompletionAuthorityV1) {
///     let _second = authority.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_completion::{CompletionAuthorityV1, CompletionNodeIdV1};
/// fn use_after_finish(authority: CompletionAuthorityV1) {
///     let node = CompletionNodeIdV1::new(1).unwrap();
///     let _ = authority.try_into_report();
///     let _ = authority.state(node);
/// }
/// ```
#[derive(Debug)]
#[must_use = "completion authority must be driven to terminal outcomes or retained"]
pub struct CompletionAuthorityV1 {
    graph: CompletionGraphV1,
    states: Vec<CompletionNodeStateV1>,
}

impl CompletionAuthorityV1 {
    fn new(graph: CompletionGraphV1) -> Self {
        let dependencies = dependencies(&graph.nodes);
        let states = dependencies
            .iter()
            .map(|dependencies| {
                if dependencies.is_empty() {
                    CompletionNodeStateV1::Ready
                } else {
                    CompletionNodeStateV1::Blocked
                }
            })
            .collect();
        Self { graph, states }
    }

    pub const fn graph(&self) -> &CompletionGraphV1 {
        &self.graph
    }

    pub fn graph_identity(&self) -> CompletionGraphIdentityV1 {
        self.graph.identity()
    }

    pub fn state(
        &self,
        node: CompletionNodeIdV1,
    ) -> Result<CompletionNodeStateV1, CompletionTransitionErrorV1> {
        self.graph
            .node_index(node)
            .map(|index| self.states[index])
            .ok_or(CompletionTransitionErrorV1::UnknownNode(node))
    }

    /// Records a cancellation request for a node that is ready for submission.
    ///
    /// Cancellation remains nonterminal until [`Self::mark_cancelled`] receives
    /// an independently authenticated backend observation. Repeating the exact
    /// request is idempotent; substituting a reason fails closed.
    pub fn request_cancel(
        &mut self,
        node: CompletionNodeIdV1,
        reason: CancellationCodeV1,
    ) -> Result<bool, CompletionTransitionErrorV1> {
        let index = self.require_node(node)?;
        match self.states[index] {
            CompletionNodeStateV1::Ready => {
                self.states[index] = CompletionNodeStateV1::CancelRequested(reason);
                Ok(true)
            }
            CompletionNodeStateV1::CancelRequested(existing) if existing == reason => Ok(false),
            CompletionNodeStateV1::CancelRequested(existing) => Err(
                CompletionTransitionErrorV1::CancellationReasonSubstitution {
                    node,
                    expected: existing,
                    actual: reason,
                },
            ),
            state if state.is_terminal() => {
                Err(CompletionTransitionErrorV1::Terminal { node, state })
            }
            state => Err(CompletionTransitionErrorV1::NotReady { node, state }),
        }
    }

    /// Cancels a node that cannot have been submitted because it is still blocked.
    ///
    /// Its descendants deterministically inherit the cancellation cause.
    pub fn cancel_blocked(
        &mut self,
        node: CompletionNodeIdV1,
        reason: CancellationCodeV1,
    ) -> Result<(), CompletionTransitionErrorV1> {
        let index = self.require_node(node)?;
        match self.states[index] {
            CompletionNodeStateV1::Blocked => {
                self.states[index] = CompletionNodeStateV1::Cancelled {
                    origin: node,
                    reason,
                };
                self.propagate_terminal_causes();
                Ok(())
            }
            state if state.is_terminal() => {
                Err(CompletionTransitionErrorV1::Terminal { node, state })
            }
            state => Err(CompletionTransitionErrorV1::NotBlocked { node, state }),
        }
    }

    /// Records successful completion of the exact node and unblocks successors.
    ///
    /// A cancellation request may lose its race with successful completion.
    ///
    /// # Safety
    ///
    /// The caller must have authenticated a quiescent successful completion for
    /// this exact graph, device, context, stream, and node identity.
    pub unsafe fn mark_succeeded(
        &mut self,
        node: CompletionNodeIdV1,
    ) -> Result<(), CompletionTransitionErrorV1> {
        let index = self.require_node(node)?;
        match self.states[index] {
            CompletionNodeStateV1::Ready | CompletionNodeStateV1::CancelRequested(_) => {
                self.states[index] = CompletionNodeStateV1::Succeeded;
                self.refresh_blocked_nodes();
                Ok(())
            }
            state if state.is_terminal() => {
                Err(CompletionTransitionErrorV1::Terminal { node, state })
            }
            state => Err(CompletionTransitionErrorV1::NotReady { node, state }),
        }
    }

    /// Records a quiescent failure and propagates its exact origin and code.
    ///
    /// # Safety
    ///
    /// The caller must have authenticated a quiescent failure for this exact
    /// graph, device, context, stream, and node identity.
    pub unsafe fn mark_failed(
        &mut self,
        node: CompletionNodeIdV1,
        error: FailureCodeV1,
    ) -> Result<(), CompletionTransitionErrorV1> {
        let index = self.require_node(node)?;
        match self.states[index] {
            CompletionNodeStateV1::Ready | CompletionNodeStateV1::CancelRequested(_) => {
                self.states[index] = CompletionNodeStateV1::Failed {
                    origin: node,
                    error,
                };
                self.propagate_terminal_causes();
                Ok(())
            }
            state if state.is_terminal() => {
                Err(CompletionTransitionErrorV1::Terminal { node, state })
            }
            state => Err(CompletionTransitionErrorV1::NotReady { node, state }),
        }
    }

    /// Confirms that a requested cancellation reached quiescence.
    ///
    /// # Safety
    ///
    /// The caller must have authenticated cancellation and quiescence for this
    /// exact graph, device, context, stream, and node identity.
    pub unsafe fn mark_cancelled(
        &mut self,
        node: CompletionNodeIdV1,
    ) -> Result<(), CompletionTransitionErrorV1> {
        let index = self.require_node(node)?;
        match self.states[index] {
            CompletionNodeStateV1::CancelRequested(reason) => {
                self.states[index] = CompletionNodeStateV1::Cancelled {
                    origin: node,
                    reason,
                };
                self.propagate_terminal_causes();
                Ok(())
            }
            state if state.is_terminal() => {
                Err(CompletionTransitionErrorV1::Terminal { node, state })
            }
            state => Err(CompletionTransitionErrorV1::CancellationNotRequested { node, state }),
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.states.iter().all(|state| state.is_terminal())
    }

    /// Converts a fully terminal authority into a descriptive report.
    ///
    /// Incomplete authority is returned unchanged so linear state is not lost.
    pub fn try_into_report(self) -> Result<CompletionReportV1, Box<Self>> {
        if !self.is_terminal() {
            return Err(Box::new(self));
        }
        let graph_identity = self.graph.identity();
        let entries = self
            .graph
            .nodes
            .iter()
            .zip(self.states)
            .map(|(node, state)| CompletionReportEntryV1 {
                node: node.id(),
                state,
            })
            .collect();
        Ok(CompletionReportV1 {
            graph_identity,
            entries,
        })
    }

    pub const fn authenticates_backend_observations(&self) -> bool {
        false
    }

    pub const fn grants_hardware_execution_authority(&self) -> bool {
        false
    }

    fn require_node(&self, node: CompletionNodeIdV1) -> Result<usize, CompletionTransitionErrorV1> {
        self.graph
            .node_index(node)
            .ok_or(CompletionTransitionErrorV1::UnknownNode(node))
    }

    fn refresh_blocked_nodes(&mut self) {
        let dependencies = dependencies(&self.graph.nodes);
        for node in &self.graph.topological_order {
            let index = self
                .graph
                .node_index(*node)
                .expect("validated topological node exists");
            if self.states[index] == CompletionNodeStateV1::Blocked
                && dependencies[index].iter().all(|dependency| {
                    self.state_unchecked(*dependency) == CompletionNodeStateV1::Succeeded
                })
            {
                self.states[index] = CompletionNodeStateV1::Ready;
            }
        }
    }

    fn propagate_terminal_causes(&mut self) {
        let dependencies = dependencies(&self.graph.nodes);
        for node in &self.graph.topological_order {
            let index = self
                .graph
                .node_index(*node)
                .expect("validated topological node exists");
            if self.states[index] != CompletionNodeStateV1::Blocked {
                continue;
            }
            let cause = dependencies[index]
                .iter()
                .find_map(|dependency| dependency_cause(self.state_unchecked(*dependency)));
            if let Some(cause) = cause {
                self.states[index] = cause;
            }
        }
    }

    fn state_unchecked(&self, node: CompletionNodeIdV1) -> CompletionNodeStateV1 {
        self.states[self
            .graph
            .node_index(node)
            .expect("validated dependency node exists")]
    }
}

fn dependency_cause(state: CompletionNodeStateV1) -> Option<CompletionNodeStateV1> {
    match state {
        CompletionNodeStateV1::Failed { origin, error }
        | CompletionNodeStateV1::DependencyFailed { origin, error } => {
            Some(CompletionNodeStateV1::DependencyFailed { origin, error })
        }
        CompletionNodeStateV1::Cancelled { origin, reason }
        | CompletionNodeStateV1::DependencyCancelled { origin, reason } => {
            Some(CompletionNodeStateV1::DependencyCancelled { origin, reason })
        }
        _ => None,
    }
}

/// One terminal graph report entry in ascending node order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionReportEntryV1 {
    node: CompletionNodeIdV1,
    state: CompletionNodeStateV1,
}

impl CompletionReportEntryV1 {
    pub const fn node(self) -> CompletionNodeIdV1 {
        self.node
    }

    pub const fn state(self) -> CompletionNodeStateV1 {
        self.state
    }
}

/// Descriptive terminal outcome of every node in one consumed authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionReportV1 {
    graph_identity: CompletionGraphIdentityV1,
    entries: Vec<CompletionReportEntryV1>,
}

impl CompletionReportV1 {
    pub const fn graph_identity(&self) -> CompletionGraphIdentityV1 {
        self.graph_identity
    }

    pub fn entries(&self) -> &[CompletionReportEntryV1] {
        &self.entries
    }

    pub const fn authenticates_backend_observations(&self) -> bool {
        false
    }

    pub const fn grants_resource_reclamation_authority(&self) -> bool {
        false
    }
}

/// Failure to apply a completion transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompletionTransitionErrorV1 {
    UnknownNode(CompletionNodeIdV1),
    NotReady {
        node: CompletionNodeIdV1,
        state: CompletionNodeStateV1,
    },
    NotBlocked {
        node: CompletionNodeIdV1,
        state: CompletionNodeStateV1,
    },
    CancellationNotRequested {
        node: CompletionNodeIdV1,
        state: CompletionNodeStateV1,
    },
    CancellationReasonSubstitution {
        node: CompletionNodeIdV1,
        expected: CancellationCodeV1,
        actual: CancellationCodeV1,
    },
    Terminal {
        node: CompletionNodeIdV1,
        state: CompletionNodeStateV1,
    },
}

impl fmt::Display for CompletionTransitionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode(node) => write!(formatter, "unknown completion node {}", node.get()),
            Self::NotReady { node, state } => write!(
                formatter,
                "completion node {} is not ready: {state:?}",
                node.get()
            ),
            Self::NotBlocked { node, state } => write!(
                formatter,
                "completion node {} is not blocked: {state:?}",
                node.get()
            ),
            Self::CancellationNotRequested { node, state } => write!(
                formatter,
                "completion node {} has no pending cancellation: {state:?}",
                node.get()
            ),
            Self::CancellationReasonSubstitution {
                node,
                expected,
                actual,
            } => write!(
                formatter,
                "completion node {} cancellation reason changed from {} to {}",
                node.get(),
                expected.get(),
                actual.get()
            ),
            Self::Terminal { node, state } => write!(
                formatter,
                "completion node {} is already terminal: {state:?}",
                node.get()
            ),
        }
    }
}

impl Error for CompletionTransitionErrorV1 {}

/// Structural reason a completion graph was rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompletionGraphErrorV1 {
    EmptyStreams,
    EmptyNodes,
    TooManyStreams {
        actual: usize,
        maximum: usize,
    },
    TooManyNodes {
        actual: usize,
        maximum: usize,
    },
    TooManyEdges {
        actual: usize,
        maximum: usize,
    },
    DuplicateStream(StreamIdentityV1),
    ForeignStreamContext(StreamIdentityV1),
    ForeignEventContext(EventIdentityV1),
    UndeclaredStream(StreamIdentityV1),
    UnusedStream(StreamIdentityV1),
    DuplicateNode(CompletionNodeIdV1),
    MissingNode(CompletionNodeIdV1),
    DuplicateFuture {
        first: CompletionNodeIdV1,
        duplicate: CompletionNodeIdV1,
    },
    DuplicateEventRecord {
        event: EventIdentityV1,
        first: CompletionNodeIdV1,
        duplicate: CompletionNodeIdV1,
    },
    CrossStreamPredecessor {
        node: CompletionNodeIdV1,
        predecessor: CompletionNodeIdV1,
    },
    StreamFork {
        predecessor: CompletionNodeIdV1,
        first: CompletionNodeIdV1,
        second: CompletionNodeIdV1,
    },
    InvalidStreamHeadCount {
        stream: StreamIdentityV1,
        actual: usize,
    },
    EventRecordMismatch {
        wait: CompletionNodeIdV1,
        recorded_by: CompletionNodeIdV1,
    },
    Cycle,
}

impl fmt::Display for CompletionGraphErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyStreams => formatter.write_str("completion graph has no streams"),
            Self::EmptyNodes => formatter.write_str("completion graph has no nodes"),
            Self::TooManyStreams { actual, maximum } => write!(
                formatter,
                "completion graph has {actual} streams, exceeding {maximum}"
            ),
            Self::TooManyNodes { actual, maximum } => write!(
                formatter,
                "completion graph has {actual} nodes, exceeding {maximum}"
            ),
            Self::TooManyEdges { actual, maximum } => write!(
                formatter,
                "completion graph has {actual} edges, exceeding {maximum}"
            ),
            Self::DuplicateStream(stream) => write!(formatter, "duplicate stream {stream:?}"),
            Self::ForeignStreamContext(stream) => {
                write!(
                    formatter,
                    "stream belongs to a different context: {stream:?}"
                )
            }
            Self::ForeignEventContext(event) => {
                write!(formatter, "event belongs to a different context: {event:?}")
            }
            Self::UndeclaredStream(stream) => write!(formatter, "undeclared stream {stream:?}"),
            Self::UnusedStream(stream) => write!(formatter, "unused stream {stream:?}"),
            Self::DuplicateNode(node) => write!(formatter, "duplicate node {}", node.get()),
            Self::MissingNode(node) => write!(formatter, "missing node {}", node.get()),
            Self::DuplicateFuture { first, duplicate } => write!(
                formatter,
                "future identity is reused by nodes {} and {}",
                first.get(),
                duplicate.get()
            ),
            Self::DuplicateEventRecord {
                event,
                first,
                duplicate,
            } => write!(
                formatter,
                "event {event:?} is recorded by nodes {} and {}",
                first.get(),
                duplicate.get()
            ),
            Self::CrossStreamPredecessor { node, predecessor } => write!(
                formatter,
                "node {} names cross-stream predecessor {}",
                node.get(),
                predecessor.get()
            ),
            Self::StreamFork {
                predecessor,
                first,
                second,
            } => write!(
                formatter,
                "stream predecessor {} forks to nodes {} and {}",
                predecessor.get(),
                first.get(),
                second.get()
            ),
            Self::InvalidStreamHeadCount { stream, actual } => write!(
                formatter,
                "stream {stream:?} has {actual} heads instead of exactly one"
            ),
            Self::EventRecordMismatch { wait, recorded_by } => write!(
                formatter,
                "event wait node {} does not match record node {}",
                wait.get(),
                recorded_by.get()
            ),
            Self::Cycle => formatter.write_str("completion graph contains a dependency cycle"),
        }
    }
}

impl Error for CompletionGraphErrorV1 {}

fn adjacent_duplicate<T: Eq>(values: &[T]) -> Option<&T> {
    values
        .windows(2)
        .find(|pair| pair[0] == pair[1])
        .map(|pair| &pair[0])
}

pub(crate) fn dependencies(nodes: &[CompletionNodeV1]) -> Vec<Vec<CompletionNodeIdV1>> {
    nodes
        .iter()
        .map(|node| {
            let mut dependencies = Vec::with_capacity(2);
            if let Some(predecessor) = node.stream_predecessor() {
                dependencies.push(predecessor);
            }
            if let CompletionNodeKindV1::EventWait { recorded_by, .. } = node.kind()
                && !dependencies.contains(recorded_by)
            {
                dependencies.push(*recorded_by);
            }
            dependencies.sort_unstable();
            dependencies
        })
        .collect()
}

fn deterministic_topological_order(
    nodes: &[CompletionNodeV1],
    dependencies: &[Vec<CompletionNodeIdV1>],
) -> Result<Vec<CompletionNodeIdV1>, CompletionGraphErrorV1> {
    let node_indices: BTreeMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id(), index))
        .collect();
    let mut indegrees: Vec<_> = dependencies.iter().map(Vec::len).collect();
    let mut successors = vec![Vec::new(); nodes.len()];
    for (node_index, predecessors) in dependencies.iter().enumerate() {
        for predecessor in predecessors {
            let predecessor_index = node_indices[predecessor];
            successors[predecessor_index].push(node_index);
        }
    }
    for successor_list in &mut successors {
        successor_list.sort_unstable_by_key(|index| nodes[*index].id());
    }

    let mut ready: BTreeSet<_> = nodes
        .iter()
        .enumerate()
        .filter(|(index, _)| indegrees[*index] == 0)
        .map(|(_, node)| node.id())
        .collect();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(node) = ready.pop_first() {
        order.push(node);
        let node_index = node_indices[&node];
        for successor in &successors[node_index] {
            indegrees[*successor] -= 1;
            if indegrees[*successor] == 0 {
                ready.insert(nodes[*successor].id());
            }
        }
    }
    if order.len() != nodes.len() {
        return Err(CompletionGraphErrorV1::Cycle);
    }
    Ok(order)
}
