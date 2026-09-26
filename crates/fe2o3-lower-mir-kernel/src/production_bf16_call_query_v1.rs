// Singular observational query for the closed source-authenticated nominal
// helper. No generic call/ABI, ranked, normal, execution or publication authority.

/// A refusal from the bounded nominal source/canonical call query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bf16NominalCallQueryErrorV1 {
    /// The original cumulative ledger refused.
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    /// The exact same-owner association is unavailable.
    Unavailable(&'static str),
    /// Consumer unwound; its payload was dropped before the query refund.
    CallbackPanicked,
}
impl From<ArgumentResourceV1> for Bf16NominalCallQueryErrorV1 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Bf16NominalCallQueryErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BF16 nominal call query: {self:?}")
    }
}
impl std::error::Error for Bf16NominalCallQueryErrorV1 {}
type Bf16CallQueryResultV1<T> = Result<T, Bf16NominalCallQueryErrorV1>;

fn bf16_query_semantic_error_v1(
    error: ProductionSemanticKirErrorV1,
) -> Bf16NominalCallQueryErrorV1 {
    match error {
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => {
            Bf16NominalCallQueryErrorV1::Resource(error)
        }
        _ => Bf16NominalCallQueryErrorV1::Unavailable("nominal component/source relation differs"),
    }
}
fn bf16_query_inventory_error_v1(
    error: fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
) -> Bf16NominalCallQueryErrorV1 {
    match error {
        fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => error.into(),
        _ => Bf16NominalCallQueryErrorV1::Unavailable("canonical inventory relation differs"),
    }
}
fn bf16_query_require_v1(condition: bool, why: &'static str) -> Bf16CallQueryResultV1<()> {
    if condition {
        Ok(())
    } else {
        Err(Bf16NominalCallQueryErrorV1::Unavailable(why))
    }
}

/// A scoped association with the actual source call and canonical Call/Matrix.
/// Ordered components are logical SSA values, never physical Rust FnABI slots.
/// The original immutable owner retains the full-wave and source-coverage
/// checks; this view is not a new convergence, effect, or normal-compilation proof.
///
/// No construction or borrowed escape:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::CheckedBf16NominalCallV1;
/// fn forge() { let _ = CheckedBf16NominalCallV1 {}; }
/// ```
pub struct CheckedBf16NominalCallV1<'a> {
    emission: Bf16CallInstanceEmissionViewV1<'a>,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    source_call: &'a SemanticDirectCallV1,
    caller: &'a CanonicalKirFunctionRefV1<'a>,
    helper: &'a CanonicalKirFunctionRefV1<'a>,
    call: &'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'a>,
    matrix: &'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'a>,
}
impl CheckedBf16NominalCallV1<'_> {
    /// Exact inventory identity, not matching serialized bytes or digest.
    pub fn belongs_to(&self, inventory: &CanonicalKirInventoryV1<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }
    /// Exact retained source call, including its original callee and destination.
    pub const fn source_call(&self) -> &SemanticDirectCallV1 {
        self.source_call
    }
    /// Actual root-qualified caller, resolved by correspondence rather than ordinal.
    pub const fn caller(&self) -> &CanonicalKirFunctionRefV1<'_> {
        self.caller
    }
    /// Actual helper, with twelve logical scalar formals and four F32 results.
    pub const fn helper(&self) -> &CanonicalKirFunctionRefV1<'_> {
        self.helper
    }
    /// Actual canonical Call at the joined source span.
    pub const fn call(&self) -> &fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_> {
        self.call
    }
    /// Actual helper Matrix at the joined source span.
    pub const fn matrix(&self) -> &fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_> {
        self.matrix
    }
    /// Same-owner ordered source/emission components and return permutation.
    pub const fn emission(&self) -> &Bf16CallInstanceEmissionViewV1<'_> {
        &self.emission
    }
}

// No heap graph/table is constructed. Fixed headers plus resolver stack are
// charged before the scope. This is logical accounting, not a native RSS bound.
const BF16_CALL_QUERY_SCRATCH_V1: usize = std::mem::size_of::<CheckedBf16NominalCallV1<'static>>()
    + std::mem::size_of::<MatrixOperation>()
    + std::mem::size_of::<Bf16NominalCallQueryErrorV1>()
    + 4096;
const BF16_CALL_QUERY_ENTRY_WORK_V1: usize = 8;

