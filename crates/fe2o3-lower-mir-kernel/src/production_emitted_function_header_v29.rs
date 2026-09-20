// These copies outlive their source plan. Keep their charges on the scoped
// attempt owner; partial copies drop before that owner's error/unwind cleanup.
fn copy_emitted_function_header_v29(
    id: &FunctionId,
    parameters: &[Type],
    results: &[Type],
    values: &[ValueId],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(FunctionId, Signature, Vec<ValueId>), ProductionSemanticKirErrorV1> {
    let id = FunctionId::new(scoped_copy_string_v29(id.as_str(), budget)?);
    let mut copied_parameters = emission_vec_v1(parameters.len(), budget)?;
    for ty in parameters {
        copied_parameters.push(copy_emitted_header_type_v29(ty, budget)?);
    }
    let mut copied_results = emission_vec_v1(results.len(), budget)?;
    for ty in results {
        copied_results.push(copy_emitted_header_type_v29(ty, budget)?);
    }
    budget.charge_work(values.len())?;
    let mut copied_values = emission_vec_v1(values.len(), budget)?;
    copied_values.extend_from_slice(values);
    Ok((
        id,
        Signature::new(copied_parameters, copied_results),
        copied_values,
    ))
}

// Type owns a chain, not a branching tree. Walk it without recursive work or
// extra scratch, preserving every variant rather than imposing CFG admission.
fn copy_emitted_header_type_v29(
    mut source: &Type,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let mut copied = Type::Unit;
    let mut target = &mut copied;
    loop {
        budget.charge_work(1)?;
        match source {
            Type::Unit => {
                *target = Type::Unit;
                break;
            }
            Type::Scalar(scalar) => {
                *target = Type::Scalar(*scalar);
                break;
            }
            Type::Execution(role) => {
                *target = Type::Execution(*role);
                break;
            }
            Type::Vector(vector) => {
                *target = Type::Vector(*vector);
                break;
            }
            Type::Pointer(pointer) => {
                budget.reserve_storage(std::mem::size_of::<Type>())?;
                *target = Type::pointer(Type::Unit, pointer.address_space, pointer.access);
                let Type::Pointer(next) = target else {
                    unreachable!();
                };
                target = &mut next.pointee;
                source = &pointer.pointee;
            }
            Type::Slice(slice) => {
                budget.reserve_storage(std::mem::size_of::<Type>())?;
                *target = Type::slice(Type::Unit, slice.address_space, slice.access);
                let Type::Slice(next) = target else {
                    unreachable!();
                };
                target = &mut next.element;
                source = &slice.element;
            }
        }
    }
    Ok(copied)
}

#[cfg(test)]
mod emitted_function_header_tests {
    include!("production_emitted_function_header_v29_tests.rs");
}
