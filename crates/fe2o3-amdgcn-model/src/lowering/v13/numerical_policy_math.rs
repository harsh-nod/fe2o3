use super::*;
use fe2o3_kernel_ir::{
    AssemblyOperandKind, BlockId, ExecutionCapabilityTypeV1, F32MathFunction, FloatOperation,
    FunctionRole, MAX_BLOCKS_V1, MAX_FUNCTIONS_V1, MAX_MODULE_BYTES_V1, MAX_OPERATIONS_V1,
    NumericalModeV1, NumericalPolicyMathOperationV1 as Math,
};
use std::mem::size_of;

type Site = (BlockId, usize);

struct Entry<'a> {
    site: Site,
    operation: &'a Operation,
}

#[derive(Clone, Copy)]
struct Key {
    value: ValueId,
    entry: usize,
}

// This is scratch projection state for the already verified canonical module,
// not a numerical, lifetime, or provider proof. Dominance remains the canonical
// verifier's responsibility. Complete indexing makes emission order irrelevant.
pub(super) struct Plan<'a> {
    module: &'a Module,
    function_index: usize,
    entries: Vec<Entry<'a>>,
    keys: Vec<Key>,
}

impl<'a> Plan<'a> {
    pub(super) fn new(
        module: &'a Module,
        function_index: usize,
        types: &BTreeMap<ValueId, Type>,
    ) -> Result<Self, LoweringErrors> {
        Self::with_source_limit(module, function_index, types, MAX_MODULE_BYTES_V1)
    }

