// Non-inlined memory call bodies keep unrelated intrinsic temporaries off the dispatcher stack.
impl SemanticFunctionLoweringV1<'_> {
    #[inline(never)]
    fn lower_intrinsic_memory_volatile_load_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        destination: &fe2o3_mir_model::semantic_mir_v1::SemanticCallDestinationV1,
        element: &SemanticTypeIdV1,
        runtime_guard: &mut Option<ValueId>,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 2)?;
            if !semantic_volatile_load_contract_v1(
                self.types,
                semantic_operand_type(&call.arguments()[0]),
                semantic_operand_type(&call.arguments()[1]),
                *element,
            ) || destination.place().ty() != *element
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "volatile load semantic slice, index, or result contract changed",
                ));
            }
            let (slice, slice_ty) = self
                .lower_operand(block, None, &call.arguments()[0], operations)?
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            let Type::Slice(slice_contract) = slice_ty else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "volatile load source is not a lowered slice",
                ));
            };
            if slice_contract.address_space != AddressSpace::Global
                || slice_contract.access != AccessMode::ReadOnly
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "volatile load source does not retain immutable global-slice access",
                ));
            }
            let element_ty = lower_scalar_type(self.types, *element)?;
            if *slice_contract.element != element_ty || destination.place().ty() != *element {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "volatile load source or destination element type changed",
                ));
            }
            let index = self.lower_operand(block, None, &call.arguments()[1], operations)?;
            let index = self
                .coerce_index(block, operations, index)?
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?
                .0;
            let length = self.emit_id(
                operations,
                Type::INDEX,
                OperationKind::SliceLength { slice },
            )?;
            let present =
                self.emit_compare(operations, ComparePredicate::LessThan, index, length)?;
            let zero_index = self.emit_index_constant(operations, 0)?;
            let safe_index = self.emit_select_index(operations, present, index, zero_index)?;
            let pointer_ty = Type::pointer(
                element_ty.clone(),
                slice_contract.address_space,
                slice_contract.access,
            );
            let base = self.emit_id(
                operations,
                pointer_ty.clone(),
                OperationKind::SliceData { slice },
            )?;
            let pointer = self.emit_id(
                operations,
                pointer_ty,
                OperationKind::GetElementPointer {
                    base,
                    offset: safe_index,
                },
            )?;
            let fallback = volatile_load_zero_constant_v1(&element_ty).ok_or_else(|| {
                unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "volatile load element has no supported scalar fallback",
                )
            })?;
            let fallback = self.emit_id(
                operations,
                element_ty.clone(),
                OperationKind::Constant(fallback),
            )?;
            let alignment = strided_read_scalar_alignment_v1(&element_ty).ok_or_else(|| {
                unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "volatile load element has no supported scalar alignment",
                )
            })?;
            let mut access = MemoryAccess::new(slice_contract.address_space, alignment);
            access.volatile = true;
            let value = self.emit_id(
                operations,
                element_ty.clone(),
                OperationKind::GuardedLoad {
                    pointer,
                    predicate: present,
                    fallback,
                    access,
                },
            )?;
            *runtime_guard = Some(present);
            SemanticValueBindingV1::Value {
                id: value,
                ty: element_ty,
            }
        })
    }
}
