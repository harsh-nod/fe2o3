//! Checked erasure of lifecycle-only operations, not source or launch authority.

use std::{error::Error, fmt, mem::size_of};

use fe2o3_kernel_ir::{
    BasicBlock, CanonicalKernelIrReplayAdmissionErrorV12, CanonicalKernelIrReplayAdmissionErrorV15,
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    ExecutionOperationV15, Function, FunctionBody, Module, OperationKind,
    VerifiedCanonicalKernelIrIdentityV15, VerifiedCanonicalKernelIrModuleV12,
    VerifiedCanonicalKernelIrModuleV15,
};

type Budget<'a> = CanonicalKernelIrVerificationResourceBudgetV1<'a>;
type ResourceError = CanonicalKernelIrVerificationResourceErrorV1;
type DischargeError = ProductionExecutionDischargeErrorV29;

/// The closed set of lifecycle-only operations erased by this rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionExecutionErasureKindV29 {
    /// Issue a nominal context; no physical instruction is emitted.
    ContextIssue,
    /// Acquire a nominal workgroup from its context.
    WorkgroupDerive,
    /// End a workgroup scope with no tile or fragment descendants.
    ScopeEnd,
}

/// One exact input operation ordinal, not a source or instance identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionExecutionErasureV29 {
    function_ordinal: usize,
    block_ordinal: usize,
    operation_ordinal: usize,
    kind: ProductionExecutionErasureKindV29,
}

impl ProductionExecutionErasureV29 {
    /// Index in the input module's function vector.
    pub const fn function_ordinal(self) -> usize {
        self.function_ordinal
    }

    /// Index in that function's block vector.
    pub const fn block_ordinal(self) -> usize {
        self.block_ordinal
    }

    /// Index in that input block's operation vector, before erasure.
    pub const fn operation_ordinal(self) -> usize {
        self.operation_ordinal
    }

    /// Independently replayed erasure rule at this location.
    pub const fn kind(self) -> ProductionExecutionErasureKindV29 {
        self.kind
    }
}

/// Conservative retained output owner, wrapper header and erasure vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionExecutionDischargeStorageV29 {
    retained: usize,
}

impl ProductionExecutionDischargeStorageV29 {
    /// Reserve this transfer before allocating while the returned owner lives.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// A closed refusal from local execution erasure or fresh physical admission.
#[derive(Debug)]
pub enum ProductionExecutionDischargeErrorV29 {
    /// Copying the exact immutable V15 subject failed.
    Input(CanonicalKernelIrReplayAdmissionErrorV15),
    /// Fresh physical V12 admission failed.
    Output(CanonicalKernelIrReplayAdmissionErrorV12),
    /// Work, coexistence storage, arithmetic or allocation admission failed.
    Resource(CanonicalKernelIrVerificationResourceErrorV1),
    /// Ordinary-only modules belong to the existing physical pipeline.
    NoExecution,
    /// This rule does not lower tiles, fragments or descendant disposal.
    UnsupportedExecution,
    /// Independent replay found a changed ordinary graph or erasure roster.
    ReplayMismatch,
}

impl From<ResourceError> for DischargeError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for DischargeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => write!(formatter, "execution input copy failed: {error}"),
            Self::Output(error) => write!(formatter, "execution output admission failed: {error}"),
            Self::Resource(error) => error.fmt(formatter),
            Self::NoExecution => {
                formatter.write_str("execution discharge has no execution subject")
            }
            Self::UnsupportedExecution => formatter
                .write_str("execution discharge requires a separate tile/fragment lowering"),
            Self::ReplayMismatch => formatter.write_str("execution erasure replay mismatch"),
        }
    }
}

impl Error for DischargeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::NoExecution | Self::UnsupportedExecution | Self::ReplayMismatch => None,
        }
    }
}

/// Immutable result of an exact, independently replayed graph erasure.
///
/// V15 admission establishes same-function lifecycle validity. This rule erases
/// only its three lifecycle-only operations, then freshly admits the complete
/// output as V12 and replays every retained field and operation. It establishes
/// neither Rust source correspondence nor cross-invocation memory, numerical,
/// target, protected-proof or safe-launch correctness. Production activation
/// additionally requires authenticated source and complete instance custody.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionExecutionDischargeV29;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionExecutionDischargeV29>();
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionExecutionDischargeV29;
/// fn mutate(owner: &mut ProductionExecutionDischargeV29) {
///     owner.output().module().functions.clear();
/// }
/// ```
#[derive(Debug)]
pub struct ProductionExecutionDischargeV29 {
    input_identity: VerifiedCanonicalKernelIrIdentityV15,
    output: VerifiedCanonicalKernelIrModuleV12,
    erased_operations: Vec<ProductionExecutionErasureV29>,
}