    fn with_source_limit(
        module: &'a Module,
        function_index: usize,
        types: &BTreeMap<ValueId, Type>,
        source_limit: usize,
    ) -> Result<Self, LoweringErrors> {
        let function = module
            .functions
            .get(function_index)
            .ok_or_else(|| incomplete(module, "policy-math projection has no original function"))?;
        let body = function.body.as_ref().ok_or_else(|| {
            incomplete(module, "policy-math projection requires an original body")
        })?;
        if body.blocks.len() > MAX_BLOCKS_V1 {
            return Err(resource(module));
        }
        // Each counted block/op/result/operand has at least one distinct wire
        // byte. The existing canonical byte bound thus bounds aggregate work,
        // without mistaking the per-block operation limit for a function limit.
        let mut units = 0usize;
        let mut count = 0usize;
        for block in &body.blocks {
            charge(module, &mut units, 1, source_limit)?;
            if block.operations.len() > MAX_OPERATIONS_V1 {
                return Err(resource(module));
            }
            for operation in &block.operations {
                charge(module, &mut units, 1, source_limit)?;
                charge(module, &mut units, operation.results.len(), source_limit)?;
                operation_uses(operation, |_| charge(module, &mut units, 1, source_limit))?;
                if indexed(operation) {
                    count = count.checked_add(1).ok_or_else(|| resource(module))?;
                }
            }
            if let Some(terminator) = &block.terminator {
                charge(module, &mut units, 1, source_limit)?;
                terminator_uses(terminator, |_| charge(module, &mut units, 1, source_limit))?;
            }
        }
        let _storage_bytes = storage_bytes(module, count, source_limit)?;
        let mut entries = Vec::new();
        let mut keys = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| resource(module))?;
        keys.try_reserve_exact(count)
            .map_err(|_| resource(module))?;
        for block in &body.blocks {
            for (index, operation) in block.operations.iter().enumerate() {
                if indexed(operation) {
                    if operation.results.len() != 1 {
                        return Err(incomplete(
                            module,
                            "policy-math issuer/result roster changed",
                        ));
                    }
                    entries.push(Entry {
                        site: (block.id, index),
                        operation,
                    });
                }
            }
        }
        entries.sort_unstable_by_key(|entry| entry.site);
        if entries.windows(2).any(|pair| pair[0].site == pair[1].site) {
            return Err(incomplete(module, "policy-math source site is duplicated"));
        }
        for (entry, item) in entries.iter().enumerate() {
            keys.push(Key {
                value: item.operation.results[0].id,
                entry,
            });
        }
        keys.sort_unstable_by_key(|key| key.value);
        if keys.windows(2).any(|pair| pair[0].value == pair[1].value) {
            return Err(incomplete(module, "policy-math issuer value is duplicated"));
        }
        let plan = Self {
            module,
            function_index,
            entries,
            keys,
        };
        for entry in &plan.entries {
            plan.check_entry(entry, types)?;
        }
        // Inspect every original occurrence, not merely the set of used IDs.
        // Only three positional logical edges are physically erasable.
        for block in &body.blocks {
            for operation in &block.operations {
                let mut position = 0usize;
                operation_uses(operation, |value| {
                    let result = plan.check_use(value, Some((operation, position)));
                    position += 1;
                    result
                })?;
            }
            if let Some(terminator) = &block.terminator {
                terminator_uses(terminator, |value| plan.check_use(value, None))?;
            }
        }
        Ok(plan)
    }

    fn definition(&self, value: ValueId) -> Option<&Operation> {
        let index = self
            .keys
            .binary_search_by_key(&value, |key| key.value)
            .ok()?;
        Some(self.entries[self.keys[index].entry].operation)
    }

    fn issuer(&self, value: ValueId) -> Option<&ExecutionCapabilityOpV1> {
        let OperationKind::ExecutionCapability(contract) = &self.definition(value)?.kind else {
            return None;
        };
        Some(contract)
    }

    fn check_entry(
        &self,
        entry: &Entry<'_>,
        types: &BTreeMap<ValueId, Type>,
    ) -> Result<(), LoweringErrors> {
        let operation = entry.operation;
        let OperationKind::ExecutionCapability(contract) = &operation.kind else {
            return Ok(()); // Context issuance was authenticated by canonical verification.
        };
        let function = &self.module.functions[self.function_index];
        let valid = contract.is_complete()
            && contract.provenance.root == function.id
            && function.role == FunctionRole::KernelEntry;
        let result = &operation.results[0];
        let capability_result = |source_type, role| {
            result.ty
                == Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
                    source_type,
                    role,
                    provenance: contract.provenance.clone(),
                    workgroup_brand: None,
                    epoch: None,
                })
        };
        let valid = valid && match contract.operation {
            ExecutionCapabilityOperationV1::NumericalPolicyIssue { capability, policy, mode, .. } => {
                self.is_context(contract.operands[0], contract, types)
                    && capability_result(capability, ExecutionCapabilityRoleV1::NumericalPolicy { policy, mode })
            }
            ExecutionCapabilityOperationV1::NumericalPolicyMath(math) => match math {
                Math::MathDerive { binding, .. } => {
                    self.is_context(contract.operands[0], contract, types)
                        && capability_result(binding.math, ExecutionCapabilityRoleV1::NumericalPolicyMathSource(binding))
                }
                Math::Bind { binding } => {
                    let math = self.issuer(contract.operands[0]);
                    let policy = self.issuer(contract.operands[1]);
                    math.zip(policy).is_some_and(|(math, policy)| {
                        matches!(math.operation, ExecutionCapabilityOperationV1::NumericalPolicyMath(Math::MathDerive { binding: issued, .. }) if issued == binding)
                            && matches!(policy.operation, ExecutionCapabilityOperationV1::NumericalPolicyIssue { capability, policy, mode, .. }
                                if capability == binding.capability && policy == binding.policy && mode == binding.mode)
                            && same_custody(math, contract) && same_custody(policy, contract)
                            && math.operands == policy.operands
                    }) && capability_result(binding.bound, ExecutionCapabilityRoleV1::NumericalPolicyMathBound(binding))
                }
                Math::F32 { binding, function, .. } => {
                    self.issuer(contract.operands[0]).is_some_and(|issuer| {
                        matches!(issuer.operation, ExecutionCapabilityOperationV1::NumericalPolicyMath(Math::Bind { binding: issued }) if issued == binding)
                            && same_custody(issuer, contract)
                    }) && math.numerical_requirements() == Some((NumericalModeV1::StrictIeee, function.required_implementation()))
                        && result.ty == Type::Scalar(ScalarType::F32)
                        && contract.operands[1..].iter().all(|value| types.get(value) == Some(&Type::Scalar(ScalarType::F32)))
                }
            },
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(incomplete(
                self.module,
                "policy-math exact issuer/type/custody relation changed",
            ))
        }
    }

    fn is_context(
        &self,
        value: ValueId,
        consumer: &ExecutionCapabilityOpV1,
        types: &BTreeMap<ValueId, Type>,
    ) -> bool {
        self.definition(value)
            .is_some_and(|operation| matches!(operation.kind, OperationKind::KernelContextIssue(_)))
            && matches!(types.get(&value), Some(Type::KernelContext(context))
                if context.root() == &consumer.provenance.root
                    && context.kernel_marker() == &consumer.provenance.kernel_marker
                    && context.target() == &consumer.provenance.target_brand
                    && context.launch() == &consumer.provenance.launch_brand)
    }

    fn check_use(
        &self,
        value: ValueId,
        consumer: Option<(&Operation, usize)>,
    ) -> Result<(), LoweringErrors> {
        let Some(issuer) = self.issuer(value) else {
            return Ok(());
        };
        let kind = match issuer.operation {
            ExecutionCapabilityOperationV1::NumericalPolicyIssue { .. } => 1,
            ExecutionCapabilityOperationV1::NumericalPolicyMath(Math::MathDerive { .. }) => 0,
            ExecutionCapabilityOperationV1::NumericalPolicyMath(Math::Bind { .. }) => 2,
            _ => return Ok(()),
        };
        let allowed = consumer.is_some_and(|(operation, position)| {
            matches!(
                (&operation.kind, kind, position),
                (
                    OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                        operation: ExecutionCapabilityOperationV1::NumericalPolicyMath(
                            Math::Bind { .. }
                        ),
                        ..
                    }),
                    0,
                    0
                ) | (
                    OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                        operation: ExecutionCapabilityOperationV1::NumericalPolicyMath(
                            Math::Bind { .. }
                        ),
                        ..
                    }),
                    1,
                    1
                ) | (
                    OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                        operation: ExecutionCapabilityOperationV1::NumericalPolicyMath(
                            Math::F32 { .. }
                        ),
                        ..
                    }),
                    2,
                    0
                )
            )
        });
        if allowed {
            Ok(())
        } else {
            Err(incomplete(
                self.module,
                "policy-math logical token has an unapproved operation or terminator use",
            ))
        }
    }

    pub(super) fn approves(
        &self,
        module: &Module,
        function_index: usize,
        site: Site,
        operation: &Operation,
    ) -> bool {
        std::ptr::eq(self.module, module)
            && self.function_index == function_index
            && self
                .entries
                .binary_search_by_key(&site, |entry| entry.site)
                .ok()
                .is_some_and(|index| self.entries[index].operation == operation)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower(
        &self,
        module: &Module,
        function_index: usize,
        site: Site,
        operation: &Operation,
        contract: &ExecutionCapabilityOpV1,
        aliases: &mut BTreeMap<ValueId, ExecutionAliasV1>,
        output: &mut Vec<Operation>,
    ) -> Result<(), LoweringErrors> {
        if !self.approves(module, function_index, site, operation)
            || !matches!(&operation.kind, OperationKind::ExecutionCapability(original) if original == contract)
        {
            return Err(incomplete(
                module,
                "policy-math projection does not match its original function/site/operation",
            ));
        }
        match contract.operation {
            ExecutionCapabilityOperationV1::NumericalPolicyMath(
                math @ Math::F32 { function, .. },
            ) => {
                let Some((NumericalModeV1::StrictIeee, implementation)) =
                    math.numerical_requirements()
                else {
                    return Err(incomplete(
                        module,
                        "policy-math implementation is not exact strict FP32",
                    ));
                };
                // Positional source operands, never filtered erased aliases.
                let physical = FloatOperation::F32Math {
                    function,
                    implementation,
                    arguments: contract.operands[1..].to_vec(),
                }
                .operation(operation.results[0].id);
                output.try_reserve(1).map_err(|_| resource(module))?;
                output.push(physical);
                Ok(())
            }
            ExecutionCapabilityOperationV1::NumericalPolicyMath(
                Math::MathDerive { .. } | Math::Bind { .. },
            ) => erase_capability_results(module, operation, aliases),
            _ => Err(incomplete(
                module,
                "policy-math projection selected a foreign operation",
            )),
        }
    }
}

