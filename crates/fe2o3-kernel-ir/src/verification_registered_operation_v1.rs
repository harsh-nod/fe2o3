use std::{collections::BTreeSet, fmt};

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    DiagnosticCode, MatrixOperationIssueSinkV1, MatrixVerificationIssueKind, Operation,
    OperationKind, SemanticOperationBorrowedVerificationContextV1, SemanticOperationIssueKind,
    SemanticOperationIssueSinkV1, TargetCapability, TargetCapabilityRefV1, Type, ValueId,
    VerificationDiagnosticLocationV1, target_capability_is_supported_with_budget_v1,
    try_verify_semantic_operation_with_sink_v1,
};
use crate::{
    verification_diagnostics_v1::VerificationDiagnosticCollectorV1,
    verification_function_state_v1::VerificationFunctionStateV1,
};

const REGISTERED_LOCATION_CLONE_FIELDS_V1: usize = 5;
const REGISTERED_CAPABILITY_MESSAGE_FIXED_UPPER_V1: usize = 128;
const REGISTERED_DEBUG_ESCAPE_BYTES_PER_INPUT_BYTE_V1: usize = 10;

fn emit_registered_diagnostic_v1(
    location: &VerificationDiagnosticLocationV1<'_>,
    code: DiagnosticCode,
    message_work_upper: usize,
    arguments: fmt::Arguments<'_>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    // Copy borrowed fields here; the collector meters owned error identifiers.
    budget.charge_work(REGISTERED_LOCATION_CLONE_FIELDS_V1)?;
    diagnostics.emit(*location, code, message_work_upper, arguments, budget)
}

fn registered_capability_message_work_upper_v1(
    required: TargetCapabilityRefV1<'_>,
) -> Result<usize, CanonicalKernelIrVerificationResourceErrorV1> {
    let variable_bytes = match required {
        TargetCapabilityRefV1::Extension { namespace, name } => namespace
            .len()
            .checked_add(
                name.visible_len()
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
            )
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
        _ => 0,
    };
    variable_bytes
        .checked_mul(REGISTERED_DEBUG_ESCAPE_BYTES_PER_INPUT_BYTE_V1)
        .and_then(|work| work.checked_add(REGISTERED_CAPABILITY_MESSAGE_FIXED_UPPER_V1))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
}

fn verify_registered_required_capabilities_v1(
    operation: &Operation,
    location: &VerificationDiagnosticLocationV1<'_>,
    supported: Option<&BTreeSet<TargetCapability>>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    let Some(supported) = supported else {
        return Ok(());
    };
    budget.charge_work(
        operation
            .required_capability_visitation_work_v1()
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    )?;
    operation.try_visit_required_capabilities_v1(|required| {
        budget.charge_work(1)?;
        if !target_capability_is_supported_with_budget_v1(required, supported, budget)? {
            emit_registered_diagnostic_v1(
                location,
                DiagnosticCode::UnsupportedCapability,
                registered_capability_message_work_upper_v1(required)?,
                format_args!("target does not support required capability {required:?}"),
                diagnostics,
                budget,
            )?;
        }
        Ok(())
    })
}

struct RegisteredOperandRosterV1<'module> {
    operands: Vec<ValueId>,
    types: Vec<Option<&'module Type>>,
    retained_storage: usize,
}

impl<'module> RegisteredOperandRosterV1<'module> {
    fn build(
        operation: &Operation,
        function_state: &VerificationFunctionStateV1<'module>,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Self, CanonicalKernelIrVerificationResourceErrorV1> {
        let mut count = 0_usize;
        operation.kind.try_visit_operands(|_| {
            budget.charge_work(1)?;
            count = count
                .checked_add(1)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            Ok(())
        })?;
        // One exact ValueId cell and one nullable borrowed Type cell per
        // operand. Neither vector owns any part of a recursive Type.
        let retained_storage = count
            .checked_mul(2)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.reserve_storage(retained_storage)?;
        let mut operands = Vec::new();
        let mut types = Vec::new();
        let built = (|| {
            operands
                .try_reserve_exact(count)
                .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
            types
                .try_reserve_exact(count)
                .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
            operation.kind.try_visit_operands(|value| {
                // Source visit and the two fixed-cell publications are
                // admitted before the independent definition-index lookup.
                budget.charge_work(3)?;
                if operands.len() >= count {
                    return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
                }
                let ty = function_state.definition(value, budget)?.map(|row| row.ty);
                operands.push(value);
                types.push(ty);
                Ok(())
            })?;
            budget.charge_work(1)?;
            if operands.len() != count || types.len() != count {
                return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
            }
            Ok(())
        })();
        if let Err(error) = built {
            drop(types);
            drop(operands);
            budget.release_storage(retained_storage)?;
            return Err(error);
        }
        Ok(Self {
            operands,
            types,
            retained_storage,
        })
    }

    fn release(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        drop(self.types);
        drop(self.operands);
        budget.release_storage(self.retained_storage)
    }
}

