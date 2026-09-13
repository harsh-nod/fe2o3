use crate::{
    AccessMode, AddressSpace, BinaryOp, CanonicalKernelIrVerificationResourceErrorV1, CastKind,
    CheckedBinaryOperator, ComparePredicate, DiagnosticCode, Operation, OperationKind, ScalarType,
    Type, ValueId, VerificationDiagnosticLocationV1, VerificationFunctionPassV1,
    identifier_message_work_v1, valid_scalar_cast, verification_type_message_work_upper_v1,
    verification_types_equal_v1,
};

// Borrow the expected pointee while reproducing Type::Pointer's compact Debug form.
struct ExpectedPointerTypeDebugV1<'a> {
    pointee: &'a Type,
    address_space: AddressSpace,
    access: AccessMode,
}

impl std::fmt::Debug for ExpectedPointerTypeDebugV1<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Pointer(")?;
        formatter
            .debug_struct("PointerType")
            .field("pointee", &self.pointee)
            .field("address_space", &self.address_space)
            .field("access", &self.access)
            .finish()?;
        formatter.write_str(")")
    }
}

impl<'a, 'module, 'work> VerificationFunctionPassV1<'a, 'module, 'work> {
    pub(crate) fn verify_legacy_operation_v1(
        &mut self,
        operation: &Operation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(1)?;
        match &operation.kind {
            OperationKind::VerificationContract(
                crate::VerificationContractOperationV12::WorkgroupPipelineEvent {
                    storage,
                    epoch,
                    ..
                },
            ) => {
                self.expect_no_results_v1(operation, location)?;
                if let Some(ty) = self.definition_type_v1(*storage)?
                    && !matches!(ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Workgroup)
                {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidOperandType,
                        "verification contract storage must be a workgroup pointer",
                    )?;
                }
                self.expect_type_v1(*epoch, &Type::INDEX, location)
            }
            OperationKind::Constant(constant) => {
                let expected = constant.ty();
                self.expect_single_result_type_v1(operation, &expected, location)
            }
            OperationKind::Intrinsic(_)
            | OperationKind::MemoryIntrinsic(_)
            | OperationKind::Matrix(_) => {
                Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
            }
            OperationKind::Unary { op, operand } => {
                let Some(ty) = self.definition_type_v1(*operand)? else {
                    return Ok(());
                };
                let valid = match (op, ty.as_scalar()) {
                    (crate::UnaryOp::Negate, Some(scalar)) => {
                        scalar.is_signed_integer() || scalar.is_float()
                    }
                    (crate::UnaryOp::Not, Some(ScalarType::Bool)) => true,
                    (crate::UnaryOp::Not, Some(scalar)) => scalar.is_integer(),
                    _ => false,
                };
                if !valid {
                    let type_work = verification_type_message_work_upper_v1(ty, self.budget)?;
                    self.emit_dynamic(
                        location,
                        DiagnosticCode::InvalidOperandType,
                        type_work
                            .checked_add(256)
                            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
                        format_args!("unary {op:?} does not accept {ty:?}"),
                    )?;
                }
                self.expect_single_result_type_v1(operation, ty, location)
            }
            OperationKind::Binary { op, lhs, rhs } => {
                self.verify_binary_v1(operation, *op, *lhs, *rhs, location)
            }
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            } => self.verify_compare_v1(operation, *predicate, *lhs, *rhs, location),
            OperationKind::Cast { kind, value, to } => {
                let Some(from) = self.definition_type_v1(*value)? else {
                    return Ok(());
                };
                let (valid, code) = match (kind, from, to) {
                    (
                        CastKind::RestrictPointerAccess,
                        Type::Pointer(from_pointer),
                        Type::Pointer(to_pointer),
                    ) => (
                        verification_types_equal_v1(
                            &from_pointer.pointee,
                            &to_pointer.pointee,
                            self.budget,
                        )? && from_pointer.address_space == to_pointer.address_space
                            && from_pointer.access == AccessMode::ReadWrite
                            && to_pointer.access == AccessMode::ReadOnly,
                        DiagnosticCode::InvalidCast,
                    ),
                    (CastKind::RestrictPointerAccess, _, _) => (false, DiagnosticCode::InvalidCast),
                    (_, Type::Scalar(from_scalar), Type::Scalar(to_scalar)) => (
                        valid_scalar_cast(*kind, *from_scalar, *to_scalar),
                        DiagnosticCode::InvalidCast,
                    ),
                    _ => (false, DiagnosticCode::InvalidOperandType),
                };
                if !valid {
                    let from_work = verification_type_message_work_upper_v1(from, self.budget)?;
                    let to_work = verification_type_message_work_upper_v1(to, self.budget)?;
                    let message_work = from_work
                        .checked_add(to_work)
                        .and_then(|work| work.checked_add(384))
                        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
                    if code == DiagnosticCode::InvalidCast {
                        self.emit_dynamic(
                            location,
                            code,
                            message_work,
                            format_args!("invalid {kind:?} cast from {from:?} to {to:?}"),
                        )?;
                    } else {
                        self.emit_dynamic(
                            location,
                            code,
                            message_work,
                            format_args!(
                                "casts require compatible scalar or pointer types, found {from:?} to {to:?}"
                            ),
                        )?;
                    }
                }
                self.expect_single_result_type_v1(operation, to, location)
            }
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => {
                self.expect_type_v1(*condition, &Type::BOOL, location)?;
                let (Some(true_ty), Some(false_ty)) = (
                    self.definition_type_v1(*true_value)?,
                    self.definition_type_v1(*false_value)?,
                ) else {
                    return Ok(());
                };
                if !verification_types_equal_v1(true_ty, false_ty, self.budget)? {
                    let true_work = verification_type_message_work_upper_v1(true_ty, self.budget)?;
                    let false_work =
                        verification_type_message_work_upper_v1(false_ty, self.budget)?;
                    self.emit_dynamic(
                        location,
                        DiagnosticCode::TypeMismatch,
                        true_work
                            .checked_add(false_work)
                            .and_then(|work| work.checked_add(256))
                            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
                        format_args!("select alternatives differ: {true_ty:?} and {false_ty:?}"),
                    )?;
                }
                self.expect_single_result_type_v1(operation, true_ty, location)
            }
            OperationKind::Call { callee, arguments } => {
                crate::verify_reserved_call_shape_v1(
                    callee,
                    arguments.len(),
                    location,
                    self.diagnostics,
                    self.budget,
                )?;
                let Some(callee_function) = self.module_state.find_function(callee, self.budget)?
                else {
                    return self.emit_dynamic(
                        location,
                        DiagnosticCode::UnknownCallee,
                        identifier_message_work_v1(callee.as_str().len(), 128)?,
                        format_args!("callee {callee} is not in the module"),
                    );
                };
                self.verify_argument_list_v1(
                    arguments,
                    &callee_function.signature.parameters,
                    location,
                )?;
                self.expect_results_v1(operation, &callee_function.signature.results, location)
            }
            OperationKind::Alloca {
                element,
                count,
                address_space,
                alignment,
            } => {
                if !element.is_storable()
                    || !matches!(
                        address_space,
                        AddressSpace::Private | AddressSpace::Workgroup
                    )
                {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidMemoryAccess,
                        "alloca requires a storable type in private or workgroup memory",
                    )?;
                }
                if let Some(count) = count {
                    self.expect_integer_v1(*count, location)?;
                }
                self.verify_alignment_v1(*alignment, location)?;
                self.expect_pointer_result_v1(
                    operation,
                    element,
                    *address_space,
                    AccessMode::ReadWrite,
                    location,
                )
            }
            OperationKind::SliceLength { slice } => {
                if !matches!(self.definition_type_v1(*slice)?, Some(Type::Slice(_))) {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidOperandType,
                        "slice_length operand must have slice type",
                    )?;
                }
                self.expect_single_result_type_v1(operation, &Type::INDEX, location)
            }
            OperationKind::SliceData { slice } => {
                let Some(Type::Slice(slice_ty)) = self.definition_type_v1(*slice)? else {
                    return self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidOperandType,
                        "slice_data operand must have slice type",
                    );
                };
                self.expect_pointer_result_v1(
                    operation,
                    &slice_ty.element,
                    slice_ty.address_space,
                    slice_ty.access,
                    location,
                )
            }
            OperationKind::GetElementPointer { base, offset } => {
                let Some(base_ty) = self.definition_type_v1(*base)? else {
                    return Ok(());
                };
                if !matches!(base_ty, Type::Pointer(_)) {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidOperandType,
                        "get_element_pointer base must have pointer type",
                    )?;
                }
                self.expect_integer_v1(*offset, location)?;
                self.expect_single_result_type_v1(operation, base_ty, location)
            }
            OperationKind::Load { pointer, access } => {
                let Some(pointee) =
                    self.verify_pointer_access_v1(*pointer, *access, false, location)?
                else {
                    return Ok(());
                };
                self.expect_single_result_type_v1(operation, pointee, location)
            }
            OperationKind::GuardedLoad {
                pointer,
                predicate,
                fallback,
                access,
            } => {
                let Some(pointee) =
                    self.verify_pointer_access_v1(*pointer, *access, false, location)?
                else {
                    return Ok(());
                };
                self.expect_type_v1(*predicate, &Type::BOOL, location)?;
                self.expect_type_v1(*fallback, pointee, location)?;
                self.expect_single_result_type_v1(operation, pointee, location)
            }
            OperationKind::GuardedStore {
                pointer,
                predicate,
                value,
                access,
            } => {
                self.expect_no_results_v1(operation, location)?;
                let Some(pointee) =
                    self.verify_pointer_access_v1(*pointer, *access, true, location)?
                else {
                    return Ok(());
                };
                self.expect_type_v1(*predicate, &Type::BOOL, location)?;
                self.expect_type_v1(*value, pointee, location)
            }
            OperationKind::Store {
                pointer,
                value,
                access,
            } => {
                self.expect_no_results_v1(operation, location)?;
                let Some(pointee) =
                    self.verify_pointer_access_v1(*pointer, *access, true, location)?
                else {
                    return Ok(());
                };
                self.expect_type_v1(*value, pointee, location)
            }
            OperationKind::VectorLoad(load) => {
                let pointer = load.provenance.pointer();
                self.verify_vector_access_v1(load.access, pointer, false, location)?;
                let expected = Type::Vector(load.access.vector);
                self.expect_single_result_type_v1(operation, &expected, location)
            }
            OperationKind::VectorStore(store) => {
                self.expect_no_results_v1(operation, location)?;
                let pointer = store.provenance.pointer();
                self.verify_vector_access_v1(store.access, pointer, true, location)?;
                let expected = Type::Vector(store.access.vector);
                self.expect_type_v1(store.value, &expected, location)
            }
            OperationKind::VectorLayoutConvert(conversion) => {
                let Some(Type::Vector(source)) = self.definition_type_v1(conversion.value)? else {
                    return self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidOperandType,
                        "vector layout conversion requires a fixed-vector operand",
                    );
                };
                let target = source.with_layout(conversion.to);
                if source.layout == conversion.to {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidVectorOperation,
                        "vector layout conversion must change the physical layout",
                    )?;
                }
                if let Err(error) = target.validate() {
                    self.emit_dynamic(
                        location,
                        DiagnosticCode::InvalidVectorOperation,
                        256,
                        format_args!("{error}"),
                    )?;
                }
                let expected = Type::Vector(target);
                self.expect_single_result_type_v1(operation, &expected, location)
            }
            OperationKind::Barrier(barrier) => {
                self.expect_no_results_v1(operation, location)?;
                self.verify_barrier_v1(barrier, location)
            }
            OperationKind::Atomic(atomic) => self.verify_atomic_v1(operation, atomic, location),
            OperationKind::Fence(fence) => {
                self.expect_no_results_v1(operation, location)?;
                self.verify_fence_v1(fence, location)
            }
            OperationKind::WorkgroupBarrier(barrier) => {
                self.expect_no_results_v1(operation, location)?;
                self.verify_workgroup_barrier_v1(barrier, location)
            }
            OperationKind::WorkgroupMemory(memory) => {
                self.verify_workgroup_memory_v1(operation, memory, location)
            }
            OperationKind::Gfx950LdsTranspose(transpose) => {
                self.verify_gfx950_lds_transpose_v1(operation, transpose, location)
            }
            OperationKind::Wave(wave) => self.verify_wave_v1(operation, wave, location),
            OperationKind::InlineAssembly(assembly) => {
                self.verify_inline_assembly_v1(operation, assembly, location)
            }
        }
    }

    fn verify_binary_v1(
        &mut self,
        operation: &Operation,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        if let BinaryOp::Checked(operator) = op {
            return self.verify_checked_binary_v1(operation, operator, lhs, rhs, location);
        }
        let (Some(lhs_ty), Some(rhs_ty)) =
            (self.definition_type_v1(lhs)?, self.definition_type_v1(rhs)?)
        else {
            return Ok(());
        };
        let same = verification_types_equal_v1(lhs_ty, rhs_ty, self.budget)?;
        let lhs_scalar = lhs_ty.as_scalar();
        let rhs_scalar = rhs_ty.as_scalar();
        let valid = match op {
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                lhs_scalar.is_some_and(ScalarType::is_integer)
                    && rhs_scalar.is_some_and(ScalarType::is_integer)
            }
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                same && lhs_scalar
                    .is_some_and(|scalar| scalar == ScalarType::Bool || scalar.is_integer())
            }
            _ => same && lhs_scalar.is_some_and(ScalarType::is_numeric),
        };
        if !valid {
            self.emit_two_types_v1(
                location,
                DiagnosticCode::InvalidOperandType,
                lhs_ty,
                rhs_ty,
                format_args!("binary {op:?} does not accept {lhs_ty:?} and {rhs_ty:?}"),
            )?;
        }
        self.expect_single_result_type_v1(operation, lhs_ty, location)
    }

    fn verify_checked_binary_v1(
        &mut self,
        operation: &Operation,
        operator: CheckedBinaryOperator,
        lhs: ValueId,
        rhs: ValueId,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let (Some(lhs_ty), Some(rhs_ty)) =
            (self.definition_type_v1(lhs)?, self.definition_type_v1(rhs)?)
        else {
            return Ok(());
        };
        let valid = verification_types_equal_v1(lhs_ty, rhs_ty, self.budget)?
            && lhs_ty.as_scalar().is_some_and(ScalarType::is_integer);
        if !valid {
            self.emit_two_types_v1(
                location,
                DiagnosticCode::InvalidOperandType,
                lhs_ty,
                rhs_ty,
                format_args!("checked {operator:?} does not accept {lhs_ty:?} and {rhs_ty:?}"),
            )?;
        }
        self.expect_result_pair_v1(operation, lhs_ty, &Type::BOOL, location)
    }

    fn verify_compare_v1(
        &mut self,
        operation: &Operation,
        predicate: ComparePredicate,
        lhs: ValueId,
        rhs: ValueId,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let (Some(lhs_ty), Some(rhs_ty)) =
            (self.definition_type_v1(lhs)?, self.definition_type_v1(rhs)?)
        else {
            return Ok(());
        };
        let comparable = verification_types_equal_v1(lhs_ty, rhs_ty, self.budget)?
            && lhs_ty.as_scalar().is_some_and(|scalar| {
                scalar.is_numeric()
                    || (scalar == ScalarType::Bool
                        && matches!(
                            predicate,
                            ComparePredicate::Equal | ComparePredicate::NotEqual
                        ))
            });
        if !comparable {
            self.emit_two_types_v1(
                location,
                DiagnosticCode::InvalidOperandType,
                lhs_ty,
                rhs_ty,
                format_args!("comparison does not accept {lhs_ty:?} and {rhs_ty:?}"),
            )?;
        }
        self.expect_single_result_type_v1(operation, &Type::BOOL, location)
    }

    pub(crate) fn expect_results_v1(
        &mut self,
        operation: &Operation,
        expected: &[Type],
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        if operation.results.len() != expected.len() {
            self.emit_dynamic(
                location,
                DiagnosticCode::ResultArity,
                256,
                format_args!(
                    "operation defines {} results but {} are required",
                    operation.results.len(),
                    expected.len()
                ),
            )?;
        }
        self.budget
            .charge_work(operation.results.len().min(expected.len()))?;
        for (result, expected_ty) in operation.results.iter().zip(expected) {
            if !verification_types_equal_v1(&result.ty, expected_ty, self.budget)? {
                self.emit_result_type_mismatch_v1(result, expected_ty, location)?;
            }
        }
        Ok(())
    }

    pub(crate) fn expect_no_results_v1(
        &mut self,
        operation: &Operation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.expect_result_count_v1(operation, 0, location)
    }

    pub(crate) fn expect_single_result_type_v1(
        &mut self,
        operation: &Operation,
        expected: &Type,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(5)?;
        self.expect_result_count_v1(operation, 1, location)?;
        if let Some(result) = operation.results.first()
            && !verification_types_equal_v1(&result.ty, expected, self.budget)?
        {
            self.emit_result_type_mismatch_v1(result, expected, location)?;
        }
        Ok(())
    }

    pub(crate) fn expect_result_pair_v1(
        &mut self,
        operation: &Operation,
        first: &Type,
        second: &Type,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        if operation.results.len() != 2 {
            self.emit_dynamic(
                location,
                DiagnosticCode::ResultArity,
                256,
                format_args!(
                    "operation defines {} results but 2 are required",
                    operation.results.len()
                ),
            )?;
        }
        for (result, expected) in operation.results.iter().zip([first, second]) {
            if !verification_types_equal_v1(&result.ty, expected, self.budget)? {
                self.emit_result_type_mismatch_v1(result, expected, location)?;
            }
        }
        Ok(())
    }

    fn expect_result_count_v1(
        &mut self,
        operation: &Operation,
        expected_count: usize,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(1)?;
        if operation.results.len() != expected_count {
            self.emit_dynamic(
                location,
                DiagnosticCode::ResultArity,
                256,
                format_args!(
                    "operation defines {} results but {} are required",
                    operation.results.len(),
                    expected_count
                ),
            )?;
        }
        Ok(())
    }

    pub(crate) fn emit_result_type_mismatch_v1(
        &mut self,
        result: &crate::ValueDef,
        expected: &Type,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let actual_work = verification_type_message_work_upper_v1(&result.ty, self.budget)?;
        let expected_work = verification_type_message_work_upper_v1(expected, self.budget)?;
        self.emit_dynamic(
            location,
            DiagnosticCode::TypeMismatch,
            actual_work
                .checked_add(expected_work)
                .and_then(|work| work.checked_add(256))
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
            format_args!(
                "result {} has type {:?}, expected {expected:?}",
                result.id, result.ty
            ),
        )
    }

    fn emit_two_types_v1(
        &mut self,
        location: &VerificationDiagnosticLocationV1<'_>,
        code: DiagnosticCode,
        left: &Type,
        right: &Type,
        arguments: std::fmt::Arguments<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let left_work = verification_type_message_work_upper_v1(left, self.budget)?;
        let right_work = verification_type_message_work_upper_v1(right, self.budget)?;
        self.emit_dynamic(
            location,
            code,
            left_work
                .checked_add(right_work)
                .and_then(|work| work.checked_add(384))
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
            arguments,
        )
    }

    pub(crate) fn expect_pointer_result_v1(
        &mut self,
        operation: &Operation,
        pointee: &Type,
        address_space: AddressSpace,
        access: AccessMode,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        if operation.results.len() != 1 {
            self.emit_dynamic(
                location,
                DiagnosticCode::ResultArity,
                256,
                format_args!(
                    "operation defines {} results but 1 are required",
                    operation.results.len()
                ),
            )?;
        }
        let Some(result) = operation.results.first() else {
            return Ok(());
        };
        let matches = match &result.ty {
            Type::Pointer(pointer) => {
                pointer.address_space == address_space
                    && pointer.access == access
                    && verification_types_equal_v1(&pointer.pointee, pointee, self.budget)?
            }
            _ => false,
        };
        if !matches {
            let actual_work = verification_type_message_work_upper_v1(&result.ty, self.budget)?;
            let expected_work = verification_type_message_work_upper_v1(pointee, self.budget)?;
            let expected = ExpectedPointerTypeDebugV1 {
                pointee,
                address_space,
                access,
            };
            // The fixed 384 allowance covers both Debug constructors, pointer
            // metadata, the result ID, and the surrounding diagnostic text.
            self.emit_dynamic(
                location,
                DiagnosticCode::TypeMismatch,
                actual_work
                    .checked_add(expected_work)
                    .and_then(|work| work.checked_add(384))
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
                format_args!(
                    "result {} has type {:?}, expected {expected:?}",
                    result.id, result.ty
                ),
            )?;
        }
        Ok(())
    }

    pub(crate) fn verify_pointer_access_v1(
        &mut self,
        pointer: ValueId,
        access: crate::MemoryAccess,
        write: bool,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<Option<&'module Type>, CanonicalKernelIrVerificationResourceErrorV1> {
        self.verify_alignment_v1(access.alignment, location)?;
        let Some(pointer_ty) = self.definition_type_v1(pointer)? else {
            return Ok(None);
        };
        let Type::Pointer(pointer_ty) = pointer_ty else {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidOperandType,
                128,
                format_args!("memory operand {pointer} does not have pointer type"),
            )?;
            return Ok(None);
        };
        if pointer_ty.address_space != access.address_space {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                256,
                format_args!(
                    "access names {:?} memory but pointer is in {:?} memory",
                    access.address_space, pointer_ty.address_space
                ),
            )?;
        }
        if write
            && (!matches!(
                pointer_ty.access,
                AccessMode::WriteOnly | AccessMode::ReadWrite
            ) || pointer_ty.address_space == AddressSpace::Constant)
        {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "write requires a writable pointer outside constant memory",
            )?;
        }
        if !write && pointer_ty.access == AccessMode::WriteOnly {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "read requires a readable pointer",
            )?;
        }
        if !pointer_ty.pointee.is_storable() {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "memory operation pointee is not storable",
            )?;
        }
        Ok(Some(&pointer_ty.pointee))
    }

    fn verify_vector_access_v1(
        &mut self,
        access: crate::VectorMemoryAccessV12,
        pointer: ValueId,
        write: bool,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        if let Err(error) = access.vector.validate() {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidVectorOperation,
                256,
                format_args!("{error}"),
            )?;
        }
        let Some(pointee) =
            self.verify_pointer_access_v1(pointer, access.memory, write, location)?
        else {
            return Ok(());
        };
        let expected = Type::Scalar(access.vector.element);
        if !verification_types_equal_v1(pointee, &expected, self.budget)? {
            let pointee_work = verification_type_message_work_upper_v1(pointee, self.budget)?;
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidOperandType,
                pointee_work.checked_add(384).ok_or(
                    CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                )?,
                format_args!(
                    "vector access through {pointer} requires scalar pointee {expected:?}, found {pointee:?}"
                ),
            )?;
        }
        Ok(())
    }

    pub(crate) fn verify_alignment_v1(
        &mut self,
        alignment: u32,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(1)?;
        if alignment == 0 || !alignment.is_power_of_two() {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidAlignment,
                128,
                format_args!("alignment {alignment} is not a non-zero power of two"),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "verification_pointer_result_diagnostics_v1_tests.rs"]
mod pointer_result_diagnostics_tests;