impl ProductionExecutionDischargeV29 {
    /// Discharge one exact verified input under one cumulative resource ledger.
    ///
    /// Keep the input owner's reservation live throughout. Every Result path
    /// restores the incoming storage floor, without refunding work, peak or
    /// first-denial history. Reserve the returned storage transfer before any
    /// further allocation. Failed payloads drop before floor restoration; owned
    /// verifier diagnostics transfer to the caller as in canonical admission.
    /// This accounts conservative logical payload, not RSS or unwind cleanup.
    pub fn try_discharge(
        input: &VerifiedCanonicalKernelIrModuleV15,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(Self, ProductionExecutionDischargeStorageV29), ProductionExecutionDischargeErrorV29>
    {
        let floor = budget.storage();
        let result = discharge(input, budget);
        let release = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ResourceError::Accounting)?;
        budget.release_storage(release)?;
        result
    }

    /// Identity of the complete immutable V15 subject, including erased values.
    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV15 {
        &self.input_identity
    }

    /// Freshly verified physical graph; not a source or safe-launch certificate.
    pub const fn output(&self) -> &VerifiedCanonicalKernelIrModuleV12 {
        &self.output
    }

    /// Exact input-order roster checked independently against both graphs.
    pub fn erased_operations(&self) -> &[ProductionExecutionErasureV29] {
        &self.erased_operations
    }
}

fn erasure_kind(
    kind: &OperationKind,
) -> Result<Option<ProductionExecutionErasureKindV29>, DischargeError> {
    use ProductionExecutionErasureKindV29 as Kind;
    match kind {
        OperationKind::Execution(ExecutionOperationV15::ContextIssue) => {
            Ok(Some(Kind::ContextIssue))
        }
        OperationKind::Execution(ExecutionOperationV15::WorkgroupDerive { .. }) => {
            Ok(Some(Kind::WorkgroupDerive))
        }
        OperationKind::Execution(ExecutionOperationV15::ScopeEnd { discarded, .. })
            if discarded.is_empty() =>
        {
            Ok(Some(Kind::ScopeEnd))
        }
        OperationKind::Execution(_) => Err(DischargeError::UnsupportedExecution),
        _ => Ok(None),
    }
}

fn discharge(
    input: &VerifiedCanonicalKernelIrModuleV15,
    budget: &mut Budget<'_>,
) -> Result<
    (
        ProductionExecutionDischargeV29,
        ProductionExecutionDischargeStorageV29,
    ),
    DischargeError,
> {
    // The exact encoding bounds all vector walks, including empty containers.
    budget.charge_work(input.canonical_bytes().len())?;
    let mut count = 0usize;
    for function in &input.module().functions {
        if let Some(body) = &function.body {
            for block in &body.blocks {
                for operation in &block.operations {
                    if erasure_kind(&operation.kind)?.is_some() {
                        count = count.checked_add(1).ok_or(ResourceError::Arithmetic)?;
                    }
                }
            }
        }
    }
    if count == 0 {
        return Err(DischargeError::NoExecution);
    }
    let wrapper_bytes = size_of::<ProductionExecutionDischargeV29>()
        .checked_sub(size_of::<VerifiedCanonicalKernelIrModuleV12>())
        .ok_or(ResourceError::Accounting)?;
    let rows_bytes = count
        .checked_mul(size_of::<ProductionExecutionErasureV29>())
        .ok_or(ResourceError::Arithmetic)?;
    let extra = wrapper_bytes
        .checked_add(rows_bytes)
        .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(extra)?;
    let mut erased_operations = Vec::new();
    erased_operations
        .try_reserve_exact(count)
        .map_err(|_| ResourceError::Allocation)?;
    if erased_operations.capacity() != count {
        return Err(ResourceError::Accounting.into());
    }
    let (mut candidate, candidate_storage) = input
        .copy_module_for_transformation_v15(budget)
        .map_err(DischargeError::Input)?;
    budget.reserve_storage(candidate_storage.retained_storage())?;
    budget.charge_work(input.canonical_bytes().len())?;
    for (function_ordinal, function) in candidate.functions.iter_mut().enumerate() {
        if let Some(body) = &mut function.body {
            for (block_ordinal, block) in body.blocks.iter_mut().enumerate() {
                let mut operation_ordinal = 0;
                block.operations.retain(|operation| {
                    let ordinal = operation_ordinal;
                    operation_ordinal += 1;
                    // Preflight checked the immutable inverse before any mutation.
                    if let Ok(Some(kind)) = erasure_kind(&operation.kind) {
                        erased_operations.push(ProductionExecutionErasureV29 {
                            function_ordinal,
                            block_ordinal,
                            operation_ordinal: ordinal,
                            kind,
                        });
                        false
                    } else {
                        true
                    }
                });
            }
        }
    }
    if erased_operations.len() != count {
        return Err(DischargeError::ReplayMismatch);
    }
    let (output, output_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &candidate, budget,
        )
        .map_err(DischargeError::Output)?;
    budget.reserve_storage(output_storage.retained_storage())?;
    drop(candidate);
    budget.release_storage(candidate_storage.retained_storage())?;
    replay_erasure(input, &output, &erased_operations, budget)?;
    let retained = extra
        .checked_add(output_storage.retained_storage())
        .ok_or(ResourceError::Arithmetic)?;
    Ok((
        ProductionExecutionDischargeV29 {
            input_identity: *input.identity(),
            output,
            erased_operations,
        },
        ProductionExecutionDischargeStorageV29 { retained },
    ))
}