fn indexed(operation: &Operation) -> bool {
    matches!(
        operation.kind,
        OperationKind::KernelContextIssue(_)
            | OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                operation: ExecutionCapabilityOperationV1::NumericalPolicyMath(_)
                    | ExecutionCapabilityOperationV1::NumericalPolicyIssue { .. },
                ..
            })
    )
}

pub(super) fn append_declarations(
    module: &Module,
    helpers: &mut Vec<Function>,
) -> Result<(), LoweringErrors> {
    const FUNCTIONS: [F32MathFunction; 13] = [
        F32MathFunction::Sqrt,
        F32MathFunction::FusedMultiplyAdd,
        F32MathFunction::Floor,
        F32MathFunction::Ceil,
        F32MathFunction::Truncate,
        F32MathFunction::RoundTiesEven,
        F32MathFunction::Sin,
        F32MathFunction::Cos,
        F32MathFunction::Exp,
        F32MathFunction::Exp2,
        F32MathFunction::Ln,
        F32MathFunction::Log2,
        F32MathFunction::Log10,
    ];
    let mut required = [false; FUNCTIONS.len()];
    for operation in module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        if let OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
            operation:
                ExecutionCapabilityOperationV1::NumericalPolicyMath(Math::F32 { function, .. }),
            ..
        }) = &operation.kind
        {
            let index = FUNCTIONS
                .iter()
                .position(|candidate| candidate == function)
                .ok_or_else(|| {
                    incomplete(
                        module,
                        "policy-math declaration is outside the closed FP32 roster",
                    )
                })?;
            required[index] = true;
        }
    }
    // At most thirteen declarations and thirteen function-roster scans. These
    // are internal KIR signatures, not provider selection or link evidence.
    for (function, required) in FUNCTIONS.into_iter().zip(required) {
        if !required {
            continue;
        }
        let declaration = FloatOperation::F32Math {
            function,
            implementation: function.required_implementation(),
            arguments: Vec::new(),
        }
        .declaration();
        let mut existing = module
            .functions
            .iter()
            .chain(helpers.iter())
            .filter(|candidate| candidate.id == declaration.id);
        if let Some(existing_declaration) = existing.next() {
            if existing_declaration != &declaration || existing.next().is_some() {
                return Err(incomplete(
                    module,
                    "policy-math reserved declaration does not match its exact float contract",
                ));
            }
        } else {
            module
                .functions
                .len()
                .checked_add(helpers.len())
                .and_then(|count| count.checked_add(1))
                .filter(|count| *count <= MAX_FUNCTIONS_V1)
                .ok_or_else(|| resource(module))?;
            helpers.try_reserve(1).map_err(|_| resource(module))?;
            helpers.push(declaration);
        }
    }
    Ok(())
}

