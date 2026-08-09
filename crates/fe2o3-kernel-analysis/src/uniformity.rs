use crate::{AnalysisReport, Diagnostic, UnsupportedReason, Variation};
use fe2o3_kernel_ir::{
    AddressSpace, BasicBlock, BlockId, Function, FunctionBody, IndexKind, IntrinsicKind, Operation,
    OperationKind, Terminator, ValueId, WaveOperationKind,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Conservatively classifies SSA values and barrier control in one function.
///
/// The caller should run the kernel IR verifier first. This function still
/// fails closed for malformed input: unknown values become [`Variation::Varying`]
/// and produce diagnostics. Function parameters are varying because kernel IR
/// v1 has no uniform-argument metadata. Loads and atomic results are varying
/// because it has no immutable-region or inter-thread value summaries. Calls
/// are unsupported because it has no convergence or return-value summaries.
/// Postdominance is established only for structurally terminating CFG regions;
/// cycles are fail-closed until a loop proof can establish compatible dynamic
/// barrier counts and order. Even for accepted acyclic control flow, this pass
/// checks barrier reachability only. Compatible order among distinct dynamic
/// barrier instances remains an unsupported obligation.
///
/// The returned report is analysis evidence only and grants no assurance.
pub fn analyze_function(function: &Function) -> AnalysisReport {
    let mut report = AnalysisReport {
        function: function.id.clone(),
        values: BTreeMap::new(),
        block_controls: BTreeMap::new(),
        diagnostics: Vec::new(),
    };
    let Some(body) = &function.body else {
        report.diagnostics.push(Diagnostic::Unsupported {
            block: None,
            operation_index: None,
            reason: UnsupportedReason::FunctionDeclaration,
        });
        return report;
    };

    Analyzer::new(body, report).run()
}

struct Analyzer<'a> {
    body: &'a FunctionBody,
    reachable: BTreeSet<BlockId>,
    incoming: BTreeMap<BlockId, Vec<Edge>>,
    control_regions: BTreeMap<BlockId, BTreeSet<BlockId>>,
    control_unknown: BTreeSet<BlockId>,
    report: AnalysisReport,
}

#[derive(Clone, Debug)]
struct Edge {
    source: BlockId,
    arguments: Vec<ValueId>,
    discriminator: Option<ValueId>,
}

impl<'a> Analyzer<'a> {
    fn new(body: &'a FunctionBody, mut report: AnalysisReport) -> Self {
        let mut blocks = BTreeMap::new();
        let mut malformed = false;
        for block in &body.blocks {
            if blocks.insert(block.id, block).is_some() {
                malformed = true;
            }
        }
        if malformed || body.blocks.is_empty() {
            report.diagnostics.push(Diagnostic::Unsupported {
                block: None,
                operation_index: None,
                reason: UnsupportedReason::MalformedControlFlow,
            });
        }

        let reachable = reachable_blocks(body, &blocks);
        let (incoming, malformed_edges) = incoming_edges(body, &blocks, &reachable);
        if malformed_edges {
            report.diagnostics.push(Diagnostic::Unsupported {
                block: None,
                operation_index: None,
                reason: UnsupportedReason::MalformedControlFlow,
            });
        }
        let postdominance_available = postdominance_available(&blocks, &reachable);
        let control_unknown = reachable
            .difference(&postdominance_available)
            .copied()
            .collect::<BTreeSet<_>>();
        if !control_unknown.is_empty() {
            report.diagnostics.push(Diagnostic::Unsupported {
                block: None,
                operation_index: None,
                reason: UnsupportedReason::PostdominanceUnavailable {
                    blocks: control_unknown.iter().copied().collect(),
                },
            });
        }
        let control_regions = control_regions(body, &blocks, &reachable, &postdominance_available);

        Self {
            body,
            reachable,
            incoming,
            control_regions,
            control_unknown,
            report,
        }
    }

    fn run(mut self) -> AnalysisReport {
        self.collect_unsupported_diagnostics();
        self.initialize_facts();
        self.solve();
        self.diagnose_barriers();
        self.report
    }

