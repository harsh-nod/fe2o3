//! Exact dormant-handle transfer; this index is not source or graph authority.
use super::*;
use fe2o3_kernel_ir::{ExecutionCapabilityOpV1, ExecutionCapabilityTypeV1};

type Result<T> = std::result::Result<T, FinalKirOutputEquivalenceErrorV1>;
const INVALID: FinalKirOutputEquivalenceErrorV1 =
    FinalKirOutputEquivalenceErrorV1::MalformedValueFlow;

#[derive(Clone, Debug)]
pub(super) struct Handle {
    pub(super) view: WorkgroupViewV1,
    capability: ExecutionCapabilityTypeV1,
}

impl Handle {
    pub(super) fn matches_type(&self, capability: &ExecutionCapabilityTypeV1) -> bool {
        self.capability == *capability
            && !self.view.initialized
            && !self.view.published
            && matches!(capability.role,
                fe2o3_kernel_ir::ExecutionCapabilityRoleV1::ReusableLds { layout, elements, .. }
                if layout == self.view.layout && elements == self.view.elements)
    }
}

struct Transfer<'a> {
    block: BlockId,
    operation: &'a Operation,
    output: ValueId,
    output_type: Option<ExecutionCapabilityTypeV1>,
    used: bool,
}

pub(super) struct Transfers<'a> {
    inputs: BTreeMap<ValueId, Transfer<'a>>,
}

fn charge(nodes: &mut usize, amount: usize) -> Result<()> {
    *nodes = nodes
        .checked_add(amount)
        .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    if *nodes > MAX_FINAL_KIR_SYMBOLIC_NODES_V1 {
        return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
    }
    Ok(())
}

fn lookup_work(entries: usize) -> usize {
    (usize::BITS - entries.max(1).leading_zeros()) as usize
}

fn contract(operation: &Operation) -> Result<&ExecutionCapabilityOpV1> {
    match &operation.kind {
        OperationKind::ExecutionCapability(value) if value.is_complete() => Ok(value),
        _ => Err(INVALID),
    }
}

impl<'a> Transfers<'a> {
    // Called only when the existing definition walk encountered a conversion.
    // All added scans, records and lookups debit the executor's retained counter.
    pub(super) fn collect(function: &'a Function, nodes: &mut usize) -> Result<Self> {
        let body = function.body.as_ref().ok_or(INVALID)?;
        let mut inputs = BTreeMap::new();
        for block in &body.blocks {
            charge(nodes, 1)?;
            for operation in &block.operations {
                charge(nodes, 1)?;
                if !matches!(
                    &operation.kind,
                    OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                        operation: ExecutionCapabilityOperationV1::ReusableLdsConversion(_),
                        ..
                    })
                ) {
                    continue;
                }
                let capability = contract(operation)?;
                let ([input], [output]) =
                    (capability.operands.as_slice(), operation.results.as_slice())
                else {
                    return Err(INVALID);
                };
                charge(nodes, 1 + lookup_work(inputs.len()))?;
                if *input == output.id
                    || inputs
                        .insert(
                            *input,
                            Transfer {
                                block: block.id,
                                operation,
                                output: output.id,
                                output_type: None,
                                used: false,
                            },
                        )
                        .is_some()
                {
                    return Err(INVALID);
                }
            }
        }
        let lookup = lookup_work(inputs.len());
        for block in &body.blocks {
            charge(nodes, 1)?;
            for operation in &block.operations {
                charge(nodes, 1)?;
                for result in &operation.results {
                    charge(nodes, 1 + lookup)?;
                    if let Some(transfer) = inputs.get_mut(&result.id) {
                        if transfer.output_type.is_some() {
                            return Err(INVALID);
                        }
                        transfer.output_type = Some(check_allocation(operation, transfer)?);
                    }
                }
                // The existing operand API allocates a Vec. Bound its variable-
                // length cases before calling it; all remaining shapes are fixed.
                let variable_slots = match &operation.kind {
                    OperationKind::Call { arguments, .. } => arguments.len(),
                    OperationKind::ExecutionCapability(value) => value.operands.len(),
                    OperationKind::InlineAssembly(value) => value.operands.len(),
                    _ => 0,
                };
                charge(nodes, variable_slots)?;
                for operand in operation.operands() {
                    charge(nodes, 1 + lookup)?;
                    if let Some(transfer) = inputs.get_mut(&operand) {
                        if transfer.used || !std::ptr::eq(operation, transfer.operation) {
                            return Err(INVALID);
                        }
                        transfer.used = true;
                    }
                }
            }
            charge(nodes, 1)?;
            visit_terminator(
                block.terminator.as_ref().ok_or(INVALID)?,
                nodes,
                |operand, nodes| {
                    charge(nodes, lookup)?;
                    if inputs.contains_key(&operand) {
                        Err(INVALID)
                    } else {
                        Ok(())
                    }
                },
            )?;
        }
        for transfer in inputs.values() {
            charge(nodes, 1)?;
            if !transfer.used || transfer.output_type.is_none() {
                return Err(INVALID);
            }
        }
        Ok(Self { inputs })
    }

    pub(super) fn execute(
        &self,
        operation: &Operation,
        state: &mut ExecutionStateV1,
        nodes: &mut usize,
    ) -> Result<()> {
        let capability = contract(operation)?;
        let [input] = capability.operands.as_slice() else {
            return Err(INVALID);
        };
        charge(nodes, lookup_work(self.inputs.len()))?;
        let transfer = self.inputs.get(input).ok_or(INVALID)?;
        if transfer.block != state.block || !std::ptr::eq(transfer.operation, operation) {
            return Err(INVALID);
        }
        let ExecutionCapabilityOperationV1::ReusableLdsConversion(conversion) =
            capability.operation
        else {
            return Err(INVALID);
        };
        let Some(SymbolicValueV1::WorkgroupView(view)) = state.values.get(input) else {
            return Err(INVALID);
        };
        let memory = state.memories.get(&view.memory_root).ok_or(INVALID)?;
        if view.initialized
            || view.published
            || view.layout != conversion.layout
            || view.elements != conversion.elements
            || view.memory_root < WORKGROUP_MEMORY_ROOT_BASE_V1
            || view.memory_root - WORKGROUP_MEMORY_ROOT_BASE_V1 >= state.next_workgroup_allocation
            || memory.bounded_elements != Some(view.elements)
            || !memory
                .cross_lane_width
                .is_some_and(|width| width != 0 && width <= MAX_FINAL_KIR_WORKGROUP_INVOCATIONS_V1)
            || memory.initial_parameter.is_some()
            || memory.element.is_some()
            || !memory.writes.is_empty()
            || state.written_roots.contains(&view.memory_root)
            || state.values.contains_key(&transfer.output)
        {
            return Err(INVALID);
        }
        let value = SymbolicValueV1::ReusableLds(Box::new(Handle {
            view: view.clone(),
            capability: transfer.output_type.as_ref().ok_or(INVALID)?.clone(),
        }));
        if !value_matches_type(&value, &operation.results[0].ty) {
            return Err(INVALID);
        }
        // No fallible step follows consumption. Memory, events, counters and
        // publication remain untouched; the old SSA handle cannot be replayed.
        state.values.remove(input);
        state.values.insert(transfer.output, value);
        Ok(())
    }
}

