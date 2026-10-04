//! Fresh local shape verification using the existing bounded definition index.

use crate::{
    CanonicalKernelIrVerificationResourceErrorV1, DiagnosticCode, Operation, OperationKind,
    VerificationDiagnosticLocationV1, VerificationFunctionPassV1,
    validate_gfx942_ordered_region_v1,
};

impl VerificationFunctionPassV1<'_, '_, '_> {
    pub(crate) fn verify_ordered_region_v16(
        &mut self,
        operation: &Operation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        // Fixed source/binding/result validation and three fixed-array lookups.
        // Definition index work is independently charged by definition_type_v1.
        self.budget.charge_work(180)?;
        let OperationKind::Gfx942OrderedRegion(region) = &operation.kind else {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        };
        let inputs = *region.inputs();
        let mut types = [None; 3];
        for (index, value) in inputs.iter().enumerate() {
            types[index] = self
                .definition_type_v1(*value)?
                .and_then(crate::Type::as_scalar);
        }
        if let Err(error) = validate_gfx942_ordered_region_v1(operation, |value| {
            inputs
                .iter()
                .position(|input| *input == value)
                .and_then(|index| types[index])
        }) {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidOrderedRegion,
                192,
                format_args!("{error}"),
            )?;
        }
        Ok(())
    }
}
