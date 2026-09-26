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

#[path = "production_execution_discharge_core_v29.rs"]
mod erasure_core;
#[path = "production_execution_discharge_v18.rs"]
mod storage_profile;
pub use storage_profile::*;

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

impl From<erasure_core::RuleError> for DischargeError {
    fn from(error: erasure_core::RuleError) -> Self {
        match error {
            erasure_core::RuleError::Resource(error) => Self::Resource(error),
            erasure_core::RuleError::NoExecution => Self::NoExecution,
            erasure_core::RuleError::UnsupportedExecution => Self::UnsupportedExecution,
            erasure_core::RuleError::ReplayMismatch => Self::ReplayMismatch,
        }
    }
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
    let count = erasure_core::preflight(input.module(), input.canonical_bytes().len(), budget)?;
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
    erasure_core::erase(
        &mut candidate,
        input.canonical_bytes().len(),
        count,
        &mut erased_operations,
        budget,
    )?;
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
    erasure_core::replay(
        input.module(),
        input.canonical_bytes().len(),
        output.module(),
        output.canonical().canonical_bytes().len(),
        rows,
        erasure_core::TableProfile::LegacyEmpty,
        budget,
    )
    .map_err(DischargeError::from)
}

#[cfg(test)]
#[path = "production_execution_discharge_v29_tests.rs"]
mod tests;

// Only fixed table headers are inspected. No row equality or graph walk occurs.
fn execution_discharge_legacy_storage_tables_v29(
    input: &[fe2o3_kernel_ir::StorageLayoutV1],
    output: &[fe2o3_kernel_ir::StorageLayoutV1],
) -> bool {
    input.is_empty() && output.is_empty()
}

#[cfg(test)]
mod legacy_storage_schema_tests {
    use fe2o3_kernel_ir::{Module, ScalarType, StorageLayoutKindV1, StorageLayoutV1};

    fn eligible(input: &Module, output: &Module) -> bool {
        super::execution_discharge_legacy_storage_tables_v29(
            &input.storage_layouts,
            &output.storage_layouts,
        )
    }

    #[test]
    fn production_execution_discharge_v29_refuses_nonempty_storage_tables() {
        let empty = Module::new("ordinary");
        assert!(eligible(&empty, &empty));
        let mut occupied = empty.clone();
        occupied.storage_layouts.push(StorageLayoutV1 {
            size: 1,
            alignment: 1,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U8),
        });
        assert!(!eligible(&occupied, &empty));
        assert!(!eligible(&empty, &occupied));
        // Equal, structurally valid tables are still outside the old profile.
        assert!(!eligible(&occupied, &occupied));
        let mut other = occupied.clone();
        other.storage_layouts[0].kind = StorageLayoutKindV1::Scalar(ScalarType::I8);
        assert!(!eligible(&occupied, &other));
        // This predicate proves schema eligibility only, not payload equality.
    }
}
