//! Local carrier checks; whole physical SSA/CFG verification uses the same ledger.
use crate::gfx942_physical_lds_exchange_profile_v22::{
    PhysicalLdsExchangeProfileErrorV22, check_gfx942_physical_lds_exchange_function_v22,
};
use crate::{
    CanonicalKernelIrVerificationResourceErrorV1, DiagnosticCode,
    GFX942_PHYSICAL_ENTRY_REGISTERS_V20, Operation, OperationKind, Type,
    VerificationDiagnosticLocationV1, VerificationFunctionPassV1,
};
impl VerificationFunctionPassV1<'_, '_, '_> {
    pub(crate) fn verify_physical_lds_exchange_operation_v22(
        &mut self,
        operation: &Operation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(64)?;
        let mut valid = true;
        match &operation.kind {
            OperationKind::Gfx942PhysicalLdsExchangeDeclaration(declaration) => {
                valid &= declaration.validate_shape().is_ok() && operation.results.len() == 5;
                for (value, register) in operation
                    .results
                    .iter()
                    .zip(GFX942_PHYSICAL_ENTRY_REGISTERS_V20)
                {
                    valid &= value.ty == Type::Scalar(register.scalar_type());
                }
            }
            OperationKind::Gfx942PhysicalLdsExchangeStep(step) => {
                valid &= step.validate_shape().is_ok();
                let results = step.instruction.result_registers();
                valid &= operation.results.len() == results.iter().flatten().count();
                for (value, register) in operation.results.iter().zip(results.into_iter().flatten())
                {
                    valid &= value.ty == Type::Scalar(register.scalar_type());
                }
                for (value, register) in step
                    .operands
                    .iter()
                    .zip(step.instruction.operand_registers())
                {
                    match (value, register) {
                        (Some(value), Some(register)) => {
                            valid &= self.definition_type_v1(*value)?
                                == Some(&Type::Scalar(register.scalar_type()));
                        }
                        (None, None) => {}
                        _ => valid = false,
                    }
                }
            }
            _ => return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting),
        }
        if !valid {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidPhysicalLdsExchangeV22,
                "physical-LDS-exchange local result/operand/type shape",
            )?;
        }
        Ok(())
    }
    pub(crate) fn verify_physical_lds_exchange_function_v22(
        &mut self,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        match check_gfx942_physical_lds_exchange_function_v22(
            self.module,
            self.function,
            self.budget,
        ) {
            Ok(()) => Ok(()),
            Err(PhysicalLdsExchangeProfileErrorV22::Resource(error)) => Err(error),
            Err(PhysicalLdsExchangeProfileErrorV22::Invalid(message)) => self.emit_fixed(
                location,
                DiagnosticCode::InvalidPhysicalLdsExchangeV22,
                message,
            ),
        }
    }
}
