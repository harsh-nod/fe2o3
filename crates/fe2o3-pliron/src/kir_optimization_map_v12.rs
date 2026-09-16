//! Bounded, stage-local provenance for the closed V12 optimization policy.
//!
//! This is an observed rewrite relation, not a semantic-preservation proof.
//! Pointer/Value identities exist only in CaptureV12 and never enter the report.
//! Retained means the operation identity survived, not that its operands stayed
//! unchanged. Value endpoints are not fabricated instruction coordinates.

#[cfg(test)]
use crate::KIR_PLIRON_PRODUCTION_PASSES_V12;
use crate::fixed_policy_v3::FixedPolicy;
use crate::{KirBridgeCoordinateV1 as Coordinate, OperationGraphEpochV1, PlironOptimizationPassV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError, Module,
    VerifiedCanonicalKernelIrIdentityV12 as Identity, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use pliron::{
    context::{Context, Ptr},
    irbuild::observer::{RewriteEvent, RewriteObserver},
    linked_list::ContainsLinkedList,
    operation::Operation,
    value::Value,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    error::Error,
    fmt,
    mem::size_of,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum KirOptimizationEndpointV12 {
    Operation(Coordinate),
    Result {
        operation: Coordinate,
        result: u32,
    },
    FunctionArgument {
        function: u32,
        argument: u32,
    },
    BlockArgument {
        function: u32,
        block: u32,
        argument: u32,
    },
}

impl KirOptimizationEndpointV12 {
    pub fn operation(self) -> Option<Coordinate> {
        match self {
            Self::Operation(c) | Self::Result { operation: c, .. } => Some(c),
            Self::FunctionArgument { .. } | Self::BlockArgument { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KirOptimizationDispositionV12 {
    Retained,
    Moved,
    Replaced,
    Merged,
    Eliminated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KirOptimizationRelationV12 {
    source: Coordinate,
    disposition: KirOptimizationDispositionV12,
    identity_survived: bool,
    moved: bool,
    targets: std::ops::Range<usize>,
}
impl KirOptimizationRelationV12 {
    pub const fn source(&self) -> Coordinate {
        self.source
    }
    pub const fn disposition(&self) -> KirOptimizationDispositionV12 {
        self.disposition
    }
    pub const fn identity_survived(&self) -> bool {
        self.identity_survived
    }
    pub const fn moved(&self) -> bool {
        self.moved
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Operation,
    Value { producer: Option<u32> },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Node {
    kind: Kind,
    input: Option<KirOptimizationEndpointV12>,
    result_index: Option<u32>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Change {
    Create(u32),
    Erase(u32),
    Replace(u32, u32),
    Move(u32),
    Modify(u32),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Event {
    pass: u8,
    change: Change,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PassSpan {
    pass: PlironOptimizationPassV1,
    input_epoch: u64,
    output_epoch: u64,
    start: usize,
    end: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CaptureLimitsV12 {
    nodes: usize,
    events: usize,
    targets: usize,
}

impl CaptureLimitsV12 {
    pub(crate) fn for_policy_bytes(bytes: usize, policy: FixedPolicy) -> Result<Self> {
        let historical = Self::for_bytes(bytes)?;
        match policy {
            FixedPolicy::Historical2 => Ok(historical),
            FixedPolicy::Checked3 => historical.for_policy3_nodes(historical.nodes),
        }
    }
    pub(crate) fn for_policy3_nodes(self, bound: usize) -> Result<Self> {
        if bound == 0 {
            return Err(KirOptimizationMapErrorV12::Limit);
        }
        let nodes = self.nodes.min(bound);
        // The extra pass creates no operations. A separate policy-3 cap allows
        // two additional replacement/erasure observations per registered node.
        // Exceeding it is a sticky refusal, never an incomplete transcript.
        let events = nodes
            .checked_mul(10)
            .ok_or(KirOptimizationMapErrorV12::Arithmetic)?;
        Ok(Self {
            nodes,
            events,
            targets: events,
        })
    }
    pub(crate) const fn node_limit(self) -> usize {
        self.nodes
    }
    pub(crate) fn for_native_node_bound(self, bound: usize) -> Result<Self> {
        if bound == 0 {
            return Err(KirOptimizationMapErrorV12::Limit);
        }
        let nodes = self.nodes.min(bound);
        let occurrences = nodes
            .checked_mul(8)
            .ok_or(KirOptimizationMapErrorV12::Arithmetic)?;
        Ok(Self {
            nodes,
            events: self.events.min(occurrences),
            targets: self.targets.min(occurrences),
        })
    }
    pub(crate) fn for_bytes(bytes: usize) -> Result<Self> {
        let nodes = bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(64))
            .ok_or(KirOptimizationMapErrorV12::Arithmetic)?
            .min(131_072);
        Ok(Self {
            nodes,
            events: nodes
                .checked_mul(8)
                .ok_or(KirOptimizationMapErrorV12::Arithmetic)?,
            targets: nodes
                .checked_mul(8)
                .ok_or(KirOptimizationMapErrorV12::Arithmetic)?,
        })
    }
    /// Closed logical profile: includes registries, trace, final rosters and
    /// per-source graph search, including quadratic collision/search fallback.
    pub(crate) fn work(self) -> Result<usize> {
        self.nodes
            .checked_mul(self.events)
            .and_then(|n| n.checked_mul(8))
            .and_then(|n| n.checked_add(64 * self.nodes))
            .ok_or(KirOptimizationMapErrorV12::Arithmetic)
    }
    pub(crate) fn storage(self) -> Result<usize> {
        self.nodes
            .checked_mul(512)
            .and_then(|n| n.checked_add(self.events * 64))
            .and_then(|n| n.checked_add(self.targets * 64))
            .and_then(|n| n.checked_add(4096))
            .ok_or(KirOptimizationMapErrorV12::Arithmetic)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KirOptimizationMapErrorV12 {
    Arithmetic,
    Allocation,
    Limit,
    Identity,
    Lifecycle,
    Coverage,
    Passes,
    Relation,
    UnsupportedMutation,
    Resources(ResourceError),
}
impl fmt::Display for KirOptimizationMapErrorV12 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V12 optimizer mapping: {self:?}")
    }
}
impl Error for KirOptimizationMapErrorV12 {}
impl From<ResourceError> for KirOptimizationMapErrorV12 {
    fn from(e: ResourceError) -> Self {
        Self::Resources(e)
    }
}
type Result<T> = std::result::Result<T, KirOptimizationMapErrorV12>;
type Endpoint = KirOptimizationEndpointV12;
#[path = "kir_optimization_map_v12_work.rs"]
mod replay_work;
use replay_work::ReplayCensusV12;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MapData {
    input: Identity,
    output: Identity,
    nodes: Vec<Node>,
    events: Vec<Event>,
    terminal: Vec<Option<Endpoint>>,
    passes: Vec<PassSpan>,
    relations: Vec<KirOptimizationRelationV12>,
    targets: Vec<Endpoint>,
    synthesized: Vec<Coordinate>,
    digest: [u8; 32],
}

/// Historical seven-pass policy-2 observation. Its wire identity is unchanged.
#[derive(Debug, Eq, PartialEq)]
pub struct KirOptimizationMapV12 {
    data: MapData,
}

/// Separate eight-pass policy-3 observation, not semantic or execution authority.
#[derive(Debug, Eq, PartialEq)]
pub struct KirOptimizationMapPolicy3V12 {
    data: MapData,
}

// A single owned payload does not add a policy tag or duplicate graph storage.
const _: () = assert!(size_of::<KirOptimizationMapV12>() == size_of::<MapData>());
const _: () = assert!(size_of::<KirOptimizationMapPolicy3V12>() == size_of::<MapData>());

macro_rules! map_accessors {
    ($owner:ident, $policy:expr) => {
        impl $owner {
            pub const fn input_identity(&self) -> &Identity {
                self.data.input_identity()
            }
            pub const fn output_identity(&self) -> &Identity {
                self.data.output_identity()
            }
            pub const fn digest(&self) -> &[u8; 32] {
                self.data.digest()
            }
            pub fn comparison_work(&self) -> std::result::Result<usize, ResourceError> {
                self.data.comparison_work()
            }
            pub fn relations(&self) -> &[KirOptimizationRelationV12] {
                self.data.relations()
            }
            pub fn targets(&self, relation: &KirOptimizationRelationV12) -> Option<&[Endpoint]> {
                self.data.targets(relation)
            }
            pub fn synthesized_operations(&self) -> &[Coordinate] {
                self.data.synthesized_operations()
            }
            pub fn matches_execution(&self, report: &crate::PlironOptimizationReportV1) -> bool {
                self.data.matches_execution(report)
            }
            pub fn check_against(
                &self,
                input: &Owner,
                output: &Owner,
                budget: &mut Budget<'_>,
            ) -> Result<()> {
                self.data.check_against(input, output, budget, $policy)
            }
            pub(crate) const fn neutral_data_v1(&self) -> &MapData {
                &self.data
            }
        }
    };
}
map_accessors!(KirOptimizationMapV12, FixedPolicy::Historical2);
map_accessors!(KirOptimizationMapPolicy3V12, FixedPolicy::Checked3);

#[cfg(test)]
impl KirOptimizationMapV12 {
    fn compute_digest(&self) -> [u8; 32] {
        self.data.compute_digest(FixedPolicy::Historical2)
    }
}

impl MapData {
    pub const fn input_identity(&self) -> &Identity {
        &self.input
    }
    pub const fn output_identity(&self) -> &Identity {
        &self.output
    }
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    /// Upper bound on the logical payload compared by structural replay.
    pub fn comparison_work(&self) -> std::result::Result<usize, ResourceError> {
        self.retained_storage()
            .map_err(|_| ResourceError::Arithmetic)
    }
    pub fn relations(&self) -> &[KirOptimizationRelationV12] {
        &self.relations
    }
    pub fn targets(&self, relation: &KirOptimizationRelationV12) -> Option<&[Endpoint]> {
        // A relation from a different map is not accepted merely because its
        // range is in bounds.
        self.relations
            .binary_search_by_key(&relation.source, |r| r.source)
            .ok()
            .filter(|index| &self.relations[*index] == relation)
            .and_then(|_| self.targets.get(relation.targets.clone()))
    }
    pub fn synthesized_operations(&self) -> &[Coordinate] {
        &self.synthesized
    }
    pub fn matches_execution(&self, report: &crate::PlironOptimizationReportV1) -> bool {
        self.passes.len() == report.passes().len()
            && self.passes.iter().zip(report.passes()).all(|(map, pass)| {
                map.pass == pass.pass()
                    && map.input_epoch == pass.input_epoch().sequence()
                    && map.output_epoch == pass.output_epoch().sequence()
            })
            && self.passes.last().is_some_and(|pass| {
                pass.output_epoch == report.final_graph_identity().epoch().sequence()
            })
    }

    /// Rechecks exact snapshot binding and the observed structural relation.
    /// No raw bytes, unverified Module or externally supplied graph is admitted.
    pub fn check_against(
        &self,
        input: &Owner,
        output: &Owner,
        budget: &mut Budget<'_>,
        policy: FixedPolicy,
    ) -> Result<()> {
        if self.input != *input.canonical().identity()
            || self.output != *output.canonical().identity()
        {
            return Err(KirOptimizationMapErrorV12::Identity);
        }
        let limits =
            CaptureLimitsV12::for_policy_bytes(input.canonical().canonical_bytes().len(), policy)?;
        let census = ReplayCensusV12::derive(
            &self.nodes,
            &self.events,
            input.module(),
            output.module(),
            limits,
            budget,
        )?;
        budget.charge_work(census.check_work(limits.targets)?)?;
        let floor = budget.storage();
        budget.reserve_storage(limits.storage()?)?;
        let result = self.check_inner(input.module(), output.module(), limits, policy);
        // All scratch owned by check_inner has dropped on both Result paths.
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(KirOptimizationMapErrorV12::Arithmetic)?,
        )?;
        result
    }

    fn check_inner(
        &self,
        input: &Module,
        output: &Module,
        limits: CaptureLimitsV12,
        policy: FixedPolicy,
    ) -> Result<()> {
        if self.nodes.len() > limits.nodes
            || self.events.len() > limits.events
            || self.targets.len() > limits.targets
            || self.terminal.len() != self.nodes.len()
            || self.relations.len() > limits.nodes
            || self.synthesized.len() > limits.nodes
        {
            return Err(KirOptimizationMapErrorV12::Limit);
        }
        let mut inputs = self
            .nodes
            .iter()
            .filter_map(|node| node.input)
            .collect::<Vec<_>>();
        inputs.sort_unstable();
        if inputs.windows(2).any(|w| w[0] == w[1]) || inputs != module_endpoints(input)? {
            return Err(KirOptimizationMapErrorV12::Coverage);
        }
        let mut outputs = self.terminal.iter().flatten().copied().collect::<Vec<_>>();
        outputs.sort_unstable();
        if outputs.windows(2).any(|w| w[0] == w[1]) || outputs != module_endpoints(output)? {
            return Err(KirOptimizationMapErrorV12::Coverage);
        }
        match policy {
            FixedPolicy::Historical2 => {
                validate_lifecycle(&self.nodes, &self.events, &self.terminal, &self.passes)?
            }
            FixedPolicy::Checked3 => validate_lifecycle_for_policy(
                &self.nodes,
                &self.events,
                &self.terminal,
                &self.passes,
                policy,
            )?,
        }
        let (relations, targets, synthesized) =
            derive_relations(&self.nodes, &self.events, &self.terminal, limits.targets)?;
        if relations != self.relations
            || targets != self.targets
            || synthesized != self.synthesized
            || self.compute_digest(policy) != self.digest
        {
            return Err(KirOptimizationMapErrorV12::Relation);
        }
        Ok(())
    }

    fn retained_storage(&self) -> Result<usize> {
        let mut n = size_of::<Self>();
        for (capacity, size) in [
            (self.nodes.capacity(), size_of::<Node>()),
            (self.events.capacity(), size_of::<Event>()),
            (self.terminal.capacity(), size_of::<Option<Endpoint>>()),
            (self.passes.capacity(), size_of::<PassSpan>()),
            (
                self.relations.capacity(),
                size_of::<KirOptimizationRelationV12>(),
            ),
            (self.targets.capacity(), size_of::<Endpoint>()),
            (self.synthesized.capacity(), size_of::<Coordinate>()),
        ] {
            n = n
                .checked_add(
                    capacity
                        .checked_mul(size)
                        .ok_or(KirOptimizationMapErrorV12::Arithmetic)?,
                )
                .ok_or(KirOptimizationMapErrorV12::Arithmetic)?;
        }
        Ok(n)
    }

    fn compute_digest(&self, policy: FixedPolicy) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(policy.map_domain());
        h.update(self.input.digest());
        h.update(self.input.canonical_length().to_le_bytes());
        h.update(self.output.digest());
        h.update(self.output.canonical_length().to_le_bytes());
        number(&mut h, self.nodes.len());
        for node in &self.nodes {
            match node.kind {
                Kind::Operation => h.update([0]),
                Kind::Value { producer } => {
                    h.update([1]);
                    match producer {
                        None => h.update([0]),
                        Some(p) => {
                            h.update([1]);
                            h.update(p.to_le_bytes());
                        }
                    }
                }
            }
            optional_endpoint(&mut h, node.input);
            match node.result_index {
                None => h.update([0]),
                Some(index) => {
                    h.update([1]);
                    h.update(index.to_le_bytes());
                }
            }
        }
        number(&mut h, self.events.len());
        for event in &self.events {
            h.update([event.pass]);
            let (tag, a, b) = match event.change {
                Change::Create(a) => (0, a, 0),
                Change::Erase(a) => (1, a, 0),
                Change::Replace(a, b) => (2, a, b),
                Change::Move(a) => (3, a, 0),
                Change::Modify(a) => (4, a, 0),
            };
            h.update([tag]);
            h.update(a.to_le_bytes());
            h.update(b.to_le_bytes());
        }
        number(&mut h, self.terminal.len());
        for endpoint in &self.terminal {
            optional_endpoint(&mut h, *endpoint);
        }
        number(&mut h, self.passes.len());
        for (index, pass) in self.passes.iter().enumerate() {
            // The checker requires the literal roster; index is its wire tag.
            number(&mut h, index);
            h.update(pass.input_epoch.to_le_bytes());
            h.update(pass.output_epoch.to_le_bytes());
            number(&mut h, pass.start);
            number(&mut h, pass.end);
        }
        // Relations are deterministically derived from the hashed witness.
        h.finalize().into()
    }
}
fn number(h: &mut Sha256, value: usize) {
    h.update((value as u64).to_le_bytes());
}
fn coordinate(h: &mut Sha256, c: Coordinate) {
    let (tag, f, b, o) = match c {
        Coordinate::Function { function } => (0, function, 0, 0),
        Coordinate::Block { function, block } => (1, function, block, 0),
        Coordinate::Operation {
            function,
            block,
            operation,
        } => (2, function, block, operation),
        Coordinate::Terminator { function, block } => (3, function, block, 0),
    };
    h.update([tag]);
    h.update(f.to_le_bytes());
    h.update(b.to_le_bytes());
    h.update(o.to_le_bytes());
}
fn optional_endpoint(h: &mut Sha256, endpoint: Option<Endpoint>) {
    match endpoint {
        None => h.update([0]),
        Some(Endpoint::Operation(c)) => {
            h.update([1]);
            coordinate(h, c);
        }
        Some(Endpoint::Result { operation, result }) => {
            h.update([2]);
            coordinate(h, operation);
            h.update(result.to_le_bytes());
        }
        Some(Endpoint::FunctionArgument { function, argument }) => {
            h.update([3]);
            h.update(function.to_le_bytes());
            h.update(argument.to_le_bytes());
        }
        Some(Endpoint::BlockArgument {
            function,
            block,
            argument,
        }) => {
            h.update([4]);
            h.update(function.to_le_bytes());
            h.update(block.to_le_bytes());
            h.update(argument.to_le_bytes());
        }
    }
}
fn u32_index(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| KirOptimizationMapErrorV12::Arithmetic)
}
fn module_endpoints(module: &Module) -> Result<Vec<Endpoint>> {
    let mut endpoints = Vec::new();
    for (f, function) in module.functions.iter().enumerate() {
        let Some(body) = &function.body else { continue };
        let function = u32_index(f)?;
        for argument in 0..body.parameters.len() {
            endpoints.push(Endpoint::FunctionArgument {
                function,
                argument: u32_index(argument)?,
            });
        }
        for (b, body_block) in body.blocks.iter().enumerate() {
            let block = u32_index(b)?;
            for argument in 0..body_block.parameters.len() {
                endpoints.push(Endpoint::BlockArgument {
                    function,
                    block,
                    argument: u32_index(argument)?,
                });
            }
            for (o, op) in body_block.operations.iter().enumerate() {
                let operation = Coordinate::Operation {
                    function,
                    block,
                    operation: u32_index(o)?,
                };
                endpoints.push(Endpoint::Operation(operation));
                for result in 0..op.results.len() {
                    endpoints.push(Endpoint::Result {
                        operation,
                        result: u32_index(result)?,
                    });
                }
            }
            if body_block.terminator.is_some() {
                endpoints.push(Endpoint::Operation(Coordinate::Terminator {
                    function,
                    block,
                }));
            }
        }
    }
    endpoints.sort_unstable();
    Ok(endpoints)
}

fn validate_lifecycle(
    nodes: &[Node],
    events: &[Event],
    terminal: &[Option<Endpoint>],
    passes: &[PassSpan],
) -> Result<()> {
    validate_lifecycle_for_policy(nodes, events, terminal, passes, FixedPolicy::Historical2)
}
fn validate_lifecycle_for_policy(
    nodes: &[Node],
    events: &[Event],
    terminal: &[Option<Endpoint>],
    passes: &[PassSpan],
    policy: FixedPolicy,
) -> Result<()> {
    if passes.len() != policy.passes().len() || passes[0].input_epoch == 0 {
        return Err(KirOptimizationMapErrorV12::Passes);
    }
    let mut end = 0;
    let mut epoch = passes[0].input_epoch;
    for (index, span) in passes.iter().enumerate() {
        if span.pass != policy.passes()[index]
            || span.input_epoch != epoch
            || span.output_epoch < span.input_epoch
            || span.output_epoch - span.input_epoch > 1
            || span.start != end
            || span.end < span.start
            || span.end > events.len()
            || events[span.start..span.end]
                .iter()
                .any(|event| usize::from(event.pass) != index)
        {
            return Err(KirOptimizationMapErrorV12::Passes);
        }
        if span.start != span.end && span.input_epoch == span.output_epoch {
            return Err(KirOptimizationMapErrorV12::Passes);
        }
        end = span.end;
        epoch = span.output_epoch;
    }
    if end != events.len() {
        return Err(KirOptimizationMapErrorV12::Passes);
    }
    let mut alive = nodes
        .iter()
        .map(|node| node.input.is_some())
        .collect::<Vec<_>>();
    let mut born = alive.clone();
    for (index, node) in nodes.iter().enumerate() {
        match (node.kind, node.input) {
            (Kind::Operation, Some(Endpoint::Operation(_))) | (Kind::Operation, None) => {}
            (Kind::Value { producer: Some(p) }, Some(Endpoint::Result { operation, .. })) => {
                if p as usize >= index
                    || nodes[p as usize].input != Some(Endpoint::Operation(operation))
                {
                    return Err(KirOptimizationMapErrorV12::Lifecycle);
                }
            }
            (
                Kind::Value { producer: None },
                Some(Endpoint::FunctionArgument { .. } | Endpoint::BlockArgument { .. }),
            ) => {}
            (Kind::Value { producer }, None) => {
                if producer.is_some_and(|p| {
                    p as usize >= index || nodes[p as usize].kind != Kind::Operation
                }) {
                    return Err(KirOptimizationMapErrorV12::Lifecycle);
                }
            }
            _ => return Err(KirOptimizationMapErrorV12::Lifecycle),
        }
    }
    // The existing 512*N scratch component covers two bool rosters plus one
    // usize counter per node. Update counts instead of scanning result nodes
    // for every producer erasure.
    let mut live_results = vec![0_usize; nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        if let Kind::Value {
            producer: Some(producer),
        } = node.kind
            && alive[index]
        {
            if !alive[producer as usize] {
                return Err(KirOptimizationMapErrorV12::Lifecycle);
            }
            live_results[producer as usize] += 1;
        }
    }
    for event in events {
        let a = match event.change {
            Change::Create(a)
            | Change::Erase(a)
            | Change::Replace(a, _)
            | Change::Move(a)
            | Change::Modify(a) => a as usize,
        };
        if a >= nodes.len() {
            return Err(KirOptimizationMapErrorV12::Lifecycle);
        }
        match event.change {
            Change::Create(_) => {
                if born[a] || nodes[a].input.is_some() {
                    return Err(KirOptimizationMapErrorV12::Lifecycle);
                }
                if let Kind::Value { producer: Some(p) } = nodes[a].kind {
                    if !alive[p as usize] {
                        return Err(KirOptimizationMapErrorV12::Lifecycle);
                    }
                    live_results[p as usize] += 1;
                }
                born[a] = true;
                alive[a] = true;
            }
            Change::Erase(_) => {
                if !alive[a] {
                    return Err(KirOptimizationMapErrorV12::Lifecycle);
                }
                match nodes[a].kind {
                    Kind::Operation => {
                        if live_results[a] != 0 {
                            return Err(KirOptimizationMapErrorV12::Lifecycle);
                        }
                    }
                    Kind::Value {
                        producer: Some(producer),
                    } => {
                        if !alive[producer as usize] {
                            return Err(KirOptimizationMapErrorV12::Lifecycle);
                        }
                        live_results[producer as usize] = live_results[producer as usize]
                            .checked_sub(1)
                            .ok_or(KirOptimizationMapErrorV12::Lifecycle)?;
                    }
                    Kind::Value { producer: None } => {}
                }
                alive[a] = false;
            }
            Change::Replace(_, b) => {
                let b = b as usize;
                if b >= nodes.len()
                    || a == b
                    || !alive[a]
                    || !alive[b]
                    || matches!(nodes[a].kind, Kind::Operation)
                        != matches!(nodes[b].kind, Kind::Operation)
                {
                    return Err(KirOptimizationMapErrorV12::Lifecycle);
                }
            }
            Change::Move(_) => {
                if !alive[a] || nodes[a].kind != Kind::Operation {
                    return Err(KirOptimizationMapErrorV12::Lifecycle);
                }
            }
            Change::Modify(_) => {
                if !alive[a] || !matches!(nodes[a].kind, Kind::Value { .. }) {
                    return Err(KirOptimizationMapErrorV12::Lifecycle);
                }
            }
        }
    }
    if born.iter().any(|born| !born)
        || alive
            .iter()
            .zip(terminal)
            .any(|(live, end)| *live != end.is_some())
    {
        return Err(KirOptimizationMapErrorV12::Lifecycle);
    }
    for (node, end) in nodes.iter().zip(terminal) {
        if let Kind::Value {
            producer: Some(producer),
        } = node.kind
        {
            let index = node
                .result_index
                .ok_or(KirOptimizationMapErrorV12::Lifecycle)?;
            match end {
                Some(Endpoint::Result { operation, result })
                    if *result == index
                        && terminal[producer as usize] == Some(Endpoint::Operation(*operation)) => {
                }
                Some(_) => return Err(KirOptimizationMapErrorV12::Lifecycle),
                None => {}
            }
            if let Some(Endpoint::Result { result, .. }) = node.input
                && result != index
            {
                return Err(KirOptimizationMapErrorV12::Lifecycle);
            }
        } else if node.result_index.is_some()
            || matches!(
                (node.kind, end),
                (
                    Kind::Value { producer: None },
                    Some(Endpoint::Result { .. })
                )
            )
        {
            return Err(KirOptimizationMapErrorV12::Lifecycle);
        }
        if matches!(node.kind, Kind::Operation) != matches!(end, Some(Endpoint::Operation(_)))
            && end.is_some()
        {
            return Err(KirOptimizationMapErrorV12::Lifecycle);
        }
    }
    Ok(())
}

// Counters are test-only; they neither change resource policy nor enter the map.
#[derive(Default)]
struct RelationTraversalCountsV12 {
    #[cfg(test)]
    suffix_nodes: usize,
    #[cfg(test)]
    suffix_edges: usize,
    #[cfg(test)]
    search_nodes: usize,
    #[cfg(test)]
    search_edges: usize,
}

fn merge_terminal_suffix_v12(a: usize, b: usize, empty: usize, multiple: usize) -> usize {
    if a == empty {
        b
    } else if b == empty || a == b {
        a
    } else {
        multiple
    }
}

// Empty/singleton suffixes are shared across sources. Multiple-terminal
// suffixes keep the generation-tagged traversal, without cached endpoint sets.
fn derive_relations(
    nodes: &[Node],
    events: &[Event],
    terminal: &[Option<Endpoint>],
    target_limit: usize,
) -> Result<(
    Vec<KirOptimizationRelationV12>,
    Vec<Endpoint>,
    Vec<Coordinate>,
)> {
    derive_relations_with_counts_v12(
        nodes,
        events,
        terminal,
        target_limit,
        &mut RelationTraversalCountsV12::default(),
    )
}

fn derive_relations_with_counts_v12(
    nodes: &[Node],
    events: &[Event],
    terminal: &[Option<Endpoint>],
    target_limit: usize,
    _counts: &mut RelationTraversalCountsV12,
) -> Result<(
    Vec<KirOptimizationRelationV12>,
    Vec<Endpoint>,
    Vec<Coordinate>,
)> {
    let mut edges = vec![Vec::<usize>::new(); nodes.len()];
    let mut results = vec![Vec::<usize>::new(); nodes.len()];
    for (i, node) in nodes.iter().enumerate() {
        if let Kind::Value { producer: Some(p) } = node.kind {
            results[p as usize].push(i);
        }
    }
    for event in events {
        if let Change::Replace(a, b) = event.change {
            edges[a as usize].push(b as usize);
        }
    }
    // A cyclic replacement witness is rejected, not silently classified as
    // elimination after a visited-set traversal.
    let mut indegree = vec![0_usize; nodes.len()];
    for outgoing in &edges {
        for &target in outgoing {
            indegree[target] += 1;
        }
    }
    let mut order = indegree
        .iter()
        .enumerate()
        .filter_map(|(i, degree)| (*degree == 0).then_some(i))
        .collect::<Vec<_>>();
    let mut consumed = 0;
    while consumed < order.len() {
        let node = order[consumed];
        consumed += 1;
        for &target in &edges[node] {
            indegree[target] -= 1;
            if indegree[target] == 0 {
                order.push(target);
            }
        }
    }
    if consumed != nodes.len() {
        return Err(KirOptimizationMapErrorV12::Relation);
    }

    // Reuse the cycle-check allocation. Node indices encode One(terminal);
    // N and N+1 encode Empty and Multiple, so every entry occupies one word.
    let empty = nodes.len();
    let multiple = empty
        .checked_add(1)
        .ok_or(KirOptimizationMapErrorV12::Arithmetic)?;
    let mut suffix = indegree;
    suffix.fill(empty);
    // Observed replacement edges preserve operation/value kind. Values have
    // only value successors; operation result paths also point into that DAG.
    for operations in [false, true] {
        for &node in order.iter().rev() {
            if (nodes[node].kind == Kind::Operation) != operations {
                continue;
            }
            #[cfg(test)]
            {
                _counts.suffix_nodes += 1;
            }
            let mut summary = if terminal[node].is_some() {
                node
            } else {
                empty
            };
            #[cfg(test)]
            {
                _counts.suffix_edges += edges[node].len();
            }
            for &target in &edges[node] {
                summary = merge_terminal_suffix_v12(summary, suffix[target], empty, multiple);
            }
            if operations {
                for &result in &results[node] {
                    #[cfg(test)]
                    {
                        _counts.suffix_edges += edges[result].len();
                    }
                    for &target in &edges[result] {
                        summary =
                            merge_terminal_suffix_v12(summary, suffix[target], empty, multiple);
                    }
                }
            }
            suffix[node] = summary;
        }
    }
    drop(order);
    let mut rows = Vec::new();
    let mut targets = Vec::new();
    let mut visited = vec![0_usize; nodes.len()];
    let mut stack = Vec::new();
    for (source_node, node) in nodes.iter().enumerate() {
        let Some(Endpoint::Operation(source)) = node.input else {
            continue;
        };
        let generation = source_node + 1;
        stack.push(source_node);
        let start = targets.len();
        while let Some(next) = stack.pop() {
            if visited[next] == generation {
                continue;
            }
            #[cfg(test)]
            {
                _counts.search_nodes += 1;
            }
            if suffix[next] == empty {
                visited[next] = generation;
                continue;
            }
            if suffix[next] != multiple {
                let end = suffix[next];
                // One(end) implies end itself is One(end). Marking it visited
                // deduplicates cached/direct paths without hiding other ends.
                if visited[end] != generation {
                    if targets.len() == target_limit {
                        return Err(KirOptimizationMapErrorV12::Limit);
                    }
                    targets.push(terminal[end].ok_or(KirOptimizationMapErrorV12::Relation)?);
                }
                visited[end] = generation;
                visited[next] = generation;
                continue;
            }
            visited[next] = generation;
            if let Some(endpoint) = terminal[next] {
                if targets.len() == target_limit {
                    return Err(KirOptimizationMapErrorV12::Limit);
                }
                targets.push(endpoint);
            }
            // A conservative effect may retain the producer after SCCP replaces
            // its result uses. Identity survival and replacement provenance both
            // remain observable; a live intermediate value can also be replaced.
            #[cfg(test)]
            {
                _counts.search_edges += edges[next].len();
            }
            stack.extend(edges[next].iter().copied());
            if nodes[next].kind == Kind::Operation {
                for &result in &results[next] {
                    // Do not add unchanged results beside their surviving
                    // operation. Only an observed replacement starts this path.
                    #[cfg(test)]
                    {
                        _counts.search_edges += edges[result].len();
                    }
                    stack.extend(edges[result].iter().copied());
                }
            }
        }
        targets[start..].sort_unstable();
        let mut unique = start;
        for i in start..targets.len() {
            if i == start || targets[i] != targets[unique - 1] {
                targets[unique] = targets[i];
                unique += 1;
            }
        }
        targets.truncate(unique);
        let function_of = |c| match c {
            Coordinate::Function { function }
            | Coordinate::Block { function, .. }
            | Coordinate::Operation { function, .. }
            | Coordinate::Terminator { function, .. } => function,
        };
        let source_function = function_of(source);
        if targets[start..unique].iter().any(|target| {
            (match target {
                Endpoint::Operation(c) | Endpoint::Result { operation: c, .. } => function_of(*c),
                Endpoint::FunctionArgument { function, .. }
                | Endpoint::BlockArgument { function, .. } => *function,
            }) != source_function
        }) {
            return Err(KirOptimizationMapErrorV12::Relation);
        }
        let identity_survived = terminal[source_node].is_some();
        let moved = identity_survived && terminal[source_node] != node.input;
        let disposition = if unique == start {
            KirOptimizationDispositionV12::Eliminated
        } else if moved {
            KirOptimizationDispositionV12::Moved
        } else if identity_survived {
            KirOptimizationDispositionV12::Retained
        } else {
            KirOptimizationDispositionV12::Replaced
        };
        rows.push(KirOptimizationRelationV12 {
            source,
            disposition,
            identity_survived,
            moved,
            targets: start..unique,
        });
    }
    rows.sort_unstable_by_key(|r| r.source);
    let mut destination_sources = std::collections::BTreeMap::<Coordinate, usize>::new();
    for row in &rows {
        let mut ops = targets[row.targets.clone()]
            .iter()
            .filter_map(|endpoint| endpoint.operation())
            .collect::<Vec<_>>();
        ops.sort_unstable();
        ops.dedup();
        for op in ops {
            *destination_sources.entry(op).or_default() += 1;
        }
    }
    for row in &mut rows {
        if targets[row.targets.clone()]
            .iter()
            .filter_map(|endpoint| endpoint.operation())
            .any(|op| destination_sources[&op] > 1)
        {
            row.disposition = KirOptimizationDispositionV12::Merged;
        }
    }
    let mut synthesized = terminal
        .iter()
        .filter_map(|end| match end {
            Some(Endpoint::Operation(c)) if !destination_sources.contains_key(c) => Some(*c),
            _ => None,
        })
        .collect::<Vec<_>>();
    synthesized.sort_unstable();
    Ok((rows, targets, synthesized))
}

#[cfg(test)]
#[path = "kir_optimization_map_v12_suffix_tests.rs"]
mod suffix_tests;

#[cfg(test)]
#[path = "kir_optimization_map_v12_lifecycle_tests.rs"]
mod lifecycle_tests;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum LiveKeyV12 {
    Operation(Ptr<Operation>),
    Value(Value),
}
pub(crate) type LiveRosterV12 = Vec<(LiveKeyV12, Endpoint)>;

#[derive(Clone)]
pub(crate) struct CaptureV12(Arc<Mutex<CaptureStateV12>>);
struct CaptureStateV12 {
    policy: FixedPolicy,
    limits: CaptureLimitsV12,
    ids: HashMap<LiveKeyV12, u32>,
    nodes: Vec<Node>,
    alive: Vec<bool>,
    events: Vec<Event>,
    passes: Vec<PassSpan>,
    current: Option<PassSpan>,
    failure: Option<KirOptimizationMapErrorV12>,
}
impl CaptureV12 {
    pub(crate) fn new(limits: CaptureLimitsV12, roster: &LiveRosterV12) -> Result<Self> {
        Self::new_for_policy(limits, roster, FixedPolicy::Historical2)
    }
    pub(crate) fn new_for_policy(
        limits: CaptureLimitsV12,
        roster: &LiveRosterV12,
        policy: FixedPolicy,
    ) -> Result<Self> {
        let mut state = CaptureStateV12 {
            policy,
            limits,
            ids: HashMap::new(),
            nodes: Vec::new(),
            alive: Vec::new(),
            events: Vec::new(),
            passes: Vec::new(),
            current: None,
            failure: None,
        };
        state
            .ids
            .try_reserve(limits.nodes)
            .map_err(|_| KirOptimizationMapErrorV12::Allocation)?;
        state
            .nodes
            .try_reserve_exact(limits.nodes)
            .map_err(|_| KirOptimizationMapErrorV12::Allocation)?;
        state
            .alive
            .try_reserve_exact(limits.nodes)
            .map_err(|_| KirOptimizationMapErrorV12::Allocation)?;
        state
            .events
            .try_reserve_exact(limits.events)
            .map_err(|_| KirOptimizationMapErrorV12::Allocation)?;
        state
            .passes
            .try_reserve_exact(policy.passes().len())
            .map_err(|_| KirOptimizationMapErrorV12::Allocation)?;
        for &(key, endpoint) in roster {
            state.register(key, Some(endpoint))?;
        }
        Ok(Self(Arc::new(Mutex::new(state))))
    }
    pub(crate) fn observer(&self) -> Box<dyn RewriteObserver> {
        Box::new(self.clone())
    }
    pub(crate) fn require_policy(&self, policy: FixedPolicy) -> Result<()> {
        if self.0.is_poisoned() {
            return Err(KirOptimizationMapErrorV12::UnsupportedMutation);
        }
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        if state.policy != policy {
            return Err(KirOptimizationMapErrorV12::Passes);
        }
        Ok(())
    }
    pub(crate) fn failure(&self) -> Option<KirOptimizationMapErrorV12> {
        if self.0.is_poisoned() {
            return Some(KirOptimizationMapErrorV12::UnsupportedMutation);
        }
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .failure
            .clone()
    }
    pub(crate) fn begin_pass(
        &self,
        pass: PlironOptimizationPassV1,
        epoch: OperationGraphEpochV1,
    ) -> bool {
        if self.0.is_poisoned() {
            return false;
        }
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.failure.is_some()
            || state.current.is_some()
            || state.passes.len() >= state.policy.passes().len()
            || state.policy.passes()[state.passes.len()] != pass
        {
            state.failure = Some(KirOptimizationMapErrorV12::Passes);
            return false;
        }
        let start = state.events.len();
        state.current = Some(PassSpan {
            pass,
            input_epoch: epoch.sequence(),
            output_epoch: epoch.sequence(),
            start,
            end: start,
        });
        true
    }
    pub(crate) fn end_pass(&self, ctx: &Context, epoch: OperationGraphEpochV1) -> bool {
        if self.0.is_poisoned() {
            return false;
        }
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let result = (|| {
            // DCE removes dead block arguments directly, outside IRRewriter.
            // Reconcile only dead argument identities here. Missing operations
            // and results always require explicit erasure observations.
            if let Some(error) = &state.failure {
                return Err(error.clone());
            }
            let gone = state
                .ids
                .iter()
                .filter_map(|(key, id)| {
                    if !state.alive[*id as usize] {
                        return None;
                    }
                    match key {
                        LiveKeyV12::Value(v)
                            if v.defining_block().is_some() && v.try_find_index(ctx).is_err() =>
                        {
                            Some(*id)
                        }
                        _ => None,
                    }
                })
                .collect::<Vec<_>>();
            // HashMap order must never enter the deterministic transcript.
            let mut gone = gone;
            gone.sort_unstable();
            for id in gone {
                state.erase(id)?;
            }
            if let Some(error) = &state.failure {
                return Err(error.clone());
            }
            let mut span = state
                .current
                .take()
                .ok_or(KirOptimizationMapErrorV12::Passes)?;
            span.output_epoch = epoch.sequence();
            span.end = state.events.len();
            state.passes.push(span);
            Ok(())
        })();
        if let Err(error) = result {
            state.failure = Some(error);
            false
        } else {
            true
        }
    }
    pub(crate) fn finish(
        &self,
        input: &Owner,
        output: &Owner,
        roster: &LiveRosterV12,
        budget: &mut Budget<'_>,
    ) -> Result<(KirOptimizationMapV12, usize)> {
        self.finish_data(input, output, roster, budget, FixedPolicy::Historical2)
            .map(|(data, storage)| (KirOptimizationMapV12 { data }, storage))
    }
    pub(crate) fn finish_policy3(
        &self,
        input: &Owner,
        output: &Owner,
        roster: &LiveRosterV12,
        budget: &mut Budget<'_>,
    ) -> Result<(KirOptimizationMapPolicy3V12, usize)> {
        self.finish_data(input, output, roster, budget, FixedPolicy::Checked3)
            .map(|(data, storage)| (KirOptimizationMapPolicy3V12 { data }, storage))
    }
    fn finish_data(
        &self,
        input: &Owner,
        output: &Owner,
        roster: &LiveRosterV12,
        budget: &mut Budget<'_>,
        policy: FixedPolicy,
    ) -> Result<(MapData, usize)> {
        if self.0.is_poisoned() {
            return Err(KirOptimizationMapErrorV12::UnsupportedMutation);
        }
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        if state.policy != policy {
            return Err(KirOptimizationMapErrorV12::Passes);
        }
        let floor = budget.storage();
        let census = ReplayCensusV12::derive(
            &state.nodes,
            &state.events,
            input.module(),
            output.module(),
            state.limits,
            budget,
        )?;
        budget.charge_work(census.finish_work(state.limits.targets, roster.len())?)?;
        // Capture and graph remain charged in graph custody. This is the
        // additional coexistence/scratch/returned-map reservation.
        budget.reserve_storage(state.limits.storage()?)?;
        let result = (|| {
            let mut terminal = vec![None; state.nodes.len()];
            for &(key, endpoint) in roster {
                let id = *state
                    .ids
                    .get(&key)
                    .ok_or(KirOptimizationMapErrorV12::Coverage)? as usize;
                if !state.alive[id] || terminal[id].replace(endpoint).is_some() {
                    return Err(KirOptimizationMapErrorV12::Lifecycle);
                }
            }
            let (relations, targets, synthesized) =
                derive_relations(&state.nodes, &state.events, &terminal, state.limits.targets)?;
            let mut map = MapData {
                input: *input.canonical().identity(),
                output: *output.canonical().identity(),
                nodes: state.nodes.clone(),
                events: state.events.clone(),
                terminal,
                passes: state.passes.clone(),
                relations,
                targets,
                synthesized,
                digest: [0; 32],
            };
            map.digest = map.compute_digest(policy);
            map.check_inner(input.module(), output.module(), state.limits, policy)?;
            let retained = map.retained_storage()?;
            if retained > state.limits.storage()? {
                return Err(KirOptimizationMapErrorV12::Limit);
            }
            Ok((map, retained))
        })();
        // Error payload owners and all checker scratch have dropped before
        // restoring the caller floor; success transfers a separate receipt.
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(KirOptimizationMapErrorV12::Arithmetic)?,
        )?;
        result
    }
}
impl CaptureStateV12 {
    fn event(&mut self, change: Change) -> Result<()> {
        if self.events.len() == self.limits.events {
            return Err(KirOptimizationMapErrorV12::Limit);
        }
        if self.current.is_none() {
            return Err(KirOptimizationMapErrorV12::Passes);
        }
        self.events.push(Event {
            pass: u8::try_from(self.passes.len())
                .map_err(|_| KirOptimizationMapErrorV12::Arithmetic)?,
            change,
        });
        Ok(())
    }
    fn register(&mut self, key: LiveKeyV12, input: Option<Endpoint>) -> Result<u32> {
        if let Some(id) = self.ids.get(&key).copied() {
            // Reinsertion of a moved object is allowed. A tombstoned key never
            // revives, even if an upstream allocation generation were to wrap.
            if input.is_some() || !self.alive[id as usize] {
                return Err(KirOptimizationMapErrorV12::Lifecycle);
            }
            return Ok(id);
        }
        if self.nodes.len() == self.limits.nodes {
            return Err(KirOptimizationMapErrorV12::Limit);
        }
        let kind = match key {
            LiveKeyV12::Operation(_) => Kind::Operation,
            LiveKeyV12::Value(v) => Kind::Value {
                producer: match v.defining_op() {
                    Some(op) => Some(
                        *self
                            .ids
                            .get(&LiveKeyV12::Operation(op))
                            .ok_or(KirOptimizationMapErrorV12::Coverage)?,
                    ),
                    None => None,
                },
            },
        };
        let id = u32_index(self.nodes.len())?;
        self.ids.insert(key, id);
        self.nodes.push(Node {
            kind,
            input,
            result_index: match input {
                Some(Endpoint::Result { result, .. }) => Some(result),
                _ => None,
            },
        });
        self.alive.push(true);
        if input.is_none() {
            self.event(Change::Create(id))?;
        }
        Ok(id)
    }
    fn require(&self, key: LiveKeyV12) -> Result<u32> {
        self.ids
            .get(&key)
            .copied()
            .filter(|id| self.alive[*id as usize])
            .ok_or(KirOptimizationMapErrorV12::Lifecycle)
    }
    fn erase(&mut self, id: u32) -> Result<()> {
        if !self.alive[id as usize] {
            return Err(KirOptimizationMapErrorV12::Lifecycle);
        }
        self.event(Change::Erase(id))?;
        self.alive[id as usize] = false;
        Ok(())
    }
    fn replace(&mut self, old: LiveKeyV12, new: LiveKeyV12) -> Result<()> {
        if old == new {
            return Ok(());
        }
        let old = self.require(old)?;
        let new = self.require(new)?;
        self.event(Change::Replace(old, new))
    }
    fn observe(&mut self, ctx: &Context, event: RewriteEvent) -> Result<()> {
        match event {
            RewriteEvent::OperationInserted(op) => {
                self.register(LiveKeyV12::Operation(op), None)?;
                for (index, value) in op.deref(ctx).results().enumerate() {
                    let id = self.register(LiveKeyV12::Value(value), None)?;
                    self.nodes[id as usize].result_index = Some(u32_index(index)?);
                }
            }
            RewriteEvent::BlockInserted(block) => {
                for value in block.deref(ctx).arguments() {
                    self.register(LiveKeyV12::Value(value), None)?;
                }
            }
            RewriteEvent::OperationErased(op) => {
                for value in op.deref(ctx).results() {
                    let id = self.require(LiveKeyV12::Value(value))?;
                    self.erase(id)?;
                }
                let id = self.require(LiveKeyV12::Operation(op))?;
                self.erase(id)?;
            }
            RewriteEvent::BlockErased(block) => {
                for value in block.deref(ctx).arguments() {
                    let id = self.require(LiveKeyV12::Value(value))?;
                    self.erase(id)?;
                }
            }
            RewriteEvent::ValueReplaced { old, new } => {
                self.replace(LiveKeyV12::Value(old), LiveKeyV12::Value(new))?
            }
            RewriteEvent::OperationReplaced { old, new } => {
                self.replace(LiveKeyV12::Operation(old), LiveKeyV12::Operation(new))?
            }
            RewriteEvent::OperationUnlinked(op) => {
                let id = self.require(LiveKeyV12::Operation(op))?;
                self.event(Change::Move(id))?;
            }
            RewriteEvent::BlockUnlinked(block) => {
                for op in block.deref(ctx).iter(ctx) {
                    let id = self.require(LiveKeyV12::Operation(op))?;
                    self.event(Change::Move(id))?;
                }
            }
            RewriteEvent::ValueTypeChanged { value, .. } => {
                let id = self.require(LiveKeyV12::Value(value))?;
                self.event(Change::Modify(id))?;
            }
            RewriteEvent::RegionErased(_) => {
                return Err(KirOptimizationMapErrorV12::UnsupportedMutation);
            }
        }
        Ok(())
    }
}
impl RewriteObserver for CaptureV12 {
    fn observe(&mut self, ctx: &Context, event: RewriteEvent) {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.failure.is_none()
            && let Err(error) = state.observe(ctx, event)
        {
            state.failure = Some(error);
        }
    }
}

// Private read-only witness access for the separately metered neutral occurrence
// rows. Existing map bytes, lifecycle validation and target reports are frozen.
impl MapData {
    pub(crate) fn neutral_node_count_v1(&self) -> usize {
        self.nodes.len()
    }
    pub(crate) fn neutral_event_count_v1(&self) -> usize {
        self.events.len()
    }
    pub(crate) fn neutral_value_nodes_v1(
        &self,
    ) -> impl Iterator<
        Item = (
            usize,
            Option<KirOptimizationEndpointV12>,
            Option<KirOptimizationEndpointV12>,
        ),
    > + '_ {
        self.nodes.iter().enumerate().filter_map(|(id, node)| {
            matches!(node.kind, Kind::Value { .. }).then_some((id, node.input, self.terminal[id]))
        })
    }
    pub(crate) fn neutral_value_edges_v1(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.events.iter().filter_map(|event| match event.change {
            Change::Replace(source, target)
                if matches!(self.nodes[source as usize].kind, Kind::Value { .. }) =>
            {
                Some((source as usize, target as usize))
            }
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1, Function,
        Operation as KirOperation, OperationKind, ScalarType, Signature, Terminator, Type,
        ValueDef, ValueId,
    };
    const WORK: usize = 1_000_000_000_000;
    const STORAGE: usize = 512_000_000;

    mod actual_work_tests {
        include!("kir_optimization_map_v12_work_tests.rs");
    }

    include!("kir_optimization_map_v12_live_tests.rs");

    fn owner(module: &Module) -> Owner {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget)
            .unwrap()
            .0
    }
    fn source() -> Module {
        let ty = Type::Scalar(ScalarType::U32);
        let scalar = |id| ValueDef::new(ValueId(id), ty.clone());
        let mut first = BasicBlock::new(BlockId(7));
        for result in [3, 4] {
            first.operations.push(KirOperation::effect_free(
                scalar(result),
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: ValueId(1),
                    rhs: ValueId(2),
                },
            ));
        }
        first.operations.push(KirOperation::effect_free(
            scalar(5),
            OperationKind::Select {
                condition: ValueId(0),
                true_value: ValueId(1),
                false_value: ValueId(1),
            },
        ));
        first.operations.push(KirOperation::effect_free(
            scalar(6),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ));
        first.operations.push(KirOperation::effect_free(
            scalar(7),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(3),
                rhs: ValueId(4),
            },
        ));
        first.terminator = Some(Terminator::Branch {
            target: BlockId(99),
            arguments: vec![ValueId(7)],
        });
        let mut second = BasicBlock::new(BlockId(99));
        second.parameters.push(scalar(8));
        second.terminator = Some(Terminator::Return {
            values: vec![ValueId(8)],
        });
        let mut module = Module::new("observed");
        module.functions.push(Function::internal_helper(
            "f",
            Signature::new(vec![Type::BOOL, ty.clone(), ty.clone()], vec![ty]),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![first, second],
        ));
        module
    }
    fn run(
        input: &Owner,
    ) -> (
        Owner,
        crate::PlironOptimizationReportV1,
        KirOptimizationMapV12,
    ) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(7).unwrap();
        let (mut graph, imported) = crate::KirPlironGraphV12::import(input, &mut budget).unwrap();
        budget.reserve_storage(imported.retained_storage()).unwrap();
        let (report, execution) = graph
            .execute_production_optimization_v12(&mut budget)
            .unwrap();
        budget
            .reserve_storage(execution.retained_storage())
            .unwrap();
        let (output, _, map, extracted) = graph
            .extract_optimized_canonical_kir_module_with_map_v12(&mut budget)
            .unwrap();
        budget
            .reserve_storage(extracted.retained_storage())
            .unwrap();
        assert!(map.matches_execution(&report));
        map.check_against(input, &output, &mut budget).unwrap();
        let graph_storage = graph.retained_storage();
        drop(graph);
        budget.release_storage(graph_storage).unwrap();
        // Returned owners are explicitly transferred out of this test ledger.
        budget
            .release_storage(extracted.retained_storage() + execution.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 7);
        (output, report, map)
    }
    fn coord(operation: u32) -> Coordinate {
        Coordinate::Operation {
            function: 0,
            block: 0,
            operation,
        }
    }
    fn row(map: &KirOptimizationMapV12, coordinate: Coordinate) -> &KirOptimizationRelationV12 {
        map.data
            .relations
            .iter()
            .find(|row| row.source == coordinate)
            .unwrap()
    }

    #[test]
    fn seven_real_passes_capture_merge_argument_replacement_elimination_and_movement() {
        let source = source();
        let input = owner(&source);
        let (output, report, map) = run(&input);
        assert!(report.passes().iter().any(|pass| pass.changed()));
        assert_eq!(
            row(&map, coord(0)).disposition(),
            KirOptimizationDispositionV12::Merged
        );
        assert_eq!(
            row(&map, coord(1)).disposition(),
            KirOptimizationDispositionV12::Merged
        );
        assert!(row(&map, coord(0)).identity_survived());
        assert!(!row(&map, coord(1)).identity_survived());
        assert_eq!(
            map.targets(row(&map, coord(2))).unwrap(),
            &[Endpoint::FunctionArgument {
                function: 0,
                argument: 1
            }]
        );
        assert_eq!(
            row(&map, coord(2)).disposition(),
            KirOptimizationDispositionV12::Replaced
        );
        assert_eq!(
            row(&map, coord(3)).disposition(),
            KirOptimizationDispositionV12::Eliminated
        );
        assert_eq!(
            row(&map, coord(4)).disposition(),
            KirOptimizationDispositionV12::Moved
        );
        let source_return = Coordinate::Terminator {
            function: 0,
            block: 1,
        };
        assert_eq!(
            map.targets(row(&map, source_return)).unwrap(),
            &[Endpoint::Operation(Coordinate::Terminator {
                function: 0,
                block: 0
            })]
        );
        assert_eq!(
            row(&map, source_return).disposition(),
            KirOptimizationDispositionV12::Moved
        );
        // Source coordinates use physical ordinals, not source BlockId(7/99).
        assert!(map.data.relations.iter().all(|row| !matches!(
            row.source,
            Coordinate::Operation { block: 7 | 99, .. }
                | Coordinate::Terminator { block: 7 | 99, .. }
        )));
        assert_eq!(input.module(), &source);
        let (_, replay_report, replay_map) = run(&input);
        assert_eq!(report, replay_report);
        assert_eq!(map, replay_map);
        assert_ne!(input.canonical().identity(), output.canonical().identity());
    }

    #[test]
    fn forged_coverage_lifecycle_relation_and_epoch_offsets_are_rejected() {
        let input = owner(&source());
        for alteration in 0..6 {
            let (output, report, mut map) = run(&input);
            match alteration {
                0 => {
                    map.data.relations[0].disposition = KirOptimizationDispositionV12::Eliminated;
                }
                1 => {
                    map.data.terminal[0] = None;
                }
                2 => {
                    map.data.events.push(Event {
                        pass: 6,
                        change: Change::Erase(u32::MAX),
                    });
                    map.data.passes[6].end += 1;
                }
                3 => {
                    map.data.nodes[0].input = None;
                }
                4 => {
                    for pass in &mut map.data.passes {
                        pass.input_epoch += 1;
                        pass.output_epoch += 1;
                    }
                }
                _ => {
                    map.data.digest[0] ^= 1;
                }
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(7).unwrap();
            if alteration == 4 {
                // A uniformly shifted live epoch chain is not bound to the
                // actual execution, even though its relative shape is valid.
                assert!(!map.matches_execution(&report));
            }
            assert!(map.check_against(&input, &output, &mut budget).is_err());
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn exact_and_one_under_checker_budgets_preserve_seeded_prefixes() {
        let input = owner(&Module::new("m"));
        assert_eq!(input.canonical().canonical_bytes().len(), 37);
        let (output, _, map) = run(&input);
        // Independently: N=2*37+64=138, E=T=8*N=1104.
        // The frozen capacity/storage profile is unchanged: storage=216064.
        // Actual empty replay census costs 3, and check envelope=128*(1+1)=256.
        for (work_allowance, storage_allowance) in [(258, 216_064), (259, 216_063), (259, 216_064)]
        {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + work_allowance);
            work.charge_work(11).unwrap();
            let mut budget = Budget::new(&mut work, 7 + storage_allowance);
            budget.reserve_storage(7).unwrap();
            assert!(budget.reserve_storage(usize::MAX).is_err());
            assert!(budget.charge_work(usize::MAX).is_err());
            let result = map.check_against(&input, &output, &mut budget);
            match (work_allowance, storage_allowance) {
                (258, _) => assert!(matches!(result,
                    Err(KirOptimizationMapErrorV12::Resources(ResourceError::Work(error)))
                    if error.actual() == 270 && error.limit() == 269)),
                (_, 216_063) => assert!(matches!(result,
                    Err(KirOptimizationMapErrorV12::Resources(ResourceError::Storage(error)))
                    if error.actual() == 216_071 && error.limit() == 216_070)),
                _ => result.unwrap(),
            }
            assert_eq!(budget.storage(), 7);
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
            assert_eq!(budget.work(), if work_allowance == 258 { 14 } else { 270 });
            assert_eq!(work.failed_work(), Some(usize::MAX));
        }
    }

    #[test]
    fn identity_denial_is_allocation_free_and_retained_sources_are_independent() {
        let mut module = source();
        let input = owner(&module);
        let (output, _, map) = run(&input);
        module.id = "changed-source".into();
        let wrong = owner(&module);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(11);
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, 7);
        budget.reserve_storage(7).unwrap();
        assert_eq!(
            map.check_against(&wrong, &output, &mut budget),
            Err(KirOptimizationMapErrorV12::Identity)
        );
        assert_eq!(budget.work(), 11);
        assert_eq!(budget.storage(), 7);
        assert_eq!(map.input_identity(), input.canonical().identity());
    }

    #[test]
    fn capped_trace_cannot_revive_an_erased_key_or_infer_a_cycle_as_elimination() {
        let ctx = &mut Context::new();
        let op = pliron::builtin::ops::ModuleOp::new(ctx, "r".try_into().unwrap());
        use pliron::op::Op;
        let key = LiveKeyV12::Operation(op.get_operation());
        let capture = CaptureV12::new(
            CaptureLimitsV12 {
                nodes: 2,
                events: 2,
                targets: 2,
            },
            &vec![(key, Endpoint::Operation(coord(0)))],
        )
        .unwrap();
        let mut state = capture.0.lock().unwrap();
        state.current = Some(PassSpan {
            pass: KIR_PLIRON_PRODUCTION_PASSES_V12[0],
            input_epoch: 1,
            output_epoch: 1,
            start: 0,
            end: 0,
        });
        state.erase(0).unwrap();
        assert_eq!(
            state.register(key, None),
            Err(KirOptimizationMapErrorV12::Lifecycle)
        );
        assert_eq!(state.nodes.len(), 1);
        let nodes = [
            Node {
                kind: Kind::Operation,
                input: Some(Endpoint::Operation(coord(0))),
                result_index: None,
            },
            Node {
                kind: Kind::Operation,
                input: Some(Endpoint::Operation(coord(1))),
                result_index: None,
            },
        ];
        let events = [
            Event {
                pass: 0,
                change: Change::Replace(0, 1),
            },
            Event {
                pass: 0,
                change: Change::Replace(1, 0),
            },
        ];
        assert!(matches!(
            derive_relations(&nodes, &events, &[None, None], 2),
            Err(KirOptimizationMapErrorV12::Relation)
        ));
        state.event(Change::Move(0)).unwrap();
        assert_eq!(
            state.event(Change::Move(0)),
            Err(KirOptimizationMapErrorV12::Limit)
        );
        assert_eq!(state.events.len(), 2);
    }
    #[test]
    fn mapping_storage_denial_after_changed_passes_drops_candidate_payload_before_restore() {
        let input = owner(&source());
        let source_copy = input.module().clone();
        let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let (mut graph, imported) = crate::KirPlironGraphV12::import(&input, &mut setup).unwrap();
        setup.reserve_storage(imported.retained_storage()).unwrap();
        let (report, execution) = graph
            .execute_production_optimization_v12(&mut setup)
            .unwrap();
        setup.reserve_storage(execution.retained_storage()).unwrap();
        assert!(report.passes().iter().any(|pass| pass.changed()));
        let (baseline, bridge, extracted) = graph
            .extract_optimized_canonical_kir_module_v12(&mut setup)
            .unwrap();
        // The existing connected extractor's returned receipt is observed, not
        // called an independently derived global storage oracle. The extra
        // mapping reservation itself is independently derived from source B.
        // Transfer the preexisting live owners to the seeded phase ledger.
        let b = input.canonical().canonical_bytes().len();
        let n = (2 * b + 64).min(131_072);
        let map_scratch = 1536 * n + 4096;
        let graph_storage = graph.retained_storage();
        let floor = 7 + graph_storage + execution.retained_storage() + extracted.retained_storage();
        let denied_total = floor + extracted.retained_storage() + map_scratch;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, denied_total - 1);
        budget.reserve_storage(floor).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(budget.charge_work(usize::MAX).is_err());
        let error = graph
            .extract_optimized_canonical_kir_module_with_map_v12(&mut budget)
            .unwrap_err();
        assert!(matches!(error, crate::KirMappedExtractionErrorV12::Mapping(
            KirOptimizationMapErrorV12::Resources(ResourceError::Storage(error)))
            if error.actual() == denied_total && error.limit() == denied_total - 1));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
        assert_eq!(input.module(), &source_copy);
        assert!(graph.optimization_started);
        assert_eq!(work.failed_work(), Some(usize::MAX));
        // A retry does not rerun transformations or rely on a damaged partial map.
        let mut retry_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut retry = Budget::new(&mut retry_work, STORAGE);
        retry.reserve_storage(floor).unwrap();
        let (fresh, fresh_bridge, mapping, receipt) = graph
            .extract_optimized_canonical_kir_module_with_map_v12(&mut retry)
            .unwrap();
        assert_eq!(fresh, baseline);
        assert_eq!(fresh_bridge, bridge);
        assert!(mapping.matches_execution(&report));
        assert_eq!(retry.storage(), floor);
        drop((fresh, fresh_bridge, mapping, receipt));
        drop(graph);
        retry.release_storage(graph_storage).unwrap();
        drop((baseline, bridge, report));
        retry
            .release_storage(extracted.retained_storage() + execution.retained_storage())
            .unwrap();
        assert_eq!(retry.storage(), 7);
    }

    #[test]
    fn surviving_producer_keeps_observed_result_replacement_chains_without_unchanged_results() {
        let nodes = [
            Node {
                kind: Kind::Operation,
                input: Some(Endpoint::Operation(coord(0))),
                result_index: None,
            },
            Node {
                kind: Kind::Value { producer: Some(0) },
                input: Some(Endpoint::Result {
                    operation: coord(0),
                    result: 0,
                }),
                result_index: Some(0),
            },
            Node {
                kind: Kind::Value { producer: Some(0) },
                input: Some(Endpoint::Result {
                    operation: coord(0),
                    result: 1,
                }),
                result_index: Some(1),
            },
            Node {
                kind: Kind::Operation,
                input: None,
                result_index: None,
            },
            Node {
                kind: Kind::Value { producer: Some(3) },
                input: None,
                result_index: Some(0),
            },
            Node {
                kind: Kind::Operation,
                input: None,
                result_index: None,
            },
            Node {
                kind: Kind::Value { producer: Some(5) },
                input: None,
                result_index: Some(0),
            },
        ];
        let events = [
            Event {
                pass: 0,
                change: Change::Replace(1, 4),
            },
            Event {
                pass: 1,
                change: Change::Replace(4, 6),
            },
        ];
        let terminal = [
            Some(Endpoint::Operation(coord(0))),
            Some(Endpoint::Result {
                operation: coord(0),
                result: 0,
            }),
            Some(Endpoint::Result {
                operation: coord(0),
                result: 1,
            }),
            Some(Endpoint::Operation(coord(1))),
            Some(Endpoint::Result {
                operation: coord(1),
                result: 0,
            }),
            Some(Endpoint::Operation(coord(2))),
            Some(Endpoint::Result {
                operation: coord(2),
                result: 0,
            }),
        ];
        let (rows, targets, synthesized) = derive_relations(&nodes, &events, &terminal, 3).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].disposition(),
            KirOptimizationDispositionV12::Retained
        );
        assert!(rows[0].identity_survived());
        assert_eq!(
            targets,
            [
                Endpoint::Operation(coord(0)),
                Endpoint::Result {
                    operation: coord(1),
                    result: 0
                },
                Endpoint::Result {
                    operation: coord(2),
                    result: 0
                },
            ]
        );
        assert!(synthesized.is_empty());
        assert_eq!(
            derive_relations(&nodes, &events, &terminal, 2),
            Err(KirOptimizationMapErrorV12::Limit)
        );
    }

    #[test]
    fn sccp_keeps_effectful_producers_and_observed_materialized_result_provenance() {
        for (operator, lhs, rhs, folded) in [(BinaryOp::Add, 4, 5, 9), (BinaryOp::Divide, 20, 4, 5)]
        {
            let ty = Type::Scalar(ScalarType::U32);
            let mut block = BasicBlock::new(BlockId(42));
            for (id, value) in [(0, lhs), (1, rhs)] {
                block.operations.push(KirOperation::effect_free(
                    ValueDef::new(ValueId(id), ty.clone()),
                    OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(value)),
                ));
            }
            block.operations.push(KirOperation::effect_free(
                ValueDef::new(ValueId(2), ty.clone()),
                OperationKind::Binary {
                    op: operator,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            ));
            block.terminator = Some(Terminator::Return {
                values: vec![ValueId(2)],
            });
            let mut module = Module::new("fold");
            module.functions.push(Function::internal_helper(
                "f",
                Signature::new(vec![], vec![ty]),
                vec![],
                vec![block],
            ));
            let input = owner(&module);
            // run performs the full live capture and independent map checker.
            let (output, _, map) = run(&input);
            let operations =
                &output.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
            let retained = operations
                .iter()
                .enumerate()
                .filter_map(|(index, operation)| {
                    matches!(&operation.kind, OperationKind::Binary { op, .. } if *op == operator)
                        .then_some(coord(u32::try_from(index).unwrap()))
                })
                .collect::<Vec<_>>();
            let materialized = operations.iter().enumerate().filter_map(|(index, operation)| {
                matches!(&operation.kind,
                    OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(value)) if *value == folded)
                    .then_some(coord(u32::try_from(index).unwrap()))
            }).collect::<Vec<_>>();
            assert_eq!(retained.len(), 1, "the arithmetic effect must survive DCE");
            assert_eq!(materialized.len(), 1);
            let relation = row(&map, coord(2));
            assert!(relation.identity_survived());
            assert_eq!(relation.moved(), retained[0] != coord(2));
            assert_eq!(
                relation.disposition(),
                if relation.moved() {
                    KirOptimizationDispositionV12::Moved
                } else {
                    KirOptimizationDispositionV12::Retained
                }
            );
            assert_eq!(
                map.targets(relation).unwrap(),
                [
                    Endpoint::Operation(retained[0]),
                    Endpoint::Result {
                        operation: materialized[0],
                        result: 0
                    },
                ]
            );
            assert!(!map.synthesized_operations().contains(&materialized[0]));
            // Operand constants remain used by the conservatively retained op.
            assert!(row(&map, coord(0)).identity_survived());
            assert!(row(&map, coord(1)).identity_survived());
            assert!(map.data.events.iter().any(|event| matches!(event.change,
                Change::Replace(a, b) if map.data.nodes[a as usize].input ==
                    Some(Endpoint::Result { operation: coord(2), result: 0 })
                    && map.data.nodes[b as usize].input.is_none())));
        }
    }
}
