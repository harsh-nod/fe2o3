use crate::{
    AccessMode, AddressSpace, BinaryOp, CanonicalKernelIrVerificationResourceErrorV1, Constant,
    DiagnosticCode, FunctionRole, Gfx950LdsTransposeFormatV1, Gfx950LdsTransposeOperationKindV1,
    Gfx950LdsTransposeOperationV1, Operation, OperationKind, ScalarType, SynchronizationScope,
    Type, ValueId, VerificationDefinitionSiteV1, VerificationDiagnosticLocationV1,
    VerificationFunctionPassV1, WaveOperation, WaveOperationKind, verification_types_equal_v1,
};

impl<'a, 'module, 'work> VerificationFunctionPassV1<'a, 'module, 'work> {
    pub(crate) fn verify_gfx950_lds_transpose_v1(
        &mut self,
        operation: &Operation,
        transpose: &Gfx950LdsTransposeOperationV1,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(2)?;
        if transpose.width != crate::WaveWidth::Wave64 || transpose.active_lanes != 64 {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidGfx950LdsTranspose,
                "gfx950 LDS transpose requires one fully active Wave64",
            )?;
        }
        if transpose.convergence.scope() != SynchronizationScope::Workgroup {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidConvergence,
                "gfx950 LDS transpose requires uniform workgroup convergence",
            )?;
        }
        match transpose.kind {
            Gfx950LdsTransposeOperationKindV1::Current { format } => {
                self.expect_gfx950_storage_result_v1(operation, location)?;
                self.budget.charge_work(self.module.kernels.len())?;
                let mut wave_private_entry = false;
                for kernel in &self.module.kernels {
                    self.budget.charge_work(
                        kernel
                            .entry
                            .as_str()
                            .len()
                            .max(self.function.id.as_str().len())
                            .checked_add(1)
                            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
                    )?;
                    if kernel.entry == self.function.id
                        && kernel.workgroup_size.is_some_and(|workgroup| {
                            workgroup.y == 1
                                && workgroup.z == 1
                                && (64..=256).contains(&workgroup.x)
                                && workgroup.x.is_multiple_of(64)
                        })
                    {
                        wave_private_entry = true;
                        break;
                    }
                }
                if self.function.role != FunctionRole::KernelEntry || !wave_private_entry {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidGfx950LdsTranspose,
                        "gfx950 LDS transpose storage requires a kernel entry with one-dimensional workgroup size [64, 1, 1] through [256, 1, 1] in exact Wave64 multiples",
                    )?;
                }
                let mask = match format {
                    Gfx950LdsTransposeFormatV1::Fp4E2M1 => 1,
                    Gfx950LdsTransposeFormatV1::Fp8E4M3 => 2,
                };
                self.budget.charge_work(1)?;
                if self.gfx950_lds_transpose_current_formats & mask != 0 {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidGfx950LdsTranspose,
                        "one kernel entry may declare at most one gfx950 LDS transpose tile per format",
                    )?;
                }
                self.gfx950_lds_transpose_current_formats |= mask;
            }
            Gfx950LdsTransposeOperationKindV1::Stage {
                format,
                storage,
                source_slice,
                offset,
                rows,
                columns,
                stride,
                token_base,
                reduction_base,
            } => {
                self.expect_gfx950_storage_result_v1(operation, location)?;
                self.expect_gfx950_storage_value_v1(storage, location)?;
                let valid_source = match self.definition_type_v1(source_slice)? {
                    Some(Type::Slice(slice)) => {
                        slice.address_space == AddressSpace::Global
                            && slice.access == AccessMode::ReadOnly
                            && verification_types_equal_v1(
                                &slice.element,
                                &Type::Scalar(ScalarType::U8),
                                self.budget,
                            )?
                    }
                    _ => false,
                };
                if !valid_source {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidOperandType,
                        "gfx950 LDS transpose stage source must be an exact global read-only u8 slice",
                    )?;
                }
                self.budget.charge_work(6)?;
                for value in [offset, rows, columns, stride, token_base, reduction_base] {
                    self.expect_type_v1(value, &Type::INDEX, location)?;
                }
                if !matches!(
                    self.gfx950_lds_transpose_producer_v1(storage)?,
                    Some(Gfx950LdsTransposeOperationKindV1::Current { format: producer })
                        if producer == format
                ) {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidGfx950LdsTranspose,
                        "gfx950 LDS transpose stage must directly consume the matching Current token",
                    )?;
                }
            }
            Gfx950LdsTransposeOperationKindV1::Publish { format, storage } => {
                self.expect_gfx950_storage_result_v1(operation, location)?;
                self.expect_gfx950_storage_value_v1(storage, location)?;
                if !matches!(
                    self.gfx950_lds_transpose_producer_v1(storage)?,
                    Some(Gfx950LdsTransposeOperationKindV1::Stage { format: producer, .. })
                        if producer == format
                ) {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidGfx950LdsTranspose,
                        "gfx950 LDS transpose publish must directly consume the matching staged token",
                    )?;
                }
            }
            Gfx950LdsTransposeOperationKindV1::Read { format, storage } => {
                self.expect_repeated_scalar_results_v1(operation, ScalarType::U32, 8, location)?;
                self.expect_gfx950_storage_value_v1(storage, location)?;
                if !matches!(
                    self.gfx950_lds_transpose_producer_v1(storage)?,
                    Some(Gfx950LdsTransposeOperationKindV1::Publish { format: producer, .. })
                        if producer == format
                ) {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidGfx950LdsTranspose,
                        "gfx950 LDS transpose read must directly consume the matching dominating Publish token",
                    )?;
                }
            }
        }
        Ok(())
    }

    fn expect_gfx950_storage_result_v1(
        &mut self,
        operation: &Operation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.expect_pointer_result_v1(
            operation,
            &Type::Scalar(ScalarType::U8),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
            location,
        )
    }

    fn expect_gfx950_storage_value_v1(
        &mut self,
        value: ValueId,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let valid = match self.definition_type_v1(value)? {
            Some(Type::Pointer(pointer)) => {
                pointer.address_space == AddressSpace::Workgroup
                    && pointer.access == AccessMode::ReadWrite
                    && verification_types_equal_v1(
                        &pointer.pointee,
                        &Type::Scalar(ScalarType::U8),
                        self.budget,
                    )?
            }
            _ => false,
        };
        if !valid {
            self.emit_fixed(
                location,
                DiagnosticCode::TypeMismatch,
                "gfx950 LDS transpose storage must be a workgroup read-write u8 pointer",
            )?;
        }
        Ok(())
    }

    fn gfx950_lds_transpose_producer_v1(
        &mut self,
        value: ValueId,
    ) -> Result<
        Option<Gfx950LdsTransposeOperationKindV1>,
        CanonicalKernelIrVerificationResourceErrorV1,
    > {
        let Some(definition) = self.function_state.definition(value, self.budget)? else {
            return Ok(None);
        };
        let VerificationDefinitionSiteV1::Operation(block, operation_index) = definition.site
        else {
            return Ok(None);
        };
        let Some(block) = self.function_state.block(block, self.budget)? else {
            return Ok(None);
        };
        self.budget.charge_work(1)?;
        let Some(operation) = block.operations.get(operation_index) else {
            return Ok(None);
        };
        let OperationKind::Gfx950LdsTranspose(transpose) = &operation.kind else {
            return Ok(None);
        };
        self.budget.charge_work(
            operation
                .results
                .len()
                .checked_add(1)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
        )?;
        Ok(operation
            .results
            .iter()
            .any(|result| result.id == value)
            .then_some(transpose.kind))
    }

    pub(crate) fn verify_wave_v1(
        &mut self,
        operation: &Operation,
        wave: &WaveOperation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(2)?;
        if wave.active_lanes != wave.width.lanes() {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidWaveOperation,
                192,
                format_args!(
                    "the first wave-operation subset requires all {} lanes active, found {}",
                    wave.width.lanes(),
                    wave.active_lanes
                ),
            )?;
        }
        if wave.convergence.scope() != SynchronizationScope::Subgroup {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidConvergence,
                "wave operation requires a uniform subgroup convergence claim",
            )?;
        }
        match wave.kind {
            WaveOperationKind::LaneId => {
                self.expect_repeated_scalar_results_v1(operation, ScalarType::U32, 1, location)?
            }
            WaveOperationKind::Ballot { predicate } => {
                self.expect_type_v1(predicate, &Type::BOOL, location)?;
                let result = match wave.width {
                    crate::WaveWidth::Wave32 => ScalarType::U32,
                    crate::WaveWidth::Wave64 => ScalarType::U64,
                };
                self.expect_repeated_scalar_results_v1(operation, result, 1, location)?;
            }
            WaveOperationKind::Any { predicate } | WaveOperationKind::All { predicate } => {
                self.expect_type_v1(predicate, &Type::BOOL, location)?;
                self.expect_repeated_scalar_results_v1(operation, ScalarType::Bool, 1, location)?;
            }
            WaveOperationKind::ShuffleIndex {
                value,
                source_lane,
                tile_width,
            } => {
                let value_ty = self.definition_type_v1(value)?;
                if !matches!(
                    value_ty,
                    Some(Type::Scalar(ScalarType::I32 | ScalarType::U32))
                ) {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidOperandType,
                        "wave shuffle supports only i32 and u32 values",
                    )?;
                }
                self.expect_type_v1(source_lane, &Type::Scalar(ScalarType::U32), location)?;
                self.verify_wave_tile_width_v1(wave, tile_width, "shuffle", location)?;
                if let Some(value_ty) = value_ty {
                    self.expect_single_result_type_v1(operation, value_ty, location)?;
                }
            }
            WaveOperationKind::ReduceF32 {
                value, tile_width, ..
            } => {
                self.expect_type_v1(value, &Type::F32, location)?;
                self.verify_wave_tile_width_v1(wave, tile_width, "wave", location)?;
                self.expect_repeated_scalar_results_v1(operation, ScalarType::F32, 1, location)?;
            }
            WaveOperationKind::BroadcastF32 {
                value,
                source_lane,
                tile_width,
            } => {
                self.expect_type_v1(value, &Type::F32, location)?;
                self.expect_type_v1(source_lane, &Type::Scalar(ScalarType::U32), location)?;
                self.verify_wave_tile_width_v1(wave, tile_width, "wave", location)?;
                if !self.bounded_u32_source_lane_v1(source_lane, tile_width)? {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidWaveOperation,
                        "wave f32 broadcast requires a statically bounded tile-local source lane",
                    )?;
                }
                self.expect_repeated_scalar_results_v1(operation, ScalarType::F32, 1, location)?;
            }
        }
        Ok(())
    }

    fn verify_wave_tile_width_v1(
        &mut self,
        wave: &WaveOperation,
        tile_width: u32,
        operation_name: &'static str,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(1)?;
        if tile_width == 0 || !tile_width.is_power_of_two() || tile_width > wave.width.lanes() {
            self.emit_dynamic(
                location,
                DiagnosticCode::InvalidWaveOperation,
                192,
                format_args!(
                    "{operation_name} tile width {tile_width} must be a non-zero power of two no larger than {}",
                    wave.width.lanes()
                ),
            )?;
        }
        Ok(())
    }

    fn bounded_u32_source_lane_v1(
        &mut self,
        value: ValueId,
        width: u32,
    ) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(1)?;
        if width == 0 || !width.is_power_of_two() || width > 64 {
            return Ok(false);
        }
        if self
            .constant_u32_v1(value)?
            .is_some_and(|lane| lane < width)
        {
            return Ok(true);
        }
        let Some(operation) = self.defining_operation_v1(value)? else {
            return Ok(false);
        };
        let OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs,
            rhs,
        } = &operation.kind
        else {
            return Ok(false);
        };
        Ok(self.constant_u32_v1(*lhs)?.is_some_and(|mask| mask < width)
            || self.constant_u32_v1(*rhs)?.is_some_and(|mask| mask < width))
    }

    fn constant_u32_v1(
        &mut self,
        value: ValueId,
    ) -> Result<Option<u32>, CanonicalKernelIrVerificationResourceErrorV1> {
        Ok(self
            .defining_operation_v1(value)?
            .and_then(|operation| match operation.kind {
                OperationKind::Constant(Constant::U32(value)) => Some(value),
                _ => None,
            }))
    }

    fn defining_operation_v1(
        &mut self,
        value: ValueId,
    ) -> Result<Option<&'module Operation>, CanonicalKernelIrVerificationResourceErrorV1> {
        let Some(definition) = self.function_state.definition(value, self.budget)? else {
            return Ok(None);
        };
        let VerificationDefinitionSiteV1::Operation(block, operation_index) = definition.site
        else {
            return Ok(None);
        };
        let Some(block) = self.function_state.block(block, self.budget)? else {
            return Ok(None);
        };
        self.budget.charge_work(1)?;
        Ok(block.operations.get(operation_index))
    }

    fn expect_repeated_scalar_results_v1(
        &mut self,
        operation: &Operation,
        scalar: ScalarType,
        count: usize,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.budget.charge_work(1)?;
        if operation.results.len() != count {
            self.emit_dynamic(
                location,
                DiagnosticCode::ResultArity,
                256,
                format_args!(
                    "operation defines {} results but {count} are required",
                    operation.results.len()
                ),
            )?;
        }
        let expected = Type::Scalar(scalar);
        self.budget
            .charge_work(operation.results.len().min(count))?;
        for result in operation.results.iter().take(count) {
            if !verification_types_equal_v1(&result.ty, &expected, self.budget)? {
                self.emit_result_type_mismatch_v1(result, &expected, location)?;
            }
        }
        Ok(())
    }
}