fn replay_erasure(
    input: &VerifiedCanonicalKernelIrModuleV15,
    output: &VerifiedCanonicalKernelIrModuleV12,
    rows: &[ProductionExecutionErasureV29],
    budget: &mut Budget<'_>,
) -> Result<(), DischargeError> {
    let work = input
        .canonical_bytes()
        .len()
        .checked_add(output.canonical().canonical_bytes().len())
        .and_then(|value| value.checked_add(rows.len()))
        .ok_or(ResourceError::Arithmetic)?;
    budget.charge_work(work)?;
    // Exhaustive destructuring makes newly added IR fields require a replay decision.
    let Module {
        id,
        functions,
        kernels,
        required_capabilities,
    } = input.module();
    let Module {
        id: out_id,
        functions: out_functions,
        kernels: out_kernels,
        required_capabilities: out_capabilities,
    } = output.module();
    if (id, kernels, required_capabilities) != (out_id, out_kernels, out_capabilities)
        || functions.len() != out_functions.len()
    {
        return Err(DischargeError::ReplayMismatch);
    }
    let mut remaining = rows.iter();
    for (function_ordinal, (function, out_function)) in
        functions.iter().zip(out_functions).enumerate()
    {
        let Function {
            id,
            signature,
            role,
            body,
            required_capabilities,
        } = function;
        let Function {
            id: out_id,
            signature: out_signature,
            role: out_role,
            body: out_body,
            required_capabilities: out_capabilities,
        } = out_function;
        if (id, signature, role, required_capabilities)
            != (out_id, out_signature, out_role, out_capabilities)
        {
            return Err(DischargeError::ReplayMismatch);
        }
        let (body, out_body) = match (body, out_body) {
            (Some(body), Some(out_body)) => (body, out_body),
            (None, None) => continue,
            _ => return Err(DischargeError::ReplayMismatch),
        };
        let FunctionBody { parameters, blocks } = body;
        let FunctionBody {
            parameters: out_parameters,
            blocks: out_blocks,
        } = out_body;
        if parameters != out_parameters || blocks.len() != out_blocks.len() {
            return Err(DischargeError::ReplayMismatch);
        }
        for (block_ordinal, (block, out_block)) in blocks.iter().zip(out_blocks).enumerate() {
            let BasicBlock {
                id,
                parameters,
                operations,
                terminator,
            } = block;
            let BasicBlock {
                id: out_id,
                parameters: out_parameters,
                operations: out_operations,
                terminator: out_terminator,
            } = out_block;
            if (id, parameters, terminator) != (out_id, out_parameters, out_terminator) {
                return Err(DischargeError::ReplayMismatch);
            }
            let mut physical = out_operations.iter();
            for (operation_ordinal, operation) in operations.iter().enumerate() {
                // Deliberately independent of the producer's erasure classifier.
                let kind = match &operation.kind {
                    OperationKind::Execution(ExecutionOperationV15::ContextIssue) => {
                        ProductionExecutionErasureKindV29::ContextIssue
                    }
                    OperationKind::Execution(ExecutionOperationV15::WorkgroupDerive { .. }) => {
                        ProductionExecutionErasureKindV29::WorkgroupDerive
                    }
                    OperationKind::Execution(ExecutionOperationV15::ScopeEnd {
                        discarded, ..
                    }) if discarded.is_empty() => ProductionExecutionErasureKindV29::ScopeEnd,
                    OperationKind::Execution(_) => return Err(DischargeError::ReplayMismatch),
                    _ => {
                        if physical.next() != Some(operation) {
                            return Err(DischargeError::ReplayMismatch);
                        }
                        continue;
                    }
                };
                let expected = ProductionExecutionErasureV29 {
                    function_ordinal,
                    block_ordinal,
                    operation_ordinal,
                    kind,
                };
                if remaining.next() != Some(&expected) {
                    return Err(DischargeError::ReplayMismatch);
                }
            }
            if physical.next().is_some() {
                return Err(DischargeError::ReplayMismatch);
            }
        }
    }
    if remaining.next().is_some() || rows.is_empty() {
        return Err(DischargeError::ReplayMismatch);
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_execution_discharge_v29_tests.rs"]
mod tests;
