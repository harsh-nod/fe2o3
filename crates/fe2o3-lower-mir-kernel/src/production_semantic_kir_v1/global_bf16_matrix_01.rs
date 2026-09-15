use fe2o3_mir_model::semantic_mir_v1::{
    SemanticGlobalBf16MatrixLoadV1, semantic_global_bf16_matrix_layout_matches_v1,
};

include!("global_bf16_matrix_01/transport.rs");

mod global_bf16_live_v1 {
    include!("global_bf16_matrix_01/live.rs");
}

pub use global_bf16_live_v1::{
    ProductionGlobalBf16SourceBatchV1, ProductionGlobalBf16SourceOperandV1,
    ProductionGlobalBf16SourceRowV1,
};

fn global_bf16_shared_reference_v1(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    owned: SemanticTypeIdV1,
) -> bool {
    matches!(types.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(pointer))
            if pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.mutability() == SemanticMutabilityV1::Immutable
                && pointer.pointee() == owned && pointer.address_space() == 0
                && pointer.pointer_width_bits() == 64
                && pointer.metadata() == SemanticPointerMetadataV1::None)
}

enum Bf16MatrixStorageV1 {
    Slice {
        value: ValueId,
        slice: fe2o3_kernel_ir::SliceType,
    },
    Global {
        value: ValueId,
    },
}

impl Bf16MatrixStorageV1 {
    fn value(&self) -> ValueId {
        match self {
            Self::Slice { value, .. } | Self::Global { value, .. } => *value,
        }
    }
    fn address_space(&self) -> AddressSpace {
        match self {
            Self::Slice { slice, .. } => slice.address_space,
            Self::Global { .. } => AddressSpace::Global,
        }
    }
    fn access(&self) -> AccessMode {
        match self {
            Self::Slice { slice, .. } => slice.access,
            Self::Global { .. } => AccessMode::ReadOnly,
        }
    }
    fn memory_access(&self) -> MemoryAccess {
        let mut access = MemoryAccess::new(self.address_space(), 2);
        // Global::load is a retained volatile read in the source pipeline.
        access.volatile = matches!(self, Self::Global { .. });
        access
    }
}

