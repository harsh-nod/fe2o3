// Exact deletion is a separate relation. No Call is reclassified as pure and
// no generic source, optimizer, formal-memory, artifact, or target owner is made.

/// Inert location information in one checked Unit-local deletion relation.
/// A copied row is not authority to alter another graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionUnitLocalOperationDeletionV1 {
    /// The complete original operation survives at this exact output location.
    Retained(fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1),
    /// This exact zero-argument, zero-result Unit call was removed.
    DeletedUnitCall,
    /// This operation belonged to a fully removed certified local helper.
    DeletedLocalHelper,
}

/// Scoped exact N-to-E relation for the closed, caller-unobservable Unit domain.
/// Original source/N and call custody remain borrowed, not discarded. This is
/// not a generic optimizer transition, target stack-feasibility proof, or an
/// owning source/output admission. No use of E by later stages is authorized.
pub struct CheckedUnitLocalCallDeletionV1<'s> {
    stage: &'s ProductionUnitLocalRankedStageV1<'s>,
    output: &'s fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    functions: &'s [Option<u32>],
    operations: &'s [Option<ProductionUnitLocalOperationDeletionV1>],
    deleted_functions: usize,
    ledger: usize,
    work_ledger: ArgumentLedgerV1,
    floor: usize,
}

impl CheckedUnitLocalCallDeletionV1<'_> {
    /// Exact original source/N owner, including every erased source call.
    pub fn source(&self) -> &ProductionPreRankedKirOwnerV1 {
        self.stage.owner
    }

    /// The independently admitted candidate whose exact deletion was checked.
    pub fn output(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.output
    }

    /// Number of distinct removed call occurrences, not helper definitions.
    pub fn deleted_call_count(&self) -> usize {
        self.stage.calls.len()
    }

    /// Number of removed physical helper definitions, not root associations.
    pub fn deleted_function_count(&self) -> usize {
        self.deleted_functions
    }

    /// Exact retained function ordinal, or None for a deleted/absent function.
    pub fn function(
        &self,
        original: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<u32>, ProductionSemanticKirErrorV1> {
        self.require_live(budget)?;
        Ok(self.functions.get(original as usize).copied().flatten())
    }

    /// Exact operation outcome under the same live ledger and source inventory.
    pub fn operation(
        &self,
        original: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionUnitLocalOperationDeletionV1>, ProductionSemanticKirErrorV1> {
        self.require_live(budget)?;
        let Some(index) =
            unit_deletion_operation_index_v1(self.stage.source.inventory, original, budget)?
        else {
            return Ok(None);
        };
        Ok(self.operations.get(index).copied().flatten())
    }

    /// Caller-visible deletion is not artifact, launch, or hardware authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    fn require_live(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(5)?;
        if self.ledger != budget as *const ArgumentBudgetV1<'_> as usize
            || self.work_ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }
}

fn unit_deletion_refused_v1(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}

fn unit_deletion_operation_index_v1(
    inventory: &CanonicalKirInventoryV1<'_>,
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
    budget.charge_work(7)?;
    let Some(function) = inventory
        .functions()
        .get(coordinate.block.function.0 as usize)
    else {
        return Ok(None);
    };
    let block = argument_sum_v1(&[function.blocks.start, coordinate.block.block as usize])?;
    if block >= function.blocks.end {
        return Ok(None);
    }
    let block = inventory
        .blocks()
        .get(block)
        .ok_or_else(unit_local_mismatch_v1)?;
    if block.coordinate != coordinate.block {
        return Err(unit_local_mismatch_v1());
    }
    let index = argument_sum_v1(&[block.operations.start, coordinate.operation as usize])?;
    if index >= block.operations.end {
        return Ok(None);
    }
    if inventory.operations().get(index).map(|row| row.coordinate) != Some(coordinate) {
        return Err(unit_local_mismatch_v1());
    }
    Ok(Some(index))
}