fn same_custody(a: &ExecutionCapabilityOpV1, b: &ExecutionCapabilityOpV1) -> bool {
    a.provenance == b.provenance
        && a.workgroup_brand.is_none()
        && b.workgroup_brand.is_none()
        && a.epoch_before.is_none()
        && b.epoch_before.is_none()
        && a.epoch_after.is_none()
        && b.epoch_after.is_none()
}

fn charge(
    module: &Module,
    used: &mut usize,
    amount: usize,
    limit: usize,
) -> Result<(), LoweringErrors> {
    let next = used
        .checked_add(amount)
        .filter(|next| *next <= limit)
        .ok_or_else(|| resource(module))?;
    *used = next;
    Ok(())
}

fn storage_bytes(
    module: &Module,
    count: usize,
    source_limit: usize,
) -> Result<usize, LoweringErrors> {
    if count > source_limit {
        return Err(resource(module));
    }
    // Requested Vec backing bytes, including the sorted ValueId key index.
    // Not an allocator/RSS ceiling. No BTree node estimate or dense ID table.
    count
        .checked_mul(size_of::<Entry<'_>>() + size_of::<Key>())
        .and_then(|bytes| bytes.checked_add(size_of::<Plan<'_>>()))
        .ok_or_else(|| resource(module))
}

fn resource(module: &Module) -> LoweringErrors {
    LoweringErrors::one(
        LoweringLocation::module(module),
        LoweringDiagnosticCode::ResourceLimit,
        "policy-math projection exceeds canonical source/index storage bounds",
    )
}

// Borrow variable-width lists. All other closed variants allocate at most 20
// ValueIds (the scaled matrix operation); changes to the enum require review.
fn operation_uses(
    operation: &Operation,
    mut visit: impl FnMut(ValueId) -> Result<(), LoweringErrors>,
) -> Result<(), LoweringErrors> {
    match &operation.kind {
        OperationKind::Call { arguments, .. } => {
            for value in arguments {
                visit(*value)?;
            }
        }
        OperationKind::ExecutionCapability(contract) => {
            for value in &contract.operands {
                visit(*value)?;
            }
        }
        OperationKind::ReusablePhase(contract) => {
            for value in &contract.operands {
                visit(*value)?;
            }
        }
        OperationKind::InlineAssembly(assembly) => {
            for operand in &assembly.operands {
                match operand.kind {
                    AssemblyOperandKind::Input(value)
                    | AssemblyOperandKind::InOut { input: value, .. } => visit(value)?,
                    AssemblyOperandKind::Output { .. } | AssemblyOperandKind::ImmediateI32(_) => {}
                }
            }
        }
        OperationKind::Constant(_)
        | OperationKind::Intrinsic(_)
        | OperationKind::Barrier(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::KernelContextIssue(_)
        | OperationKind::Wave(_)
        | OperationKind::MemoryIntrinsic(_)
        | OperationKind::Matrix(_)
        | OperationKind::Gfx950LdsTranspose(_)
        | OperationKind::Unary { .. }
        | OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::Cast { .. }
        | OperationKind::Select { .. }
        | OperationKind::Alloca { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. }
        | OperationKind::Load { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::Store { .. }
        | OperationKind::GlobalCapabilityBind(_)
        | OperationKind::GlobalCapabilityIndex(_)
        | OperationKind::Atomic(_) => {
            for value in operation.operands() {
                visit(value)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn test_phase_operands(operation: &Operation, output: &mut Vec<ValueId>) -> Result<(), LoweringErrors> {
    operation_uses(operation, |value| { output.push(value); Ok(()) })
}

fn terminator_uses(
    terminator: &Terminator,
    mut visit: impl FnMut(ValueId) -> Result<(), LoweringErrors>,
) -> Result<(), LoweringErrors> {
    match terminator {
        Terminator::Branch { arguments, .. } | Terminator::Return { values: arguments } => {
            for value in arguments {
                visit(*value)?;
            }
        }
        Terminator::ConditionalBranch {
            condition,
            then_arguments,
            else_arguments,
            ..
        } => {
            visit(*condition)?;
            for value in then_arguments.iter().chain(else_arguments) {
                visit(*value)?;
            }
        }
        Terminator::Switch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            visit(*selector)?;
            for value in cases
                .iter()
                .flat_map(|case| &case.arguments)
                .chain(default_arguments)
            {
                visit(*value)?;
            }
        }
        Terminator::IntegerSwitch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            visit(*selector)?;
            for value in cases
                .iter()
                .flat_map(|case| &case.arguments)
                .chain(default_arguments)
            {
                visit(*value)?;
            }
        }
        Terminator::Unreachable => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests;
