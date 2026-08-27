use crate::{AnalysisReport, Diagnostic, UnsupportedReason, Variation};
use fe2o3_kernel_ir::{
    AddressSpace, AmdGpuDiagnosticOperation, Axis, BasicBlock, BinaryOp, BlockId, CastKind,
    CheckedBinaryOperator, ComparePredicate, Constant, FloatOperation, Function, FunctionBody,
    IndexKind, IntrinsicKind, Module, Operation, OperationKind, ScalarType, Terminator, Type,
    UnaryOp, ValueId, WaveOperationKind, WorkgroupSize,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Conservatively classifies SSA values and barrier control in one function.
///
/// The caller should run the kernel IR verifier first. This function still
/// fails closed for malformed input: unknown values become [`Variation::Varying`]
/// and produce diagnostics. Function parameters are varying because kernel IR
/// v1 has no uniform-argument metadata. Loads and atomic results are varying
/// because it has no immutable-region or inter-thread value summaries. Calls
/// are unsupported unless they are closed compiler intrinsics or reachable
/// helpers whose bodies can be summarized as pure.
/// Postdominance is established only for CFG regions with a path to an exit.
/// Divergent loop-exiting control remains active beyond the loop, while
/// branches that reconverge within the same natural-loop nest use their normal
/// postdominator. This pass checks barrier reachability only; compatible order
/// among distinct dynamic barrier instances remains an unsupported obligation.
///
/// The returned report is analysis evidence only and grants no assurance.
pub fn analyze_function(function: &Function) -> AnalysisReport {
    analyze_function_with_contract(function, &[], &BTreeSet::new(), None)
}

/// Classifies one kernel entry using uniform ABI parameters and conservative
/// summaries for reachable pure helpers.
pub fn analyze_kernel_entry(module: &Module, function: &Function) -> AnalysisReport {
    let mut summarized_calls = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    if let Some(body) = &function.body {
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            if let OperationKind::Call { callee, arguments } = &operation.kind {
                if FloatOperation::from_intrinsic_call(callee, arguments).is_some() {
                    summarized_calls.insert(callee.clone());
                } else {
                    summarize_pure_helper(module, callee, &mut visiting, &mut summarized_calls);
                }
            }
        }
    }
    let parameters = vec![Variation::GridUniform; function.signature.parameters.len()];
    let mut matching_contracts = module
        .kernels
        .iter()
        .filter(|kernel| kernel.entry == function.id)
        .map(|kernel| kernel.workgroup_size);
    let workgroup_size = matching_contracts
        .next()
        .filter(|first| matching_contracts.all(|contract| contract == *first))
        .flatten();
    analyze_function_with_contract(function, &parameters, &summarized_calls, workgroup_size)
}

fn analyze_function_with_contract(
    function: &Function,
    parameter_variations: &[Variation],
    summarized_calls: &BTreeSet<fe2o3_kernel_ir::FunctionId>,
    workgroup_size: Option<WorkgroupSize>,
) -> AnalysisReport {
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

    Analyzer::new(
        function,
        body,
        report,
        parameter_variations,
        summarized_calls,
        workgroup_size,
    )
    .run()
}

fn summarize_pure_helper(
    module: &Module,
    function: &fe2o3_kernel_ir::FunctionId,
    visiting: &mut BTreeSet<fe2o3_kernel_ir::FunctionId>,
    summarized: &mut BTreeSet<fe2o3_kernel_ir::FunctionId>,
) -> bool {
    if summarized.contains(function) {
        return true;
    }
    if !visiting.insert(function.clone()) {
        return false;
    }
    let accepted = module
        .function(function)
        .and_then(|function| function.body.as_ref())
        .is_some_and(|body| {
            body.blocks.iter().all(|block| {
                !matches!(block.terminator, Some(Terminator::Unreachable) | None)
                    && block
                        .operations
                        .iter()
                        .all(|operation| match &operation.kind {
                            OperationKind::Call { callee, arguments }
                                if FloatOperation::from_intrinsic_call(callee, arguments)
                                    .is_some() =>
                            {
                                true
                            }
                            OperationKind::Call { callee, .. } => {
                                summarize_pure_helper(module, callee, visiting, summarized)
                            }
                            OperationKind::Intrinsic(_)
                            | OperationKind::Atomic(_)
                            | OperationKind::Barrier(_)
                            | OperationKind::Fence(_)
                            | OperationKind::Matrix(_)
                            | OperationKind::InlineAssembly(_)
                            | OperationKind::Wave(_)
                            | OperationKind::WorkgroupBarrier(_)
                            | OperationKind::WorkgroupMemory(_) => false,
                            _ => operation.memory_effects().is_empty(),
                        })
            })
        });
    visiting.remove(function);
    if accepted {
        summarized.insert(function.clone());
    }
    accepted
}

struct Analyzer<'a> {
    body: &'a FunctionBody,
    reachable: BTreeSet<BlockId>,
    incoming: BTreeMap<BlockId, Vec<Edge>>,
    control_regions: BTreeMap<BlockId, BTreeSet<BlockId>>,
    control_unknown: BTreeSet<BlockId>,
    trivial_phi_representatives: BTreeMap<ValueId, ValueId>,
    private_load_slots: BTreeMap<ValueId, ValueId>,
    private_slot_stores: BTreeMap<ValueId, Vec<PrivateStore>>,
    proven_no_overflow: BTreeSet<ValueId>,
    parameter_variations: &'a [Variation],
    summarized_calls: &'a BTreeSet<fe2o3_kernel_ir::FunctionId>,
    workgroup_size: Option<WorkgroupSize>,
    value_definitions: BTreeMap<ValueId, &'a Operation>,
    report: AnalysisReport,
}

#[derive(Clone, Debug)]
struct Edge {
    source: BlockId,
    arguments: Vec<ValueId>,
    discriminator: Option<ValueId>,
}

#[derive(Clone, Copy, Debug)]
struct PrivateStore {
    block: BlockId,
    operation_index: usize,
    value: ValueId,
}

