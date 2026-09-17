//! Opt-in finite success-chain local-memory obligations.

use super::*;
use crate::{AmdGpuDiagnosticIntrinsicDescriptorV1, AmdGpuDiagnosticOperation, ComparePredicate};

/// Actual control within one classified physical function, not source proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalFrameControlKindV1 {
    Return,
    Branch {
        target: BlockId,
    },
    Selected {
        condition: ValueId,
        value: bool,
        successor: u8,
        target: BlockId,
        inactive: BlockId,
    },
    InactiveTrap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalFrameControlV1 {
    function_ordinal: usize,
    block: BlockId,
    kind: LocalFrameControlKindV1,
}
impl LocalFrameControlV1 {
    pub const fn function_ordinal(self) -> usize {
        self.function_ordinal
    }
    pub const fn block(self) -> BlockId {
        self.block
    }
    pub const fn kind(self) -> LocalFrameControlKindV1 {
        self.kind
    }
}

/// Simultaneously substituted scalar argument of one actual selected edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalFrameEdgeBindingV1 {
    function_ordinal: usize,
    source: BlockId,
    successor: u8,
    target: BlockId,
    ordinal: usize,
    argument: ValueId,
    parameter: ValueId,
    ty: ScalarType,
    constant: Option<KnownUnsigned>,
}
impl LocalFrameEdgeBindingV1 {
    pub const fn function_ordinal(self) -> usize {
        self.function_ordinal
    }
    pub const fn source(self) -> BlockId {
        self.source
    }
    pub const fn successor(self) -> u8 {
        self.successor
    }
    pub const fn target(self) -> BlockId {
        self.target
    }
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }
    pub const fn argument(self) -> ValueId {
        self.argument
    }
    pub const fn parameter(self) -> ValueId {
        self.parameter
    }
    pub const fn ty(self) -> ScalarType {
        self.ty
    }
    pub const fn known_unsigned(self) -> Option<u64> {
        match self.constant {
            Some(fact) => Some(fact.bits),
            None => None,
        }
    }
}

/// A borrowed local-memory classification together with its complete checked
/// chain. This is not a graph owner, raw purity, source proof or call authority.
/// Consumers must retain the control/substitution obligations with memory rows.
pub struct CheckedLocalFrameChainV1<'scope, 'module> {
    memory: CheckedLocalFrameV1<'scope, 'module>,
    control: &'scope [LocalFrameControlV1],
    edge_bindings: &'scope [LocalFrameEdgeBindingV1],
}

