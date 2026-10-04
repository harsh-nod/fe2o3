use crate::{
    AssemblyConstraint, AssemblyEffect, AssemblyOperandKind, AssemblyOption,
    CanonicalKernelIrVerificationResourceErrorV1, DiagnosticCode, InlineAssembly, Operation,
    VerificationDiagnosticLocationV1, VerificationFunctionPassV1, is_assembly_register_type,
    verification_type_message_work_upper_v1,
};

impl<'a, 'module, 'work> VerificationFunctionPassV1<'a, 'module, 'work> {
    pub(crate) fn verify_inline_assembly_v1(
        &mut self,
        operation: &Operation,
        assembly: &InlineAssembly,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(
            assembly
                .mnemonic
                .len()
                .checked_add(assembly.operands.len())
                .and_then(|work| work.checked_add(4))
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
        )?;
        if !assembly.source.is_complete() {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidInlineAssembly,
                "inline assembly requires nonzero frontend-unit, function, contract, and statement identities",
            )?;
        }
        if assembly.mnemonic.is_empty()
            || !assembly
                .mnemonic
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidInlineAssembly,
                "inline assembly mnemonic must be nonempty canonical lowercase ASCII",
            )?;
        }
        if assembly.operands.is_empty() {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidInlineAssembly,
                "inline assembly requires at least one exact operand",
            )?;
        }

        let result_count = operation.results.len();
        self.budget.charge_work(result_count)?;
        self.budget.reserve_storage(result_count)?;
        let mut referenced_results = Vec::new();
        if referenced_results.try_reserve_exact(result_count).is_err() {
            self.budget.release_storage(result_count)?;
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation);
        }
        referenced_results.resize(result_count, false);
        let verify = (|| {
            for (operand_index, operand) in assembly.operands.iter().enumerate() {
                self.budget.charge_work(1)?;
                let value = match operand.kind {
                    AssemblyOperandKind::Input(value) => Some(value),
                    AssemblyOperandKind::InOut {
                        input,
                        result_index,
                    } => {
                        self.verify_assembly_result_v1(
                            operation,
                            result_index,
                            operand.constraint,
                            operand_index,
                            &mut referenced_results,
                            location,
                        )?;
                        Some(input)
                    }
                    AssemblyOperandKind::Output { result_index } => {
                        self.verify_assembly_result_v1(
                            operation,
                            result_index,
                            operand.constraint,
                            operand_index,
                            &mut referenced_results,
                            location,
                        )?;
                        None
                    }
                    AssemblyOperandKind::ImmediateI32(_) => {
                        if operand.constraint != AssemblyConstraint::ImmediateI32 {
                            self.emit_dynamic(
                                location,
                                DiagnosticCode::InvalidInlineAssembly,
                                192,
                                format_args!(
                                    "inline assembly immediate operand {operand_index} requires ImmediateI32 constraint"
                                ),
                            )?;
                        }
                        None
                    }
                };
                if let Some(value) = value {
                    if operand.constraint == AssemblyConstraint::ImmediateI32 {
                        self.emit_dynamic(
                            location,
                            DiagnosticCode::InvalidInlineAssembly,
                            192,
                            format_args!(
                                "inline assembly SSA operand {operand_index} cannot use an immediate constraint"
                            ),
                        )?;
                    }
                    if let Some(ty) = self.definition_type_v1(value)?
                        && !is_assembly_register_type(ty)
                    {
                        let type_work = verification_type_message_work_upper_v1(ty, self.budget)?;
                        self.emit_dynamic(
                            location,
                            DiagnosticCode::InvalidOperandType,
                            type_work.checked_add(192).ok_or(
                                CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                            )?,
                            format_args!(
                                "inline assembly register operand {operand_index} requires i32 or u32, found {ty:?}"
                            ),
                        )?;
                    }
                }
            }
            self.budget.charge_work(referenced_results.len())?;
            let referenced_count = referenced_results.iter().filter(|seen| **seen).count();
            if referenced_count != operation.results.len() {
                self.emit_fixed(
                    location,
                    DiagnosticCode::ResultArity,
                    "every inline assembly result must be referenced exactly once by an output or inout operand",
                )?;
            }

            self.budget.charge_work(
                assembly
                    .options
                    .len()
                    .checked_mul(5)
                    .and_then(|work| {
                        assembly
                            .declared_effects
                            .len()
                            .checked_mul(4)
                            .and_then(|effects| work.checked_add(effects))
                    })
                    .and_then(|work| work.checked_add(8))
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
            )?;
            let no_memory = assembly.options.contains(&AssemblyOption::NoMemory);
            let read_only = assembly.options.contains(&AssemblyOption::ReadOnly);
            if no_memory && read_only {
                self.emit_fixed(
                    location,
                    DiagnosticCode::InvalidInlineAssembly,
                    "NoMemory and ReadOnly assembly options are mutually exclusive",
                )?;
            }
            if assembly.options.contains(&AssemblyOption::Pure) && !(no_memory || read_only) {
                self.emit_fixed(
                    location,
                    DiagnosticCode::InvalidInlineAssembly,
                    "Pure inline assembly requires NoMemory or ReadOnly",
                )?;
            }
            let has_memory_effect = assembly
                .declared_effects
                .iter()
                .any(|effect| !matches!(effect, AssemblyEffect::ControlFlow));
            if no_memory && has_memory_effect {
                self.emit_fixed(
                    location,
                    DiagnosticCode::InvalidInlineAssembly,
                    "NoMemory inline assembly cannot declare memory, atomic, or barrier effects",
                )?;
            }
            let has_write = assembly.declared_effects.iter().any(|effect| {
                matches!(
                    effect,
                    AssemblyEffect::WriteGlobal
                        | AssemblyEffect::WriteWorkgroup
                        | AssemblyEffect::Atomic
                )
            });
            if read_only && has_write {
                self.emit_fixed(
                    location,
                    DiagnosticCode::InvalidInlineAssembly,
                    "ReadOnly inline assembly cannot declare write or atomic effects",
                )?;
            }
            if assembly.options.contains(&AssemblyOption::Pure)
                && assembly
                    .declared_effects
                    .contains(&AssemblyEffect::ControlFlow)
            {
                self.emit_fixed(
                    location,
                    DiagnosticCode::InvalidInlineAssembly,
                    "Pure inline assembly cannot declare control-flow effects",
                )?;
            }
            if assembly.declared_effects.is_empty() && !no_memory {
                self.emit_fixed(
                    location,
                    DiagnosticCode::InvalidInlineAssembly,
                    "effect-free inline assembly requires an explicit NoMemory option",
                )?;
            }
            Ok(())
        })();
        drop(referenced_results);
        let released = self.budget.release_storage(result_count);
        verify.and(released)
    }

    #[allow(clippy::too_many_arguments)]
    fn verify_assembly_result_v1(
        &mut self,
        operation: &Operation,
        result_index: u32,
        constraint: AssemblyConstraint,
        operand_index: usize,
        referenced_results: &mut [bool],
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(1)?;
        let Some(result) = usize::try_from(result_index)
            .ok()
            .and_then(|index| operation.results.get(index))
        else {
            return self.emit_dynamic(
                location,
                DiagnosticCode::ResultArity,
                192,
                format_args!(
                    "inline assembly operand {operand_index} references missing result {result_index}"
                ),
            );
        };
        let seen = referenced_results
            .get_mut(result_index as usize)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
        if *seen {
            self.emit_dynamic(
                location,
                DiagnosticCode::DuplicateValue,
                192,
                format_args!("inline assembly result {result_index} is referenced more than once"),
            )?;
        }
        *seen = true;
        if constraint == AssemblyConstraint::ImmediateI32 {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidInlineAssembly,
                192,
                format_args!("inline assembly output operand {operand_index} cannot be immediate"),
            )?;
        }
        if !is_assembly_register_type(&result.ty) {
            let type_work = verification_type_message_work_upper_v1(&result.ty, self.budget)?;
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidOperandType,
                type_work
                    .checked_add(192)
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
                format_args!(
                    "inline assembly result operand {operand_index} requires i32 or u32, found {:?}",
                    result.ty
                ),
            )?;
        }
        Ok(())
    }
}