fn bf16_call_query_scratch_v1<R>() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        BF16_CALL_QUERY_SCRATCH_V1,
        std::mem::size_of::<R>()
            .checked_mul(2)
            .ok_or(ArgumentResourceV1::Arithmetic)?,
    ])
}
fn bf16_query_policy_v1(policy: ProductionHelperSourcePolicyV1) -> Bf16CallQueryResultV1<()> {
    bf16_query_require_v1(
        policy == ProductionHelperSourcePolicyV1::Bf16Nominal,
        "closed nominal helper policy required",
    )
}

fn bf16_call_query_scope_v1<'w, R: Copy + 'static>(
    budget: &mut ArgumentBudgetV1<'w>,
    inspect: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> Bf16CallQueryResultV1<R>,
) -> Bf16CallQueryResultV1<R> {
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *mut _ as usize;
    let scratch = bf16_call_query_scratch_v1::<R>()?;
    budget.reserve_storage(scratch)?;
    let protected = budget.storage();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget.charge_work(BF16_CALL_QUERY_ENTRY_WORK_V1)?;
        inspect(budget)
    }));
    if slot != budget as *mut _ as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < protected
    {
        // Never repair a replaced ledger or hide a caller's parent-floor debit.
        drop(outcome);
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
    let result = match outcome {
        Ok(Ok(_)) if denied => Err(ArgumentResourceV1::Accounting.into()),
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Bf16NominalCallQueryErrorV1::CallbackPanicked)
        }
    };
    // Views, scratch and any panic payload are gone. Callback-owned additional
    // reservations remain; accepted work, peak and sticky denials are untouched.
    budget.release_storage(scratch)?;
    result
}

// Shared exact retained-floor calculation. The legacy query preserves its
// original call order and the inventory's fourteen charged header visits.
fn bf16_nominal_retained_floor_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Bf16CallQueryResultV1<usize> {
    Ok(argument_sum_v1(&[
        owner.retained_analysis_storage_v1(),
        owner.helper_memory.capture.preexisting_storage(),
        inventory
            .retained_storage_v1(budget)
            .map_err(bf16_query_inventory_error_v1)?,
    ])?)
}

include!("production_bf16_nominal_entry_resources_v1.rs");