fn check_allocation(
    operation: &Operation,
    transfer: &Transfer<'_>,
) -> Result<ExecutionCapabilityTypeV1> {
    let allocation = contract(operation)?;
    let conversion_contract = contract(transfer.operation)?;
    let ExecutionCapabilityOperationV1::ReusableLdsConversion(conversion) =
        conversion_contract.operation
    else {
        return Err(INVALID);
    };
    let (ExecutionCapabilityOperationV1::LdsAllocate {
        lds,
        element,
        layout,
        elements,
        ..
    }
    | ExecutionCapabilityOperationV1::LdsAllocateBorrowed {
        lds,
        element,
        layout,
        elements,
        ..
    }) = allocation.operation
    else {
        return Err(INVALID);
    };
    let [result] = operation.results.as_slice() else {
        return Err(INVALID);
    };
    let Type::ExecutionCapability(input_type) = &result.ty else {
        return Err(INVALID);
    };
    let output_type = conversion.output_type(input_type).ok_or(INVALID)?;
    let source = conversion_contract.source.occurrence.ok_or(INVALID)?;
    // Legacy op2 can lack occurrence data; when supplied, all root coordinates
    // must agree. Different original caller instances/blocks are legitimate.
    if allocation
        .source
        .occurrence
        .is_some_and(|allocation_source| {
            allocation_source.root_source_identity() != source.root_source_identity()
                || allocation_source.expansion_identity() != source.expansion_identity()
                || allocation_source.expanded_root_identity() != source.expanded_root_identity()
        })
        || lds != conversion.input
        || element != conversion.element
        || layout != conversion.layout
        || elements != conversion.elements
        || allocation.provenance != conversion_contract.provenance
        || allocation.workgroup_brand != conversion_contract.workgroup_brand
        || allocation.epoch_before != conversion_contract.epoch_before
        || allocation.epoch_after.is_some()
        || conversion_contract.epoch_after.is_some()
        || conversion_contract.source.operation != conversion.defined_function
        || input_type.provenance != allocation.provenance
        || input_type.workgroup_brand != allocation.workgroup_brand
        || input_type.epoch != allocation.epoch_before
        || transfer.operation.results[0].ty != Type::ExecutionCapability(output_type.clone())
    {
        return Err(INVALID);
    }
    Ok(output_type)
}

fn visit_terminator(
    terminator: &Terminator,
    nodes: &mut usize,
    mut visit: impl FnMut(ValueId, &mut usize) -> Result<()>,
) -> Result<()> {
    let mut values = |values: &[ValueId], nodes: &mut usize| -> Result<()> {
        for value in values {
            charge(nodes, 1)?;
            visit(*value, nodes)?;
        }
        Ok(())
    };
    match terminator {
        Terminator::Branch { arguments, .. } | Terminator::Return { values: arguments } => {
            values(arguments, nodes)
        }
        Terminator::ConditionalBranch {
            condition,
            then_arguments,
            else_arguments,
            ..
        } => {
            values(std::slice::from_ref(condition), nodes)?;
            values(then_arguments, nodes)?;
            values(else_arguments, nodes)
        }
        Terminator::Switch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            values(std::slice::from_ref(selector), nodes)?;
            for case in cases {
                charge(nodes, 1)?;
                values(&case.arguments, nodes)?;
            }
            values(default_arguments, nodes)
        }
        Terminator::IntegerSwitch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            values(std::slice::from_ref(selector), nodes)?;
            for case in cases {
                charge(nodes, 1)?;
                values(&case.arguments, nodes)?;
            }
            values(default_arguments, nodes)
        }
        Terminator::Unreachable => Ok(()),
    }
}