struct RegisteredOperationIssueSinkV1<'a, 'm, 'work> {
    location: VerificationDiagnosticLocationV1<'m>,
    diagnostics: &'a mut VerificationDiagnosticCollectorV1,
    budget: &'a mut CanonicalKernelIrVerificationResourceBudgetV1<'work>,
}

impl SemanticOperationIssueSinkV1 for RegisteredOperationIssueSinkV1<'_, '_, '_> {
    type Error = CanonicalKernelIrVerificationResourceErrorV1;

    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.budget.charge_work(amount)
    }

    fn emit(
        &mut self,
        kind: SemanticOperationIssueKind,
        message_work_upper: usize,
        arguments: fmt::Arguments<'_>,
    ) -> Result<(), Self::Error> {
        self.budget.charge_work(1)?;
        let code = match kind {
            SemanticOperationIssueKind::InvalidStructure => {
                DiagnosticCode::InvalidSemanticOperation
            }
            SemanticOperationIssueKind::InvalidOperandType => DiagnosticCode::InvalidOperandType,
            SemanticOperationIssueKind::ResultArity => DiagnosticCode::ResultArity,
            SemanticOperationIssueKind::TypeMismatch => DiagnosticCode::TypeMismatch,
        };
        emit_registered_diagnostic_v1(
            &self.location,
            code,
            message_work_upper,
            arguments,
            self.diagnostics,
            self.budget,
        )
    }
}

impl MatrixOperationIssueSinkV1 for RegisteredOperationIssueSinkV1<'_, '_, '_> {
    type Error = CanonicalKernelIrVerificationResourceErrorV1;

    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.budget.charge_work(amount)
    }

    fn allocate_coordinates(&mut self, count: usize) -> Result<Vec<[u64; 2]>, Self::Error> {
        // Prepay fixed allocation/release bookkeeping so cleanup itself cannot
        // be prevented by a later rejected work charge.
        self.budget.charge_work(6)?;
        let storage = count
            .checked_mul(2)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        self.budget.reserve_storage(storage)?;
        let mut coordinates = Vec::new();
        if coordinates.try_reserve_exact(count).is_err() {
            drop(coordinates);
            self.budget.release_storage(storage)?;
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation);
        }
        Ok(coordinates)
    }

    fn release_coordinates(
        &mut self,
        coordinates: Vec<[u64; 2]>,
        count: usize,
    ) -> Result<(), Self::Error> {
        drop(coordinates);
        let storage = count
            .checked_mul(2)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        self.budget.release_storage(storage)
    }

    fn emit(
        &mut self,
        kind: MatrixVerificationIssueKind,
        message_work_upper: usize,
        arguments: fmt::Arguments<'_>,
    ) -> Result<(), Self::Error> {
        self.budget.charge_work(1)?;
        let code = match kind {
            MatrixVerificationIssueKind::InvalidStructure => {
                DiagnosticCode::InvalidSemanticOperation
            }
            MatrixVerificationIssueKind::InvalidOperandType => DiagnosticCode::InvalidOperandType,
            MatrixVerificationIssueKind::InvalidResult => DiagnosticCode::TypeMismatch,
        };
        emit_registered_diagnostic_v1(
            &self.location,
            code,
            message_work_upper,
            arguments,
            self.diagnostics,
            self.budget,
        )
    }
}

/// Verifies registered payloads from the canonical decoder's depth-bounded
/// module. Required capabilities are checked for every operation, including
/// those delegated to the legacy dispatcher by a false return.
pub(crate) fn try_verify_registered_operation_with_resources_v1<'module>(
    operation: &'module Operation,
    location: &VerificationDiagnosticLocationV1<'_>,
    function_state: &VerificationFunctionStateV1<'module>,
    supported: Option<&BTreeSet<TargetCapability>>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    verify_registered_required_capabilities_v1(
        operation,
        location,
        supported,
        diagnostics,
        budget,
    )?;
    budget.charge_work(1)?;
    if !matches!(
        operation.kind,
        OperationKind::Intrinsic(_)
            | OperationKind::MemoryIntrinsic(_)
            | OperationKind::Matrix(_)
            | OperationKind::Execution(_)
    ) {
        return Ok(false);
    }

    let roster = RegisteredOperandRosterV1::build(operation, function_state, budget)?;
    let result = {
        let mut sink = RegisteredOperationIssueSinkV1 {
            location: *location,
            diagnostics,
            budget,
        };
        match &operation.kind {
            OperationKind::Matrix(matrix) => matrix
                .try_verify_with_sink_v1(&roster.types, &operation.results, &mut sink)
                .map(|()| true),
            _ => try_verify_semantic_operation_with_sink_v1(
                &operation.kind,
                SemanticOperationBorrowedVerificationContextV1 {
                    operands: &roster.operands,
                    results: &operation.results,
                    operand_types: &roster.types,
                },
                &mut sink,
            ),
        }
    };
    let released = roster.release(budget);
    result.and_then(|handled| released.map(|()| handled))
}

#[cfg(test)]
#[path = "verification_registered_operation_v1_tests.rs"]
mod tests;