impl SemanticFunctionLoweringV1<'_> {
    fn lower_global_bf16_matrix_load_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        contract: SemanticGlobalBf16MatrixLoadV1,
        callable_source_identity: SemanticFunctionIdentityV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.require_call_argument_count(block, call, 4)?;
        let authenticated = self.kernel_context.ok_or_else(|| {
            unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "global BF16 matrix load lacks authenticated root",
            )
        })?;
        let t = contract.types();
        if contract.source_identity() != callable_source_identity
            || call.destination().map(|d| d.place().ty()) != Some(t.fragment)
            || !global_capability_provenance_matches_v1(authenticated, contract.provenance())
            || !semantic_global_bf16_matrix_layout_matches_v1(self.types, t)
            || global_capability_descriptor_v1(self.callables, t.global)
                != Some((t.element, contract.memory(), contract.provenance()))
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let matrix = self.lower_operand(block, None, &call.arguments()[0], operations)?;
        let components =
            global_bf16_view_transport_values_v1(contract, &matrix).map_err(|detail| {
                unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    None,
                    detail,
                )
            })?;
        let (value, Type::GlobalCapability(capability)) = &components[0] else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if capability
            != &lower_global_capability_type_v1(
                self.types,
                t.element,
                contract.memory(),
                &authenticated.context_type,
            )?
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let coordinates: [(ValueId, Type); 4] = components[1..]
            .to_vec()
            .try_into()
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        self.lower_bf16_matrix_load_components_v1(
            block,
            call,
            operations,
            contract.operand(),
            contract.storage_layout(),
            Bf16MatrixStorageV1::Global { value: *value },
            coordinates,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_bf16_matrix_load_components_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        contract: SemanticMfmaOperandContractV1,
        storage_layout: SemanticMfmaStorageLayoutV1,
        storage: Bf16MatrixStorageV1,
        coordinates: [(ValueId, Type); 4],
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let [offset, rows, columns, stride] = &coordinates;
        let offset = self.coerce_typed_index_component(block, operations, offset)?;
        let rows = self.coerce_typed_index_component(block, operations, rows)?;
        let columns = self.coerce_typed_index_component(block, operations, columns)?;
        let stride = self.coerce_typed_index_component(block, operations, stride)?;
        let (lane, wave) = require_current_wave_lane(
            block,
            self.lower_operand(block, None, &call.arguments()[1], operations)?,
            contract.wave_width,
            "typed matrix load lane",
        )?;
        let lane_index = self.emit_id(
            operations,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: lane,
                to: Type::INDEX,
            },
        )?;
        let first_base = self.lower_operand(block, None, &call.arguments()[2], operations)?;
        let first_base = self
            .coerce_index(block, operations, first_base)?
            .value()
            .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?
            .0;
        let second_base = self.lower_operand(block, None, &call.arguments()[3], operations)?;
        let second_base = self
            .coerce_index(block, operations, second_base)?
            .value()
            .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?
            .0;
        let fifteen = self.emit_index_constant(operations, 15)?;
        let sixteen = self.emit_index_constant(operations, 16)?;
        let four = self.emit_index_constant(operations, 4)?;
        let lane_minor =
            self.emit_index_binary(operations, BinaryOp::BitAnd, lane_index, fifteen)?;
        let lane_group =
            self.emit_index_binary(operations, BinaryOp::Divide, lane_index, sixteen)?;
        let lane_group =
            self.emit_index_binary(operations, BinaryOp::Multiply, lane_group, four)?;
        let bases = semantic_mfma_operand_bases_v1(contract.role, first_base, second_base);
        let (row_or_column, minor_safe) = self.emit_checked_index(
            operations,
            CheckedBinaryOperator::Add,
            bases.minor,
            lane_minor,
        )?;
        let (first_reduction, reduction_safe) = self.emit_checked_index(
            operations,
            CheckedBinaryOperator::Add,
            bases.reduction,
            lane_group,
        )?;
        let mut present = self.emit_bool_and(operations, minor_safe, reduction_safe)?;
        let data = self.emit_id(
            operations,
            Type::pointer(
                Type::Scalar(ScalarType::U16),
                storage.address_space(),
                storage.access(),
            ),
            OperationKind::SliceData {
                slice: storage.value(),
            },
        )?;
        let length = self.emit_id(
            operations,
            Type::INDEX,
            OperationKind::SliceLength {
                slice: storage.value(),
            },
        )?;
        let zero_index = self.emit_index_constant(operations, 0)?;
        let zero_bits = self.emit_id(
            operations,
            Type::Scalar(ScalarType::U16),
            OperationKind::Constant(Constant::U16(0)),
        )?;
        let component_count = contract.profile.operand_components_per_lane();
        let mut values = Vec::with_capacity(component_count);
        for component in 0..u64::try_from(component_count).expect("MFMA component count fits u64") {
            let component_value = self.emit_index_constant(operations, component)?;
            let (reduction, component_safe) = self.emit_checked_index(
                operations,
                CheckedBinaryOperator::Add,
                first_reduction,
                component_value,
            )?;
            present = self.emit_bool_and(operations, present, component_safe)?;
            let (row, column) = match contract.role {
                fe2o3_mir_model::semantic_mir_v1::SemanticMfmaOperandRoleV1::A => {
                    (row_or_column, reduction)
                }
                fe2o3_mir_model::semantic_mir_v1::SemanticMfmaOperandRoleV1::B => {
                    (reduction, row_or_column)
                }
            };
            let row_valid = self.emit_compare(operations, ComparePredicate::LessThan, row, rows)?;
            let column_valid =
                self.emit_compare(operations, ComparePredicate::LessThan, column, columns)?;
            let (row_offset, row_safe) =
                self.emit_checked_index(operations, CheckedBinaryOperator::Multiply, row, stride)?;
            let (index, offset_safe) = self.emit_checked_index(
                operations,
                CheckedBinaryOperator::Add,
                offset,
                row_offset,
            )?;
            let (index, column_safe) =
                self.emit_checked_index(operations, CheckedBinaryOperator::Add, index, column)?;
            let index = match &storage {
                Bf16MatrixStorageV1::Global { value, .. } => self.emit_id(
                    operations,
                    Type::INDEX,
                    OperationKind::GlobalCapabilityIndex(
                        fe2o3_kernel_ir::GlobalCapabilityIndexV1 {
                            capability: *value,
                            index,
                            index_space: None,
                        },
                    ),
                )?,
                Bf16MatrixStorageV1::Slice { .. } => index,
            };
            let index_in_bounds =
                self.emit_compare(operations, ComparePredicate::LessThan, index, length)?;
            let mut guard = self.emit_bool_and(operations, present, row_valid)?;
            guard = self.emit_bool_and(operations, guard, column_valid)?;
            guard = self.emit_bool_and(operations, guard, row_safe)?;
            guard = self.emit_bool_and(operations, guard, offset_safe)?;
            guard = self.emit_bool_and(operations, guard, column_safe)?;
            guard = self.emit_bool_and(operations, guard, index_in_bounds)?;
            let safe_index = self.emit_select_index(operations, guard, index, zero_index)?;
            let pointer = self.emit_id(
                operations,
                Type::pointer(
                    Type::Scalar(ScalarType::U16),
                    storage.address_space(),
                    storage.access(),
                ),
                OperationKind::GetElementPointer {
                    base: data,
                    offset: safe_index,
                },
            )?;
            let loaded = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U16),
                OperationKind::GuardedLoad {
                    pointer,
                    predicate: guard,
                    fallback: zero_bits,
                    access: storage.memory_access(),
                },
            )?;
            let value = self.emit_id(
                operations,
                Type::Scalar(ScalarType::Bf16),
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: loaded,
                    to: Type::Scalar(ScalarType::Bf16),
                },
            )?;
            values.push((value, Type::Scalar(ScalarType::Bf16)));
        }
        Ok(SemanticValueBindingV1::MatrixFragment {
            values,
            contract,
            storage_layout,
            wave,
        })
    }
}
