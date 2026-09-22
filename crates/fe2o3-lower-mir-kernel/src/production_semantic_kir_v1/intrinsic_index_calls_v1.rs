// Non-inlined index call bodies keep unrelated intrinsic temporaries off the dispatcher stack.
impl SemanticFunctionLoweringV1<'_> {
    #[inline(never)]
    fn lower_intrinsic_thread_index_1d_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            if !call.arguments().is_empty() {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "thread index intrinsic has arguments",
                ));
            }
            let (id, _) = self
                .emit(
                    operations,
                    Type::INDEX,
                    OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                )?
                .value()
                .expect("emitted index value");
            SemanticValueBindingV1::IndexWitness {
                id,
                index_space: SemanticDisjointIndexSpaceV1::Index1d,
                disjoint: false,
                availability: None,
            }
        })
    }

    #[inline(never)]
    fn lower_intrinsic_thread_index_into_disjoint_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        index_space: &SemanticDisjointIndexSpaceV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 1)?;
            let binding = self.lower_operand(block, None, &call.arguments()[0], operations)?;
            let SemanticValueBindingV1::IndexWitness {
                id,
                availability,
                index_space: actual,
                disjoint: false,
            } = binding
            else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "into_disjoint receiver is not a thread-index witness",
                ));
            };
            if actual != *index_space {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "into_disjoint mapping identity changed",
                ));
            }
            SemanticValueBindingV1::IndexWitness {
                availability,
                id,
                index_space: actual,
                disjoint: true,
            }
        })
    }

    #[inline(never)]
    fn lower_intrinsic_disjoint_index_get_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        index_space: &SemanticDisjointIndexSpaceV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 1)?;
            let binding = self.lower_operand(block, None, &call.arguments()[0], operations)?;
            let SemanticValueBindingV1::IndexWitness {
                id,
                index_space: actual,
                disjoint: true,
                ..
            } = binding
            else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "DisjointIndex::get receiver is not disjoint authority",
                ));
            };
            if actual != *index_space {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "DisjointIndex::get mapping identity changed",
                ));
            }
            SemanticValueBindingV1::Value {
                id,
                ty: Type::INDEX,
            }
        })
    }

    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn lower_intrinsic_disjoint_block_component_index_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        destination: &fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1,
        index_space: &SemanticDisjointIndexSpaceV1,
        lanes_per_block: &u64,
        elements_per_lane: &u64,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 2)?;
            let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                lanes_per_block: *lanes_per_block,
                elements_per_lane: *elements_per_lane,
            };
            if *index_space != expected {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "DisjointBlock::component_index mapping identity changed",
                ));
            }
            let witness = self.lower_operand(block, None, &call.arguments()[0], operations)?;
            let raw = require_block_component_witness_v1(block, witness, expected)?;
            let component = self.lower_operand(block, None, &call.arguments()[1], operations)?;
            let component = self.coerce_index(block, operations, component)?;
            let (component, _) = component
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            let (index, present) = self.lower_block_component_index(
                block,
                operations,
                raw,
                component,
                *lanes_per_block,
                *elements_per_lane,
            )?;
            let result_type = destination.place().ty();
            let (discriminant, variants) = semantic_enum_shape(self.types, result_type)?;
            let [none, some] = variants else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "component-index result is not an exact two-variant Option",
                ));
            };
            if none.discriminant() != 0
                || !none.fields().fields().is_empty()
                || some.discriminant() != 1
                || some.fields().fields() != [call.arguments()[1].ty()]
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "component-index Option layout changed",
                ));
            }
            let discriminant_ty = lower_scalar_type(self.types, discriminant)?;
            let none_discriminant = self
                .emit(
                    operations,
                    discriminant_ty.clone(),
                    OperationKind::Constant(integer_constant(&discriminant_ty, 0)?),
                )?
                .value()
                .expect("emitted None discriminant")
                .0;
            let some_discriminant = self
                .emit(
                    operations,
                    discriminant_ty.clone(),
                    OperationKind::Constant(integer_constant(&discriminant_ty, 1)?),
                )?
                .value()
                .expect("emitted Some discriminant")
                .0;
            let discriminant_value = self
                .emit(
                    operations,
                    discriminant_ty.clone(),
                    OperationKind::Select {
                        condition: present,
                        true_value: some_discriminant,
                        false_value: none_discriminant,
                    },
                )?
                .value()
                .expect("emitted component-index discriminant")
                .0;
            ordinary_option_index_binding_v1(
                result_type,
                discriminant_value,
                discriminant_ty,
                index,
            )
        })
    }

    #[inline(never)]
    fn lower_intrinsic_disjoint_slice_len_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 1)?;
            let (slice, slice_ty) = self
                .lower_operand(block, None, &call.arguments()[0], operations)?
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            if !matches!(slice_ty, Type::Slice(_)) {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "DisjointSlice::len receiver is not a lowered slice",
                ));
            }
            let (id, _) = self
                .emit(
                    operations,
                    Type::INDEX,
                    OperationKind::SliceLength { slice },
                )?
                .value()
                .expect("emitted slice length");
            SemanticValueBindingV1::Value {
                id,
                ty: Type::INDEX,
            }
        })
    }

    #[inline(never)]
    fn lower_intrinsic_disjoint_slice_get_mut_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 2)?;
            let index_binding =
                self.lower_operand(block, None, &call.arguments()[1], operations)?;
            if !matches!(
                index_binding,
                SemanticValueBindingV1::IndexWitness {
                    index_space: SemanticDisjointIndexSpaceV1::Index1d,
                    disjoint: false,
                    ..
                }
            ) {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "DisjointSlice::get_mut requires the identity thread-index witness",
                ));
            }
            self.lower_checked_slice_access(block, call, operations, 0, index_binding, None)?
        })
    }

    #[inline(never)]
    fn lower_intrinsic_disjoint_slice_get_disjoint_mut_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        index_space: &SemanticDisjointIndexSpaceV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 2)?;
            let index_binding =
                self.lower_operand(block, None, &call.arguments()[1], operations)?;
            if !matches!(index_binding, SemanticValueBindingV1::IndexWitness {
                index_space: actual,
                disjoint: true,
                ..
            } if actual == *index_space)
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "get_disjoint_mut mapping authority does not match the slice",
                ));
            }
            self.lower_checked_slice_access(block, call, operations, 0, index_binding, None)?
        })
    }

    #[inline(never)]
    fn lower_intrinsic_grid_leader_current_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        destination: &fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 0)?;
            let (index, _) = self
                .emit(
                    operations,
                    Type::INDEX,
                    OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                )?
                .value()
                .expect("emitted index value");
            let (one, _) = self
                .emit(
                    operations,
                    Type::INDEX,
                    OperationKind::Constant(Constant::Index(1)),
                )?
                .value()
                .expect("emitted index constant");
            let (present, _) = self
                .emit(
                    operations,
                    Type::BOOL,
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: index,
                        rhs: one,
                    },
                )?
                .value()
                .expect("emitted leader predicate");
            let availability = self
                .option_dominance
                .availability(destination.place().local())
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "grid-leader Option lacks an authenticated Some edge",
                    )
                })?;
            SemanticValueBindingV1::OptionGridLeader {
                present,
                availability,
            }
        })
    }

    #[inline(never)]
    fn lower_intrinsic_disjoint_slice_get_mut_exclusive_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 3)?;
            let leader = self.lower_operand(block, None, &call.arguments()[1], operations)?;
            if !matches!(leader, SemanticValueBindingV1::GridLeader { .. }) {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "exclusive access lacks grid-leader authority",
                ));
            }
            let index = self.lower_operand(block, None, &call.arguments()[2], operations)?;
            let index = self.coerce_index(block, operations, index)?;
            self.lower_checked_slice_access(block, call, operations, 0, index, None)?
        })
    }

    #[inline(never)]
    fn lower_intrinsic_disjoint_slice_get_block_mut_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        index_space: &SemanticDisjointIndexSpaceV1,
        lanes_per_block: &u64,
        elements_per_lane: &u64,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 3)?;
            let witness = self.lower_operand(block, None, &call.arguments()[1], operations)?;
            let SemanticValueBindingV1::ComponentWitness {
                raw,
                index_space: actual,
                ..
            } = witness
            else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "get_block_mut lacks blocked ownership authority",
                ));
            };
            let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                lanes_per_block: *lanes_per_block,
                elements_per_lane: *elements_per_lane,
            };
            if actual != expected || *index_space != expected {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "get_block_mut mapping identity changed",
                ));
            }
            let component = self.lower_operand(block, None, &call.arguments()[2], operations)?;
            let component = self.coerce_index(block, operations, component)?;
            let (component, _) = component
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            let (index, present) = self.lower_block_component_index(
                block,
                operations,
                raw,
                component,
                *lanes_per_block,
                *elements_per_lane,
            )?;
            self.lower_checked_slice_access(
                block,
                call,
                operations,
                0,
                SemanticValueBindingV1::Value {
                    id: index,
                    ty: Type::INDEX,
                },
                Some(present),
            )?
        })
    }

    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn lower_intrinsic_disjoint_slice_get_tiled_2d_mut_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        index_space: &SemanticDisjointIndexSpaceV1,
        lanes_per_tile: &u64,
        tile_rows: &u64,
        tile_columns: &u64,
        elements_per_lane: &u64,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 6)?;
            let witness = self.lower_operand(block, None, &call.arguments()[1], operations)?;
            let SemanticValueBindingV1::ComponentWitness {
                raw,
                index_space: actual,
                ..
            } = witness
            else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "get_tiled_2d_mut lacks tiled ownership authority",
                ));
            };
            let expected = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                lanes_per_tile: *lanes_per_tile,
                tile_rows: *tile_rows,
                tile_columns: *tile_columns,
                elements_per_lane: *elements_per_lane,
            };
            if actual != expected || *index_space != expected {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "get_tiled_2d_mut mapping identity changed",
                ));
            }
            let mut indices = Vec::with_capacity(4);
            for argument in &call.arguments()[2..6] {
                let value = self.lower_operand(block, None, argument, operations)?;
                let value = self.coerce_index(block, operations, value)?;
                indices.push(
                    value
                        .value()
                        .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?
                        .0,
                );
            }
            let [component, rows, columns, row_stride] = indices
                .try_into()
                .expect("four checked tiled-2d index operands");
            let (index, present) = self.lower_tiled_2d_component_index(
                block,
                operations,
                raw,
                component,
                rows,
                columns,
                row_stride,
                *lanes_per_tile,
                *tile_rows,
                *tile_columns,
                *elements_per_lane,
            )?;
            self.lower_checked_slice_access(
                block,
                call,
                operations,
                0,
                SemanticValueBindingV1::Value {
                    id: index,
                    ty: Type::INDEX,
                },
                Some(present),
            )?
        })
    }

    #[inline(never)]
    fn lower_intrinsic_disjoint_slice_get_row_striped_2d_mut_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        index_space: &SemanticDisjointIndexSpaceV1,
        lanes_per_row: &u64,
        elements_per_lane: &u64,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 6)?;
            let witness = self.lower_operand(block, None, &call.arguments()[1], operations)?;
            let SemanticValueBindingV1::ComponentWitness {
                raw,
                index_space: actual,
                ..
            } = witness
            else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "get_row_striped_2d_mut lacks row ownership authority",
                ));
            };
            let expected = SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
                lanes_per_row: *lanes_per_row,
                elements_per_lane: *elements_per_lane,
            };
            if actual != expected || *index_space != expected {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "get_row_striped_2d_mut mapping identity changed",
                ));
            }
            let mut indices = Vec::with_capacity(4);
            for argument in &call.arguments()[2..6] {
                let value = self.lower_operand(block, None, argument, operations)?;
                let value = self.coerce_index(block, operations, value)?;
                indices.push(
                    value
                        .value()
                        .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?
                        .0,
                );
            }
            let [component, rows, columns, row_stride] = indices.try_into().map_err(|_| {
                unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "row-striped operand count changed",
                )
            })?;
            let (index, present) = self.lower_row_striped_2d_component_index(
                block,
                operations,
                raw,
                component,
                rows,
                columns,
                row_stride,
                *lanes_per_row,
                *elements_per_lane,
            )?;
            self.lower_checked_slice_access(
                block,
                call,
                operations,
                0,
                SemanticValueBindingV1::Value {
                    id: index,
                    ty: Type::INDEX,
                },
                Some(present),
            )?
        })
    }
}
