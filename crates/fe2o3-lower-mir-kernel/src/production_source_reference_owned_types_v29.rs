fn source_reference_owned_type_headers_v29() -> Result<usize, ArgumentResourceV1> {
    use std::mem::size_of;
    type Move<'a> = (Type, &'a mut dyn SemanticEmissionBudgetV1);
    type Charge<'a> = (&'a mut usize, &'a mut dyn SemanticEmissionBudgetV1);
    argument_sum_v1(&[
        size_of::<std::vec::IntoIter<Type>>(),
        size_of::<&mut std::vec::IntoIter<Type>>(),
        size_of::<Option<Type>>(),
        size_of::<Type>(),
        size_of::<Move<'_>>(),
        size_of::<Result<Type, ProductionSemanticKirErrorV1>>(),
        size_of::<&Type>(),
        size_of::<&Box<Type>>(),
        size_of::<&fe2o3_kernel_ir::PointerType>(),
        size_of::<&fe2o3_kernel_ir::SliceType>(),
        size_of::<usize>(),
        size_of::<Charge<'_>>(),
        size_of::<Option<usize>>(),
        size_of::<Result<usize, ArgumentResourceV1>>(),
        size_of::<Result<(), ArgumentResourceV1>>(),
        size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
    ])
}

// The payload producer already owns and pays every box. Preserve the clone's
// per-node work, fresh node limit and recursive Execution refusal without
// allocating a second tree. The caller retains all original type credits.
fn source_reference_move_type_v29(
    ty: Type,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let mut nodes = 0;
    let mut current = &ty;
    loop {
        execution_cfg_charge_node_v29(&mut nodes, budget)?;
        match current {
            Type::Unit | Type::Scalar(_) | Type::Vector(_) | Type::StorageObject(_) => break,
            Type::Pointer(pointer) => current = &pointer.pointee,
            Type::Slice(slice) => current = &slice.element,
            Type::Execution(_) => return Err(execution_cfg_error_v29()),
        }
    }
    Ok(ty)
}

#[cfg(test)]
#[path = "production_source_reference_owned_types_v29_tests.rs"]
mod source_reference_owned_types_v29_tests;
