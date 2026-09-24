use std::collections::BTreeSet;
use std::fmt;

use crate::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1, ControlFlowError, ControlFlowLimits,
    DiagnosticCode, Function, MeteredControlFlowErrorV1, MeteredIndexedControlFlowV1, Module,
    Operation, OperationKind, TargetCapability, Terminator, Type, ValueId,
    VerificationDefinitionSiteV1, VerificationDiagnosticCollectorV1,
    VerificationDiagnosticLocationV1, VerificationFunctionStateV1, VerificationModuleStateV1,
    analyze_control_flow_with_verification_budget_v1, clone_diagnostic_location_v1,
    emit_dynamic_v1, emit_fixed_v1, function_diagnostic_location_v1,
    reserved_diagnostic_call_is_terminating_v1, try_verify_registered_operation_with_resources_v1,
    verify_type_v12_with_budget_v1,
};

pub(crate) fn run_verification_function_pass_v1<'module>(
    module: &'module Module,
    function: &'module Function,
    module_state: &VerificationModuleStateV1<'module>,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    let Some(body) = &function.body else {
        return Ok(());
    };
    if body.blocks.is_empty() {
        return Ok(());
    }

    let control_flow = match analyze_control_flow_with_verification_budget_v1(
        function,
        ControlFlowLimits::DEFAULT,
        budget,
    ) {
        Ok(control_flow) => Some(control_flow),
        Err(MeteredControlFlowErrorV1::Resource(error)) => return Err(error),
        Err(MeteredControlFlowErrorV1::ControlFlow(
            error @ (ControlFlowError::ResourceLimit { .. }
            | ControlFlowError::ArithmeticOverflow(_)),
        )) => {
            emit_dynamic_v1(
                diagnostics,
                function_diagnostic_location_v1(module, function, budget)?,
                DiagnosticCode::ResourceLimit,
                512,
                format_args!("{error}"),
                budget,
            )?;
            return Ok(());
        }
        Err(MeteredControlFlowErrorV1::ControlFlow(_)) => None,
    };

    let function_state = match VerificationFunctionStateV1::build(function, budget) {
        Ok(Some(state)) => state,
        Ok(None) => {
            if let Some(control_flow) = control_flow {
                control_flow.release(budget)?;
            }
            return Ok(());
        }
        Err(error) => {
            if let Some(control_flow) = control_flow {
                let _ = control_flow.release(budget);
            }
            return Err(error);
        }
    };
    let result = {
        let mut pass = VerificationFunctionPassV1 {
            module,
            function,
            module_state,
            function_state: &function_state,
            supported_capabilities,
            diagnostics,
            budget,
            control_flow: control_flow.as_ref(),
            dynamic_workgroup_memory_declarations: 0,
            gfx950_lds_transpose_current_formats: 0,
        };
        pass.verify()
    };
    let function_release = function_state.release(budget);
    let control_flow_release = match control_flow {
        Some(control_flow) => control_flow.release(budget),
        None => Ok(()),
    };
    result.and(function_release).and(control_flow_release)
}

pub(crate) struct VerificationFunctionPassV1<'a, 'module, 'work> {
    pub(crate) module: &'module Module,
    pub(crate) function: &'module Function,
    pub(crate) module_state: &'a VerificationModuleStateV1<'module>,
    pub(crate) function_state: &'a VerificationFunctionStateV1<'module>,
    pub(crate) supported_capabilities: Option<&'a BTreeSet<TargetCapability>>,
    pub(crate) diagnostics: &'a mut VerificationDiagnosticCollectorV1,
    pub(crate) budget: &'a mut CanonicalKernelIrVerificationResourceBudgetV1<'work>,
    pub(crate) control_flow: Option<&'a MeteredIndexedControlFlowV1>,
    pub(crate) dynamic_workgroup_memory_declarations: usize,
    pub(crate) gfx950_lds_transpose_current_formats: u8,
}

impl<'a, 'module, 'work> VerificationFunctionPassV1<'a, 'module, 'work> {
    fn verify(&mut self) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let body = self
            .function
            .body
            .as_ref()
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
        let base_location =
            function_diagnostic_location_v1(self.module, self.function, self.budget)?;

        let has_execution_roles = self.verify_definition_rosters(&base_location)?;
        self.budget.charge_work(body.blocks.len())?;
        for block in &body.blocks {
            if block.terminator.is_none() {
                let location =
                    clone_diagnostic_location_v1(&base_location, self.budget)?.at_block(block.id);
                self.emit_fixed(
                    &location,
                    DiagnosticCode::MissingTerminator,
                    "basic block has no terminator",
                )?;
            }
        }

