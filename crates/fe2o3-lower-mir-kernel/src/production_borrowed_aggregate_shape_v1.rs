// Representation selection only. Source use/lifetime replay is a separate gate.
// Paths start at the referent, not the reference local; they never denote copies.
#[derive(Clone, Debug, Eq, PartialEq)]
enum BorrowedAggregateLeafTransportV1 {
    ScalarSlot { value_type: Type, alignment: u32 },
    InvariantReference { kernel_type: Type },
    InvariantSlice { kernel_type: Type },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BorrowedAggregateLeafV1 {
    path: BorrowedAggregateFieldPathV1,
    semantic_type: SemanticTypeIdV1,
    transport: BorrowedAggregateLeafTransportV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BorrowedAggregateShapeV1 {
    reference_type: SemanticTypeIdV1,
    aggregate_type: SemanticTypeIdV1,
    access: AccessMode,
    leaves: Vec<BorrowedAggregateLeafV1>,
}

fn borrowed_aggregate_error_v1(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}

impl BorrowedAggregateLeafV1 {
    fn parameter_type(
        &self,
        access: AccessMode,
        budget: &mut dyn BorrowedAggregateBudgetV1,
    ) -> Result<Type, ProductionSemanticKirErrorV1> {
        budget.charge_work(5)?;
        budget.reserve_storage(std::mem::size_of::<Type>())?;
        Ok(match &self.transport {
            BorrowedAggregateLeafTransportV1::ScalarSlot { value_type, .. } => {
                Type::pointer(value_type.clone(), AddressSpace::Private, access)
            }
            BorrowedAggregateLeafTransportV1::InvariantReference { kernel_type } => {
                let Type::Pointer(pointer) = kernel_type else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                Type::pointer(
                    pointer.pointee.as_ref().clone(),
                    pointer.address_space,
                    if access == AccessMode::ReadOnly {
                        AccessMode::ReadOnly
                    } else {
                        pointer.access
                    },
                )
            }
            BorrowedAggregateLeafTransportV1::InvariantSlice { kernel_type } => kernel_type.clone(),
        })
    }
}

impl BorrowedAggregateShapeV1 {
    fn clone_in(
        &self,
        budget: &mut dyn BorrowedAggregateBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        borrowed_aggregate_build_v1(budget, |budget| {
            budget.charge_work(4)?;
            let mut leaves = borrowed_aggregate_vec_v1(self.leaves.len(), budget)?;
            for leaf in &self.leaves {
                let path = leaf.path.clone_in(budget)?;
                budget.charge_work(4)?;
                if !matches!(
                    leaf.transport,
                    BorrowedAggregateLeafTransportV1::ScalarSlot { .. }
                ) {
                    budget.reserve_storage(std::mem::size_of::<Type>())?;
                }
                leaves.push(BorrowedAggregateLeafV1 {
                    path,
                    semantic_type: leaf.semantic_type,
                    transport: leaf.transport.clone(),
                });
            }
            Ok(Self {
                reference_type: self.reference_type,
                aggregate_type: self.aggregate_type,
                access: self.access,
                leaves,
            })
        })
    }

    fn leaf_index(
        &self,
        path: &[u32],
        budget: &mut dyn BorrowedAggregateBudgetV1,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        for (index, leaf) in self.leaves.iter().enumerate() {
            budget.charge_work(argument_sum_v1(&[path.len(), leaf.path.fields.len(), 2])?)?;
            if leaf.path.fields.as_ref() == path {
                return Ok(index);
            }
        }
        Err(borrowed_aggregate_error_v1(
            "borrowed aggregate place is not an exact represented field",
        ))
    }

    fn same_fields(
        &self,
        other: &Self,
        budget: &mut dyn BorrowedAggregateBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if self.aggregate_type != other.aggregate_type || self.leaves.len() != other.leaves.len() {
            return Ok(false);
        }
        for (left, right) in self.leaves.iter().zip(&other.leaves) {
            budget.charge_work(argument_sum_v1(&[
                left.path.fields.len(),
                right.path.fields.len(),
                16,
            ])?)?;
            if left != right {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn borrowed_aggregate_helper_parameter_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<Option<BorrowedAggregateShapeV1>, ProductionSemanticKirErrorV1> {
    budget.charge_work(32)?;
    let argument = mapped.abi();
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(argument.ty().index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(None);
    };
    if !matches!(
        types
            .get(pointer.pointee().index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Tuple(_) | SemanticTypeShapeV1::Aggregate(_))
    ) {
        return Ok(None);
    }
    let abi = function.abi();
    if mapped.tuple_field().is_some()
        || mapped.local_field().is_some()
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || !matches!(
            abi.extern_abi(),
            fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::Rust
                | fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::RustCall
        )
        || abi.can_unwind()
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || abi
            .source_input_types()
            .get(mapped.source_argument() as usize)
            != Some(&argument.ty())
        || argument.value().adjusted().is_some()
        || argument.value().pointee_override().is_some()
        || !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
        || argument.role() != SemanticAbiArgumentRoleV1::Source
        || !matches!(
            (pointer.mutability(), mapped.source_ownership()),
            (
                SemanticMutabilityV1::Immutable,
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            ) | (
                SemanticMutabilityV1::Mutable,
                SemanticSourceArgumentOwnershipV1::UniqueBorrow
            )
        )
    {
        return Err(unsupported(
            function_id.index(),
            None,
            None,
            "borrowed aggregate helper requires its exact source reference ABI",
        ));
    }
    borrowed_aggregate_shape_v1(types, argument.ty(), budget).map(Some)
}

fn borrowed_aggregate_shape_v1(
    types: &[SemanticTypeDeclV1],
    reference_type: SemanticTypeIdV1,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<BorrowedAggregateShapeV1, ProductionSemanticKirErrorV1> {
    borrowed_aggregate_build_v1(budget, |budget| {
        budget.charge_work(24)?;
        let declaration = types.get(reference_type.index() as usize).ok_or_else(|| {
            borrowed_aggregate_error_v1("borrowed aggregate reference type is missing")
        })?;
        let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
            return Err(borrowed_aggregate_error_v1(
                "borrowed aggregate requires a reference",
            ));
        };
        if pointer.kind() != SemanticPointerKindV1::Reference
            || !matches!(pointer.address_space(), 0 | 5)
            || pointer.pointer_width_bits() != 64
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || !borrowed_aggregate_thin_layout_v1(declaration, pointer.address_space())
            || !matches!(
                types
                    .get(pointer.pointee().index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Tuple(_) | SemanticTypeShapeV1::Aggregate(_))
            )
        {
            return Err(borrowed_aggregate_error_v1(
                "borrowed aggregate requires an exact thin reference to a local structural aggregate",
            ));
        }
        let mut shape = BorrowedAggregateShapeV1 {
            reference_type,
            aggregate_type: pointer.pointee(),
            access: match pointer.mutability() {
                SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
                SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
            },
            leaves: Vec::new(),
        };
        let mut path = borrowed_aggregate_vec_v1(MAX_SSA_VALUE_COMPONENTS_V1, budget)?;
        borrowed_aggregate_append_v1(
            types,
            pointer.pointee(),
            &mut path,
            &mut shape.leaves,
            &mut 0,
            budget,
        )?;
        let bytes = argument_product_v1(path.capacity(), std::mem::size_of::<u32>())?;
        drop(path);
        budget.release_storage(bytes)?;
        if shape.leaves.is_empty() {
            return Err(borrowed_aggregate_error_v1(
                "borrowed aggregate without represented fields is unsupported",
            ));
        }
        Ok(shape)
    })
}

fn borrowed_aggregate_thin_layout_v1(declaration: &SemanticTypeDeclV1, space: u32) -> bool {
    !declaration.layout().is_uninhabited()
        && declaration.layout().size_bytes() == Some(8)
        && declaration.layout().alignment_bytes() == 8
        && matches!(scalar_backend_pointer(declaration), Some(SemanticBackendPrimitiveV1::Pointer { address_space, size_bytes: 8, alignment_bytes: 8 }) if address_space == space)
}

fn borrowed_aggregate_append_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    path: &mut Vec<u32>,
    leaves: &mut Vec<BorrowedAggregateLeafV1>,
    nodes: &mut usize,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    *nodes = nodes.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
    if *nodes > MAX_SSA_VALUE_COMPONENTS_V1 || path.len() >= MAX_SSA_VALUE_COMPONENTS_V1 {
        return Err(borrowed_aggregate_error_v1(
            "borrowed aggregate exceeds its structural component limit",
        ));
    }
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(|| borrowed_aggregate_error_v1("borrowed aggregate field type is missing"))?;
    let layout = declaration.layout();
    if layout.is_uninhabited() || layout.size_bytes().is_none() {
        return Err(borrowed_aggregate_error_v1(
            "borrowed aggregate field is unsized or uninhabited",
        ));
    }
    let transport = match declaration.shape() {
        SemanticTypeShapeV1::Unit if layout.size_bytes() == Some(0) => return Ok(()),
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
            let facts =
                private_retained_slot_facts_v1(types, ty, &mut BorrowedAggregateWorkV1(budget))?
                    .ok_or_else(|| {
                        borrowed_aggregate_error_v1(
                            "borrowed scalar field has no exact private slot layout",
                        )
                    })?;
            let PrivateRetainedElementFactsV1::Scalar(scalar) = facts.element else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            BorrowedAggregateLeafTransportV1::ScalarSlot {
                value_type: Type::Scalar(scalar),
                alignment: facts.alignment,
            }
        }
        SemanticTypeShapeV1::Pointer(_) if shared_slice_leaf_v1(types, ty) => {
            budget.charge_work(32)?;
            budget.reserve_storage(std::mem::size_of::<Type>())?;
            BorrowedAggregateLeafTransportV1::InvariantSlice {
                kernel_type: lower_parameter_type(types, &[], ty)?,
            }
        }
        SemanticTypeShapeV1::Pointer(pointer) => {
            budget.charge_work(32)?;
            if pointer.kind() != SemanticPointerKindV1::Reference
                || pointer.metadata() != SemanticPointerMetadataV1::None
                || pointer.pointer_width_bits() != 64
                || !borrowed_aggregate_thin_layout_v1(declaration, pointer.address_space())
            {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate captured pointer is not an exact scalar reference",
                ));
            }
            let value_type = lower_parameter_scalar_v1(types, pointer.pointee())?;
            // Generic source references are admitted here only with a Private actual.
            let space = if pointer.address_space() == 0 {
                AddressSpace::Private
            } else {
                lower_address_space(pointer.address_space())?
            };
            let access = match pointer.mutability() {
                SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
                SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
            };
            if space == AddressSpace::Constant && access != AccessMode::ReadOnly {
                return Err(borrowed_aggregate_error_v1(
                    "captured constant-space reference cannot be mutable",
                ));
            }
            budget.reserve_storage(std::mem::size_of::<Type>())?;
            BorrowedAggregateLeafTransportV1::InvariantReference {
                kernel_type: Type::pointer(value_type, space, access),
            }
        }
        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
            let SemanticTypeLayoutDetailsV1::Aggregate(details) = layout.details() else {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate lacks field layout evidence",
                ));
            };
            if fields.fields().len() != details.field_offsets().len()
                || fields.fields().len() > MAX_SSA_VALUE_COMPONENTS_V1
            {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate field layout or component count is unsupported",
                ));
            }
            for (index, field) in fields.fields().iter().copied().enumerate() {
                budget.charge_work(12)?;
                let field_layout = types
                    .get(field.index() as usize)
                    .ok_or_else(|| {
                        borrowed_aggregate_error_v1("borrowed aggregate field type is missing")
                    })?
                    .layout();
                let size = field_layout.size_bytes().ok_or_else(|| {
                    borrowed_aggregate_error_v1("borrowed aggregate field is unsized")
                })?;
                let offset = details.field_offsets()[index];
                if offset
                    .checked_add(size)
                    .is_none_or(|end| Some(end) > layout.size_bytes())
                    || !offset.is_multiple_of(field_layout.alignment_bytes())
                {
                    return Err(borrowed_aggregate_error_v1(
                        "borrowed aggregate field is out of bounds or unaligned",
                    ));
                }
                for previous in 0..index {
                    budget.charge_work(12)?;
                    let start = details.field_offsets()[previous];
                    let previous_size = types
                        .get(fields.fields()[previous].index() as usize)
                        .and_then(|t| t.layout().size_bytes())
                        .ok_or_else(|| {
                            borrowed_aggregate_error_v1(
                                "borrowed aggregate field layout is missing",
                            )
                        })?;
                    let end = start
                        .checked_add(previous_size)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    let field_end = offset
                        .checked_add(size)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    if size != 0 && previous_size != 0 && offset < end && start < field_end {
                        return Err(borrowed_aggregate_error_v1(
                            "borrowed aggregate fields overlap",
                        ));
                    }
                }
                path.push(index as u32);
                borrowed_aggregate_append_v1(types, field, path, leaves, nodes, budget)?;
                path.pop();
            }
            return Ok(());
        }
        _ => {
            return Err(borrowed_aggregate_error_v1(
                "borrowed aggregate supports only structural fields, scalars, invariant scalar references and shared slices",
            ));
        }
    };
    let retained_path = BorrowedAggregateFieldPathV1::new_in(path, budget)?;
    borrowed_aggregate_push_v1(
        leaves,
        BorrowedAggregateLeafV1 {
            path: retained_path,
            semantic_type: ty,
            transport,
        },
        budget,
    )?;
    Ok(())
}