    fn collect_unsupported_diagnostics(&mut self) {
        let mut defined = BTreeSet::new();
        defined.extend(self.body.parameters.iter().copied());
        for block in &self.body.blocks {
            defined.extend(block.parameters.iter().map(|parameter| parameter.id));
            defined.extend(
                block
                    .operations
                    .iter()
                    .flat_map(|operation| operation.results.iter().map(|result| result.id)),
            );
        }

        let mut unknown = BTreeSet::new();
        for block in &self.body.blocks {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                if let OperationKind::Call { callee, .. } = &operation.kind {
                    self.report.diagnostics.push(Diagnostic::Unsupported {
                        block: Some(block.id),
                        operation_index: Some(operation_index),
                        reason: UnsupportedReason::CallWithoutSummary {
                            callee: callee.clone(),
                        },
                    });
                }
                unknown.extend(
                    operation
                        .kind
                        .operands()
                        .into_iter()
                        .filter(|value| !defined.contains(value)),
                );
            }
            if let Some(terminator) = &block.terminator {
                unknown.extend(
                    terminator
                        .operands()
                        .into_iter()
                        .filter(|value| !defined.contains(value)),
                );
            }
        }
        self.report
            .diagnostics
            .extend(unknown.into_iter().map(|value| Diagnostic::Unsupported {
                block: None,
                operation_index: None,
                reason: UnsupportedReason::UnknownValue { value },
            }));
    }

    fn initialize_facts(&mut self) {
        for parameter in &self.body.parameters {
            self.report.values.insert(*parameter, Variation::Varying);
        }
        for block in &self.body.blocks {
            let reachable = self.reachable.contains(&block.id);
            let initial = if reachable {
                Variation::GridUniform
            } else {
                Variation::Varying
            };
            if reachable {
                let control = if self.control_unknown.contains(&block.id) {
                    Variation::Varying
                } else {
                    Variation::GridUniform
                };
                self.report.block_controls.insert(block.id, control);
            }
            for parameter in &block.parameters {
                self.report.values.insert(parameter.id, initial);
            }
            for result in block
                .operations
                .iter()
                .flat_map(|operation| &operation.results)
            {
                self.report.values.insert(result.id, initial);
            }
        }
    }

    fn solve(&mut self) {
        loop {
            let mut changed = self.update_block_controls();
            for block in &self.body.blocks {
                if !self.reachable.contains(&block.id) {
                    continue;
                }
                changed |= self.update_block_parameters(block);
                for operation in &block.operations {
                    let variation = self.operation_variation(operation);
                    for result in &operation.results {
                        changed |= raise(&mut self.report.values, result.id, variation);
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn update_block_controls(&mut self) -> bool {
        let mut next = self
            .reachable
            .iter()
            .map(|block| {
                let control = if self.control_unknown.contains(block) {
                    Variation::Varying
                } else {
                    Variation::GridUniform
                };
                (*block, control)
            })
            .collect::<BTreeMap<_, _>>();

        for block in &self.body.blocks {
            if !self.reachable.contains(&block.id) {
                continue;
            }
            let Some(discriminator) = block.terminator.as_ref().and_then(discriminator) else {
                continue;
            };
            let source_control = self
                .report
                .block_controls
                .get(&block.id)
                .copied()
                .unwrap_or(Variation::Varying);
            let branch_control = source_control.join(self.value(discriminator));
            if let Some(region) = self.control_regions.get(&block.id) {
                for controlled in region {
                    raise(&mut next, *controlled, branch_control);
                }
            }
        }

        let mut changed = false;
        for (block, variation) in next {
            changed |= raise(&mut self.report.block_controls, block, variation);
        }
        changed
    }

    fn update_block_parameters(&mut self, block: &BasicBlock) -> bool {
        let Some(edges) = self.incoming.get(&block.id).cloned() else {
            let mut changed = false;
            for parameter in &block.parameters {
                changed |= raise(&mut self.report.values, parameter.id, Variation::Varying);
            }
            return changed;
        };

        let mut changed = false;
        for (index, parameter) in block.parameters.iter().enumerate() {
            let mut variation = Variation::GridUniform;
            for edge in &edges {
                let argument = edge
                    .arguments
                    .get(index)
                    .map(|value| self.value(*value))
                    .unwrap_or(Variation::Varying);
                let source_control = self
                    .report
                    .block_controls
                    .get(&edge.source)
                    .copied()
                    .unwrap_or(Variation::Varying);
                let edge_control = edge
                    .discriminator
                    .map(|value| self.value(value))
                    .unwrap_or(Variation::GridUniform);
                variation = variation.join(argument.join(source_control).join(edge_control));
            }
            changed |= raise(&mut self.report.values, parameter.id, variation);
        }
        changed
    }

    fn operation_variation(&self, operation: &Operation) -> Variation {
        match &operation.kind {
            OperationKind::Constant(_) => Variation::GridUniform,
            OperationKind::Intrinsic(intrinsic) => match intrinsic.kind {
                IntrinsicKind::LaunchExtent { .. } => Variation::GridUniform,
                IntrinsicKind::InvocationIndex { kind, .. } => match kind {
                    IndexKind::Global | IndexKind::Local => Variation::Varying,
                    IndexKind::Workgroup => Variation::WorkgroupUniform,
                    IndexKind::WorkgroupSize | IndexKind::WorkgroupCount => Variation::GridUniform,
                },
            },
            OperationKind::Call { .. } | OperationKind::Load { .. } | OperationKind::Atomic(_) => {
                Variation::Varying
            }
            OperationKind::MemoryIntrinsic(
                fe2o3_kernel_ir::MemoryIntrinsicOperation::PointerDistance { .. },
            ) => join_values(operation.kind.operands(), &self.report.values),
            OperationKind::MemoryIntrinsic(_) => Variation::Varying,
            OperationKind::InlineAssembly(_) => Variation::Varying,
            OperationKind::Alloca {
                count,
                address_space,
                ..
            } => {
                let allocation = match address_space {
                    AddressSpace::Workgroup => Variation::WorkgroupUniform,
                    AddressSpace::Private
                    | AddressSpace::Global
                    | AddressSpace::Constant
                    | AddressSpace::Generic => Variation::Varying,
                };
                allocation.join(join_values(count.iter().copied(), &self.report.values))
            }
            OperationKind::WorkgroupMemory(_) => Variation::WorkgroupUniform,
            OperationKind::Wave(wave) => match wave.kind {
                WaveOperationKind::LaneId => Variation::Varying,
                WaveOperationKind::Ballot { predicate }
                | WaveOperationKind::Any { predicate }
                | WaveOperationKind::All { predicate } => {
                    subgroup_collective_variation(self.value(predicate))
                }
                WaveOperationKind::ShuffleIndex {
                    value, source_lane, ..
                } => {
                    let value = self.value(value);
                    if value.is_uniform_for(fe2o3_kernel_ir::SynchronizationScope::Subgroup) {
                        value
                    } else if self
                        .value(source_lane)
                        .is_uniform_for(fe2o3_kernel_ir::SynchronizationScope::Subgroup)
                    {
                        Variation::SubgroupUniform
                    } else {
                        Variation::Varying
                    }
                }
            },
            OperationKind::Store { .. }
            | OperationKind::Barrier(_)
            | OperationKind::Fence(_)
            | OperationKind::WorkgroupBarrier(_) => Variation::Varying,
            OperationKind::Unary { .. }
            | OperationKind::Binary { .. }
            | OperationKind::Compare { .. }
            | OperationKind::Cast { .. }
            | OperationKind::Select { .. }
            | OperationKind::SliceLength { .. }
            | OperationKind::SliceData { .. }
            | OperationKind::GetElementPointer { .. } => {
                join_values(operation.kind.operands(), &self.report.values)
            }
        }
    }

    fn diagnose_barriers(&mut self) {
        for block in &self.body.blocks {
            if !self.reachable.contains(&block.id) {
                continue;
            }
            let control = self
                .report
                .block_controls
                .get(&block.id)
                .copied()
                .unwrap_or(Variation::Varying);
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let execution_scope = match &operation.kind {
                    OperationKind::Barrier(barrier) => barrier.execution_scope,
                    OperationKind::WorkgroupBarrier(_) => {
                        fe2o3_kernel_ir::SynchronizationScope::Workgroup
                    }
                    _ => continue,
                };
                if !control.is_uniform_for(execution_scope) {
                    self.report.diagnostics.push(Diagnostic::DivergentBarrier {
                        block: block.id,
                        operation_index,
                        execution_scope,
                        control,
                    });
                }
            }
        }
    }

    fn value(&self, value: ValueId) -> Variation {
        self.report
            .values
            .get(&value)
            .copied()
            .unwrap_or(Variation::Varying)
    }
}

fn subgroup_collective_variation(input: Variation) -> Variation {
    if input.is_uniform_for(fe2o3_kernel_ir::SynchronizationScope::Subgroup) {
        input
    } else {
        Variation::SubgroupUniform
    }
}

fn raise<K: Ord + Copy>(facts: &mut BTreeMap<K, Variation>, key: K, value: Variation) -> bool {
    let current = facts.get(&key).copied().unwrap_or(Variation::GridUniform);
    let joined = current.join(value);
    if joined == current {
        false
    } else {
        facts.insert(key, joined);
        true
    }
}

fn join_values(
    values: impl IntoIterator<Item = ValueId>,
    facts: &BTreeMap<ValueId, Variation>,
) -> Variation {
    values
        .into_iter()
        .map(|value| facts.get(&value).copied().unwrap_or(Variation::Varying))
        .fold(Variation::GridUniform, Variation::join)
}

fn discriminator(terminator: &Terminator) -> Option<ValueId> {
    match terminator {
        Terminator::ConditionalBranch { condition, .. } => Some(*condition),
        Terminator::Switch { selector, .. } | Terminator::IntegerSwitch { selector, .. } => {
            Some(*selector)
        }
        Terminator::Branch { .. } | Terminator::Return { .. } | Terminator::Unreachable => None,
    }
}

fn reachable_blocks(
    body: &FunctionBody,
    blocks: &BTreeMap<BlockId, &BasicBlock>,
) -> BTreeSet<BlockId> {
    let Some(entry) = body.blocks.first() else {
        return BTreeSet::new();
    };
    let mut reachable = BTreeSet::new();
    let mut pending = VecDeque::from([entry.id]);
    while let Some(block_id) = pending.pop_front() {
        if !reachable.insert(block_id) {
            continue;
        }
        let Some(block) = blocks.get(&block_id) else {
            continue;
        };
        if let Some(terminator) = &block.terminator {
            for successor in terminator.successors() {
                if blocks.contains_key(&successor) && !reachable.contains(&successor) {
                    pending.push_back(successor);
                }
            }
        }
    }
    reachable
}

fn incoming_edges(
    body: &FunctionBody,
    blocks: &BTreeMap<BlockId, &BasicBlock>,
    reachable: &BTreeSet<BlockId>,
) -> (BTreeMap<BlockId, Vec<Edge>>, bool) {
    let mut incoming = BTreeMap::<BlockId, Vec<Edge>>::new();
    let mut malformed = false;
    for block in &body.blocks {
        if !reachable.contains(&block.id) {
            continue;
        }
        let Some(terminator) = &block.terminator else {
            malformed = true;
            continue;
        };
        let edge_discriminator = discriminator(terminator);
        for (target, arguments) in terminator_edges(terminator) {
            if !blocks.contains_key(&target) {
                malformed = true;
                continue;
            }
            incoming.entry(target).or_default().push(Edge {
                source: block.id,
                arguments,
                discriminator: edge_discriminator,
            });
        }
    }
    (incoming, malformed)
}

fn terminator_edges(terminator: &Terminator) -> Vec<(BlockId, Vec<ValueId>)> {
    match terminator {
        Terminator::Branch { target, arguments } => vec![(*target, arguments.clone())],
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => vec![
            (*then_target, then_arguments.clone()),
            (*else_target, else_arguments.clone()),
        ],
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => cases
            .iter()
            .map(|case| (case.target, case.arguments.clone()))
            .chain([(*default_target, default_arguments.clone())])
            .collect(),
        Terminator::IntegerSwitch {
            cases,
            default_target,
            default_arguments,
            ..
        } => cases
            .iter()
            .map(|case| (case.target, case.arguments.clone()))
            .chain([(*default_target, default_arguments.clone())])
            .collect(),
        Terminator::Return { .. } | Terminator::Unreachable => Vec::new(),
    }
}

fn control_regions(
    body: &FunctionBody,
    blocks: &BTreeMap<BlockId, &BasicBlock>,
    reachable: &BTreeSet<BlockId>,
    postdominance_available: &BTreeSet<BlockId>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let postdominators = postdominators(blocks, postdominance_available);
    let mut regions = BTreeMap::new();
    for block in &body.blocks {
        if !reachable.contains(&block.id) {
            continue;
        }
        let Some(terminator) = &block.terminator else {
            continue;
        };
        if discriminator(terminator).is_none() {
            continue;
        }
        let stop = immediate_postdominator(block.id, &postdominators);
        let mut region = BTreeSet::new();
        let mut pending = VecDeque::from(terminator.successors());
        while let Some(candidate) = pending.pop_front() {
            if Some(candidate) == stop
                || !reachable.contains(&candidate)
                || !region.insert(candidate)
            {
                continue;
            }
            if let Some(candidate_block) = blocks.get(&candidate)
                && let Some(candidate_terminator) = &candidate_block.terminator
            {
                pending.extend(candidate_terminator.successors());
            }
        }
        regions.insert(block.id, region);
    }
    regions
}

fn postdominance_available(
    blocks: &BTreeMap<BlockId, &BasicBlock>,
    reachable: &BTreeSet<BlockId>,
) -> BTreeSet<BlockId> {
    let mut available = BTreeSet::new();
    loop {
        let mut changed = false;
        for block_id in reachable.iter().rev() {
            if available.contains(block_id) {
                continue;
            }
            let successors = blocks
                .get(block_id)
                .and_then(|block| block.terminator.as_ref())
                .map(Terminator::successors)
                .unwrap_or_default();
            if successors.is_empty()
                || successors
                    .iter()
                    .all(|successor| available.contains(successor))
            {
                available.insert(*block_id);
                changed = true;
            }
        }
        if !changed {
            return available;
        }
    }
}

fn postdominators(
    blocks: &BTreeMap<BlockId, &BasicBlock>,
    reachable: &BTreeSet<BlockId>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let all = reachable.clone();
    let mut facts = reachable
        .iter()
        .map(|block| (*block, all.clone()))
        .collect::<BTreeMap<_, _>>();

    loop {
        let mut changed = false;
        for block_id in reachable.iter().rev() {
            let successors = blocks
                .get(block_id)
                .and_then(|block| block.terminator.as_ref())
                .map(Terminator::successors)
                .unwrap_or_default()
                .into_iter()
                .filter(|successor| reachable.contains(successor))
                .collect::<Vec<_>>();
            let mut next = if successors.is_empty() {
                BTreeSet::new()
            } else {
                let mut successor_sets = successors.iter().map(|successor| &facts[successor]);
                let mut intersection = successor_sets.next().cloned().unwrap_or_default();
                for successor_set in successor_sets {
                    intersection.retain(|candidate| successor_set.contains(candidate));
                }
                intersection
            };
            next.insert(*block_id);
            if facts.get(block_id) != Some(&next) {
                facts.insert(*block_id, next);
                changed = true;
            }
        }
        if !changed {
            return facts;
        }
    }
}

fn immediate_postdominator(
    block: BlockId,
    postdominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Option<BlockId> {
    let strict = postdominators
        .get(&block)?
        .iter()
        .copied()
        .filter(|candidate| *candidate != block)
        .collect::<Vec<_>>();
    strict.iter().copied().find(|candidate| {
        strict.iter().all(|other| {
            candidate == other
                || !postdominators
                    .get(other)
                    .is_some_and(|dominators| dominators.contains(candidate))
        })
    })
}