impl<'a> Analyzer<'a> {
    fn new(
        function: &'a Function,
        body: &'a FunctionBody,
        mut report: AnalysisReport,
        parameter_variations: &'a [Variation],
        summarized_calls: &'a BTreeSet<fe2o3_kernel_ir::FunctionId>,
        workgroup_size: Option<WorkgroupSize>,
    ) -> Self {
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
        let mut control_flow_malformed = false;
        let natural_loop_nests = match crate::analyze_control_flow(function) {
            Ok(control_flow) => reachable
                .iter()
                .map(|block| {
                    (
                        *block,
                        control_flow
                            .containing_natural_loops(*block)
                            .unwrap_or_default()
                            .iter()
                            .copied()
                            .collect::<BTreeSet<_>>(),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
            Err(_) => {
                control_flow_malformed = true;
                report.diagnostics.push(Diagnostic::Unsupported {
                    block: None,
                    operation_index: None,
                    reason: UnsupportedReason::MalformedControlFlow,
                });
                BTreeMap::new()
            }
        };
        let trivial_phi_representatives = trivial_phi_representatives(body, &incoming);
        let dominators = compute_dominators(body, &reachable, &incoming);
        let (private_load_slots, private_slot_stores) =
            private_storage_facts(body, &reachable, &dominators);
        let known_integer_values = KnownIntegerValueAnalysis::new(
            body,
            &incoming,
            &private_load_slots,
            &private_slot_stores,
        )
        .solve_terminator_selectors();
        let effective_successors = effective_successors(body, &known_integer_values);
        let postdominance_available =
            postdominance_available(&blocks, &reachable, &effective_successors);
        let mut control_unknown = reachable
            .difference(&postdominance_available)
            .copied()
            .collect::<BTreeSet<_>>();
        if control_flow_malformed {
            control_unknown.extend(reachable.iter().copied());
        }
        if !control_unknown.is_empty() {
            report.diagnostics.push(Diagnostic::Unsupported {
                block: None,
                operation_index: None,
                reason: UnsupportedReason::PostdominanceUnavailable {
                    blocks: control_unknown.iter().copied().collect(),
                },
            });
        }
        let control_regions = control_regions(
            body,
            &blocks,
            &reachable,
            &postdominance_available,
            &natural_loop_nests,
            &effective_successors,
        );
        let proven_no_overflow = prove_unsigned_checked_arithmetic(
            function,
            body,
            &reachable,
            &incoming,
            &dominators,
            &trivial_phi_representatives,
            &private_load_slots,
            &private_slot_stores,
        );
        let value_definitions = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .flat_map(|operation| {
                operation
                    .results
                    .iter()
                    .map(move |result| (result.id, operation))
            })
            .collect();

        Self {
            body,
            reachable,
            incoming,
            control_regions,
            control_unknown,
            trivial_phi_representatives,
            private_load_slots,
            private_slot_stores,
            proven_no_overflow,
            parameter_variations,
            summarized_calls,
            workgroup_size,
            value_definitions,
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
                if let OperationKind::Call { callee, arguments } = &operation.kind
                    && !self.summarized_calls.contains(callee)
                    && AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments).is_none()
                {
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
        for (index, parameter) in self.body.parameters.iter().enumerate() {
            self.report.values.insert(
                *parameter,
                self.parameter_variations
                    .get(index)
                    .copied()
                    .unwrap_or(Variation::Varying),
            );
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
                    for result in &operation.results {
                        let variation = self.operation_result_variation(operation, result.id);
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
            let distinct_origins = edges
                .iter()
                .filter_map(|edge| edge.arguments.get(index))
                .map(|value| self.trivial_phi_representative(*value))
                .collect::<BTreeSet<_>>();
            let control_can_select_value = distinct_origins.len() != 1;
            let mut variation = Variation::GridUniform;
            for edge in &edges {
                let argument = edge
                    .arguments
                    .get(index)
                    .map(|value| self.value(*value))
                    .unwrap_or(Variation::Varying);
                variation = variation.join(argument);
                if control_can_select_value {
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
                    variation = variation.join(source_control).join(edge_control);
                }
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
            OperationKind::Call { callee, .. } if self.summarized_calls.contains(callee) => {
                join_values(operation.kind.operands(), &self.report.values)
            }
            OperationKind::Load { .. } => self
                .private_load_variation(operation)
                .unwrap_or(Variation::Varying),
            OperationKind::Call { .. }
            | OperationKind::GuardedLoad { .. }
            | OperationKind::Atomic(_) => Variation::Varying,
            OperationKind::MemoryIntrinsic(
                fe2o3_kernel_ir::MemoryIntrinsicOperation::PointerDistance { .. },
            ) => join_values(operation.kind.operands(), &self.report.values),
            OperationKind::MemoryIntrinsic(_) => Variation::Varying,
            OperationKind::InlineAssembly(_) => Variation::Varying,
            OperationKind::Matrix(_) => Variation::Varying,
            OperationKind::Gfx950LdsTranspose(transpose) => match transpose.kind {
                fe2o3_kernel_ir::Gfx950LdsTransposeOperationKindV1::Current { .. }
                | fe2o3_kernel_ir::Gfx950LdsTransposeOperationKindV1::Stage { .. }
                | fe2o3_kernel_ir::Gfx950LdsTransposeOperationKindV1::Publish { .. } => {
                    Variation::WorkgroupUniform
                }
                fe2o3_kernel_ir::Gfx950LdsTransposeOperationKindV1::Read { .. } => {
                    Variation::Varying
                }
            },
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
                WaveOperationKind::ReduceF32 {
                    value, tile_width, ..
                } => {
                    let value = self.value(value);
                    if value.is_uniform_for(fe2o3_kernel_ir::SynchronizationScope::Subgroup) {
                        value
                    } else if tile_width == wave.width.lanes() {
                        Variation::SubgroupUniform
                    } else {
                        Variation::Varying
                    }
                }
                WaveOperationKind::BroadcastF32 {
                    value, tile_width, ..
                } => {
                    let value = self.value(value);
                    if value.is_uniform_for(fe2o3_kernel_ir::SynchronizationScope::Subgroup) {
                        value
                    } else if tile_width == wave.width.lanes() {
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
            OperationKind::Binary {
                op: BinaryOp::Divide,
                lhs,
                rhs,
            } if self.is_workgroup_index_quotient(*lhs, *rhs) => Variation::WorkgroupUniform,
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

    fn is_workgroup_index_quotient(&self, lhs: ValueId, rhs: ValueId) -> bool {
        let Some(workgroup_size) = self.workgroup_size else {
            return false;
        };
        let Some(Operation {
            kind:
                OperationKind::Intrinsic(fe2o3_kernel_ir::IntrinsicOperation {
                    kind:
                        IntrinsicKind::InvocationIndex {
                            kind: IndexKind::Global,
                            axis,
                        },
                    ..
                }),
            ..
        }) = self.value_definitions.get(&lhs).copied()
        else {
            return false;
        };
        let extent = match axis {
            Axis::X => workgroup_size.x,
            Axis::Y => workgroup_size.y,
            Axis::Z => workgroup_size.z,
        };
        extent != 0 && self.unsigned_constant(rhs) == Some(u64::from(extent))
    }

    fn unsigned_constant(&self, value: ValueId) -> Option<u64> {
        let operation = self.value_definitions.get(&value).copied()?;
        match &operation.kind {
            OperationKind::Constant(constant) => match constant {
                Constant::U8(value) => Some(u64::from(*value)),
                Constant::U16(value) => Some(u64::from(*value)),
                Constant::U32(value) => Some(u64::from(*value)),
                Constant::U64(value) | Constant::Index(value) => Some(*value),
                Constant::I8(value) => u64::try_from(*value).ok(),
                Constant::I16(value) => u64::try_from(*value).ok(),
                Constant::I32(value) => u64::try_from(*value).ok(),
                Constant::I64(value) => u64::try_from(*value).ok(),
                Constant::Bool(_)
                | Constant::F16Bits(_)
                | Constant::Bf16Bits(_)
                | Constant::F32Bits(_)
                | Constant::F64Bits(_) => None,
            },
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value,
                to,
            } if self.zero_extend_preserves_unsigned_value(*value, to) => {
                self.unsigned_constant(*value)
            }
            _ => None,
        }
    }

    fn zero_extend_preserves_unsigned_value(&self, value: ValueId, to: &Type) -> bool {
        let Some(source) = self
            .value_definitions
            .get(&value)
            .and_then(|operation| operation.results.first())
            .map(|result| &result.ty)
        else {
            return false;
        };
        let (Type::Scalar(source), Type::Scalar(destination)) = (source, to) else {
            return false;
        };
        let source_width = match source {
            ScalarType::U8 => 8,
            ScalarType::U16 => 16,
            ScalarType::U32 => 32,
            ScalarType::U64 => 64,
            ScalarType::U128
            | ScalarType::Index
            | ScalarType::Bool
            | ScalarType::I8
            | ScalarType::I16
            | ScalarType::I32
            | ScalarType::I64
            | ScalarType::I128
            | ScalarType::F16
            | ScalarType::Bf16
            | ScalarType::F32
            | ScalarType::F64 => return false,
        };
        match destination {
            ScalarType::U16 => source_width < 16,
            ScalarType::U32 => source_width < 32,
            ScalarType::U64 => source_width < 64,
            ScalarType::U128 => source_width < 128,
            // KIR's verified ZeroExtend-to-Index contract is the only
            // target-neutral value-preserving Index conversion admitted here.
            ScalarType::Index => source_width <= 32,
            ScalarType::Bool
            | ScalarType::I8
            | ScalarType::I16
            | ScalarType::I32
            | ScalarType::I64
            | ScalarType::I128
            | ScalarType::U8
            | ScalarType::F16
            | ScalarType::Bf16
            | ScalarType::F32
            | ScalarType::F64 => false,
        }
    }

    fn operation_result_variation(&self, operation: &Operation, result: ValueId) -> Variation {
        if self.proven_no_overflow.contains(&result) {
            Variation::GridUniform
        } else {
            self.operation_variation(operation)
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

    fn private_load_variation(&self, operation: &Operation) -> Option<Variation> {
        let result = operation.results.first()?.id;
        let slot = self.private_load_slots.get(&result)?;
        let stores = self.private_slot_stores.get(slot)?;
        Some(
            stores
                .iter()
                .fold(Variation::GridUniform, |variation, store| {
                    variation.join(self.value(store.value)).join(
                        self.report
                            .block_controls
                            .get(&store.block)
                            .copied()
                            .unwrap_or(Variation::Varying),
                    )
                }),
        )
    }

    fn trivial_phi_representative(&self, mut value: ValueId) -> ValueId {
        let mut remaining = self.trivial_phi_representatives.len();
        while remaining != 0 {
            let Some(next) = self.trivial_phi_representatives.get(&value).copied() else {
                break;
            };
            if next == value {
                break;
            }
            value = next;
            remaining -= 1;
        }
        value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnsignedRange {
    min: u128,
    max: u128,
}

impl UnsignedRange {
    const fn exact(value: u128) -> Self {
        Self {
            min: value,
            max: value,
        }
    }

    fn join(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    fn intersect(self, other: Self) -> Self {
        let intersection = Self {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        };
        if intersection.min <= intersection.max {
            intersection
        } else {
            // An inconsistent path fact must not manufacture a proof.
            self
        }
    }
}

#[derive(Clone, Copy)]
struct OperationDefinition<'a> {
    result_index: usize,
    operation: &'a Operation,
}

const MAX_KNOWN_INTEGER_VALUES: usize = 32;

struct KnownIntegerValueAnalysis<'a> {
    body: &'a FunctionBody,
    incoming: &'a BTreeMap<BlockId, Vec<Edge>>,
    private_load_slots: &'a BTreeMap<ValueId, ValueId>,
    private_slot_stores: &'a BTreeMap<ValueId, Vec<PrivateStore>>,
    operation_definitions: BTreeMap<ValueId, (usize, &'a Operation)>,
    block_parameters: BTreeMap<ValueId, (BlockId, usize)>,
    cache: BTreeMap<ValueId, Option<BTreeSet<u64>>>,
    visiting: BTreeSet<ValueId>,
}

impl<'a> KnownIntegerValueAnalysis<'a> {
    fn new(
        body: &'a FunctionBody,
        incoming: &'a BTreeMap<BlockId, Vec<Edge>>,
        private_load_slots: &'a BTreeMap<ValueId, ValueId>,
        private_slot_stores: &'a BTreeMap<ValueId, Vec<PrivateStore>>,
    ) -> Self {
        let mut operation_definitions = BTreeMap::new();
        let mut block_parameters = BTreeMap::new();
        for block in &body.blocks {
            for (index, parameter) in block.parameters.iter().enumerate() {
                block_parameters.insert(parameter.id, (block.id, index));
            }
            for operation in &block.operations {
                for (result_index, result) in operation.results.iter().enumerate() {
                    operation_definitions.insert(result.id, (result_index, operation));
                }
            }
        }
        Self {
            body,
            incoming,
            private_load_slots,
            private_slot_stores,
            operation_definitions,
            block_parameters,
            cache: BTreeMap::new(),
            visiting: BTreeSet::new(),
        }
    }

    fn solve_terminator_selectors(mut self) -> BTreeMap<ValueId, BTreeSet<u64>> {
        let selectors = self
            .body
            .blocks
            .iter()
            .filter_map(|block| block.terminator.as_ref().and_then(discriminator))
            .collect::<BTreeSet<_>>();
        selectors
            .into_iter()
            .filter_map(|selector| self.values(selector).map(|values| (selector, values)))
            .collect()
    }

    fn values(&mut self, value: ValueId) -> Option<BTreeSet<u64>> {
        if let Some(values) = self.cache.get(&value) {
            return values.clone();
        }
        if !self.visiting.insert(value) {
            return None;
        }
        let values = self.derive_values(value);
        self.visiting.remove(&value);
        self.cache.insert(value, values.clone());
        values
    }

    fn derive_values(&mut self, value: ValueId) -> Option<BTreeSet<u64>> {
        if let Some((block, index)) = self.block_parameters.get(&value).copied() {
            let edges = self.incoming.get(&block)?.clone();
            if edges.is_empty() {
                return None;
            }
            let mut values = BTreeSet::new();
            for edge in edges {
                let argument = *edge.arguments.get(index)?;
                extend_known_values(&mut values, self.values(argument)?)?;
            }
            return Some(values);
        }

        let (result_index, operation) = self.operation_definitions.get(&value).copied()?;
        if result_index != 0 {
            return None;
        }
        let kind = operation.kind.clone();
        match kind {
            OperationKind::Constant(constant) => {
                Some(BTreeSet::from([known_u64_constant(&constant)?]))
            }
            OperationKind::Cast {
                kind: CastKind::ZeroExtend | CastKind::Bitcast,
                value,
                ..
            } => self.values(value),
            OperationKind::Select {
                true_value,
                false_value,
                ..
            } => {
                let mut values = self.values(true_value)?;
                extend_known_values(&mut values, self.values(false_value)?)?;
                Some(values)
            }
            OperationKind::Load { .. } => {
                let slot = *self.private_load_slots.get(&value)?;
                let stores = self.private_slot_stores.get(&slot)?.clone();
                if stores.is_empty() {
                    return None;
                }
                let mut values = BTreeSet::new();
                for store in stores {
                    extend_known_values(&mut values, self.values(store.value)?)?;
                }
                Some(values)
            }
            _ => None,
        }
    }
}

fn extend_known_values(destination: &mut BTreeSet<u64>, source: BTreeSet<u64>) -> Option<()> {
    if destination.len().saturating_add(source.len()) > MAX_KNOWN_INTEGER_VALUES {
        return None;
    }
    destination.extend(source);
    (destination.len() <= MAX_KNOWN_INTEGER_VALUES).then_some(())
}

fn known_u64_constant(constant: &Constant) -> Option<u64> {
    match constant {
        Constant::Bool(value) => Some(u64::from(*value)),
        Constant::I8(value) => u64::try_from(*value).ok(),
        Constant::I16(value) => u64::try_from(*value).ok(),
        Constant::I32(value) => u64::try_from(*value).ok(),
        Constant::I64(value) => u64::try_from(*value).ok(),
        Constant::U8(value) => Some((*value).into()),
        Constant::U16(value) => Some((*value).into()),
        Constant::U32(value) => Some((*value).into()),
        Constant::U64(value) | Constant::Index(value) => Some(*value),
        Constant::F16Bits(_)
        | Constant::Bf16Bits(_)
        | Constant::F32Bits(_)
        | Constant::F64Bits(_) => None,
    }
}

fn effective_successors(
    body: &FunctionBody,
    known_values: &BTreeMap<ValueId, BTreeSet<u64>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    body.blocks
        .iter()
        .filter_map(|block| {
            let terminator = block.terminator.as_ref()?;
            let all = terminator.successors().into_iter().collect::<BTreeSet<_>>();
            let Some(selector) = discriminator(terminator) else {
                return Some((block.id, all));
            };
            let Some(values) = known_values
                .get(&selector)
                .filter(|values| !values.is_empty())
            else {
                return Some((block.id, all));
            };
            let selected = match terminator {
                Terminator::Switch {
                    cases,
                    default_target,
                    ..
                } => values
                    .iter()
                    .map(|value| {
                        cases
                            .iter()
                            .find(|case| case.value == *value)
                            .map_or(*default_target, |case| case.target)
                    })
                    .collect(),
                Terminator::IntegerSwitch {
                    cases,
                    default_target,
                    ..
                } => values
                    .iter()
                    .map(|value| {
                        cases
                            .iter()
                            .find(|case| known_u64_constant(&case.value) == Some(*value))
                            .map_or(*default_target, |case| case.target)
                    })
                    .collect(),
                _ => all.clone(),
            };
            Some((block.id, selected))
        })
        .collect()
}

fn successors_for(
    block: BlockId,
    terminator: &Terminator,
    effective_successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Vec<BlockId> {
    effective_successors
        .get(&block)
        .map(|successors| successors.iter().copied().collect())
        .unwrap_or_else(|| terminator.successors())
}

#[derive(Clone, Copy)]
struct RangeGuard {
    block: BlockId,
    then_target: BlockId,
    else_target: BlockId,
    then_edge_is_exclusive: bool,
    else_edge_is_exclusive: bool,
    predicate: ComparePredicate,
    lhs: ValueId,
    rhs: ValueId,
}

struct UnsignedRangeAnalysis<'a> {
    body: &'a FunctionBody,
    reachable: &'a BTreeSet<BlockId>,
    incoming: &'a BTreeMap<BlockId, Vec<Edge>>,
    dominators: &'a BTreeMap<BlockId, BTreeSet<BlockId>>,
    trivial_phi_representatives: &'a BTreeMap<ValueId, ValueId>,
    private_load_slots: &'a BTreeMap<ValueId, ValueId>,
    private_slot_stores: &'a BTreeMap<ValueId, Vec<PrivateStore>>,
    value_types: BTreeMap<ValueId, Type>,
    operation_definitions: BTreeMap<ValueId, OperationDefinition<'a>>,
    block_parameters: BTreeMap<ValueId, (BlockId, usize)>,
    guards: Vec<RangeGuard>,
    cache: BTreeMap<(ValueId, BlockId), UnsignedRange>,
    visiting: BTreeSet<(ValueId, BlockId)>,
}

#[allow(clippy::too_many_arguments)]
fn prove_unsigned_checked_arithmetic(
    function: &Function,
    body: &FunctionBody,
    reachable: &BTreeSet<BlockId>,
    incoming: &BTreeMap<BlockId, Vec<Edge>>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    trivial_phi_representatives: &BTreeMap<ValueId, ValueId>,
    private_load_slots: &BTreeMap<ValueId, ValueId>,
    private_slot_stores: &BTreeMap<ValueId, Vec<PrivateStore>>,
) -> BTreeSet<ValueId> {
    let mut analysis = UnsignedRangeAnalysis::new(
        function,
        body,
        reachable,
        incoming,
        dominators,
        trivial_phi_representatives,
        private_load_slots,
        private_slot_stores,
    );
    let sites = body
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
        .flat_map(|block| {
            block.operations.iter().filter_map(|operation| {
                let OperationKind::Binary {
                    op: BinaryOp::Checked(operator),
                    lhs,
                    rhs,
                } = operation.kind
                else {
                    return None;
                };
                (operation.results.len() == 2).then_some((
                    block.id,
                    operator,
                    lhs,
                    rhs,
                    operation.results[0].ty.clone(),
                    operation.results[1].id,
                ))
            })
        })
        .collect::<Vec<_>>();

    let mut proven = BTreeSet::new();
    for (block, operator, lhs, rhs, result_type, overflow) in sites {
        let accepted =
            analysis.checked_operation_cannot_overflow(block, operator, lhs, rhs, &result_type);
        if accepted {
            proven.insert(overflow);
        }
    }
    proven
}

impl<'a> UnsignedRangeAnalysis<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        function: &Function,
        body: &'a FunctionBody,
        reachable: &'a BTreeSet<BlockId>,
        incoming: &'a BTreeMap<BlockId, Vec<Edge>>,
        dominators: &'a BTreeMap<BlockId, BTreeSet<BlockId>>,
        trivial_phi_representatives: &'a BTreeMap<ValueId, ValueId>,
        private_load_slots: &'a BTreeMap<ValueId, ValueId>,
        private_slot_stores: &'a BTreeMap<ValueId, Vec<PrivateStore>>,
    ) -> Self {
        let mut value_types = BTreeMap::new();
        value_types.extend(
            body.parameters
                .iter()
                .copied()
                .zip(function.signature.parameters.iter().cloned()),
        );
        let mut operation_definitions = BTreeMap::new();
        let mut block_parameters = BTreeMap::new();
        for block in &body.blocks {
            for (index, parameter) in block.parameters.iter().enumerate() {
                value_types.insert(parameter.id, parameter.ty.clone());
                block_parameters.insert(parameter.id, (block.id, index));
            }
            for operation in &block.operations {
                for (result_index, result) in operation.results.iter().enumerate() {
                    value_types.insert(result.id, result.ty.clone());
                    operation_definitions.insert(
                        result.id,
                        OperationDefinition {
                            result_index,
                            operation,
                        },
                    );
                }
            }
        }

        let mut analysis = Self {
            body,
            reachable,
            incoming,
            dominators,
            trivial_phi_representatives,
            private_load_slots,
            private_slot_stores,
            value_types,
            operation_definitions,
            block_parameters,
            guards: Vec::new(),
            cache: BTreeMap::new(),
            visiting: BTreeSet::new(),
        };
        analysis.guards = analysis.collect_guards();
        analysis
    }

    fn collect_guards(&self) -> Vec<RangeGuard> {
        self.body
            .blocks
            .iter()
            .filter(|block| self.reachable.contains(&block.id))
            .filter_map(|block| {
                let Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    else_target,
                    ..
                } = block.terminator.as_ref()?
                else {
                    return None;
                };
                let (predicate, lhs, rhs) = self.comparison_for_condition(*condition, block.id)?;
                Some(RangeGuard {
                    block: block.id,
                    then_target: *then_target,
                    else_target: *else_target,
                    then_edge_is_exclusive: self.edge_is_exclusive(block.id, *then_target),
                    else_edge_is_exclusive: self.edge_is_exclusive(block.id, *else_target),
                    predicate,
                    lhs,
                    rhs,
                })
            })
            .collect()
    }

    fn edge_is_exclusive(&self, source: BlockId, target: BlockId) -> bool {
        self.incoming
            .get(&target)
            .is_some_and(|edges| matches!(edges.as_slice(), [edge] if edge.source == source))
    }

    fn comparison_for_condition(
        &self,
        mut value: ValueId,
        use_block: BlockId,
    ) -> Option<(ComparePredicate, ValueId, ValueId)> {
        let mut negate = false;
        let mut seen = BTreeSet::new();
        while seen.insert(value) {
            value = resolve_representative(self.trivial_phi_representatives, value);
            if let Some(slot) = self.private_load_slots.get(&value)
                && let Some([store]) = self.private_slot_stores.get(slot).map(Vec::as_slice)
                && self.dominates(store.block, use_block)
            {
                value = store.value;
                continue;
            }
            let definition = self.operation_definitions.get(&value)?;
            match &definition.operation.kind {
                OperationKind::Compare {
                    predicate,
                    lhs,
                    rhs,
                } => {
                    let predicate = if negate {
                        invert_predicate(*predicate)
                    } else {
                        *predicate
                    };
                    return Some((predicate, *lhs, *rhs));
                }
                OperationKind::Unary {
                    op: UnaryOp::Not,
                    operand,
                } if self.value_types.get(operand) == Some(&Type::BOOL) => {
                    negate = !negate;
                    value = *operand;
                }
                _ => return None,
            }
        }
        None
    }

    fn checked_operation_cannot_overflow(
        &mut self,
        block: BlockId,
        operator: CheckedBinaryOperator,
        lhs: ValueId,
        rhs: ValueId,
        result_type: &Type,
    ) -> bool {
        let Some(type_range) = unsigned_type_range(result_type) else {
            return false;
        };
        let (Some(lhs), Some(rhs)) = (self.range_at(lhs, block), self.range_at(rhs, block)) else {
            return false;
        };
        checked_result_range(operator, lhs, rhs, type_range.max).is_some()
    }

    fn range_at(&mut self, value: ValueId, query_block: BlockId) -> Option<UnsignedRange> {
        let key = (value, query_block);
        if let Some(range) = self.cache.get(&key).copied() {
            return Some(range);
        }
        let type_range = self.value_types.get(&value).and_then(unsigned_type_range)?;
        if !self.visiting.insert(key) {
            return Some(type_range);
        }

        let mut range = self
            .expression_range(value, query_block)
            .unwrap_or(type_range)
            .intersect(type_range);
        let guards = self.guards.clone();
        for guard in guards {
            let then_dominates =
                guard.then_edge_is_exclusive && self.dominates(guard.then_target, query_block);
            let else_dominates =
                guard.else_edge_is_exclusive && self.dominates(guard.else_target, query_block);
            let predicate = match (then_dominates, else_dominates) {
                (true, false) => guard.predicate,
                (false, true) => invert_predicate(guard.predicate),
                _ => continue,
            };
            let value_identity = self.immutable_identity(value);
            let lhs_identity = self.immutable_identity(guard.lhs);
            let rhs_identity = self.immutable_identity(guard.rhs);
            let (predicate, other) = if value_identity == lhs_identity {
                (predicate, guard.rhs)
            } else if value_identity == rhs_identity {
                (swap_predicate(predicate), guard.lhs)
            } else {
                continue;
            };
            if let Some(other) = self.range_at(other, guard.block) {
                range = refine_unsigned_range(range, predicate, other);
            }
        }

        self.visiting.remove(&key);
        self.cache.insert(key, range);
        Some(range)
    }

    fn expression_range(&mut self, value: ValueId, query_block: BlockId) -> Option<UnsignedRange> {
        if let Some((block, index)) = self.block_parameters.get(&value).copied() {
            let edges = self.incoming.get(&block)?.clone();
            return edges
                .into_iter()
                .filter_map(|edge| {
                    edge.arguments
                        .get(index)
                        .copied()
                        .and_then(|argument| self.range_at(argument, edge.source))
                })
                .reduce(UnsignedRange::join);
        }

        let definition = self.operation_definitions.get(&value).copied()?;
        let kind = definition.operation.kind.clone();
        let result_type = self.value_types.get(&value)?.clone();
        match kind {
            OperationKind::Constant(constant) => unsigned_constant_range(&constant),
            OperationKind::Binary { op, lhs, rhs } if definition.result_index == 0 => {
                let lhs = self.range_at(lhs, query_block)?;
                let rhs = self.range_at(rhs, query_block)?;
                binary_result_range(op, lhs, rhs, unsigned_type_range(&result_type)?.max)
            }
            OperationKind::Cast {
                kind,
                value: source,
                ..
            } => {
                let source_type = self.value_types.get(&source)?.clone();
                let source = self.range_at(source, query_block)?;
                cast_result_range(kind, &source_type, &result_type, source)
            }
            OperationKind::Select {
                true_value,
                false_value,
                ..
            } => Some(
                self.range_at(true_value, query_block)?
                    .join(self.range_at(false_value, query_block)?),
            ),
            OperationKind::Load { .. } => {
                let slot = *self.private_load_slots.get(&value)?;
                self.private_slot_stores
                    .get(&slot)?
                    .clone()
                    .into_iter()
                    .filter_map(|store| self.range_at(store.value, query_block))
                    .reduce(UnsignedRange::join)
            }
            _ => None,
        }
    }

    fn immutable_identity(&self, mut value: ValueId) -> ValueId {
        let mut seen = BTreeSet::new();
        while seen.insert(value) {
            let representative = resolve_representative(self.trivial_phi_representatives, value);
            if representative != value {
                value = representative;
                continue;
            }
            let Some(slot) = self.private_load_slots.get(&value) else {
                break;
            };
            let Some([store]) = self.private_slot_stores.get(slot).map(Vec::as_slice) else {
                break;
            };
            value = store.value;
        }
        value
    }

    fn dominates(&self, dominator: BlockId, block: BlockId) -> bool {
        self.dominators
            .get(&block)
            .is_some_and(|dominators| dominators.contains(&dominator))
    }
}

fn unsigned_type_range(ty: &Type) -> Option<UnsignedRange> {
    let max = match ty.as_scalar()? {
        ScalarType::Bool => 1,
        ScalarType::U8 => u8::MAX.into(),
        ScalarType::U16 => u16::MAX.into(),
        ScalarType::U32 => u32::MAX.into(),
        ScalarType::U64 | ScalarType::Index => u64::MAX.into(),
        ScalarType::U128 => u128::MAX,
        ScalarType::I8
        | ScalarType::I16
        | ScalarType::I32
        | ScalarType::I64
        | ScalarType::I128
        | ScalarType::F16
        | ScalarType::Bf16
        | ScalarType::F32
        | ScalarType::F64 => return None,
    };
    Some(UnsignedRange { min: 0, max })
}

fn unsigned_constant_range(constant: &Constant) -> Option<UnsignedRange> {
    let value = match constant {
        Constant::Bool(value) => u128::from(*value),
        Constant::U8(value) => (*value).into(),
        Constant::U16(value) => (*value).into(),
        Constant::U32(value) => (*value).into(),
        Constant::U64(value) | Constant::Index(value) => (*value).into(),
        Constant::I8(_)
        | Constant::I16(_)
        | Constant::I32(_)
        | Constant::I64(_)
        | Constant::F16Bits(_)
        | Constant::Bf16Bits(_)
        | Constant::F32Bits(_)
        | Constant::F64Bits(_) => return None,
    };
    Some(UnsignedRange::exact(value))
}

fn binary_result_range(
    operator: BinaryOp,
    lhs: UnsignedRange,
    rhs: UnsignedRange,
    type_max: u128,
) -> Option<UnsignedRange> {
    match operator {
        BinaryOp::Add => checked_result_range(CheckedBinaryOperator::Add, lhs, rhs, type_max),
        BinaryOp::Subtract => {
            checked_result_range(CheckedBinaryOperator::Subtract, lhs, rhs, type_max)
        }
        BinaryOp::Multiply => {
            checked_result_range(CheckedBinaryOperator::Multiply, lhs, rhs, type_max)
        }
        BinaryOp::Checked(operator) => checked_result_range(operator, lhs, rhs, type_max),
        BinaryOp::Divide if rhs.min != 0 => Some(UnsignedRange {
            min: lhs.min / rhs.max,
            max: lhs.max / rhs.min,
        }),
        BinaryOp::Remainder if rhs.min != 0 => Some(UnsignedRange {
            min: 0,
            max: lhs.max.min(rhs.max - 1),
        }),
        BinaryOp::BitAnd => Some(UnsignedRange {
            min: 0,
            max: lhs.max.min(rhs.max).min(type_max),
        }),
        BinaryOp::Divide
        | BinaryOp::Remainder
        | BinaryOp::BitOr
        | BinaryOp::BitXor
        | BinaryOp::ShiftLeft
        | BinaryOp::ShiftRight => None,
    }
}

fn checked_result_range(
    operator: CheckedBinaryOperator,
    lhs: UnsignedRange,
    rhs: UnsignedRange,
    type_max: u128,
) -> Option<UnsignedRange> {
    let range = match operator {
        CheckedBinaryOperator::Add => UnsignedRange {
            min: lhs.min.checked_add(rhs.min)?,
            max: lhs.max.checked_add(rhs.max)?,
        },
        CheckedBinaryOperator::Subtract if lhs.min >= rhs.max => UnsignedRange {
            min: lhs.min - rhs.max,
            max: lhs.max - rhs.min,
        },
        CheckedBinaryOperator::Multiply => UnsignedRange {
            min: lhs.min.checked_mul(rhs.min)?,
            max: lhs.max.checked_mul(rhs.max)?,
        },
        CheckedBinaryOperator::Subtract => return None,
    };
    (range.max <= type_max).then_some(range)
}

fn cast_result_range(
    kind: CastKind,
    source_type: &Type,
    target_type: &Type,
    source: UnsignedRange,
) -> Option<UnsignedRange> {
    let source_type = unsigned_type_range(source_type)?;
    let target_type = unsigned_type_range(target_type)?;
    match kind {
        CastKind::ZeroExtend if source_type.max <= target_type.max => Some(source),
        CastKind::Truncate if source.max <= target_type.max => Some(source),
        CastKind::Bitcast if source_type.max == target_type.max => Some(source),
        CastKind::Truncate => Some(target_type),
        CastKind::ZeroExtend
        | CastKind::SignExtend
        | CastKind::FloatExtend
        | CastKind::FloatTruncate
        | CastKind::IntegerToFloat
        | CastKind::FloatToInteger
        | CastKind::Bitcast => None,
    }
}

fn refine_unsigned_range(
    range: UnsignedRange,
    predicate: ComparePredicate,
    other: UnsignedRange,
) -> UnsignedRange {
    let refined = match predicate {
        ComparePredicate::Equal => range.intersect(other),
        ComparePredicate::NotEqual if other.min == 0 && other.max == 0 && range.min == 0 => {
            UnsignedRange {
                min: 1,
                max: range.max,
            }
        }
        ComparePredicate::NotEqual
            if other.min == range.max && other.max == range.max && range.min != range.max =>
        {
            UnsignedRange {
                min: range.min,
                max: range.max - 1,
            }
        }
        ComparePredicate::LessThan if other.max != 0 => UnsignedRange {
            min: range.min,
            max: range.max.min(other.max - 1),
        },
        ComparePredicate::LessThanOrEqual => UnsignedRange {
            min: range.min,
            max: range.max.min(other.max),
        },
        ComparePredicate::GreaterThan if other.min != u128::MAX => UnsignedRange {
            min: range.min.max(other.min + 1),
            max: range.max,
        },
        ComparePredicate::GreaterThanOrEqual => UnsignedRange {
            min: range.min.max(other.min),
            max: range.max,
        },
        ComparePredicate::NotEqual | ComparePredicate::LessThan | ComparePredicate::GreaterThan => {
            return range;
        }
    };
    range.intersect(refined)
}

const fn invert_predicate(predicate: ComparePredicate) -> ComparePredicate {
    match predicate {
        ComparePredicate::Equal => ComparePredicate::NotEqual,
        ComparePredicate::NotEqual => ComparePredicate::Equal,
        ComparePredicate::LessThan => ComparePredicate::GreaterThanOrEqual,
        ComparePredicate::LessThanOrEqual => ComparePredicate::GreaterThan,
        ComparePredicate::GreaterThan => ComparePredicate::LessThanOrEqual,
        ComparePredicate::GreaterThanOrEqual => ComparePredicate::LessThan,
    }
}

const fn swap_predicate(predicate: ComparePredicate) -> ComparePredicate {
    match predicate {
        ComparePredicate::Equal => ComparePredicate::Equal,
        ComparePredicate::NotEqual => ComparePredicate::NotEqual,
        ComparePredicate::LessThan => ComparePredicate::GreaterThan,
        ComparePredicate::LessThanOrEqual => ComparePredicate::GreaterThanOrEqual,
        ComparePredicate::GreaterThan => ComparePredicate::LessThan,
        ComparePredicate::GreaterThanOrEqual => ComparePredicate::LessThanOrEqual,
    }
}

fn private_storage_facts(
    body: &FunctionBody,
    reachable: &BTreeSet<BlockId>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> (
    BTreeMap<ValueId, ValueId>,
    BTreeMap<ValueId, Vec<PrivateStore>>,
) {
    let slots = body
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::Alloca {
                    count: None,
                    address_space: AddressSpace::Private,
                    ..
                }
            )
        })
        .flat_map(|operation| operation.results.iter().map(|result| result.id))
        .collect::<BTreeSet<_>>();
    let mut escaped = BTreeSet::new();
    for block in body
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
    {
        for operation in &block.operations {
            for operand in operation.kind.operands() {
                if slots.contains(&operand)
                    && !is_direct_private_slot_access(&operation.kind, operand)
                {
                    escaped.insert(operand);
                }
            }
        }
        if let Some(terminator) = &block.terminator {
            escaped.extend(
                terminator
                    .operands()
                    .into_iter()
                    .filter(|operand| slots.contains(operand)),
            );
        }
    }

    let eligible = slots.difference(&escaped).copied().collect::<BTreeSet<_>>();
    let mut stores = BTreeMap::<ValueId, Vec<PrivateStore>>::new();
    let mut loads = Vec::new();
    for block in body
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
    {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            match &operation.kind {
                OperationKind::Store {
                    pointer,
                    value,
                    access,
                } if eligible.contains(pointer)
                    && access.address_space == AddressSpace::Private =>
                {
                    stores.entry(*pointer).or_default().push(PrivateStore {
                        block: block.id,
                        operation_index,
                        value: *value,
                    });
                }
                OperationKind::Load { pointer, access }
                    if eligible.contains(pointer)
                        && access.address_space == AddressSpace::Private =>
                {
                    loads.extend(
                        operation
                            .results
                            .iter()
                            .map(|result| (result.id, *pointer, block.id, operation_index)),
                    );
                }
                _ => {}
            }
        }
    }

    let private_load_slots = loads
        .into_iter()
        .filter(|(_, slot, load_block, load_index)| {
            stores.get(slot).is_some_and(|slot_stores| {
                slot_stores.iter().any(|store| {
                    dominators
                        .get(load_block)
                        .is_some_and(|set| set.contains(&store.block))
                        && (store.block != *load_block || store.operation_index < *load_index)
                })
            })
        })
        .map(|(result, slot, _, _)| (result, slot))
        .collect();
    (private_load_slots, stores)
}

fn is_direct_private_slot_access(kind: &OperationKind, slot: ValueId) -> bool {
    match kind {
        OperationKind::Load { pointer, access } => {
            *pointer == slot && access.address_space == AddressSpace::Private
        }
        OperationKind::Store {
            pointer,
            value,
            access,
        } => *pointer == slot && *value != slot && access.address_space == AddressSpace::Private,
        _ => false,
    }
}

fn compute_dominators(
    body: &FunctionBody,
    reachable: &BTreeSet<BlockId>,
    incoming: &BTreeMap<BlockId, Vec<Edge>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let Some(entry) = body.blocks.first().map(|block| block.id) else {
        return BTreeMap::new();
    };
    let mut facts = reachable
        .iter()
        .map(|block| {
            let initial = if *block == entry {
                BTreeSet::from([entry])
            } else {
                reachable.clone()
            };
            (*block, initial)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in reachable.iter().copied().filter(|block| *block != entry) {
            let mut predecessor_sets = incoming
                .get(&block)
                .into_iter()
                .flat_map(|edges| edges.iter())
                .filter_map(|edge| facts.get(&edge.source));
            let mut next = predecessor_sets.next().cloned().unwrap_or_default();
            for predecessor_set in predecessor_sets {
                next.retain(|candidate| predecessor_set.contains(candidate));
            }
            next.insert(block);
            if facts.get(&block) != Some(&next) {
                facts.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return facts;
        }
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

fn trivial_phi_representatives(
    body: &FunctionBody,
    incoming: &BTreeMap<BlockId, Vec<Edge>>,
) -> BTreeMap<ValueId, ValueId> {
    let mut representatives = BTreeMap::new();
    loop {
        let mut changed = false;
        for block in &body.blocks {
            let Some(edges) = incoming.get(&block.id) else {
                continue;
            };
            for (index, parameter) in block.parameters.iter().enumerate() {
                let origins = edges
                    .iter()
                    .filter_map(|edge| edge.arguments.get(index).copied())
                    .map(|value| resolve_representative(&representatives, value))
                    .filter(|value| *value != parameter.id)
                    .collect::<BTreeSet<_>>();
                if origins.len() == 1 {
                    let origin = *origins.first().expect("singleton origin");
                    if representatives.get(&parameter.id) != Some(&origin) {
                        representatives.insert(parameter.id, origin);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return representatives;
        }
    }
}

fn resolve_representative(
    representatives: &BTreeMap<ValueId, ValueId>,
    mut value: ValueId,
) -> ValueId {
    let mut remaining = representatives.len();
    while remaining != 0 {
        let Some(next) = representatives.get(&value).copied() else {
            break;
        };
        if next == value {
            break;
        }
        value = next;
        remaining -= 1;
    }
    value
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
    natural_loop_nests: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    effective_successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let postdominators = postdominators(blocks, postdominance_available, effective_successors);
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
        let mut stop = immediate_postdominator(block.id, &postdominators);
        let collect_region = |stop: Option<BlockId>| {
            let mut region = BTreeSet::new();
            let mut pending =
                VecDeque::from(successors_for(block.id, terminator, effective_successors));
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
                    pending.extend(successors_for(
                        candidate,
                        candidate_terminator,
                        effective_successors,
                    ));
                }
            }
            region
        };
        let mut region = collect_region(stop);
        if let Some(stop_block) = stop {
            let source_loops = natural_loop_nests.get(&block.id);
            let stop_loops = natural_loop_nests.get(&stop_block);
            let exits_containing_loop = source_loops.is_some_and(|source_loops| {
                source_loops
                    .iter()
                    .any(|header| !stop_loops.is_some_and(|stop_loops| stop_loops.contains(header)))
            });
            if exits_containing_loop {
                stop = None;
                region = collect_region(stop);
            }
        }
        regions.insert(block.id, region);
    }
    regions
}

fn postdominance_available(
    blocks: &BTreeMap<BlockId, &BasicBlock>,
    reachable: &BTreeSet<BlockId>,
    effective_successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
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
                .map(|terminator| successors_for(*block_id, terminator, effective_successors))
                .unwrap_or_default();
            if successors.is_empty()
                || successors
                    .iter()
                    .any(|successor| available.contains(successor))
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
    effective_successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
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
                .map(|terminator| successors_for(*block_id, terminator, effective_successors))
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