struct BorrowedAggregateWorkV1<'b>(&'b mut dyn BorrowedAggregateBudgetV1);

impl PrivateArrayChargeV1 for BorrowedAggregateWorkV1<'_> {
    type Error = ProductionSemanticKirErrorV1;
    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.charge_work(amount).map_err(Into::into)
    }
}

// Success transfers charged storage to the enclosing lowering ledger. The root
// drops that scratch after emission; failures/unwind restore the incoming floor.
fn borrowed_aggregate_build_v1<T>(
    budget: &mut dyn BorrowedAggregateBudgetV1,
    build: impl FnOnce(&mut dyn BorrowedAggregateBudgetV1) -> Result<T, ProductionSemanticKirErrorV1>,
) -> Result<T, ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build(budget)));
    match result {
        Ok(Ok(value)) => Ok(value),
        other => {
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
            match other {
                Ok(Err(error)) => Err(error),
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(Ok(_)) => unreachable!(),
            }
        }
    }
}

fn borrowed_aggregate_push_v1<T>(
    rows: &mut Vec<T>,
    row: T,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    if rows.len() == rows.capacity() {
        let next = argument_product_v1(rows.capacity().max(2), 2)?;
        let mut replacement = borrowed_aggregate_vec_v1(next, budget)?;
        budget.charge_work(rows.len())?;
        let old_bytes = argument_product_v1(rows.capacity(), std::mem::size_of::<T>())?;
        replacement.append(rows);
        *rows = replacement;
        budget.release_storage(old_bytes)?;
    }
    rows.push(row);
    Ok(())
}