        let mut has_complete_body = false;
        let mut has_physical_entry = false;
        let mut has_physical_global_copy = false;
        self.budget.charge_work(body.blocks.len())?;
        for block in &body.blocks {
            self.budget.charge_work(block.operations.len())?;
            for (operation_index, operation) in block.operations.iter().enumerate() {
                has_complete_body |= matches!(
                    operation.kind,
                    OperationKind::Gfx942CompleteBodyDeclaration(_)
                        | OperationKind::Gfx942CompleteBodyStep(_)
                );
                has_physical_entry |= matches!(
                    operation.kind,
                    OperationKind::Gfx942PhysicalEntryDeclaration(_)
                        | OperationKind::Gfx942PhysicalEntryStep(_)
                );
                has_physical_global_copy |= matches!(
                    operation.kind,
                    OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_)
                        | OperationKind::Gfx942PhysicalGlobalCopyStep(_)
                );
                let location = clone_diagnostic_location_v1(&base_location, self.budget)?
                    .at_block(block.id)
                    .at_operation(operation_index);
                self.verify_terminating_diagnostic_position(
                    block,
                    operation,
                    operation_index,
                    &location,
                )?;
                self.budget.charge_work(1)?;
                operation.kind.try_visit_operands(|operand| {
                    self.budget.charge_work(1)?;
                    self.verify_use(operand, block.id, Some(operation_index), &location)
                })?;
                let registered = try_verify_registered_operation_with_resources_v1(
                    operation,
                    &location,
                    self.function_state,
                    self.supported_capabilities,
                    self.diagnostics,
                    self.budget,
                )?;
                if !registered {
                    self.verify_legacy_operation_v1(operation, &location)?;
                } else if matches!(operation.kind, OperationKind::Matrix(_)) {
                    self.verify_matrix_lds_allocation_v1(operation, &location)?;
                }
            }
            if let Some(terminator) = &block.terminator {
                let location =
                    clone_diagnostic_location_v1(&base_location, self.budget)?.at_block(block.id);
                self.verify_terminator_uses_v1(terminator, block.id, &location)?;
                self.verify_terminator_v1(block, terminator, &location)?;
            }
        }
        if has_complete_body {
            self.verify_complete_body_function_v19(&base_location)?;
        }
        if has_physical_entry {
            self.verify_physical_entry_function_v20(&base_location)?;
        }
        if has_physical_global_copy {
            self.verify_physical_global_copy_function_v21(&base_location)?;
        }
        if has_execution_roles {
            crate::verification_execution_lifecycle_v15::verify_execution_lifecycle_v15(
                self.module,
                self.function,
                self.function_state,
                self.control_flow,
                self.diagnostics,
                self.budget,
            )?;
        }
        Ok(())
    }

    fn verify_definition_rosters(
        &mut self,
        base_location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
        let block_rows = self.function_state.block_rows();
        self.budget.charge_work(block_rows.len())?;
        for pair in block_rows.windows(2) {
            self.budget.charge_work(1)?;
            if pair[0].key == pair[1].key {
                let location = clone_diagnostic_location_v1(base_location, self.budget)?
                    .at_block(pair[1].value.id);
                self.emit_dynamic(
                    &location,
                    DiagnosticCode::DuplicateBlock,
                    128,
                    format_args!("block {} is defined more than once", pair[1].value.id),
                )?;
            }
        }

        let definition_rows = self.function_state.definition_rows();
        self.budget.charge_work(definition_rows.len())?;
        let mut has_execution_roles = false;
        for row in definition_rows {
            has_execution_roles |= matches!(row.value.ty, Type::Execution(_));
            if !matches!(
                row.value.site,
                VerificationDefinitionSiteV1::FunctionParameter
            ) {
                let location = self.definition_location_v1(row.value.site, base_location)?;
                verify_type_v12_with_budget_v1(
                    row.value.ty,
                    &location,
                    self.diagnostics,
                    self.budget,
                )?;
            }
        }
        self.budget.charge_work(definition_rows.len())?;
        for pair in definition_rows.windows(2) {
            self.budget.charge_work(1)?;
            if pair[0].key == pair[1].key {
                let location = self.definition_location_v1(pair[1].value.site, base_location)?;
                self.emit_dynamic(
                    &location,
                    DiagnosticCode::DuplicateValue,
                    128,
                    format_args!("SSA value %{} is defined more than once", pair[1].key),
                )?;
            }
        }
        Ok(has_execution_roles)
    }

    fn definition_location_v1<'m>(
        &mut self,
        site: VerificationDefinitionSiteV1,
        base: &VerificationDiagnosticLocationV1<'m>,
    ) -> Result<VerificationDiagnosticLocationV1<'m>, CanonicalKernelIrVerificationResourceErrorV1>
    {
        let location = clone_diagnostic_location_v1(base, self.budget)?;
        Ok(match site {
            VerificationDefinitionSiteV1::FunctionParameter => location,
            VerificationDefinitionSiteV1::BlockParameter(block) => location.at_block(block),
            VerificationDefinitionSiteV1::Operation(block, operation) => {
                location.at_block(block).at_operation(operation)
            }
        })
    }

    fn verify_terminating_diagnostic_position(
        &mut self,
        block: &BasicBlock,
        operation: &Operation,
        operation_index: usize,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let OperationKind::Call { callee, arguments } = &operation.kind else {
            self.budget.charge_work(1)?;
            return Ok(());
        };
        let terminating =
            reserved_diagnostic_call_is_terminating_v1(callee, arguments.len(), self.budget)?;
        if terminating
            && (operation_index + 1 != block.operations.len()
                || !matches!(block.terminator, Some(Terminator::Unreachable)))
        {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidAmdGpuDiagnosticOperation,
                "terminating AMDGPU diagnostic must be the final operation of a block terminated by unreachable",
            )?;
        }
        Ok(())
    }

    pub(crate) fn definition_type_v1(
        &mut self,
        value: ValueId,
    ) -> Result<Option<&'module Type>, CanonicalKernelIrVerificationResourceErrorV1> {
        self.function_state
            .definition(value, self.budget)
            .map(|definition| definition.map(|definition| definition.ty))
    }

    pub(crate) fn verify_matrix_lds_allocation_v1(
        &mut self,
        operation: &Operation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(2)?;
        let OperationKind::Matrix(matrix) = &operation.kind else {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        };
        let (base, profile) = match &matrix.kind {
            crate::MatrixOperationKind::MultiplyAccumulate { .. }
            | crate::MatrixOperationKind::ScaledMultiplyAccumulate { .. } => return Ok(()),
            crate::MatrixOperationKind::LdsLoad { base, profile }
            | crate::MatrixOperationKind::LdsStore { base, profile, .. } => (*base, *profile),
        };
        let Some(definition) = self.function_state.definition(base, self.budget)? else {
            return Ok(());
        };
        self.budget.charge_work(1)?;
        let VerificationDefinitionSiteV1::Operation(block, operation_ordinal) = definition.site
        else {
            return self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "matrix LDS base must be the direct result of an authenticated workgroup-memory allocation",
            );
        };
        let source_block = self.function_state.block(block, self.budget)?;
        self.budget.charge_work(2)?;
        let allocation = source_block
            .and_then(|block| block.operations.get(operation_ordinal))
            .and_then(|operation| match &operation.kind {
                OperationKind::WorkgroupMemory(memory) => Some(memory),
                _ => None,
            });
        let Some(allocation) = allocation else {
            return self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "matrix LDS base must be the direct result of an authenticated workgroup-memory allocation",
            );
        };
        self.budget.charge_work(3)?;
        let required_elements = profile.required_elements();
        match allocation.extent.guaranteed_elements() {
            Some(elements) if elements >= required_elements => {}
            Some(elements) => self.emit_dynamic(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                256,
                format_args!(
                    "matrix LDS allocation guarantees {elements} elements but requires at least {required_elements}"
                ),
            )?,
            None => self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "matrix LDS operation requires a statically authenticated allocation extent",
            )?,
        }
        let required_alignment = profile.required_alignment();
        if allocation.alignment < required_alignment {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidAlignment,
                256,
                format_args!(
                    "matrix LDS allocation alignment {} is below the required {required_alignment}",
                    allocation.alignment
                ),
            )?;
        }
        Ok(())
    }

    pub(crate) fn verify_use(
        &mut self,
        value: ValueId,
        use_block: BlockId,
        use_operation: Option<usize>,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let Some(definition) = self.function_state.definition(value, self.budget)? else {
            return self.emit_dynamic(
                location,
                DiagnosticCode::UndefinedValue,
                128,
                format_args!("SSA value {value} is not defined in this function"),
            );
        };
        let dominates = match definition.site {
            VerificationDefinitionSiteV1::FunctionParameter => true,
            VerificationDefinitionSiteV1::BlockParameter(def_block) => {
                def_block == use_block || self.block_dominates(def_block, use_block)?
            }
            VerificationDefinitionSiteV1::Operation(def_block, def_operation)
                if def_block == use_block =>
            {
                use_operation.is_none_or(|use_operation| def_operation < use_operation)
            }
            VerificationDefinitionSiteV1::Operation(def_block, _) => {
                self.block_dominates(def_block, use_block)?
            }
        };
        if !dominates {
            self.emit_dynamic(
                location,
                DiagnosticCode::NonDominatingUse,
                128,
                format_args!("definition of {value} does not dominate this use"),
            )?;
        }
        Ok(())
    }

    fn block_dominates(
        &mut self,
        definition: BlockId,
        use_block: BlockId,
    ) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
        match self.control_flow {
            Some(control_flow) => control_flow.dominates(definition, use_block, self.budget),
            None => Ok(false),
        }
    }

    pub(crate) fn emit_fixed(
        &mut self,
        location: &VerificationDiagnosticLocationV1<'_>,
        code: DiagnosticCode,
        message: &'static str,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let location = clone_diagnostic_location_v1(location, self.budget)?;
        emit_fixed_v1(self.diagnostics, location, code, message, self.budget)
    }

    pub(crate) fn emit_dynamic(
        &mut self,
        location: &VerificationDiagnosticLocationV1<'_>,
        code: DiagnosticCode,
        message_work_upper: usize,
        arguments: fmt::Arguments<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let location = clone_diagnostic_location_v1(location, self.budget)?;
        emit_dynamic_v1(
            self.diagnostics,
            location,
            code,
            message_work_upper,
            arguments,
            self.budget,
        )
    }
}

#[cfg(test)]
#[path = "verification_registered_location_borrow_v1_tests.rs"]
mod registered_location_borrow_tests;