impl ProductionUnitLocalRankedStageV1<'_> {
    /// Checks that an independently verified candidate E is exactly N with all
    /// certified silent Unit calls and only their local helper definitions erased.
    /// Surviving function order, declarations, blocks, values, terminators, edge
    /// arguments, operation payload/order, and capability sets remain exact.
    ///
    /// The rule is relative to the admitted typed KIR/private-memory semantics,
    /// not hardware allocation failure, target stack limits, or runtime behavior.
    /// The existing source relation plus freshly rechecked physical chains and
    /// an explicit total live-op grammar are required; raw effect emptiness is
    /// not sufficient. Original source-call custody is retained in the scope.
    ///
    /// Caller reserves E and any output before entering the ranked stage. New
    /// headers/actual-capacity maps and all fresh core scratch are metered here.
    /// Complete payload comparison is prepaid by both admitted canonical lengths,
    /// following the exact-coordinate checker. No production graph is cloned.
    /// Result/error/unwind restore the incoming floor under the original ledger.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionUnitLocalRankedStageV1;
    /// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12, CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
    /// fn escape(stage: &ProductionUnitLocalRankedStageV1<'_>, output: &VerifiedCanonicalKernelIrModuleV12, budget: &mut Budget<'_>) {
    ///     let mut saved = None;
    ///     stage.with_checked_silent_unit_call_deletion_v1(output, budget, |checked, _| {
    ///         saved = Some(checked);
    ///         Ok(())
    ///     }).unwrap();
    ///     drop(saved);
    /// }
    /// ```
    pub fn with_checked_silent_unit_call_deletion_v1<'w, R>(
        &self,
        output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        budget: &mut ArgumentBudgetV1<'w>,
        next: impl for<'s> FnOnce(
            &CheckedUnitLocalCallDeletionV1<'s>,
            &mut ArgumentBudgetV1<'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        with_canonical_call_scratch_v1(budget, |budget| {
            budget.charge_work(7)?;
            if self.ledger != budget as *const ArgumentBudgetV1<'_> as usize
                || self.work_ledger != budget.work_ledger_identity_v1()
                || budget.storage() < self.floor
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            if !self.source.inventory.belongs_to(self.owner.executable()) || self.calls.is_empty() {
                return Err(unit_local_mismatch_v1());
            }
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<CheckedUnitLocalCallDeletionV1<'_>>(),
                std::mem::size_of::<Vec<Option<u32>>>(),
                std::mem::size_of::<Vec<Option<ProductionUnitLocalOperationDeletionV1>>>(),
            ])?)?;
            let inventory = self.source.inventory;
            let mut functions = unit_local_vec_v1(inventory.functions().len(), budget)?;
            let mut operations = unit_local_vec_v1(inventory.operations().len(), budget)?;
            budget.charge_work(argument_sum_v1(&[
                inventory.functions().len(),
                inventory.operations().len(),
            ])?)?;
            functions.resize(inventory.functions().len(), None::<u32>);
            operations.resize(
                inventory.operations().len(),
                None::<ProductionUnitLocalOperationDeletionV1>,
            );

            check_unit_deletion_source_silence_v1(self, &mut functions, budget)?;
            for token in self.calls {
                budget.charge_work(5)?;
                let index =
                    unit_deletion_operation_index_v1(inventory, token.native_call(), budget)?
                        .ok_or_else(unit_local_mismatch_v1)?;
                if !std::ptr::eq(inventory.operations()[index].operation, token.operation())
                    || operations[index]
                        .replace(ProductionUnitLocalOperationDeletionV1::DeletedUnitCall)
                        .is_some()
                {
                    return Err(unit_local_mismatch_v1());
                }
            }
            // This is the complete actual reference census, including calls
            // outside the token slice. No retained call may target a removed
            // body; kernel declarations are compared in full below.
            for call in inventory.calls() {
                budget.charge_work(3)?;
                if call
                    .target
                    .is_some_and(|target| functions.get(target.0 as usize) == Some(&Some(u32::MAX)))
                {
                    let index =
                        unit_deletion_operation_index_v1(inventory, call.coordinate, budget)?
                            .ok_or_else(unit_local_mismatch_v1)?;
                    if operations[index]
                        != Some(ProductionUnitLocalOperationDeletionV1::DeletedUnitCall)
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                }
            }
            let deleted_functions = check_unit_deletion_exact_output_v1(
                self,
                output,
                &mut functions,
                &mut operations,
                budget,
            )?;
            budget.charge_work(operations.len())?;
            if operations.iter().any(Option::is_none) {
                return Err(unit_local_mismatch_v1());
            }
            let checked = CheckedUnitLocalCallDeletionV1 {
                stage: self,
                output,
                functions: &functions,
                operations: &operations,
                deleted_functions,
                ledger: budget as *const ArgumentBudgetV1<'_> as usize,
                work_ledger: budget.work_ledger_identity_v1(),
                floor: budget.storage(),
            };
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| next(&checked, budget)));
            if checked.work_ledger != budget.work_ledger_identity_v1()
                || checked.floor != budget.storage()
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            match result {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        })
    }
}