impl<'scope, 'module> CheckedLocalFrameChainV1<'scope, 'module> {
    pub const fn module(&self) -> &'module Module {
        self.memory.module()
    }
    pub const fn function(&self) -> &'module Function {
        self.memory.function()
    }
    pub const fn function_ordinal(&self) -> usize {
        self.memory.function_ordinal()
    }
    pub fn allocations(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&[LocalFrameAllocationV1], LocalFrameErrorV1> {
        self.memory.allocations(budget)
    }
    pub fn accesses(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&[LocalFrameAccessV1], LocalFrameErrorV1> {
        self.memory.accesses(budget)
    }

    pub fn control(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&[LocalFrameControlV1], LocalFrameErrorV1> {
        self.memory.require_live(budget)?;
        budget.charge_work(self.control.len())?;
        Ok(self.control)
    }

    pub fn edge_bindings(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&[LocalFrameEdgeBindingV1], LocalFrameErrorV1> {
        self.memory.require_live(budget)?;
        budget.charge_work(self.edge_bindings.len())?;
        Ok(self.edge_bindings)
    }
}

pub(super) fn checked_compare_fact(
    predicate: ComparePredicate,
    left: &Definition<'_>,
    right: &Definition<'_>,
    result: &Type,
) -> Result<Option<KnownUnsigned>, LocalFrameErrorV1> {
    if left.ty != right.ty || result != &Type::BOOL {
        return Err(ResourceError::Accounting.into());
    }
    let Some(ty) = left
        .ty
        .as_scalar()
        .filter(|ty| unsigned_width(*ty).is_some())
    else {
        return Ok(None);
    };
    let (Some(left), Some(right)) = (left.constant, right.constant) else {
        return Ok(None);
    };
    let left = validate_fact(left, ty)?.bits;
    let right = validate_fact(right, ty)?.bits;
    if ty == ScalarType::Bool
        && !matches!(
            predicate,
            ComparePredicate::Equal | ComparePredicate::NotEqual
        )
    {
        return Ok(None);
    }
    let value = match predicate {
        ComparePredicate::Equal => left == right,
        ComparePredicate::NotEqual => left != right,
        ComparePredicate::LessThan => left < right,
        ComparePredicate::LessThanOrEqual => left <= right,
        ComparePredicate::GreaterThan => left > right,
        ComparePredicate::GreaterThanOrEqual => left >= right,
    };
    Ok(Some(KnownUnsigned {
        ty: ScalarType::Bool,
        bits: u64::from(value),
    }))
}

struct ChainBlock {
    id: BlockId,
    ordinal: usize,
    state: u8,
}

struct ChainWorkspace<'module> {
    memory: Workspace<'module>,
    control: Vec<LocalFrameControlV1>,
    edge_bindings: Vec<LocalFrameEdgeBindingV1>,
    blocks: Vec<ChainBlock>,
}

fn chain_block(
    blocks: &[ChainBlock],
    id: BlockId,
    budget: &mut Budget<'_>,
) -> Result<usize, LocalFrameErrorV1> {
    verification_find_last_by_v1(blocks, 1, budget, |row| row.id.cmp(&id))?
        .ok_or_else(|| ResourceError::Accounting.into())
}

fn audit_inactive_sink(
    block: &BasicBlock,
    function: usize,
    budget: &mut Budget<'_>,
) -> Result<(), LocalFrameErrorV1> {
    budget.charge_work(6)?;
    if !block.parameters.is_empty()
        || block.operations.len() != 1
        || !matches!(block.terminator, Some(Terminator::Unreachable))
    {
        return Err(refusal(
            function,
            None,
            LocalFrameRefusalReasonV1::ControlFlow,
        ));
    }
    let operation = &block.operations[0];
    let OperationKind::Call { callee, arguments } = &operation.kind else {
        return Err(refusal(
            function,
            Some(0),
            LocalFrameRefusalReasonV1::Operation,
        ));
    };
    budget.charge_work(
        AmdGpuDiagnosticOperation::intrinsic_descriptor_lookup_work_v1(callee)
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    // Verification binds this reserved identity to its canonical declaration.
    // This exact inactive, terminating operation is not an empty-effect call.
    if !operation.results.is_empty()
        || !arguments.is_empty()
        || !matches!(
            AmdGpuDiagnosticOperation::intrinsic_descriptor_v1(callee),
            Some(AmdGpuDiagnosticIntrinsicDescriptorV1::Trap)
        )
    {
        return Err(refusal(
            function,
            Some(0),
            LocalFrameRefusalReasonV1::Operation,
        ));
    }
    audit_effects(operation, function, 0, budget)?;
    Ok(())
}

fn stage_edge(
    function: usize,
    source: BlockId,
    successor: u8,
    target: &BasicBlock,
    arguments: &[ValueId],
    sequence: usize,
    workspace: &mut ChainWorkspace<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), LocalFrameErrorV1> {
    budget.charge_work(3)?;
    if arguments.len() != target.parameters.len() {
        return Err(ResourceError::Accounting.into());
    }
    let first = workspace.edge_bindings.len();
    // Capture every incoming fact before changing any destination parameter.
    for (ordinal, (argument, parameter)) in arguments.iter().zip(&target.parameters).enumerate() {
        budget.charge_work(6)?;
        let definition =
            &workspace.memory.definitions[find(&workspace.memory.definitions, *argument, budget)?];
        let Some(ty) = parameter.ty.as_scalar() else {
            return Err(refusal(
                function,
                None,
                LocalFrameRefusalReasonV1::PointerUse,
            ));
        };
        if definition.ty != &parameter.ty || definition.operation.is_some_and(|at| at >= sequence) {
            return Err(refusal(
                function,
                None,
                LocalFrameRefusalReasonV1::Definition,
            ));
        }
        let constant = definition
            .constant
            .map(|fact| validate_fact(fact, ty))
            .transpose()?;
        workspace.edge_bindings.push(LocalFrameEdgeBindingV1 {
            function_ordinal: function,
            source,
            successor,
            target: target.id,
            ordinal,
            argument: *argument,
            parameter: parameter.id,
            ty,
            constant,
        });
    }
    for row in &workspace.edge_bindings[first..] {
        budget.charge_work(3)?;
        let target = find(&workspace.memory.definitions, row.parameter, budget)?;
        workspace.memory.definitions[target].constant = row.constant;
        workspace.memory.definitions[target].operation = Some(sequence);
    }
    Ok(())
}

fn derive_chain<'module>(
    function: &'module Function,
    function_ordinal: usize,
    workspace: &mut ChainWorkspace<'module>,
    budget: &mut Budget<'_>,
) -> Result<&'module Function, LocalFrameErrorV1> {
    use LocalFrameRefusalReasonV1 as Reason;
    budget.charge_work(6)?;
    let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
    let Some(entry) = body.blocks.first() else {
        return Err(refusal(function_ordinal, None, Reason::ControlFlow));
    };
    if !entry.parameters.is_empty() {
        return Err(refusal(function_ordinal, None, Reason::ControlFlow));
    }
    for ty in function
        .signature
        .parameters
        .iter()
        .chain(&function.signature.results)
    {
        budget.charge_work(1)?;
        if !matches!(ty, Type::Scalar(_) | Type::Unit) {
            return Err(refusal(function_ordinal, None, Reason::Signature));
        }
    }
    let mut definitions = body.parameters.len();
    let mut allocations = 0_usize;
    let mut accesses = 0_usize;
    let mut parameters = 0_usize;
    for block in &body.blocks {
        budget.charge_work(5)?;
        definitions = definitions
            .checked_add(block.parameters.len())
            .ok_or(ResourceError::Arithmetic)?;
        parameters = parameters
            .checked_add(block.parameters.len())
            .ok_or(ResourceError::Arithmetic)?;
        for parameter in &block.parameters {
            budget.charge_work(1)?;
            if !matches!(parameter.ty, Type::Scalar(_)) {
                return Err(refusal(function_ordinal, None, Reason::PointerUse));
            }
        }
        if !matches!(
            block.terminator,
            Some(
                Terminator::Branch { .. }
                    | Terminator::ConditionalBranch { .. }
                    | Terminator::Return { .. }
                    | Terminator::Unreachable
            )
        ) {
            return Err(refusal(function_ordinal, None, Reason::ControlFlow));
        }
        for operation in &block.operations {
            budget.charge_work(1)?;
            definitions = definitions
                .checked_add(operation.results.len())
                .ok_or(ResourceError::Arithmetic)?;
            allocations = allocations
                .checked_add(usize::from(matches!(
                    operation.kind,
                    OperationKind::Alloca { .. }
                )))
                .ok_or(ResourceError::Arithmetic)?;
            accesses = accesses
                .checked_add(usize::from(matches!(
                    operation.kind,
                    OperationKind::Load { .. } | OperationKind::Store { .. }
                )))
                .ok_or(ResourceError::Arithmetic)?;
        }
    }
    reserve(&mut workspace.memory.definitions, definitions, budget)?;
    reserve(&mut workspace.memory.allocations, allocations, budget)?;
    reserve(&mut workspace.memory.accesses, accesses, budget)?;
    reserve(&mut workspace.memory.cells, accesses, budget)?;
    reserve(&mut workspace.blocks, body.blocks.len(), budget)?;
    reserve(&mut workspace.control, body.blocks.len(), budget)?;
    reserve(&mut workspace.edge_bindings, parameters, budget)?;
    for (value, ty) in body.parameters.iter().zip(&function.signature.parameters) {
        budget.charge_work(3)?;
        workspace.memory.definitions.push(Definition {
            value: *value,
            ty,
            operation: None,
            pointer: None,
            constant: None,
            scalar_slot: None,
        });
    }
    for (ordinal, block) in body.blocks.iter().enumerate() {
        budget.charge_work(2)?;
        workspace.blocks.push(ChainBlock {
            id: block.id,
            ordinal,
            state: 0,
        });
        for parameter in &block.parameters {
            budget.charge_work(3)?;
            workspace.memory.definitions.push(Definition {
                value: parameter.id,
                ty: &parameter.ty,
                operation: Some(usize::MAX),
                pointer: None,
                constant: None,
                scalar_slot: None,
            });
        }
        for operation in &block.operations {
            budget.charge_work(1)?;
            for result in &operation.results {
                budget.charge_work(3)?;
                workspace.memory.definitions.push(Definition {
                    value: result.id,
                    ty: &result.ty,
                    operation: Some(usize::MAX),
                    pointer: None,
                    constant: None,
                    scalar_slot: None,
                });
            }
        }
    }
    verification_bounded_sort_by_v1(&mut workspace.memory.definitions, 1, budget, |a, b| {
        a.value.cmp(&b.value)
    })?;
    for pair in workspace.memory.definitions.windows(2) {
        budget.charge_work(1)?;
        if pair[0].value >= pair[1].value {
            return Err(refusal(function_ordinal, None, Reason::Definition));
        }
    }
    verification_bounded_sort_by_v1(&mut workspace.blocks, 1, budget, |a, b| a.id.cmp(&b.id))?;
    let mut current = entry.id;
    let mut sequence = 0_usize;
    let mut sink = None;
    loop {
        budget.charge_work(6)?;
        let row = chain_block(&workspace.blocks, current, budget)?;
        if workspace.blocks[row].state != 0 {
            return Err(refusal(function_ordinal, None, Reason::ControlFlow));
        }
        workspace.blocks[row].state = 1;
        let block = &body.blocks[workspace.blocks[row].ordinal];
        let start = sequence.checked_add(1).ok_or(ResourceError::Arithmetic)?;
        for (at, operation) in block.operations.iter().enumerate() {
            budget.charge_work(2)?;
            let at = start.checked_add(at).ok_or(ResourceError::Arithmetic)?;
            for result in &operation.results {
                budget.charge_work(1)?;
                let definition = find(&workspace.memory.definitions, result.id, budget)?;
                workspace.memory.definitions[definition].operation = Some(at);
            }
        }
        derive_block(
            function_ordinal,
            block,
            start,
            true,
            &mut workspace.memory,
            budget,
        )?;
        sequence = start
            .checked_add(block.operations.len())
            .ok_or(ResourceError::Arithmetic)?;
        budget.charge_work(4)?;
        let (target, arguments, successor, kind) =
            match block.terminator.as_ref().ok_or(ResourceError::Accounting)? {
                Terminator::Return { .. } => {
                    audit_return(function_ordinal, block, &workspace.memory, budget)?;
                    workspace.control.push(LocalFrameControlV1 {
                        function_ordinal,
                        block: block.id,
                        kind: LocalFrameControlKindV1::Return,
                    });
                    break;
                }
                Terminator::Branch { target, arguments } => (
                    *target,
                    arguments.as_slice(),
                    0,
                    LocalFrameControlKindV1::Branch { target: *target },
                ),
                Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                } => {
                    budget.charge_work(9)?;
                    let definition = &workspace.memory.definitions
                        [find(&workspace.memory.definitions, *condition, budget)?];
                    if definition.ty != &Type::BOOL
                        || definition.operation.is_some_and(|at| at >= sequence)
                    {
                        return Err(refusal(function_ordinal, None, Reason::ControlFlow));
                    }
                    let fact = definition
                        .constant
                        .ok_or_else(|| refusal(function_ordinal, None, Reason::ControlFlow))?;
                    let value = validate_fact(fact, ScalarType::Bool)?.bits != 0;
                    let (selected, args, inactive, inactive_args, successor) = if value {
                        (
                            *then_target,
                            then_arguments.as_slice(),
                            *else_target,
                            else_arguments,
                            0,
                        )
                    } else {
                        (
                            *else_target,
                            else_arguments.as_slice(),
                            *then_target,
                            then_arguments,
                            1,
                        )
                    };
                    if selected == inactive
                        || !inactive_args.is_empty()
                        || sink.is_some_and(|id| id != inactive)
                    {
                        return Err(refusal(function_ordinal, None, Reason::ControlFlow));
                    }
                    if sink.is_none() {
                        let sink_row = chain_block(&workspace.blocks, inactive, budget)?;
                        if workspace.blocks[sink_row].state != 0 {
                            return Err(refusal(function_ordinal, None, Reason::ControlFlow));
                        }
                        let sink_block = &body.blocks[workspace.blocks[sink_row].ordinal];
                        audit_inactive_sink(sink_block, function_ordinal, budget)?;
                        workspace.blocks[sink_row].state = 2;
                        workspace.control.push(LocalFrameControlV1 {
                            function_ordinal,
                            block: inactive,
                            kind: LocalFrameControlKindV1::InactiveTrap,
                        });
                        sink = Some(inactive);
                    }
                    (
                        selected,
                        args,
                        successor,
                        LocalFrameControlKindV1::Selected {
                            condition: *condition,
                            value,
                            successor,
                            target: selected,
                            inactive,
                        },
                    )
                }
                _ => return Err(refusal(function_ordinal, None, Reason::ControlFlow)),
            };
        let next = chain_block(&workspace.blocks, target, budget)?;
        if workspace.blocks[next].state != 0 {
            return Err(refusal(function_ordinal, None, Reason::ControlFlow));
        }
        let target_block = &body.blocks[workspace.blocks[next].ordinal];
        stage_edge(
            function_ordinal,
            current,
            successor,
            target_block,
            arguments,
            sequence,
            workspace,
            budget,
        )?;
        workspace.control.push(LocalFrameControlV1 {
            function_ordinal,
            block: current,
            kind,
        });
        current = target;
    }
    // Every actual block is either executed once or the one exact inactive sink.
    for row in &workspace.blocks {
        budget.charge_work(1)?;
        if row.state == 0 {
            return Err(refusal(function_ordinal, None, Reason::ControlFlow));
        }
    }
    finish_cells(function_ordinal, &mut workspace.memory, budget)?;
    Ok(function)
}

