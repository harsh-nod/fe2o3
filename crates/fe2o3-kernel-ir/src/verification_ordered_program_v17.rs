//! Local program shape verification using the existing bounded definition index.

use crate::{
    CanonicalKernelIrVerificationResourceErrorV1, DiagnosticCode, Operation, OperationKind,
    VerificationDiagnosticLocationV1, VerificationFunctionPassV1,
    validate_gfx942_ordered_program_v1,
};

impl VerificationFunctionPassV1<'_, '_, '_> {
    pub(crate) fn verify_ordered_program_v17(
        &mut self,
        operation: &Operation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        // Fixed source/register/result checks, all sixteen descriptor slots and
        // initialization validation. Definition-index lookup work is separately
        // charged by definition_type_v1; no per-step Module traversal occurs.
        self.budget.charge_work(4096)?;
        let OperationKind::Gfx942OrderedProgram(program) = &operation.kind else {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        };
        let inputs = *program.inputs();
        let mut types = [None; 3];
        for (index, value) in inputs.iter().enumerate() {
            types[index] = self
                .definition_type_v1(*value)?
                .and_then(crate::Type::as_scalar);
        }
        if let Err(error) = validate_gfx942_ordered_program_v1(operation, |value| {
            inputs
                .iter()
                .position(|input| *input == value)
                .and_then(|index| types[index])
        }) {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidOrderedProgram,
                256,
                format_args!("{error}"),
            )?;
        }
        Ok(())
    }
}