fn check_unit_deletion_exact_output_v1(
    stage: &ProductionUnitLocalRankedStageV1<'_>,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    functions: &mut [Option<u32>],
    operations: &mut [Option<ProductionUnitLocalOperationDeletionV1>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    use ProductionUnitLocalOperationDeletionV1 as Row;
    let input = stage.owner.executable();
    budget.charge_work(argument_sum_v1(&[
        input.canonical().canonical_bytes().len(),
        output.canonical().canonical_bytes().len(),
        4,
    ])?)?;
    let Module {
        id,
        functions: original,
        kernels,
        required_capabilities,
    } = input.module();
    let Module {
        id: output_id,
        functions: final_functions,
        kernels: output_kernels,
        required_capabilities: output_capabilities,
    } = output.module();
    if id != output_id || kernels != output_kernels || required_capabilities != output_capabilities
    {
        return Err(unit_deletion_refused_v1(
            "Unit-local deletion changed module or kernel declarations",
        ));
    }
    let mut next_function = 0usize;
    let mut removed = 0usize;
    for (ordinal, function) in original.iter().enumerate() {
        budget.charge_work(7)?;
        let inventory = stage.source.inventory;
        let info = inventory
            .functions()
            .get(ordinal)
            .ok_or_else(unit_local_mismatch_v1)?;
        if matches!(
            stage.owner.helper_memory.functions.get(ordinal),
            Some(RetainedHelperKindV1::Local { .. })
        ) {
            // All such bodies were freshly checked and every incoming ordinary
            // call had an exact ranked-stage token. No unrelated orphan is cut.
            if functions[ordinal].take() != Some(u32::MAX) {
                return Err(unit_local_mismatch_v1());
            }
            removed = argument_sum_v1(&[removed, 1])?;
            budget.charge_work(info.operations.len())?;
            for index in info.operations.clone() {
                if operations[index].replace(Row::DeletedLocalHelper).is_some() {
                    return Err(unit_local_mismatch_v1());
                }
            }
            continue;
        }
        let Some(final_function) = final_functions.get(next_function) else {
            return Err(unit_deletion_refused_v1(
                "Unit-local deletion omitted a retained function",
            ));
        };
        if functions[ordinal].is_some() {
            return Err(unit_local_mismatch_v1());
        }
        let Function {
            id,
            signature,
            role,
            body,
            required_capabilities,
        } = function;
        let Function {
            id: final_id,
            signature: final_signature,
            role: final_role,
            body: final_body,
            required_capabilities: final_capabilities,
        } = final_function;
        if id != final_id
            || signature != final_signature
            || role != final_role
            || required_capabilities != final_capabilities
        {
            return Err(unit_deletion_refused_v1(
                "Unit-local deletion changed a retained function declaration or order",
            ));
        }
        let final_ordinal =
            u32::try_from(next_function).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        functions[ordinal] = Some(final_ordinal);
        next_function = argument_sum_v1(&[next_function, 1])?;
        match (body, final_body) {
            (None, None) => {}
            (Some(body), Some(final_body)) => {
                let FunctionBody { parameters, blocks } = body;
                let FunctionBody {
                    parameters: final_parameters,
                    blocks: final_blocks,
                } = final_body;
                if parameters != final_parameters || blocks.len() != final_blocks.len() {
                    return Err(unit_deletion_refused_v1(
                        "Unit-local deletion changed function parameters or blocks",
                    ));
                }
                for (block_ordinal, (block, final_block)) in
                    blocks.iter().zip(final_blocks).enumerate()
                {
                    budget.charge_work(6)?;
                    let BasicBlock {
                        id,
                        parameters,
                        operations: original_ops,
                        terminator,
                    } = block;
                    let BasicBlock {
                        id: final_id,
                        parameters: final_parameters,
                        operations: final_ops,
                        terminator: final_terminator,
                    } = final_block;
                    if id != final_id
                        || parameters != final_parameters
                        || terminator != final_terminator
                    {
                        return Err(unit_deletion_refused_v1(
                            "Unit-local deletion changed a block or continuation",
                        ));
                    }
                    let block_info = inventory
                        .blocks()
                        .get(argument_sum_v1(&[info.blocks.start, block_ordinal])?)
                        .ok_or_else(unit_local_mismatch_v1)?;
                    let mut next_operation = 0usize;
                    for (index, operation) in original_ops.iter().enumerate() {
                        budget.charge_work(5)?;
                        let dense = argument_sum_v1(&[block_info.operations.start, index])?;
                        match operations[dense] {
                            Some(Row::DeletedUnitCall) => {
                                if !matches!(&operation.kind, OperationKind::Call { arguments, .. } if arguments.is_empty())
                                    || !operation.results.is_empty()
                                {
                                    return Err(unit_local_mismatch_v1());
                                }
                            }
                            None => {
                                if matches!(operation.kind, OperationKind::InlineAssembly(_)) {
                                    return Err(unit_deletion_refused_v1(
                                        "Unit-local deletion refuses opaque assembly references",
                                    ));
                                }
                                if final_ops.get(next_operation) != Some(operation) {
                                    return Err(unit_deletion_refused_v1(
                                        "Unit-local deletion changed a retained operation or order",
                                    ));
                                }
                                operations[dense] = Some(Row::Retained(
                                    fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                                        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                            function:
                                                fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                                                    final_ordinal,
                                                ),
                                            block: u32::try_from(block_ordinal)
                                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                        },
                                        operation: u32::try_from(next_operation)
                                            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                    },
                                ));
                                next_operation = argument_sum_v1(&[next_operation, 1])?;
                            }
                            _ => return Err(unit_local_mismatch_v1()),
                        }
                    }
                    if next_operation != final_ops.len() {
                        return Err(unit_deletion_refused_v1(
                            "Unit-local deletion added or retained an extra operation",
                        ));
                    }
                }
            }
            _ => {
                return Err(unit_deletion_refused_v1(
                    "Unit-local deletion changed a function definition",
                ));
            }
        }
    }
    budget.charge_work(2)?;
    if removed == 0 || next_function != final_functions.len() {
        return Err(unit_deletion_refused_v1(
            "Unit-local deletion retained or added an extra function",
        ));
    }
    Ok(removed)
}

include!("production_local_helper_silence_v1.rs");
