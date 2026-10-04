//! V19 complete-body local checks and the shared general-verifier hook.
use crate::gfx942_complete_body_profile_v19::{
    CompleteBodyProfileErrorV19, check_gfx942_complete_body_function_v19,
};
use crate::{
    CanonicalKernelIrVerificationResourceErrorV1, DiagnosticCode, Operation, OperationKind,
    ScalarType, Type, VerificationDiagnosticLocationV1, VerificationFunctionPassV1,
};

impl VerificationFunctionPassV1<'_, '_, '_> {
    pub(crate) fn verify_complete_body_operation_v19(
        &mut self,
        operation: &Operation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(32)?;
        let valid = match &operation.kind {
            OperationKind::Gfx942CompleteBodyDeclaration(_) => operation.results.is_empty(),
            OperationKind::Gfx942CompleteBodyStep(step) => {
                let result = matches!(operation.results.as_slice(), [value]
                    if matches!(value.ty, Type::Scalar(ScalarType::U32)));
                let mut operands = true;
                for value in step.operands.iter().flatten() {
                    operands &= matches!(
                        self.definition_type_v1(*value)?,
                        Some(Type::Scalar(ScalarType::U32))
                    );
                }
                result && operands
            }
            _ => return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting),
        };
        if !valid {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidCompleteBodyV19,
                "complete-body operation result or operand type",
            )?;
        }
        Ok(())
    }

    pub(crate) fn verify_complete_body_function_v19(
        &mut self,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        match check_gfx942_complete_body_function_v19(self.module, self.function, self.budget) {
            Ok(()) => Ok(()),
            Err(CompleteBodyProfileErrorV19::Resource(error)) => Err(error),
            Err(CompleteBodyProfileErrorV19::Invalid(message)) => {
                self.emit_fixed(location, DiagnosticCode::InvalidCompleteBodyV19, message)
            }
        }
    }
}