impl ProductionPreRankedKirOwnerV1 {
    /// Inspect the one closed nominal helper call from this exact owner and
    /// inventory. The actual borrowed source call is required: a cloned but
    /// structurally equal call is refused. No scalar-empty or UnitLocal token
    /// is produced, and all generic/normal consumers keep their refusals.
    ///
    /// Caller reserves retained_analysis_storage_v1(), incoming occurrence
    /// storage and this inventory's receipt on its original cumulative ledger.
    /// The complete floor is checked before the consumer; query scratch is
    /// additional. Only query scratch is refunded on success, error or panic.
    /// Returned values are Copy + 'static; callback allocations must be separately
    /// reserved and remain charged. This is not a security or allocator boundary.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_mir_model::semantic_mir_v1::{
    ///     SemanticFunctionIdV1, SemanticBlockIdV1, SemanticDirectCallV1,
    /// };
    /// fn escape(owner: &ProductionPreRankedKirOwnerV1,
    ///     inventory: &CanonicalKirInventoryV1<'_>, root: SemanticFunctionIdV1,
    ///     block: SemanticBlockIdV1, call: &SemanticDirectCallV1, budget: &mut Budget<'_>) {
    ///     let mut saved = None;
    ///     owner.with_checked_bf16_nominal_call_v1(
    ///         inventory, root, root, block, call, budget, |view, _| {
    ///             saved = Some(view);
    ///             Ok(())
    ///         }).unwrap();
    ///     drop(saved);
    /// }
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn with_checked_bf16_nominal_call_v1<'w, R: Copy + 'static>(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        root: SemanticFunctionIdV1,
        caller: SemanticFunctionIdV1,
        source_block: SemanticBlockIdV1,
        source_call: &SemanticDirectCallV1,
        budget: &mut ArgumentBudgetV1<'w>,
        inspect: impl for<'s> FnOnce(
            &CheckedBf16NominalCallV1<'s>,
            &mut ArgumentBudgetV1<'w>,
        ) -> Bf16CallQueryResultV1<R>,
    ) -> Bf16CallQueryResultV1<R> {
        bf16_call_query_scope_v1(budget, |budget| {
            bf16_query_policy_v1(self.helper_source_policy_v1())?;
            bf16_query_require_v1(
                inventory.belongs_to(self.executable()),
                "foreign canonical inventory",
            )?;
            let relation = self.helper_memory.bf16_nominal.as_deref().ok_or(
                Bf16NominalCallQueryErrorV1::Unavailable("sealed nominal relation absent"),
            )?;
            let retained = bf16_nominal_retained_floor_v1(self, inventory, budget)?;
            let incoming = budget
                .storage()
                .checked_sub(bf16_call_query_scratch_v1::<R>()?)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if incoming < retained {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            bf16_query_source_v1(
                &self.semantic_ssa,
                relation,
                root,
                caller,
                source_block,
                source_call,
                budget,
            )?;
            let root_row = bf16_query_function_v1(
                &self.correspondence,
                relation.root,
                relation.root,
                SemanticKirFunctionRoleV1::KernelEntry,
                inventory,
                budget,
            )?;
            let helper_row = bf16_query_function_v1(
                &self.correspondence,
                relation.root,
                relation.helper,
                SemanticKirFunctionRoleV1::InternalHelper,
                inventory,
                budget,
            )?;
            let call = bf16_query_operation_v1(
                inventory,
                root_row,
                relation.call_block,
                relation.call_ordinal,
                budget,
            )?;
            let matrix = bf16_query_operation_v1(
                inventory,
                helper_row,
                relation.matrix_block,
                relation.matrix_ordinal,
                budget,
            )?;
            let call_span = bf16_source_span_v1(
                &self.correspondence,
                relation.root,
                relation.root,
                relation.source_call_block,
                budget,
            )
            .map_err(bf16_query_semantic_error_v1)?;
            let matrix_span = bf16_source_span_v1(
                &self.correspondence,
                relation.root,
                relation.helper,
                relation.source_matrix_block,
                budget,
            )
            .map_err(bf16_query_semantic_error_v1)?;
            bf16_query_require_v1(
                bf16_inside_span_v1(
                    call_span,
                    relation.call_block,
                    relation.call_ordinal as usize,
                ) && bf16_inside_span_v1(
                    matrix_span,
                    relation.matrix_block,
                    relation.matrix_ordinal as usize,
                ),
                "exact source operation spans differ",
            )?;
            bf16_query_components_v1(
                relation,
                root_row.function,
                helper_row.function,
                call.operation,
                matrix.operation,
                budget,
            )?;
            bf16_query_continuation_v1(
                &self.correspondence,
                relation,
                source_call,
                root_row.function,
                budget,
            )?;
            let mut matching_calls = 0usize;
            for row in inventory.calls() {
                budget.charge_work(1)?;
                if row.coordinate == call.coordinate {
                    bf16_query_require_v1(
                        std::ptr::eq(row.operation, call.operation)
                            && row.target == Some(helper_row.coordinate),
                        "canonical call target differs",
                    )?;
                    matching_calls += 1;
                }
            }
            bf16_query_require_v1(matching_calls == 1, "canonical call roster differs")?;
            inspect(
                &CheckedBf16NominalCallV1 {
                    emission: Bf16CallInstanceEmissionViewV1 {
                        owner: self,
                        relation,
                    },
                    inventory,
                    source_call,
                    caller: root_row,
                    helper: helper_row,
                    call,
                    matrix,
                },
                budget,
            )
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn bf16_query_source_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    relation: &SealedBf16CallRelationV1,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Bf16CallQueryResultV1<()> {
    budget.charge_work(12)?;
    bf16_query_require_v1(
        root == relation.root && caller == relation.root && block == relation.source_call_block,
        "source root/caller/block differs",
    )?;
    let semantic = owner.source_semantic();
    let function = semantic.functions().get(caller.index() as usize).ok_or(
        Bf16NominalCallQueryErrorV1::Unavailable("source caller absent"),
    )?;
    let statement = function.blocks().get(block.index() as usize).ok_or(
        Bf16NominalCallQueryErrorV1::Unavailable("source block absent"),
    )?;
    let SemanticTerminatorKindV1::Call(actual) = statement.terminator().kind() else {
        return Err(Bf16NominalCallQueryErrorV1::Unavailable(
            "source call absent",
        ));
    };
    bf16_query_require_v1(
        std::ptr::eq(actual, call),
        "source call is not the retained object",
    )?;
    bf16_query_require_v1(
        matches!(semantic.callables().get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::Defined { function }) if *function == relation.helper),
        "source callee differs",
    )?;
    let destination = call
        .destination()
        .ok_or(Bf16NominalCallQueryErrorV1::Unavailable(
            "source continuation absent",
        ))?;
    bf16_query_require_v1(
        call.arguments().len() == 4
            && destination.edge().role() == SemanticEdgeRoleV1::CallReturn
            && function
                .blocks()
                .get(destination.edge().target().index() as usize)
                .is_some(),
        "source argument/continuation differs",
    )
}

fn bf16_query_function_v1<'a>(
    correspondence: &SemanticKirCorrespondenceV1,
    root: SemanticFunctionIdV1,
    source: SemanticFunctionIdV1,
    role: SemanticKirFunctionRoleV1,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Bf16CallQueryResultV1<&'a CanonicalKirFunctionRefV1<'a>> {
    let mut result = None;
    for row in &correspondence.lowered_functions {
        budget.charge_work(1)?;
        if row.correspondence_owner == root && row.semantic_function == source && row.role == role {
            bf16_query_require_v1(result.is_none(), "duplicate canonical function association")?;
            result = inventory
                .function_for_name(row.kernel_ir_function.as_str(), budget)
                .map_err(bf16_query_inventory_error_v1)?;
            bf16_query_require_v1(result.is_some(), "canonical function absent")?;
        }
    }
    result.ok_or(Bf16NominalCallQueryErrorV1::Unavailable(
        "source function association absent",
    ))
}
fn bf16_query_operation_v1<'a>(
    inventory: &'a CanonicalKirInventoryV1<'a>,
    function: &CanonicalKirFunctionRefV1<'_>,
    block: BlockId,
    ordinal: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Bf16CallQueryResultV1<&'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'a>> {
    let row = inventory
        .block_for_id(function.coordinate, block, budget)
        .map_err(bf16_query_inventory_error_v1)?
        .ok_or(Bf16NominalCallQueryErrorV1::Unavailable(
            "canonical block absent",
        ))?;
    budget.charge_work(4)?;
    let index = row
        .operations
        .start
        .checked_add(ordinal as usize)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    bf16_query_require_v1(
        index < row.operations.end,
        "canonical operation ordinal absent",
    )?;
    let operation =
        inventory
            .operations()
            .get(index)
            .ok_or(Bf16NominalCallQueryErrorV1::Unavailable(
                "canonical operation absent",
            ))?;
    bf16_query_require_v1(
        operation.coordinate.block == row.coordinate && operation.coordinate.operation == ordinal,
        "canonical operation coordinate differs",
    )?;
    Ok(operation)
}

fn bf16_query_continuation_v1(
    correspondence: &SemanticKirCorrespondenceV1,
    relation: &SealedBf16CallRelationV1,
    source_call: &SemanticDirectCallV1,
    root: &Function,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Bf16CallQueryResultV1<()> {
    let target = source_call
        .destination()
        .ok_or(Bf16NominalCallQueryErrorV1::Unavailable(
            "source continuation absent",
        ))?
        .edge()
        .target();
    let mut canonical_target = None;
    for row in &correspondence.blocks {
        budget.charge_work(1)?;
        if row.correspondence_owner == relation.root
            && row.semantic_function == relation.root
            && row.semantic_block == target
        {
            bf16_query_require_v1(
                canonical_target.replace(row.kernel_ir_block).is_none(),
                "duplicate source continuation",
            )?;
        }
    }
    let body = root
        .body
        .as_ref()
        .ok_or(Bf16NominalCallQueryErrorV1::Unavailable(
            "canonical caller body absent",
        ))?;
    budget.charge_work(body.blocks.len())?;
    let call_block = body
        .blocks
        .iter()
        .find(|b| b.id == relation.call_block)
        .ok_or(Bf16NominalCallQueryErrorV1::Unavailable(
            "canonical call block absent",
        ))?;
    bf16_query_require_v1(
        matches!(call_block.terminator.as_ref(),
        Some(Terminator::Branch { target, .. }) if Some(*target) == canonical_target),
        "canonical call continuation differs",
    )
}

fn bf16_query_components_v1(
    relation: &SealedBf16CallRelationV1,
    root: &Function,
    helper: &Function,
    call: &Operation,
    matrix_operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Bf16CallQueryResultV1<()> {
    budget.charge_work(64)?;
    let capture = &relation.capture;
    bf16_query_require_v1(
        capture.seen == [true, true]
            && matches!(relation.permutation, [0, 1, 2, 3] | [1, 0, 2, 3])
            && root.role == fe2o3_kernel_ir::FunctionRole::KernelEntry
            && helper.role == fe2o3_kernel_ir::FunctionRole::InternalHelper,
        "nominal category/return map differs",
    )?;
    let OperationKind::Call { callee, arguments } = &call.kind else {
        return Err(Bf16NominalCallQueryErrorV1::Unavailable(
            "canonical Call absent",
        ));
    };
    budget.charge_work(callee.as_str().len().min(helper.id.as_str().len()) + 1)?;
    bf16_query_require_v1(
        *callee == helper.id
            && arguments.len() == 12
            && call.results.len() == 4
            && helper.signature.parameters.len() == 12
            && helper.signature.results.len() == 4,
        "nominal call/signature width differs",
    )?;
    let body = helper
        .body
        .as_ref()
        .ok_or(Bf16NominalCallQueryErrorV1::Unavailable(
            "canonical helper body absent",
        ))?;
    bf16_query_require_v1(
        body.parameters.len() == 12 && body.blocks.len() <= 32,
        "nominal helper formals/body differ",
    )?;
    for (i, argument) in arguments.iter().enumerate() {
        budget.charge_work(5)?;
        let ty = Type::Scalar(if i < 8 {
            ScalarType::Bf16
        } else {
            ScalarType::F32
        });
        bf16_query_require_v1(
            helper.signature.parameters[i] == ty
                && body.parameters[i] == capture.formals[1 + i / 4][i % 4]
                && *argument == capture.arguments[1 + i / 4][i % 4],
            "nominal ordered argument/formal component differs",
        )?;
        let actual = bf16_transport_origin_v1(root, *argument, budget)
            .map_err(bf16_query_semantic_error_v1)?;
        let expected = bf16_transport_origin_v1(root, capture.producers[2 + i / 4][i % 4], budget)
            .map_err(bf16_query_semantic_error_v1)?;
        bf16_query_require_v1(
            actual == expected,
            "nominal caller producer identity differs",
        )?;
    }
    let OperationKind::Matrix(matrix) = &matrix_operation.kind else {
        return Err(Bf16NominalCallQueryErrorV1::Unavailable(
            "canonical Matrix absent",
        ));
    };
    let MatrixOperationKind::MultiplyAccumulate {
        lhs,
        rhs,
        accumulator,
        ..
    } = &matrix.kind
    else {
        return Err(Bf16NominalCallQueryErrorV1::Unavailable(
            "nominal Matrix kind differs",
        ));
    };
    let expected = MatrixOperation::multiply_accumulate(*lhs, *rhs, *accumulator)
        .with_declared_tensor_layout(
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                .with_zero_filled_predicate_inputs(),
        );
    bf16_query_require_v1(
        *matrix == expected && matrix_operation.results.len() == 4,
        "nominal Matrix contract/results differ",
    )?;
    for (group, values) in [lhs, rhs, accumulator].into_iter().enumerate() {
        for (i, value) in values.iter().enumerate() {
            budget.charge_work(1)?;
            bf16_query_require_v1(
                bf16_transport_origin_v1(helper, *value, budget)
                    .map_err(bf16_query_semantic_error_v1)?
                    == capture.formals[group + 1][i],
                "nominal Matrix input/formal differs",
            )?;
        }
    }
    for i in 0..4 {
        budget.charge_work(8)?;
        bf16_query_require_v1(
            helper.signature.results[i] == Type::F32
                && call.results[i].ty == Type::F32
                && call.results[i].id == capture.call_result[i]
                && matrix_operation.results[i].ty == Type::F32
                && matrix_operation.results[i].id == capture.producers[5][i],
            "nominal ordered result component differs",
        )?;
        bf16_query_require_v1(
            bf16_transport_origin_v1(helper, capture.producers[6][i], budget)
                .map_err(bf16_query_semantic_error_v1)?
                == capture.producers[5][i],
            "nominal accumulator conversion identity differs",
        )?;
    }
    let mut returns = 0usize;
    for block in &body.blocks {
        budget.charge_work(1)?;
        if let Some(Terminator::Return { values }) = &block.terminator {
            budget.charge_work(8)?;
            bf16_query_require_v1(
                values.as_slice() == relation.helper_return,
                "nominal helper Return operands differ",
            )?;
            returns += 1;
            for (i, value) in values.iter().enumerate() {
                bf16_query_require_v1(
                    bf16_transport_origin_v1(helper, *value, budget)
                        .map_err(bf16_query_semantic_error_v1)?
                        == capture.producers[5][relation.permutation[i] as usize],
                    "nominal helper return permutation differs",
                )?;
            }
        }
    }
    bf16_query_require_v1(returns == 1, "nominal helper Return roster differs")
}

#[cfg(test)]
#[path = "production_bf16_call_query_v1_tests.rs"]
mod bf16_call_query_tests_v1;