/// Opt-in finite local-frame chain classification. The original entry above
/// deliberately retains its one-block boundary; existing retained consumers
/// cannot silently drop these additional control/substitution obligations.
///
/// No raw summary, source admission, interprocedural caller, or output relation
/// is changed by this API. Scratch/result rows use the same caller ledger and
/// cleanup/error precedence as the original local-memory scope.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::CheckedLocalFrameChainV1;
/// let fabricated = CheckedLocalFrameChainV1 {};
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::*;
/// fn escape(verified: VerifiedKernelIrModuleV1<'_>,
///           budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let mut escaped = None;
///     with_checked_local_frame_chain_function_v1(verified, 0, budget, |checked, _| {
///         escaped = Some(checked);
///         Ok(())
///     }).unwrap();
///     drop(escaped);
/// }
/// ```
pub fn with_checked_local_frame_chain_function_v1<'module>(
    verified: VerifiedKernelIrModuleV1<'module>,
    function_ordinal: usize,
    budget: &mut Budget<'_>,
    next: impl for<'scope> FnOnce(
        CheckedLocalFrameChainV1<'scope, 'module>,
        &mut Budget<'_>,
    ) -> Result<(), LocalFrameErrorV1>,
) -> Result<(), LocalFrameErrorV1> {
    let incoming = budget.storage();
    let ledger = budget as *const Budget<'_> as usize;
    let headers = size_of::<ChainWorkspace<'_>>()
        .checked_add(size_of::<CheckedLocalFrameChainV1<'_, '_>>())
        .ok_or(ResourceError::Arithmetic)?;
    budget.charge_work(4)?;
    let work_ledger = budget.work_ledger_identity_v1();
    budget.reserve_storage(headers)?;
    let mut workspace = ChainWorkspace {
        memory: Workspace::new(),
        control: Vec::new(),
        edge_bindings: Vec::new(),
        blocks: Vec::new(),
    };
    let mut retained_floor = None;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(8)?;
        let function = verified
            .module()
            .functions
            .get(function_ordinal)
            .filter(|function| function.body.is_some())
            .ok_or_else(|| refusal(function_ordinal, None, LocalFrameRefusalReasonV1::Function))?;
        let function = derive_chain(function, function_ordinal, &mut workspace, budget)?;
        retained_floor = Some(budget.storage());
        let checked = CheckedLocalFrameChainV1 {
            memory: CheckedLocalFrameV1 {
                module: verified.module(),
                function,
                function_ordinal,
                allocations: &workspace.memory.allocations,
                accesses: &workspace.memory.accesses,
                ledger,
                work_ledger,
                floor: budget.storage(),
            },
            control: &workspace.control,
            edge_bindings: &workspace.edge_bindings,
        };
        next(checked, budget)
    }));
    if budget.work_ledger_identity_v1() != work_ledger {
        drop(workspace);
        return Err(ResourceError::Accounting.into());
    }
    let accounting = budget.storage() < retained_floor.unwrap_or(incoming + headers);
    drop(workspace);
    let cleanup = budget.rollback_storage(incoming);
    if accounting {
        return Err(ResourceError::Accounting.into());
    }
    cleanup?;
    match outcome {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    }
}

#[cfg(test)]
#[path = "chain_v1_tests.rs"]
mod tests;
