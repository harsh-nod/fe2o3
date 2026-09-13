use crate::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceErrorV1, Constant, DiagnosticCode,
    ScalarType, Terminator, Type, ValueId, VerificationDiagnosticLocationV1,
    VerificationFunctionPassV1, verification_bounded_sort_by_v1,
    verification_type_message_work_upper_v1, verification_types_equal_v1,
};

impl VerificationFunctionPassV1<'_, '_, '_> {
    pub(crate) fn verify_terminator_uses_v1(
        &mut self,
        terminator: &Terminator,
        block: BlockId,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(1)?;
        match terminator {
            Terminator::Branch { arguments, .. } => {
                self.verify_use_slice_v1(arguments, block, location)
            }
            Terminator::ConditionalBranch {
                condition,
                then_arguments,
                else_arguments,
                ..
            } => {
                self.verify_use(*condition, block, None, location)?;
                self.verify_use_slice_v1(then_arguments, block, location)?;
                self.verify_use_slice_v1(else_arguments, block, location)
            }
            Terminator::Switch {
                selector,
                cases,
                default_arguments,
                ..
            } => {
                self.verify_use(*selector, block, None, location)?;
                self.budget.charge_work(cases.len())?;
                for case in cases {
                    self.verify_use_slice_v1(&case.arguments, block, location)?;
                }
                self.verify_use_slice_v1(default_arguments, block, location)
            }
            Terminator::IntegerSwitch {
                selector,
                cases,
                default_arguments,
                ..
            } => {
                self.verify_use(*selector, block, None, location)?;
                self.budget.charge_work(cases.len())?;
                for case in cases {
                    self.verify_use_slice_v1(&case.arguments, block, location)?;
                }
                self.verify_use_slice_v1(default_arguments, block, location)
            }
            Terminator::Return { values } => self.verify_use_slice_v1(values, block, location),
            Terminator::Unreachable => Ok(()),
        }
    }

    fn verify_use_slice_v1(
        &mut self,
        values: &[ValueId],
        block: BlockId,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(values.len())?;
        for value in values {
            self.verify_use(*value, block, None, location)?;
        }
        Ok(())
    }

    pub(crate) fn verify_terminator_v1(
        &mut self,
        _block: &BasicBlock,
        terminator: &Terminator,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(1)?;
        match terminator {
            Terminator::Branch { target, arguments } => {
                self.verify_edge_v1(*target, arguments, location)
            }
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                self.expect_type_v1(*condition, &Type::BOOL, location)?;
                self.verify_edge_v1(*then_target, then_arguments, location)?;
                self.verify_edge_v1(*else_target, else_arguments, location)
            }
            Terminator::Switch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                self.expect_integer_v1(*selector, location)?;
                self.verify_legacy_switch_cases_v1(cases, location)?;
                self.budget.charge_work(cases.len())?;
                for case in cases {
                    self.verify_edge_v1(case.target, &case.arguments, location)?;
                }
                self.verify_edge_v1(*default_target, default_arguments, location)
            }
            Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                let selector_ty = self.definition_type_v1(*selector)?;
                self.expect_integer_v1(*selector, location)?;
                let mut previous: Option<&Constant> = None;
                self.budget.charge_work(cases.len())?;
                for case in cases {
                    if let Some(previous) = previous {
                        self.budget.charge_work(1)?;
                        match previous.cmp(&case.value) {
                            std::cmp::Ordering::Equal => self.emit_dynamic(
                                location,
                                DiagnosticCode::DuplicateSwitchCase,
                                256,
                                format_args!(
                                    "integer switch case {:?} appears more than once",
                                    case.value
                                ),
                            )?,
                            std::cmp::Ordering::Greater => self.emit_dynamic(
                                location,
                                DiagnosticCode::UnsortedSwitchCase,
                                384,
                                format_args!(
                                    "integer switch case {:?} is not greater than previous case {previous:?}",
                                    case.value
                                ),
                            )?,
                            std::cmp::Ordering::Less => {}
                        }
                    }
                    previous = Some(&case.value);
                    let case_ty = case.value.ty();
                    if !case_ty.as_scalar().is_some_and(ScalarType::is_integer) {
                        self.emit_dynamic(
                            location,
                            DiagnosticCode::InvalidOperandType,
                            256,
                            format_args!(
                                "integer switch case {:?} must have integer or index type",
                                case.value
                            ),
                        )?;
                    }
                    if let Some(selector_ty) = selector_ty
                        && !verification_types_equal_v1(selector_ty, &case_ty, self.budget)?
                    {
                        let type_work =
                            verification_type_message_work_upper_v1(selector_ty, self.budget)?;
                        self.emit_dynamic(
                            location,
                            DiagnosticCode::TypeMismatch,
                            type_work.checked_add(384).ok_or(
                                CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                            )?,
                            format_args!(
                                "integer switch case {:?} has type {case_ty:?}, expected selector type {selector_ty:?}",
                                case.value
                            ),
                        )?;
                    }
                    self.verify_edge_v1(case.target, &case.arguments, location)?;
                }
                self.verify_edge_v1(*default_target, default_arguments, location)
            }
            Terminator::Return { values } => {
                self.verify_argument_list_v1(values, &self.function.signature.results, location)
            }
            Terminator::Unreachable => Ok(()),
        }
    }

    fn verify_legacy_switch_cases_v1(
        &mut self,
        cases: &[crate::SwitchCase],
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let count = cases.len();
        self.budget.charge_work(count)?;
        self.budget.reserve_storage(count)?;
        let result = (|| {
            let mut values = Vec::new();
            values
                .try_reserve_exact(count)
                .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
            values.extend(cases.iter().map(|case| case.value));
            verification_bounded_sort_by_v1(&mut values, 1, self.budget, std::cmp::Ord::cmp)?;
            self.budget.charge_work(values.len())?;
            for pair in values.windows(2) {
                self.budget.charge_work(1)?;
                if pair[0] == pair[1] {
                    self.emit_dynamic(
                        location,
                        DiagnosticCode::DuplicateSwitchCase,
                        128,
                        format_args!("switch case {} appears more than once", pair[1]),
                    )?;
                }
            }
            Ok(())
        })();
        let released = self.budget.release_storage(count);
        result.and(released)
    }

    pub(crate) fn verify_edge_v1(
        &mut self,
        target: BlockId,
        arguments: &[ValueId],
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let Some(target_block) = self.function_state.block(target, self.budget)? else {
            return self.emit_dynamic(
                location,
                DiagnosticCode::InvalidBranchTarget,
                128,
                format_args!("branch target {target} is not defined"),
            );
        };
        if arguments.len() != target_block.parameters.len() {
            self.emit_dynamic(
                location,
                DiagnosticCode::BranchArgumentCount,
                256,
                format_args!(
                    "branch to {target} supplies {} arguments for {} block parameters",
                    arguments.len(),
                    target_block.parameters.len()
                ),
            )?;
        }
        self.budget
            .charge_work(arguments.len().min(target_block.parameters.len()))?;
        for (argument, parameter) in arguments.iter().zip(&target_block.parameters) {
            let Some(argument_ty) = self.definition_type_v1(*argument)? else {
                continue;
            };
            if !verification_types_equal_v1(argument_ty, &parameter.ty, self.budget)? {
                let argument_work =
                    verification_type_message_work_upper_v1(argument_ty, self.budget)?;
                let parameter_work =
                    verification_type_message_work_upper_v1(&parameter.ty, self.budget)?;
                self.emit_dynamic(
                    location,
                    DiagnosticCode::BranchArgumentType,
                    argument_work
                        .checked_add(parameter_work)
                        .and_then(|work| work.checked_add(256))
                        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
                    format_args!(
                        "branch argument {argument} has type {argument_ty:?}, expected {:?}",
                        parameter.ty
                    ),
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn verify_argument_list_v1(
        &mut self,
        values: &[ValueId],
        expected: &[Type],
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        if values.len() != expected.len() {
            self.emit_dynamic(
                location,
                DiagnosticCode::SignatureMismatch,
                256,
                format_args!(
                    "found {} values where {} are required",
                    values.len(),
                    expected.len()
                ),
            )?;
        }
        self.budget.charge_work(values.len().min(expected.len()))?;
        for (value, expected_ty) in values.iter().zip(expected) {
            self.expect_type_v1(*value, expected_ty, location)?;
        }
        Ok(())
    }

    pub(crate) fn expect_type_v1(
        &mut self,
        value: ValueId,
        expected: &Type,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let Some(actual) = self.definition_type_v1(value)? else {
            return Ok(());
        };
        if !verification_types_equal_v1(actual, expected, self.budget)? {
            let actual_work = verification_type_message_work_upper_v1(actual, self.budget)?;
            let expected_work = verification_type_message_work_upper_v1(expected, self.budget)?;
            self.emit_dynamic(
                location,
                DiagnosticCode::TypeMismatch,
                actual_work
                    .checked_add(expected_work)
                    .and_then(|work| work.checked_add(256))
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
                format_args!("value {value} has type {actual:?}, expected {expected:?}"),
            )?;
        }
        Ok(())
    }

    pub(crate) fn expect_integer_v1(
        &mut self,
        value: ValueId,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let Some(ty) = self.definition_type_v1(value)? else {
            return Ok(());
        };
        if !ty.as_scalar().is_some_and(ScalarType::is_integer) {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidOperandType,
                128,
                format_args!("value {value} must have integer or index type"),
            )?;
        }
        Ok(())
    }
}
